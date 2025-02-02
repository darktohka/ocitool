use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

use super::enums::{PlatformArchitecture, PlatformOS};

#[derive(Serialize, Deserialize)]
pub struct ImageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub created: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    pub architecture: PlatformArchitecture,

    #[serde(rename = "os")]
    pub os: PlatformOS,
    #[serde(rename = "os.version", skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(rename = "os.features", skip_serializing_if = "Option::is_none")]
    pub os_features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,

    pub rootfs: RootFs,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<History>>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "User", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(rename = "ExposedPorts", skip_serializing_if = "Option::is_none")]
    pub exposed_ports: Option<HashMap<String, HashMap<String, String>>>,
    #[serde(rename = "Env", skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,
    #[serde(rename = "Entrypoint", skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    #[serde(rename = "Cmd", skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    #[serde(rename = "Volumes", skip_serializing_if = "Option::is_none")]
    pub volumes: Option<HashMap<String, HashMap<String, String>>>,
    #[serde(rename = "WorkingDir", skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(rename = "Labels", skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(rename = "StopSignal", skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<String>,
    #[serde(rename = "ArgsEscaped", skip_serializing_if = "Option::is_none")]
    pub args_escaped: Option<bool>,
    #[serde(rename = "Memory", skip_serializing_if = "Option::is_none")]
    pub memory: Option<i64>,
    #[serde(rename = "MemorySwap", skip_serializing_if = "Option::is_none")]
    pub memory_swap: Option<i64>,
    #[serde(rename = "CpuShares", skip_serializing_if = "Option::is_none")]
    pub cpu_shares: Option<i64>,
    #[serde(rename = "Healthcheck", skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
pub struct RootFs {
    #[serde(rename = "type")]
    pub fs_type: String,
    pub diff_ids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct History {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub created: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(rename = "created_by", skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_layer: Option<bool>,
}

impl ImageConfig {
    pub fn to_json(&self) -> Vec<u8> {
        cjson::to_vec(&self).expect("Failed to serialize ImageConfig")
    }
}
