use crate::spec::enums::{MediaType, PlatformArchitecture, PlatformOS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct ImageIndex {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,

    #[serde(rename = "mediaType")]
    pub media_type: MediaType,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "artifactType")]
    pub artifact_type: Option<String>,

    pub manifests: Vec<Manifest>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl ImageIndex {
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(&self).expect("Failed to serialize ImageIndex")
    }
}

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "mediaType")]
    pub media_type: MediaType,
    pub size: u64,
    pub digest: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
}

#[derive(Serialize, Deserialize)]
pub struct Platform {
    pub architecture: PlatformArchitecture,
    pub os: PlatformOS,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "os.version")]
    pub os_version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "os.features")]
    pub os_features: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
}
