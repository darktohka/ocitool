use serde_json::Value;
use std::{collections::HashSet, process::Command};

use crate::compose::types::compose::{Labels, NetworkSettings};

pub fn list_networks() -> Result<HashSet<String>, String> {
    let output = Command::new("nerdctl")
        .args(&["network", "ls", "--format=json"])
        .output()
        .expect("Failed to execute nerdctl command");

    if !output.status.success() {
        return Err("Failed to list networks".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let networks: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    let mut existing_networks = HashSet::new();

    for network in networks {
        if let Some(name) = network.get("Name").and_then(|n| n.as_str()) {
            existing_networks.insert(name.to_string());
        }
    }

    Ok(existing_networks)
}

pub fn create_network(name: &str, settings: &NetworkSettings) -> Result<(), String> {
    let mut command = Command::new("nerdctl");
    command.arg("network").arg("create").arg(name);

    if settings.enable_ipv6 {
        command.arg("--ipv6");
    }

    // Add labels to the network creation command
    match &settings.labels {
        Labels::List(labels) => {
            for label in labels {
                command.arg(format!("--label={}", label));
            }
        }
        Labels::Map(label_map) => {
            for (key, value) in label_map {
                command.arg(format!("--label={}={}", key, value));
            }
        }
    }

    // Add driver to the network creation command if specified
    if let Some(driver) = &settings.driver {
        command.arg(format!("--driver={}", driver));
    }

    // Add driver options to the network creation command if specified
    for (key, value) in &settings.driver_opts {
        if let Some(val) = value {
            command.arg(format!("--opt={}={}", key, val));
        }
    }

    // Add IPAM configuration to the network creation command if specified
    if let Some(ipam) = &settings.ipam {
        if !ipam.config.is_empty() {
            let config = &ipam.config[0]; // Only use the first IPAM config
            if &config.subnet != "" {
                command.arg(format!("--subnet={}", &config.subnet));
            }
            if let Some(gateway) = &config.gateway {
                if gateway != "" {
                    command.arg(format!("--gateway={}", gateway));
                }
            }
            // TODO: IPRange
        }
    }
    let output = command.output().expect("Failed to execute nerdctl command");

    if !output.status.success() {
        return Err(format!(
            "Failed to create network {}: {}",
            name,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}
