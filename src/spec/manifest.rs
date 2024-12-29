use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::enums::MediaType;

#[derive(Serialize, Deserialize)]
pub struct Descriptor {
    #[serde(rename = "mediaType")]
    pub media_type: MediaType,

    pub digest: String,
    pub size: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ImageManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "mediaType")]
    pub media_type: MediaType,

    #[serde(rename = "artifactType", skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,

    pub config: Descriptor,
    pub layers: Vec<Descriptor>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Descriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl ImageManifest {
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(&self).expect("Failed to serialize ImageManifest")
    }
}
