mod nerdctl_utils;

use crate::compose::types::compose::{ComposeNetwork, MapOrEmpty, NetworkSettings};
use crate::{compose::docker_compose_finder::find_and_parse_docker_composes, Compose};
use std::collections::{HashMap, HashSet};

pub async fn up_command(compose_settings: &Compose) -> Result<(), Box<dyn std::error::Error>> {
    let start_dir = compose_settings
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));
    let max_depth = compose_settings.max_depth.unwrap_or(1);

    let composes = find_and_parse_docker_composes(&start_dir, max_depth);

    if composes.is_empty() {
        println!("No docker-compose files found in {}", start_dir.display());
        return Ok(());
    }

    let existing_networks: HashSet<String> = nerdctl_utils::list_networks()?;
    let mut networks_to_create = HashMap::<String, NetworkSettings>::new();

    for compose in composes {
        for (network_name, network_settings) in compose.compose.networks.0.iter() {
            if let MapOrEmpty::Map(network_settings) = network_settings {
                // Check whether the network is external
                if let Some(external) = &network_settings.external {
                    if let ComposeNetwork::Bool(external) = external {
                        if *external {
                            // If the network is external, we skip creating it
                            continue;
                        }
                    }
                }

                let actual_network_name = format!("{}_{}", compose.name, network_name);

                if !existing_networks.contains(&actual_network_name) {
                    networks_to_create
                        .entry(actual_network_name)
                        .insert_entry(network_settings.clone());
                }
            }
        }
    }

    let mut network_names: Vec<_> = networks_to_create.keys().collect();
    network_names.sort();

    for network_name in network_names {
        if let Some(network_settings) = networks_to_create.get(network_name) {
            match nerdctl_utils::create_network(network_name, network_settings) {
                Ok(_) => println!("Network '{}' created successfully.", network_name),
                Err(e) => eprintln!("Failed to create network '{}': {}", network_name, e),
            }
        }
    }

    println!("All networks have been created successfully.");
    Ok(())
}
