use crate::Cleanup;
use serde_json::Value;
use std::fs;
use std::{
    collections::{HashMap, HashSet},
    io::stdin,
    path::PathBuf,
    process::exit,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub dir: PathBuf,
    pub layer_dir: PathBuf,
    pub tag_dir: PathBuf,
    pub revision_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DockerRepository {
    pub blobs_dir: PathBuf,
    pub repositories: Vec<Repository>,
}

#[derive(Debug, Clone)]
pub struct CleanupPlan<'a> {
    pub cleanup: &'a Cleanup,
    pub repository: &'a DockerRepository,
    pub cleanup_commits: HashMap<&'a Repository, HashSet<PathBuf>>,
    pub cleanup_indexes: HashMap<&'a Repository, HashSet<PathBuf>>,
    pub cleanup_revisions: HashMap<&'a Repository, HashSet<PathBuf>>,
    pub cleanup_layers: HashMap<&'a Repository, HashSet<String>>,
    pub cleanup_blobs: HashSet<String>,
}

impl<'a> CleanupPlan<'a> {
    pub fn new(cleanup: &'a Cleanup, repository: &'a DockerRepository) -> Self {
        CleanupPlan {
            cleanup,
            repository,
            cleanup_commits: HashMap::new(),
            cleanup_indexes: HashMap::new(),
            cleanup_revisions: HashMap::new(),
            cleanup_layers: HashMap::new(),
            cleanup_blobs: HashSet::new(),
        }
    }
}

pub fn strip_sha256_prefix(name: &str) -> String {
    if name.starts_with("sha256:") {
        name[7..].to_string()
    } else {
        name.to_string()
    }
}

pub fn is_commit(name: &str) -> bool {
    // A commit is a 40-character hexadecimal string
    name.len() == 40 && name.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn find_dir(dir: &PathBuf, name: &str) -> Result<PathBuf, String> {
    let mut dirs = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .collect::<Vec<_>>();
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for entry in &dirs {
        if entry.file_name() == name {
            return Ok(entry.path());
        }
    }

    for entry in &dirs {
        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            let subdir = entry.path();

            if let Ok(found) = find_dir(&subdir, name) {
                return Ok(found);
            }
        }
    }

    return Err(format!(
        "Directory '{}' not found in {}",
        name,
        dir.display()
    ));
}

pub fn find_commit_dirs(dir: &PathBuf) -> Result<HashSet<PathBuf>, String> {
    let mut commit_dirs = HashSet::new();

    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;

        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            let path = entry.path();

            if is_commit(path.file_name().and_then(|s| s.to_str()).unwrap_or("")) {
                commit_dirs.insert(path);
            }
        }
    }

    Ok(commit_dirs)
}

