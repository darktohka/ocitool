use std::path::PathBuf;

use regex_lite::Regex;
use walkdir::WalkDir;

pub fn walk_with_filters(
    root: &str,
    whitelist: &Vec<Regex>,
    blacklist: &Vec<Regex>,
) -> Vec<PathBuf> {
    let mut results = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.into_path();

        if path.is_dir() {
            continue;
        }

        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check whitelist (if any)
        let whitelist_pass =
            whitelist.is_empty() || whitelist.iter().any(|regex| regex.is_match(filename));

        // Check blacklist (if any)
        let blacklist_pass =
            blacklist.is_empty() || !blacklist.iter().any(|regex| regex.is_match(filename));

        // If both checks pass, add to results
        if whitelist_pass && blacklist_pass {
            results.push(path);
        }
    }

    results
}
