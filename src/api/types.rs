use chrono::{DateTime, FixedOffset};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub repositories: Vec<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TagListing {
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub enum Manifest {
    Image(ImageManifest),
    Fat(FatManifest),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FatManifest {
    pub manifests: Vec<ManifestSummary>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSummary {
    pub digest: String,
    pub platform: Platform,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub architecture: String,
    pub os: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageManifest {
    pub config: ImageConfigReference,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageConfigReference {
    pub digest: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageConfig {
    pub created: DateTime<FixedOffset>,
}
