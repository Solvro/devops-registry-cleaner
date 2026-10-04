use chrono::{DateTime, FixedOffset, TimeDelta, Utc};
use cloneable_errors::{ErrorContext, ResContext, bail};
use tracing::{error, info};

use crate::{
    api::{client::Client, types::Manifest},
    config::Config,
    utils::FutureSet,
};

pub mod api;
pub mod config;
pub mod utils;

async fn get_image_creation_date(
    client: &Client<'_>,
    repo: &str,
    tag: &str,
) -> Result<DateTime<FixedOffset>, ErrorContext> {
    // get the manifest for the tag
    let manifest = client
        .get_manifest(repo, tag)
        .await
        .with_context(|| format!("Failed to get manifest for {repo}:{tag}"))?;

    // the image may have a "fat" (index) manifest
    let manifest = match manifest {
        // if it is a simple image manifest, then we don't have to do anything else here
        Manifest::Image(manifest) => manifest,
        // if it's an index, pick a manifest with os & platform
        // other are often fake provenance/signing manifests
        Manifest::Fat(index) => {
            let summary = index
                .manifests
                .iter()
                .find(|m| m.platform.os != "unknown" && m.platform.architecture != "unknown")
                .with_context(|| format!("Image {repo}:{tag} has no usable manifests"))?;

            // download the manifest
            let manifest = client
                .get_manifest(repo, &summary.digest)
                .await
                .with_context(|| {
                    format!(
                        "Failed to fetch manifest {} for {repo}:{tag}",
                        summary.digest
                    )
                })?;

            // it should be an image manifest here
            // (could technically be another index, hope it is not)
            let Manifest::Image(manifest) = manifest else {
                bail!("Image {repo}:{tag} has a nested manifest",);
            };

            manifest
        }
    };

    // fetch the manifest's config
    let config = client
        .get_image_config(repo, &manifest.config.digest)
        .await
        .with_context(|| {
            format!(
                "Failed to fetch image config {} for {repo}:{tag}",
                manifest.config.digest
            )
        })?;

    // get the date, insert into tag group
    Ok(config.created)
}

async fn list_tags_for_deletion<'a>(
    config: &Config,
    client: &Client<'_>,
    repo: &str,
    tags: &'a [String],
) -> Vec<&'a str> {
    #[derive(Debug)]
    struct Tag<'a> {
        name: &'a str,
        age: TimeDelta,
    }

    let now = Utc::now().fixed_offset();
    // make a list of tags -> dates for each rule
    let mut tag_groups: Vec<Vec<Tag<'a>>> = (0..config.rules.len()).map(|_| Vec::new()).collect();

    // group tags, get their creation dates
    // any tag that doesnt match a rule is skipped
    {
        // since it's mostly just network requests, try to parallelize this without spawning tasks
        // not using tasks allows us to avoid cloning data and keep using 'a lifetime refs
        // (task spawning requires 'static)
        // so, first create all the futures for each tag
        let mut future_set: FutureSet<_, _, _> = tags
            .iter()
            .filter_map(|tag| {
                // try to match a rule
                let rule_idx = config
                    .rules
                    .iter()
                    .position(|rule| rule.is_match(repo, tag))?;

                Some((
                    Box::pin(get_image_creation_date(client, repo, tag)),
                    (rule_idx, tag),
                ))
            })
            .collect();

        // then drive all the futures at once, until they all complete
        while let Some((result, (rule_idx, tag))) = future_set.next().await {
            // unwrap the result
            let result = match result {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to get the image creation date for {repo}:{tag}: {e:?}");
                    continue;
                }
            };

            tag_groups[rule_idx].push(Tag {
                name: tag,
                age: now - result,
            });
        }
    }

    // sort each group by image creation date, from newest to oldest
    for group in &mut tag_groups {
        group.sort_unstable_by_key(|a| a.age);
    }

    // nominate tags for deletion
    tag_groups
        .iter()
        .zip(&config.rules)
        .flat_map(|(group, rule)| {
            group
                .iter()
                .enumerate()
                .filter(|(idx, tag)| {
                    //   remember that idx starts at 0 - vvv
                    rule.max_tags.is_some_and(|max_tags| {
                        *idx >= max_tags
                            && rule
                                .overflow_max_age
                                .is_none_or(|max_age| tag.age > max_age)
                    }) || rule.max_age.is_some_and(|max_age| tag.age > max_age)
                })
                .map(|(.., tag)| tag.name)
        })
        .collect()
}

async fn delete_nominated_tags(client: &Client<'_>, repo: &str, tags: Vec<&str>) {
    let mut completed = 0;

    // this is once again mostly just waiting on the network, so let's run this concurrently on the
    // same thread
    let mut future_set: FutureSet<_, _, _> = tags
        .into_iter()
        .map(|tag| (Box::pin(client.delete_tag(repo, tag)), tag))
        .collect();

    while let Some((result, tag)) = future_set.next().await {
        if let Err(e) = result {
            error!("Failed to delete {repo}:{tag}: {e:?}");
        }
        completed += 1;
    }

    info!("{repo}: deleted {completed} tags");
}

#[tokio::main]
async fn main() -> Result<(), ErrorContext> {
    tracing_subscriber::fmt::init();
    let config = Config::get().context("Failed to load config")?;
    let client = config
        .make_client()
        .context("Failed to create registry API client")?;

    let catalog = client
        .get_catalog()
        .await
        .context("Failed to list images")?;

    info!("Found {} images in catalog", catalog.repositories.len());

    let futures: FutureSet<_, _, _> = catalog
        .repositories
        .into_iter()
        .map(|repo| {
            let config = &config;
            let client = &client;
            Box::pin(async move {
                let tags = match client.list_tags(&repo).await {
                    Ok(v) => v,
                    Err(e) => {
                        error!("{repo}: failed to list tags: {e:?}");
                        return;
                    }
                };

                let to_delete = list_tags_for_deletion(config, client, &repo, &tags.tags).await;

                info!(
                    "{repo}: nominated {}/{} tags for deletion",
                    to_delete.len(),
                    tags.tags.len()
                );
                delete_nominated_tags(client, &repo, to_delete).await;
            })
        })
        .collect();
    futures.finish_all().await;

    Ok(())
}
