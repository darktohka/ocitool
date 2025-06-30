mod containerd_utils;

use crate::compose::lease::LeasedClient;
use crate::downloader::{IndexResponse, OciDownloader};
use crate::platform::PlatformMatcher;
use crate::spec::manifest::Descriptor;
use crate::{
    client::{ImagePermission, ImagePermissions, OciClient},
    compose::docker_compose_finder::find_and_parse_docker_composes,
    parser::FullImageWithTag,
    system_login::get_system_login,
    Compose,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sha256::digest;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct DownloadableIndex {
    pub full_image: FullImageWithTag,
}

#[derive(Debug, Clone)]
pub struct DownloadableManifest {
    pub full_image: FullImageWithTag,
    pub digest: String,
}

#[derive(Debug, Clone)]
pub struct DownloadableConfig {
    pub full_image: FullImageWithTag,
    pub layers: Vec<Descriptor>,
    pub digest: String,
}

#[derive(Debug, Clone)]
pub struct DownloadableLayer {
    pub full_image: FullImageWithTag,
    pub digest: String,
    pub uncompressed_digest: String,
}

#[derive(Debug, Clone)]
pub enum Downloadable {
    Index(DownloadableIndex),
    Manifest(DownloadableManifest),
    Config(DownloadableConfig),
    Layer(DownloadableLayer),
}

pub struct PullInstance {
    pub container_client: Arc<LeasedClient>,
    pub existing_digests: Arc<Mutex<HashSet<String>>>,
    pub download_queue: Arc<Mutex<Vec<Downloadable>>>,
    pub total_bytes_to_download: Arc<Mutex<u64>>,
    pub downloaded_bytes: Arc<Mutex<u64>>,

    pub digest_to_image: Arc<Mutex<HashMap<String, FullImageWithTag>>>,
}

pub async fn run_pull(pull_instance: &PullInstance) -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(OciClient::new(get_system_login(), None));

    let image_permissions = {
        let queue = pull_instance.download_queue.lock().await;
        queue
            .iter()
            .filter_map(|downloadable| {
                if let Downloadable::Index(index) = downloadable {
                    Some(ImagePermission {
                        full_image: index.full_image.image.clone(),
                        permissions: ImagePermissions::Pull,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    client.login(&image_permissions).await?;

    let m = MultiProgress::new();
    let images = {
        let queue = pull_instance.download_queue.lock().await;

        let mut images: Vec<_> = queue
            .iter()
            .filter_map(|downloadable| {
                if let Downloadable::Index(index) = downloadable {
                    Some(index.full_image.clone())
                } else {
                    None
                }
            })
            .collect();

        images.sort_by(|a, b| a.image.library_name.cmp(&b.image.library_name));
        images
    };

    let spinners: HashMap<FullImageWithTag, ProgressBar> = images
        .iter()
        .map(|image| {
            let full_name = format!("{}:{}", image.image.library_name, image.tag);
            let progress_bar = m.add(ProgressBar::new(0));
            progress_bar.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .expect("Failed to set spinner style")
                    .progress_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
            );
            progress_bar.set_message(full_name);
            (image.clone(), progress_bar)
        })
        .collect();
    let spinners = Arc::new(spinners);

    let progress_bar = m.add(ProgressBar::new(0));
    progress_bar.set_style(ProgressStyle::default_bar()
        .template("{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}")
        .expect("Failed to set progress bar style")
        .progress_chars("#>-"));

    let downloader = Arc::new(OciDownloader::new(client.clone(), true));
    let total_bytes_to_download = pull_instance.total_bytes_to_download.clone();
    let downloaded_bytes = pull_instance.downloaded_bytes.clone();
    let mut tasks = vec![];

    for _ in 0..8 {
        let downloader = downloader.clone();
        let download_queue = pull_instance.download_queue.clone();
        let existing_digests = pull_instance.existing_digests.clone();
        let container_client = pull_instance.container_client.clone();
        let progress_bar = progress_bar.clone();
        let total_bytes_to_download = total_bytes_to_download.clone();
        let downloaded_bytes = downloaded_bytes.clone();
        let digest_to_image = pull_instance.digest_to_image.clone();
        let spinners = spinners.clone();

        let task = tokio::spawn(async move {
            let platform_matcher = PlatformMatcher::new();

            let download_failed = async |full_image: FullImageWithTag, error: String| {
                if let Some(spinner) = spinners.get(&full_image) {
                    if !spinner.is_finished() {
                        spinner.finish_with_message(format!(
                            "{}: \x1b[31mFailed - {}\x1b[0m",
                            spinner.message(),
                            error
                        ));
                    }
                }
            };

            let download_complete =
                async |full_image: FullImageWithTag, digest: String, size: u64| {
                    let full_image_clone = full_image.clone();

                    let is_complete = {
                        let mut digest_to_image = digest_to_image.lock().await;
                        digest_to_image.remove(&digest);
                        !digest_to_image
                            .values()
                            .any(|image| *image == full_image_clone)
                    };

                    if is_complete {
                        if let Some(spinner) = spinners.get(&full_image_clone) {
                            if !spinner.is_finished() {
                                spinner.finish_with_message(format!(
                                    "{}: \x1b[32mComplete\x1b[0m",
                                    spinner.message()
                                ));
                            }
                        }
                    } else {
                        if let Some(spinner) = spinners.get(&full_image) {
                            spinner.tick();
                        }
                    }

                    if size != 0 {
                        {
                            *downloaded_bytes.lock().await += size;
                            progress_bar.set_position(*downloaded_bytes.lock().await);
                        }
                    }
                };

            let queue_if_not_download =
                async |digest: &str, something, full_image: FullImageWithTag, size| {
                    let mut existing_digests = existing_digests.lock().await;

                    if existing_digests.contains(digest) {
                        false
                    } else {
                        digest_to_image
                            .lock()
                            .await
                            .insert(digest.to_string(), full_image);

                        let mut queue = download_queue.lock().await;
                        queue.push(something);
                        existing_digests.insert(digest.to_string());
                        *total_bytes_to_download.lock().await += size as u64;
                        progress_bar.set_length(*total_bytes_to_download.lock().await);
                        true
                    }
                };

            while let Some(downloadable) = {
                let mut queue = download_queue.lock().await;
                queue.pop()
            } {
                match downloadable {
                    Downloadable::Index(index_to_download) => {
                        match downloader
                            .download_index(index_to_download.full_image.clone())
                            .await
                        {
                            Ok((index_response, image_json)) => {
                                let image_json_len = image_json.len();
                                let image_digest = format!("sha256:{}", digest(&image_json));

                                *total_bytes_to_download.lock().await += image_json_len as u64;
                                *downloaded_bytes.lock().await += image_json_len as u64;
                                progress_bar.set_length(*total_bytes_to_download.lock().await);
                                progress_bar.set_position(*downloaded_bytes.lock().await);

                                if !existing_digests.lock().await.contains(&image_digest) {
                                    containerd_utils::upload_content_to_containerd(
                                        container_client.clone(),
                                        &image_digest,
                                        image_json.into_bytes(),
                                        {
                                            let mut labels = HashMap::new();
                                            labels.insert(
                                                "containerd.io/distribution.source.docker.io"
                                                    .to_string(),
                                                index_to_download
                                                    .full_image
                                                    .image
                                                    .library_name
                                                    .clone(),
                                            );
                                            match index_response {
                                                IndexResponse::ImageIndex(ref image_index) => {
                                                    for (idx, manifest) in
                                                        image_index.manifests.iter().enumerate()
                                                    {
                                                        labels.insert(
                                                            format!(
                                                                "containerd.io/gc.ref.content.m.{}",
                                                                idx
                                                            ),
                                                            manifest.digest.clone(),
                                                        );
                                                    }
                                                }
                                                IndexResponse::ImageManifest(ref manifest) => {
                                                    labels.insert(
                                                        "containerd.io/gc.ref.content.config"
                                                            .to_string(),
                                                        manifest.config.digest.clone(),
                                                    );
                                                    for (idx, layer) in
                                                        manifest.layers.iter().enumerate()
                                                    {
                                                        labels.insert(
                                                            format!(
                                                                "containerd.io/gc.ref.content.l.{}",
                                                                idx
                                                            ),
                                                            layer.digest.clone(),
                                                        );
                                                    }
                                                }
                                            }
                                            labels
                                        },
                                    )
                                    .await
                                    .expect("Failed to upload index to containerd");
                                    *downloaded_bytes.lock().await += image_json_len as u64;
                                    progress_bar.set_position(*downloaded_bytes.lock().await);
                                }

                                containerd_utils::create_image_in_containerd(
                                    container_client.clone(),
                                    &index_to_download.full_image,
                                    image_digest.clone(),
                                    image_json_len as i64,
                                    match index_response {
                                        IndexResponse::ImageIndex(ref index) => {
                                            index.media_type.to_string().into()
                                        }
                                        IndexResponse::ImageManifest(ref manifest) => {
                                            manifest.media_type.to_string().into()
                                        }
                                    },
                                )
                                .await
                                .expect("Failed to create image in containerd");

                                let downloading = match index_response {
                                    IndexResponse::ImageIndex(ref image_index) => {
                                        let manifest =
                                            platform_matcher.find_manifest(&image_index.manifests);
                                        if let Some(manifest) = manifest {
                                            // Check if the manifest digest is already in the download queue
                                            queue_if_not_download(
                                                &manifest.digest,
                                                Downloadable::Manifest(DownloadableManifest {
                                                    digest: manifest.digest.clone(),
                                                    full_image: index_to_download
                                                        .full_image
                                                        .clone(),
                                                }),
                                                index_to_download.full_image.clone(),
                                                manifest.size,
                                            )
                                            .await
                                        } else {
                                            println!("\x1b[33mNo matching platform found for image: {}:{}\x1b[0m",
                                                index_to_download.full_image.image.library_name, index_to_download.full_image.tag);
                                            false
                                        }
                                    }
                                    IndexResponse::ImageManifest(manifest) => {
                                        queue_if_not_download(
                                            &manifest.config.digest,
                                            Downloadable::Config(DownloadableConfig {
                                                full_image: index_to_download.full_image.clone(),
                                                layers: manifest.layers.clone(),
                                                digest: manifest.config.digest.clone(),
                                            }),
                                            index_to_download.full_image.clone(),
                                            manifest.config.size,
                                        )
                                        .await
                                    }
                                };

                                if !downloading {
                                    if let Some(spinner) =
                                        spinners.get(&index_to_download.full_image)
                                    {
                                        spinner.finish_with_message(format!(
                                            "{}: \x1b[33mUnchanged\x1b[0m",
                                            spinner.message()
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                download_failed(
                                    index_to_download.full_image.clone(),
                                    e.to_string(),
                                )
                                .await;
                            }
                        }
                    }
                    Downloadable::Manifest(manifest_to_download) => {
                        match downloader
                            .download_manifest(
                                manifest_to_download.full_image.image.clone(),
                                &manifest_to_download.digest,
                            )
                            .await
                        {
                            Ok((manifest, manifest_json)) => {
                                // UPLOADING A MANIFEST //
                                containerd_utils::upload_content_to_containerd(
                                    container_client.clone(),
                                    &manifest_to_download.digest,
                                    manifest_json.clone().into(),
                                    {
                                        let mut labels = HashMap::new();
                                        labels.insert(
                                            "containerd.io/distribution.source.docker.io"
                                                .to_string(),
                                            manifest_to_download
                                                .full_image
                                                .image
                                                .library_name
                                                .clone(),
                                        );
                                        labels.insert(
                                            "containerd.io/gc.ref.content.config".to_string(),
                                            manifest.config.digest.clone(),
                                        );
                                        for (idx, layer) in manifest.layers.iter().enumerate() {
                                            labels.insert(
                                                format!("containerd.io/gc.ref.content.l.{}", idx),
                                                layer.digest.clone(),
                                            );
                                        }

                                        labels
                                    },
                                )
                                .await
                                .expect("Failed to upload manifest to containerd");
                                *downloaded_bytes.lock().await += manifest_json.len() as u64;
                                progress_bar.set_position(*downloaded_bytes.lock().await);

                                queue_if_not_download(
                                    &manifest.config.digest,
                                    Downloadable::Config(DownloadableConfig {
                                        full_image: manifest_to_download.full_image.clone(),
                                        layers: manifest.layers.clone(),
                                        digest: manifest.config.digest.clone(),
                                    }),
                                    manifest_to_download.full_image.clone(),
                                    manifest.config.size,
                                )
                                .await;

                                download_complete(
                                    manifest_to_download.full_image.clone(),
                                    manifest_to_download.digest.clone(),
                                    manifest_json.len() as u64,
                                )
                                .await;
                            }
                            Err(e) => {
                                download_failed(
                                    manifest_to_download.full_image.clone(),
                                    e.to_string(),
                                )
                                .await;
                            }
                        }
                    }
                    Downloadable::Config(config_to_download) => {
                        match downloader
                            .download_config(
                                config_to_download.full_image.image.clone(),
                                &config_to_download.digest,
                            )
                            .await
                        {
                            Ok((config, config_bytes)) => {
                                // UPLOADING A CONFIG //
                                containerd_utils::upload_content_to_containerd(
                                    container_client.clone(),
                                    &config_to_download.digest,
                                    config_bytes.clone().into(),
                                    {
                                        let mut labels = HashMap::new();
                                        labels.insert(
                                            "containerd.io/distribution.source.docker.io"
                                                .to_string(),
                                            config_to_download
                                                .full_image
                                                .image
                                                .library_name
                                                .clone(),
                                        );
                                        labels
                                    },
                                )
                                .await
                                .expect("Failed to upload config to containerd");

                                for (idx, layer) in config_to_download.layers.iter().enumerate() {
                                    let layer_digest = layer.digest.clone();
                                    let uncompressed_digest = config
                                        .rootfs
                                        .diff_ids
                                        .get(idx)
                                        .cloned()
                                        .expect("Missing uncompressed digest");

                                    queue_if_not_download(
                                        &layer_digest.clone(),
                                        Downloadable::Layer(DownloadableLayer {
                                            full_image: config_to_download.full_image.clone(),
                                            digest: layer_digest,
                                            uncompressed_digest,
                                        }),
                                        config_to_download.full_image.clone(),
                                        layer.size,
                                    )
                                    .await;
                                }

                                download_complete(
                                    config_to_download.full_image.clone(),
                                    config_to_download.digest.clone(),
                                    config_bytes.len() as u64,
                                )
                                .await;
                            }
                            Err(e) => {
                                download_failed(
                                    config_to_download.full_image.clone(),
                                    e.to_string(),
                                )
                                .await;
                            }
                        }
                    }
                    Downloadable::Layer(layer_to_download) => {
                        match downloader
                            .download_layer_to_containerd(
                                container_client.clone(),
                                layer_to_download.full_image.image.clone(),
                                &layer_to_download.digest,
                                &layer_to_download.uncompressed_digest,
                                progress_bar.clone(),
                                spinners.get(&layer_to_download.full_image),
                                downloaded_bytes.clone(),
                            )
                            .await
                        {
                            Ok(()) => {
                                download_complete(
                                    layer_to_download.full_image.clone(),
                                    layer_to_download.digest.clone(),
                                    0,
                                )
                                .await;
                            }
                            Err(e) => {
                                download_failed(
                                    layer_to_download.full_image.clone(),
                                    e.to_string(),
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        });
        tasks.push(task);
    }

    futures::future::join_all(tasks).await;
    progress_bar.finish_with_message("Pull complete!");
    Ok(())
}

pub async fn pull_command(compose_settings: &Compose) -> Result<(), Box<dyn std::error::Error>> {
    let start_dir = compose_settings
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));
    let socket_path: PathBuf = compose_settings
        .socket
        .clone()
        .unwrap_or_else(|| "/run/containerd/containerd.sock".into());
    let max_depth = compose_settings.max_depth.unwrap_or(1);

    let composes = find_and_parse_docker_composes(&start_dir, max_depth);

    if composes.is_empty() {
        println!("No docker-compose files found in {}", start_dir.display());
        return Ok(());
    }

    let mut images_to_pull = HashSet::<String>::new();

    for compose in composes {
        for service in compose.compose.services.0.values() {
            if let Some(service) = service {
                if let Some(image) = &service.image {
                    images_to_pull.insert(image.clone());
                }
            }
        }
    }

    let mut images: Vec<_> = images_to_pull.into_iter().collect();
    images.sort();

    let full_images: Vec<FullImageWithTag> = images
        .into_iter()
        .map(|image| FullImageWithTag::from_image_name(&image))
        .collect();

    let leased_client = Arc::new(
        LeasedClient::with_path("default".to_string(), socket_path.to_str().unwrap()).await?,
    );

    let existing_digests =
        containerd_utils::get_existing_digests_from_containerd(leased_client.clone()).await?;
    let mut download_queue = Vec::<Downloadable>::new();

    for image in full_images {
        download_queue.push(Downloadable::Index(DownloadableIndex {
            full_image: image.clone(),
        }));
    }

    /*let all_images = leased_client
        .client()
        .images()
        .list(with_client!(
            ListImagesRequest { filters: vec![] },
            leased_client
        ))
        .await?
        .into_inner();

    println!("Existing images in containerd: {:?}", all_images);
    */
    let pull_instance = PullInstance {
        container_client: leased_client,
        existing_digests: Arc::new(Mutex::new(existing_digests)),
        download_queue: Arc::new(Mutex::new(download_queue)),
        total_bytes_to_download: Arc::new(Mutex::new(0)),
        downloaded_bytes: Arc::new(Mutex::new(0)),

        digest_to_image: Arc::new(Mutex::new(HashMap::new())),
    };

    match run_pull(&pull_instance).await {
        Ok(_) => {
            pull_instance.container_client.delete_lease().await;
            Ok(())
        }
        Err(e) => {
            eprintln!("Error during pull: {}", e);
            pull_instance.container_client.delete_lease().await;
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::tests::ContainerdTestEnv;
    use crate::{Compose, ComposeCmd, Pull};
    use std::fs;

    #[tokio::test]
    async fn test_pull_command() -> Result<(), Box<dyn std::error::Error>> {
        let env = ContainerdTestEnv::new().await?;
        let temp_dir = tempfile::tempdir()?;
        let compose_path = temp_dir.path().join("docker-compose.yaml");
        fs::write(
            &compose_path,
            r#"
services:
  nginx:
    image: nginx:alpine
  alpine:
    image: alpine:edge
"#,
        )?;

        let compose_settings = Compose {
            dir: Some(temp_dir.path().to_path_buf()),
            socket: Some(env.socket_path.clone()),
            max_depth: Some(1),
            subcommand: ComposeCmd::Pull(Pull {}),
        };

        let result = pull_command(&compose_settings).await;
        assert!(result.is_ok());
        Ok(())
    }
}
