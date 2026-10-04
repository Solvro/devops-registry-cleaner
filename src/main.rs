use chrono::{DateTime, FixedOffset};
use cloneable_errors::{ErrorContext, ResContext, bail};
use futures::future::select_all;
use slab::Slab;

use crate::{
    api::{client::Client, types::Manifest},
    config::Config,
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
) -> Result<Vec<&'a str>, ErrorContext> {
    #[derive(Debug)]
    struct Tag<'a> {
        name: &'a str,
        created_at: DateTime<FixedOffset>,
    }

    // make a list of tags -> dates for each rule
    let mut tag_groups: Vec<Vec<Tag<'a>>> = (0..config.rules.len()).map(|_| Vec::new()).collect();

    // group tags, get their creation dates
    // any tag that doesnt match a rule is skipped
    {
        // since it's mostly just network requests, try to parallelize this without spawning tasks
        // not using tasks allows us to avoid cloning data and keep using 'a lifetime refs
        // (task spawning requires 'static)
        // so, first create all the futures for each tag
        let mut futures = Slab::with_capacity(tags.len());
        for tag in tags {
            // try to match a rule
            let Some(rule_idx) = config
                .rules
                .iter()
                .position(|rule| rule.is_match(repo, tag))
            else {
                continue;
            };

            futures.insert((
                rule_idx,
                tag,
                Box::pin(get_image_creation_date(client, repo, tag)),
            ));
        }

        // then drive all the futures at once, until they all complete
        //
        // to avoid allocations (i think) use select_all instead of join_all, and complete the
        // tag_groups array incrementally
        //
        // (eh, join_all would be much simpler, wouldn't it? can't learn without experimenting
        // though)
        while !futures.is_empty() {
            // complete the next future
            let (result, idx, ..) = select_all(futures.iter_mut().map(|(_, (.., fut))| fut)).await;

            // remove it from the slab
            let (rule_idx, tag, ..) = futures.remove(
                // indexes != slab keys, so we have to iterate over the entire slab to find the key
                // for an index
                futures
                    .iter()
                    .nth(idx)
                    .expect("index returned by select_all should exist")
                    .0,
            );

            // unwrap the result
            let result = result.with_context(|| {
                format!("Failed to get the image creation date for {repo}:{tag}")
            })?;

            tag_groups[rule_idx].push(Tag {
                name: tag,
                created_at: result,
            });
        }
    }

    for group in &mut tag_groups {
        group.sort_unstable_by(|a, b| a.created_at.cmp(&b.created_at).reverse());
    }

    dbg!(repo, tag_groups);

    Ok(Vec::new())
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

    for repo in catalog.repositories {
        let tags = client
            .list_tags(&repo)
            .await
            .with_context(|| format!("Failed to list tags for {repo}"))?;

        list_tags_for_deletion(&config, &client, &repo, &tags.tags)
            .await
            .with_context(|| {
                format!("Failed to pick tag candidates for deletion from repo {repo}")
            })?;
    }

    Ok(())
}
