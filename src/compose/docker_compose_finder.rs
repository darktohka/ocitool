use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::compose::types::compose::Compose;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DockerCompose {
    pub name: String,
    pub directory: PathBuf,
    pub compose_path: PathBuf,
    pub compose: Compose,
}

pub fn find_docker_compose_files(start_dir: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut found_files = Vec::new();
    let mut stack = vec![(start_dir.to_path_buf(), 0)];

    while let Some((current_path, current_depth)) = stack.first().cloned() {
        stack.remove(0); // Pop the first element

        if current_depth > max_depth {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&current_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    stack.push((path, current_depth + 1));
                } else if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        if file_name == "docker-compose.yaml" || file_name == "docker-compose.yml" {
                            found_files.push(path);

                            if current_depth == 0 {
                                // If we found a docker-compose file at the root level,
                                // we can stop searching deeper in this directory
                                return found_files;
                            }
                        }
                    }
                }
            }
        }
    }
    found_files
}

pub fn parse_docker_compose_file(path: &Path) -> Result<Compose, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let compose: Compose =
        serde_yaml_ng::from_str(&content).map_err(|e| Box::new(e) as Box<dyn Error>)?;
    Ok(compose)
}

pub fn find_and_parse_docker_composes(start_dir: &Path, max_depth: usize) -> Vec<DockerCompose> {
    let compose_files = find_docker_compose_files(start_dir, max_depth);
    let mut composes = Vec::<DockerCompose>::new();

    for compose_path in compose_files {
        let compose = parse_docker_compose_file(&compose_path);

        match compose {
            Ok(compose) => {
                if let Some(parent) = compose_path.parent() {
                    if let Some(name) = parent.file_name() {
                        composes.push(DockerCompose {
                            name: name.to_string_lossy().to_string(),
                            directory: parent.to_path_buf(),
                            compose_path,
                            compose,
                        });
                    }
                }
            }
            Err(e) => {
                eprintln!("Error parsing {}: {}", compose_path.display(), e);
            }
        }
    }

    composes
}