pub fn get_repository(dir: PathBuf) -> Result<DockerRepository, String> {
    let blobs_dir = find_dir(&dir, "sha256")?;
    let repositories_dir = find_dir(&dir, "repositories")?;
    /*let blob_dirs = fs::read_dir(&blobs_dir)
    .map_err(|e| e.to_string())?
    .flatten()
    .filter_map(|entry| {
        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            Some(entry.path())
        } else {
            None
        }
    })
    .collect::<Vec<_>>();*/

    let repositories = fs::read_dir(&repositories_dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|owner_entry| {
            if !owner_entry.file_type().map_or(false, |ft| ft.is_dir()) {
                return None;
            }

            let owner = owner_entry.file_name().to_string_lossy().to_string();
            let owner_path = owner_entry.path();
            // Iterate over names inside the owner directory
            let repos = fs::read_dir(&owner_path)
                .map_err(|e| e.to_string())
                .ok()?
                .flatten()
                .filter_map(|repo_entry| {
                    if repo_entry.file_type().map_or(false, |ft| ft.is_dir()) {
                        let name = repo_entry.file_name().to_string_lossy().to_string();
                        let repo_path = repo_entry.path();
                        let layer_dir = repo_path.join("_layers/sha256");
                        let manifest_dir = repo_path.join("_manifests");
                        let tag_dir = manifest_dir.join("tags");
                        let revision_dir = manifest_dir.join("revisions/sha256");

                        if !layer_dir.exists() {
                            eprintln!(
                                "Layer directory does not exist for repository: {}",
                                repo_path.display()
                            );
                            return None;
                        }

                        if !tag_dir.exists() {
                            eprintln!(
                                "Tag directory does not exist for repository: {}",
                                repo_path.display()
                            );
                            return None;
                        }

                        if !revision_dir.exists() {
                            eprintln!(
                                "Tag directory does not exist for repository: {}",
                                repo_path.display()
                            );
                            return None;
                        }

                        Some(Repository {
                            owner: owner.clone(),
                            name,
                            dir: repo_path,
                            layer_dir,
                            tag_dir,
                            revision_dir,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            Some(repos)
        })
        .flatten()
        .collect::<Vec<_>>();

    Ok(DockerRepository {
        //blob_dirs,
        blobs_dir,
        repositories,
    })
}

pub fn handle_digest(
    digest: &Value,
    existing_blobs: &mut HashSet<String>,
    existing_layers: &mut HashSet<String>,
) -> Option<String> {
    if let Some(digest) = digest.get("digest").and_then(|d| d.as_str()) {
        let digest = strip_sha256_prefix(digest);
        existing_blobs.insert(digest.clone());
        existing_layers.insert(digest.clone());
        return Some(digest);
    }

    None
}

pub fn handle_manifest_file(
    data_path: &PathBuf,
    repository: &DockerRepository,
    existing_blobs: &mut HashSet<String>,
    existing_layers: &mut HashSet<String>,
) {
    match fs::read_to_string(&data_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => {
                if let Some(manifests) = json.get("manifests").and_then(|m| m.as_array()) {
                    for manifest in manifests {
                        if let Some(digest) = manifest.get("digest").and_then(|d| d.as_str()) {
                            let index_blob = strip_sha256_prefix(digest);
                            let index_first_two = &index_blob[..2];
                            let index_blob_path = repository
                                .blobs_dir
                                .join(index_first_two)
                                .join(index_blob.clone())
                                .join("data");
                            existing_blobs.insert(index_blob.clone());
                            existing_layers.insert(index_blob.clone());

                            handle_manifest_file(
                                &index_blob_path,
                                repository,
                                existing_blobs,
                                existing_layers,
                            )
                        }
                    }
                }

                // Handle config.digest
                if let Some(config) = json.get("config") {
                    handle_digest(config, existing_blobs, existing_layers);
                }

                // Handle old-style Docker image
                if let Some(config) = json.get("Config") {
                    let digest = strip_sha256_prefix(
                        config
                            .as_str()
                            .and_then(|s| s.split('.').next())
                            .unwrap_or(""),
                    );
                    existing_blobs.insert(digest.clone());
                    existing_layers.insert(digest.clone());
                }

                // Handle old-style Docker layers
                if let Some(layers) = json.get("Layers").and_then(|l| l.as_array()) {
                    for layer in layers {
                        let digest = strip_sha256_prefix(
                            layer
                                .as_str()
                                .and_then(|s| s.split('/').next())
                                .unwrap_or(""),
                        );
                        existing_blobs.insert(digest.clone());
                        existing_layers.insert(digest.clone());
                    }
                }

                // Handle layers digests
                if let Some(layers) = json.get("layers").and_then(|l| l.as_array()) {
                    for layer in layers {
                        handle_digest(layer, existing_blobs, existing_layers);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to parse JSON at {}: {}", data_path.display(), e);
            }
        },
        Err(e) => {
            eprintln!("Failed to read data file at {}: {}", data_path.display(), e);
        }
    }
}

pub fn preview_plan(cleanup_plan: &CleanupPlan) {
    let cleanup = cleanup_plan.cleanup;

    let mut cleanup_commits_vec: Vec<_> = cleanup_plan.cleanup_commits.iter().collect();
    cleanup_commits_vec.sort_by(|(repo_a, commit_dirs_a), (repo_b, commit_dirs_b)| {
        let len_cmp = commit_dirs_b.len().cmp(&commit_dirs_a.len());
        if len_cmp == std::cmp::Ordering::Equal {
            repo_a.name.cmp(&repo_b.name)
        } else {
            len_cmp
        }
    });

    for (repo, commit_dirs) in cleanup_commits_vec {
        println!(
            "Would clean up {} commits for repository: {}/{}",
            commit_dirs.len(),
            repo.owner,
            repo.name
        );
    }

    let mut cleanup_indexes_vec: Vec<_> = cleanup_plan.cleanup_indexes.iter().collect();
    cleanup_indexes_vec.sort_by(|(repo_a, dirs_a), (repo_b, dirs_b)| {
        let len_cmp = dirs_b.len().cmp(&dirs_a.len());
        if len_cmp == std::cmp::Ordering::Equal {
            repo_a.name.cmp(&repo_b.name)
        } else {
            len_cmp
        }
    });

    for (repo, dirs) in cleanup_indexes_vec {
        println!(
            "Would clean up {} indices for repository: {}/{}",
            dirs.len(),
            repo.owner,
            repo.name
        );
    }

    let mut cleanup_revisions_vec: Vec<_> = cleanup_plan.cleanup_revisions.iter().collect();
    cleanup_revisions_vec.sort_by(|(repo_a, dirs_a), (repo_b, dirs_b)| {
        let len_cmp = dirs_b.len().cmp(&dirs_a.len());
        if len_cmp == std::cmp::Ordering::Equal {
            repo_a.name.cmp(&repo_b.name)
        } else {
            len_cmp
        }
    });

    for (repo, dirs) in cleanup_revisions_vec {
        println!(
            "Would clean up {} revisions for repository: {}/{}",
            dirs.len(),
            repo.owner,
            repo.name
        );
    }

    let mut cleanup_layers_vec: Vec<_> = cleanup_plan.cleanup_layers.iter().collect();
    cleanup_layers_vec.sort_by(|(repo_a, layers_a), (repo_b, layers_b)| {
        let len_cmp = layers_b.len().cmp(&layers_a.len());
        if len_cmp == std::cmp::Ordering::Equal {
            repo_a.name.cmp(&repo_b.name)
        } else {
            len_cmp
        }
    });

    for (repo, layers) in cleanup_layers_vec {
        println!(
            "Would clean up {} layers for repository: {}/{}",
            layers.len(),
            repo.owner,
            repo.name
        );
    }

    if cleanup.all || cleanup.blobs {
        println!("Would clean up {} blobs", cleanup_plan.cleanup_blobs.len());
        let mut total_bytes = 0u64;

        for blob_name in &cleanup_plan.cleanup_blobs {
            let first_two = &blob_name[..2];
            let blob_path = cleanup_plan
                .repository
                .blobs_dir
                .join(first_two)
                .join(blob_name)
                .join("data");
            if let Ok(metadata) = fs::metadata(&blob_path) {
                total_bytes += metadata.len();
            }
        }

        println!(
            "Total space that would be freed: {} ({} bytes)",
            humansize::SizeFormatter::new(total_bytes, humansize::BINARY),
            total_bytes
        );
    }
}

pub fn execute_plan(cleanup_plan: &CleanupPlan) {
    for (_repo, commit_dirs) in &cleanup_plan.cleanup_commits {
        for commit_dir in commit_dirs {
            if let Err(e) = fs::remove_dir_all(&commit_dir) {
                eprintln!(
                    "Failed to remove commit directory {}: {}",
                    commit_dir.display(),
                    e
                );
            }
        }
    }

    for (_repo, dirs) in &cleanup_plan.cleanup_indexes {
        for dir in dirs {
            if let Err(e) = fs::remove_dir_all(&dir) {
                eprintln!("Failed to remove index directory {}: {}", dir.display(), e);
            }
        }
    }

    for (_repo, dirs) in &cleanup_plan.cleanup_revisions {
        for dir in dirs {
            if let Err(e) = fs::remove_dir_all(&dir) {
                eprintln!(
                    "Failed to remove revision directory {}: {}",
                    dir.display(),
                    e
                );
            }
        }
    }

    for (repo, layers) in &cleanup_plan.cleanup_layers {
        for layer in layers {
            let layer_path = repo.layer_dir.join(&layer);
            if let Err(e) = fs::remove_dir_all(&layer_path) {
                eprintln!(
                    "Failed to remove layer directory {}: {}",
                    layer_path.display(),
                    e
                );
            }
        }
    }

    let blobs_dir = &cleanup_plan.repository.blobs_dir;

    for blob_name in &cleanup_plan.cleanup_blobs {
        let first_two = &blob_name[..2];
        let blob_path = blobs_dir.join(first_two).join(blob_name);
        if let Err(e) = fs::remove_dir_all(&blob_path) {
            eprintln!(
                "Failed to remove blob directory {}: {}",
                blob_path.display(),
                e
            );
        }
    }
}

pub fn cleanup_command(cleanup: Cleanup) -> Result<(), Box<dyn std::error::Error>> {
    let dir = &cleanup.dir;

    if !dir.exists() {
        eprintln!("Directory does not exist: {}", dir.display());
        exit(1);
    }

    if !cleanup.all && !cleanup.commits && !cleanup.indexes && !cleanup.layers && !cleanup.blobs {
        eprintln!(
            "No cleanup options specified. Use --all, --commits or --indexes or --layers or --blobs."
        );
        exit(1);
    }

    let repository = get_repository(dir.clone()).unwrap_or_else(|e| {
        eprintln!("Error finding repository: {}", e);
        exit(1);
    });

    let mut cleaned_up_tags = HashMap::<&Repository, HashSet<String>>::new();
    let mut existing_blobs = HashSet::<String>::new();
    let mut existing_blobs_by_repo = HashMap::<&Repository, HashSet<String>>::new();
    let mut cleanup_plan = CleanupPlan::new(&cleanup, &repository);

    if cleanup.all || cleanup.commits {
        for repo in &repository.repositories {
            let commit_dirs = find_commit_dirs(&repo.tag_dir).unwrap_or_else(|e| {
                eprintln!("Error finding commit directories: {}", e);
                exit(1);
            });

            if !commit_dirs.is_empty() {
                for commit_dir in &commit_dirs {
                    if let Some(commit_name) = commit_dir.file_name().and_then(|s| s.to_str()) {
                        cleaned_up_tags
                            .entry(repo)
                            .or_default()
                            .insert(commit_name.to_string());
                    }
                }

                cleanup_plan.cleanup_commits.insert(repo, commit_dirs);
            }
        }
    }

    for repo in &repository.repositories {
        let tag_dirs = fs::read_dir(&repo.tag_dir)
            .map_err(|e| e.to_string())
            .unwrap_or_else(|e| {
                eprintln!("Error reading tag directory: {}", e);
                exit(1);
            });
        let cleaned_up_tags_in_repo = cleaned_up_tags.entry(repo).or_default();
        let existing_blobs_in_repo = existing_blobs_by_repo.entry(repo).or_default();

        for entry in tag_dirs.flatten() {
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let tag_name = entry.file_name().to_string_lossy().to_string();

                if cleaned_up_tags_in_repo.contains(&tag_name) {
                    // This tag was cleaned up in the previous step, we can't use it anymore
                    println!(
                        "Skipping cleaned up tag: {}",
                        entry.file_name().to_string_lossy()
                    );
                    continue;
                }

                let tag_path = entry.path();
                let index_path = tag_path.join("index/sha256");
                let link_path = tag_path.join("current/link");

                if let Ok(link_content) = fs::read_to_string(&link_path) {
                    let commit_hash = strip_sha256_prefix(&link_content);
                    existing_blobs_in_repo.insert(commit_hash.clone());
                } else {
                    eprintln!("Could not read link file at {}", link_path.display());
                }

                if index_path.exists() {
                    if let Ok(entries) = fs::read_dir(&index_path) {
                        for entry in entries.flatten() {
                            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                                let revision_name = entry.file_name().to_string_lossy().to_string();

                                if cleanup.all || cleanup.indexes {
                                    if !existing_blobs_in_repo.contains(&revision_name) {
                                        cleanup_plan
                                            .cleanup_indexes
                                            .entry(repo)
                                            .or_default()
                                            .insert(entry.path());
                                        continue;
                                    }
                                }

                                let first_two = &revision_name[..2];
                                let data_path = repository
                                    .blobs_dir
                                    .join(first_two)
                                    .join(&revision_name)
                                    .join("data");
                                existing_blobs.insert(revision_name.clone());

                                handle_manifest_file(
                                    &data_path,
                                    &repository,
                                    &mut existing_blobs,
                                    existing_blobs_in_repo,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    for repo in &repository.repositories {
        let existing_blobs_in_repo = existing_blobs_by_repo.entry(repo).or_default();

        if cleanup.all || cleanup.layers {
            let layer_dirs = fs::read_dir(&repo.layer_dir)
                .map_err(|e| e.to_string())
                .unwrap_or_else(|e| {
                    eprintln!("Error reading layer directory: {}", e);
                    exit(1);
                });

            for entry in layer_dirs.flatten() {
                if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                    let layer_name = entry.file_name().to_string_lossy().to_string();
                    if !existing_blobs_in_repo.contains(&layer_name) {
                        cleanup_plan
                            .cleanup_layers
                            .entry(repo)
                            .or_default()
                            .insert(layer_name);
                    }
                }
            }
        }
    }

    for repo in &repository.repositories {
        let existing_blobs_in_repo = existing_blobs_by_repo.entry(repo).or_default();

        if cleanup.all || cleanup.layers {
            // Find dangling revisions
            let revision_dirs_to_remove = fs::read_dir(&repo.revision_dir)
                .map_err(|e| e.to_string())?
                .flatten()
                .filter_map(|entry| {
                    let file_name = entry.file_name();
                    let file_name_str = file_name.to_string_lossy();
                    if entry.file_type().map_or(false, |ft| ft.is_dir())
                        && !existing_blobs_in_repo.contains(&file_name_str.to_string())
                    {
                        Some(entry.path())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if !revision_dirs_to_remove.is_empty() {
                cleanup_plan
                    .cleanup_revisions
                    .entry(repo)
                    .or_default()
                    .extend(revision_dirs_to_remove);
            }

            let layer_dirs = fs::read_dir(&repo.layer_dir)
                .map_err(|e| e.to_string())
                .unwrap_or_else(|e| {
                    eprintln!("Error reading layer directory: {}", e);
                    exit(1);
                });

            for entry in layer_dirs.flatten() {
                if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                    let layer_name = entry.file_name().to_string_lossy().to_string();
                    if !existing_blobs_in_repo.contains(&layer_name) {
                        cleanup_plan
                            .cleanup_layers
                            .entry(repo)
                            .or_default()
                            .insert(layer_name);
                    }
                }
            }
        }
    }

    if cleanup.all || cleanup.blobs {
        let blob_dirs = fs::read_dir(&repository.blobs_dir)
            .map_err(|e| e.to_string())
            .unwrap_or_else(|e| {
                eprintln!("Error reading blobs directory: {}", e);
                exit(1);
            });

        for entry in blob_dirs.flatten() {
            // Recursively find all directories two levels deep (sha256/xx/actual_blob)
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let first_level = entry.path();
                let first_level_name = entry.file_name().to_string_lossy().to_string();

                // The first level should be two hex digits (e.g., "ab")
                if first_level_name.len() == 2
                    && first_level_name.chars().all(|c| c.is_ascii_hexdigit())
                {
                    if let Ok(second_level) = fs::read_dir(&first_level) {
                        for blob_entry in second_level.flatten() {
                            if blob_entry.file_type().map_or(false, |ft| ft.is_dir()) {
                                let blob_name =
                                    blob_entry.file_name().to_string_lossy().to_string();
                                if !existing_blobs.contains(&blob_name) {
                                    cleanup_plan.cleanup_blobs.insert(blob_name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    preview_plan(&cleanup_plan);

    if !cleanup.yes {
        println!("Do you want to proceed with the cleanup? (y/N)");

        let mut input = String::new();
        stdin().read_line(&mut input).expect("Failed to read line");

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cleanup aborted.");
            return Ok(());
        }
    }

    execute_plan(&cleanup_plan);

    Ok(())
}
