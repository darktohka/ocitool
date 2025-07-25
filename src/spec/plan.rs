use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::spec::{
    config::{self, Healthcheck},
    enums::PlatformArchitecture,
};

use super::config::Config;

#[derive(Serialize, Deserialize)]
pub struct ImagePlan {
    pub name: String,
    pub tags: Vec<String>,
    pub platforms: Vec<ImagePlanPlatform>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ImagePlanConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ImagePlanConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(rename = "ports", skip_serializing_if = "Option::is_none")]
    pub exposed_ports: Option<HashMap<String, HashMap<String, String>>>,
    #[serde(rename = "environment", skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<HashMap<String, HashMap<String, String>>>,
    #[serde(rename = "workingDir", skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(rename = "stopSignal", skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<String>,
    #[serde(rename = "argsEscaped", skip_serializing_if = "Option::is_none")]
    pub args_escaped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<i64>,
    #[serde(rename = "swap", skip_serializing_if = "Option::is_none")]
    pub memory_swap: Option<i64>,
    #[serde(rename = "cpu", skip_serializing_if = "Option::is_none")]
    pub cpu_shares: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<Healthcheck>,
}

impl ImagePlanConfig {
    pub fn to_config(self) -> Config {
        Config {
            user: self.user,
            exposed_ports: self.exposed_ports,
            env: self.env,
            entrypoint: self.entrypoint,
            cmd: self.cmd,
            volumes: self.volumes,
            working_dir: self.working_dir,
            labels: self.labels,
            stop_signal: self.stop_signal,
            args_escaped: self.args_escaped,
            memory: self.memory,
            memory_swap: self.memory_swap,
            cpu_shares: self.cpu_shares,
            healthcheck: self.healthcheck,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ImagePlanPlatform {
    pub architecture: PlatformArchitecture,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ImagePlanConfig>,

    pub layers: Vec<ImagePlanLayer>,
}

#[derive(Serialize, Deserialize)]
pub enum ImagePlanLayerType {
    #[serde(rename = "tar")]
    Layer,
    #[serde(rename = "dir")]
    Directory,
    #[serde(rename = "image")]
    Image,
}

#[derive(Serialize, Deserialize)]
pub struct ImagePlanLayer {
    #[serde(rename = "type")]
    pub layer_type: ImagePlanLayerType,
    pub source: String,
    pub comment: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blacklist: Option<Vec<String>>,
}

pub fn merge_image_plan_configs(
    base_config: &Option<ImagePlanConfig>,
    config: &Option<ImagePlanConfig>,
) -> Option<Config> {
    match (&base_config, &config) {
        (Some(plan), Some(original)) => Some(Config {
            user: original.user.clone().or_else(|| plan.user.clone()),
            exposed_ports: match (&original.exposed_ports, &plan.exposed_ports) {
                (Some(original_ports), Some(plan_ports)) => {
                    let mut merged_ports = original_ports.clone();
                    merged_ports.extend(plan_ports.clone());
                    Some(merged_ports)
                }
                (None, Some(plan_ports)) => Some(plan_ports.clone()),
                (Some(original_ports), None) => Some(original_ports.clone()),
                (None, None) => None,
            },
            env: original.env.clone().or_else(|| plan.env.clone()),
            entrypoint: original
                .entrypoint
                .clone()
                .or_else(|| plan.entrypoint.clone()),
            cmd: original.cmd.clone().or_else(|| plan.cmd.clone()),
            volumes: match (&original.volumes, &plan.volumes) {
                (Some(original_volumes), Some(plan_volumes)) => {
                    let mut merged_volumes = original_volumes.clone();
                    merged_volumes.extend(plan_volumes.clone());
                    Some(merged_volumes)
                }
                (None, Some(plan_volumes)) => Some(plan_volumes.clone()),
                (Some(original_volumes), None) => Some(original_volumes.clone()),
                (None, None) => None,
            },
            working_dir: original
                .working_dir
                .clone()
                .or_else(|| plan.working_dir.clone()),
            labels: match (&original.labels, &plan.labels) {
                (Some(original_labels), Some(plan_labels)) => {
                    let mut merged_labels = original_labels.clone();
                    merged_labels.extend(plan_labels.clone());
                    Some(merged_labels)
                }
                (None, Some(plan_labels)) => Some(plan_labels.clone()),
                (Some(original_labels), None) => Some(original_labels.clone()),
                (None, None) => None,
            },
            stop_signal: original
                .stop_signal
                .clone()
                .or_else(|| plan.stop_signal.clone()),
            args_escaped: original.args_escaped.or_else(|| plan.args_escaped),
            memory: original.memory.or_else(|| plan.memory),
            memory_swap: original.memory_swap.or_else(|| plan.memory_swap),
            cpu_shares: original.cpu_shares.or_else(|| plan.cpu_shares),
            healthcheck: match (&original.healthcheck, &plan.healthcheck) {
                (Some(original_healthcheck), Some(plan_healthcheck)) => Some(config::Healthcheck {
                    test: plan_healthcheck
                        .test
                        .clone()
                        .or_else(|| original_healthcheck.test.clone()),
                    interval: plan_healthcheck.interval.or(original_healthcheck.interval),
                    timeout: plan_healthcheck.timeout.or(original_healthcheck.timeout),
                    retries: plan_healthcheck.retries.or(original_healthcheck.retries),
                    start_period: plan_healthcheck
                        .start_period
                        .or(original_healthcheck.start_period),
                    start_interval: plan_healthcheck
                        .start_interval
                        .clone()
                        .or(original_healthcheck.start_interval.clone()),
                    disable: plan_healthcheck.disable || original_healthcheck.disable,
                }),
                (None, Some(plan_healthcheck)) => Some(config::Healthcheck {
                    test: plan_healthcheck.test.clone(),
                    interval: plan_healthcheck.interval,
                    timeout: plan_healthcheck.timeout,
                    retries: plan_healthcheck.retries,
                    start_period: plan_healthcheck.start_period,
                    start_interval: plan_healthcheck.start_interval.clone(),
                    disable: plan_healthcheck.disable,
                }),
                (Some(original_healthcheck), None) => Some(config::Healthcheck {
                    test: original_healthcheck.test.clone(),
                    interval: original_healthcheck.interval,
                    timeout: original_healthcheck.timeout,
                    retries: original_healthcheck.retries,
                    start_period: original_healthcheck.start_period,
                    start_interval: original_healthcheck.start_interval.clone(),
                    disable: original_healthcheck.disable,
                }),
                (None, None) => None,
            },
        }),
        (Some(plan), None) => Some(plan.clone().to_config()),
        (None, Some(original)) => Some(original.clone().to_config()),
        (None, None) => None,
    }
}
