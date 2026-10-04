use std::any::type_name;

use cloneable_errors::{ErrorContext, ResContext, bail};
use reqwest::{
    Response,
    header::{self, HeaderValue},
};
use serde::de::DeserializeOwned;

use crate::{
    api::types::{Catalog, ImageConfig, Manifest, TagListing},
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

    async fn send_get_request(
        &self,
        segments: &[&str],
        content_type: Option<HeaderValue>,
    ) -> Result<Response, ErrorContext> {
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
        Ok(response)
    }

    async fn handle_get_request<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        content_type: Option<HeaderValue>,
    ) -> Result<T, ErrorContext> {
        let response = self.send_get_request(segments, content_type).await?;

        response
            .json()
            .await
            .with_context(|| format!("Failed to deserialize response as {}", type_name::<T>()))
    }

    pub async fn get_catalog(&self) -> Result<Catalog, ErrorContext> {
        self.handle_get_request(&["v2", "_catalog"], None).await
    }

    pub async fn list_tags(&self, repo: &str) -> Result<TagListing, ErrorContext> {
        self.handle_get_request(&["v2", repo, "tags", "list"], None)
            .await
    }

    pub async fn get_manifest(
        &self,
        repo: &str,
        reference: &str,
    ) -> Result<Manifest, ErrorContext> {
        const CONTENT_TYPE: Option<HeaderValue> = Some(HeaderValue::from_static(
            "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json",
        ));
        let response = self
            .send_get_request(&["v2", repo, "manifests", reference], CONTENT_TYPE)
            .await?;

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .context("Response is missing a Content-Type header")?;
        let content_type = content_type
            .to_str()
            .context("Response has an unparseable Content-Type header")?;
        match content_type {
            "application/vnd.oci.image.index.v1+json"
            | "application/vnd.docker.distribution.manifest.list.v2+json" => {
                Ok(Manifest::Fat(response.json().await.context(
                    "Failed to deserialize response as FatManifest",
                )?))
            }
            "application/vnd.docker.distribution.manifest.v2+json"
            | "application/vnd.oci.image.manifest.v1+json" => {
                Ok(Manifest::Image(response.json().await.context(
                    "Failed to deserialize response as ImageManifest",
                )?))
            }
            _ => bail!("Response has an unknown Content-Type: {content_type}",),
        }
    }

    pub async fn get_image_config(
        &self,
        repo: &str,
        digest: &str,
    ) -> Result<ImageConfig, ErrorContext> {
        self.handle_get_request(&["v2", repo, "blobs", digest], None)
            .await
    }

    pub async fn delete_tag(&self, repo: &str, tag: &str) -> Result<(), ErrorContext> {
        let mut url = self.config.base_url.clone();
        url.extend_path(["v2", repo, "manifests", tag]);
        let request = self
            .http
            .delete(url)
            .basic_auth(&self.config.username, Some(&self.config.password));
        let response = request.send().await.context("Failed to send request")?;
        if response.status() != 202 {
            bail!(
                "Unexpected response code, expected 202, got {} with body {}",
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "[failed to read]".to_string())
            )
        }
        Ok(())
    }
}
