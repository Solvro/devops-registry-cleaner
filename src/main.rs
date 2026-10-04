use cloneable_errors::{ErrorContext, ResContext};

use crate::config::Config;

pub mod api;
pub mod config;
pub mod utils;

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
    dbg!(&catalog);

    let repo = &catalog.repositories[10];
    let tags = client
        .list_tags(repo)
        .await
        .with_context(|| format!("Failed to list tags for {repo}"))?;
    dbg!(&tags);

    let tag = &tags.tags[0];
    let manifests = client
        .list_manifests(repo, tag)
        .await
        .with_context(|| format!("Failed to list manifests for {repo}:{tag}"))?;
    dbg!(&manifests);

    let manifest_summary = manifests
        .manifests
        .iter()
        .find(|m| m.platform.os != "unknown" && m.platform.architecture != "unknown")
        .with_context(|| format!("Image {repo}:{tag} has no usable manifests"))?;
    let manifest = client
        .get_manifest(repo, &manifest_summary.digest, &manifest_summary.media_type)
        .await
        .with_context(|| {
            format!(
                "Failed to fetch manifest {} ({}) for {repo}:{tag}",
                manifest_summary.digest, manifest_summary.media_type
            )
        })?;
    dbg!(&manifest);

    let config = client
        .get_image_config(repo, &manifest.config.digest)
        .await
        .with_context(|| {
            format!(
                "Failed to fetch image config {} for {repo}:{tag}",
                manifest.config.digest
            )
        })?;
    dbg!(&config);

    Ok(())
}
