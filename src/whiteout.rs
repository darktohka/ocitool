use std::{io::Read, path::Path};

use tar::Archive;
use tokio::fs;
use walkdir::WalkDir;

pub async fn extract_tar<R: Read>(reader: R, output_dir: &Path) -> Result<(), std::io::Error> {
    let mut archive = Archive::new(reader);

    // Unpack the archive
    archive.unpack(output_dir)?;

    for entry in WalkDir::new(output_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
            if file_name.starts_with(".wh.") {
                let dir = path.parent().unwrap();
                let whiteout_file = dir.join(file_name.replace(".wh.", ""));

                // Remove the original file or directory
                if whiteout_file.is_dir() {
                    fs::remove_dir_all(&whiteout_file).await?;
                } else if whiteout_file.exists() {
                    fs::remove_file(&whiteout_file).await?;
                }

                // Remove the whiteout file itself
                fs::remove_file(path).await?;
            }
        }
    }

    Ok(())
}
