use std::any::type_name;

use cloneable_errors::{ErrorContext, ResContext, bail};
use reqwest::header::{self, HeaderValue};
use serde::de::DeserializeOwned;

use crate::{
    api::types::{Catalog, ImageConfig, Manifest, ManifestsListing, TagListing},
    utils::UrlExt,
};

pub struct Client<'a> {
    http: reqwest::Client,
    config: &'a crate::config::RegistryConfig,
}

impl<'a> Client<'a> {
    pub fn new(config: &'a crate::config::RegistryConfig) -> Result<Self, ErrorContext> {
        Ok(Self {
            http: reqwest::ClientBuilder::new()
                .build()
                .context("Failed to build reqwest client")?,
            config,
        })
    }

    async fn make_get_request<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        content_type: Option<HeaderValue>,
    ) -> Result<T, ErrorContext> {
        let mut url = self.config.base_url.clone();
        url.extend_path(segments);
        let mut request = self
            .http
            .get(url)
            .basic_auth(&self.config.username, Some(&self.config.password));
        if let Some(content_type) = content_type {
            request = request.header(header::ACCEPT, content_type);
        }
        let response = request.send().await.context("Failed to send request")?;
        if response.status() != 200 {
            bail!(
                "Unexpected response code, expected 200, got {} with body {}",
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "[failed to read]".to_string())
            )
        }

        response
            .json()
            .await
            .with_context(|| format!("Failed to deserialize response as {}", type_name::<T>()))
    }

    pub async fn get_catalog(&self) -> Result<Catalog, ErrorContext> {
        self.make_get_request(&["v2", "_catalog"], None).await
    }

    pub async fn list_tags(&self, repo: &str) -> Result<TagListing, ErrorContext> {
        self.make_get_request(&["v2", repo, "tags", "list"], None)
            .await
    }

    pub async fn list_manifests(
        &self,
        repo: &str,
        tag: &str,
    ) -> Result<ManifestsListing, ErrorContext> {
        self.make_get_request(&["v2", repo, "manifests", tag], None)
            .await
    }

    pub async fn get_manifest(
        &self,
        repo: &str,
        digest: &str,
        content_type: &str,
    ) -> Result<Manifest, ErrorContext> {
        self.make_get_request(
            &["v2", repo, "manifests", digest],
            Some(HeaderValue::from_str(content_type).context("Invalid content_type")?),
        )
        .await
    }

    pub async fn get_image_config(
        &self,
        repo: &str,
        digest: &str,
    ) -> Result<ImageConfig, ErrorContext> {
        self.make_get_request(&["v2", repo, "blobs", digest], None)
            .await
    }
}
