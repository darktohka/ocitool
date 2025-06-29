use crate::compose::containerd::client::services::v1::{
    CreateImageRequest, Image, ListContentRequest, ListImagesRequest, UpdateImageRequest,
    WriteAction, WriteContentRequest,
};
use crate::compose::containerd::client::types;
use crate::compose::lease::LeasedClient;
use crate::downloader::OciDownloader;
use crate::platform::PlatformMatcher;
use crate::spec::manifest::Descriptor;
use crate::with_client;
use crate::{
    client::{ImagePermission, ImagePermissions, OciClient},
    compose::{containerd::client::Client, docker_compose_finder::find_and_parse_docker_composes},
    parser::FullImageWithTag,
    system_login::get_system_login,
    Compose, Pull,
};
use prost_types::Timestamp;
use sha256::digest;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request};

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
}

pub async fn get_existing_digests_from_containerd(
    container_client: Arc<LeasedClient>,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let list_content_request =
        with_client!(ListContentRequest { filters: vec![] }, container_client);
    let content = container_client
        .client()
        .content()
        .list(list_content_request)
        .await;

    let mut stream = match content {
        Ok(response) => response.into_inner(),
        Err(e) => {
            eprintln!("Failed to list content: {}", e);
            return Err(Box::new(e));
        }
    };

    let mut existing_digests = HashSet::<String>::new();
    while let Some(item) = stream.message().await? {
        for info in &item.info {
            existing_digests.insert(info.digest.clone());
        }
    }

    Ok(existing_digests)
}

pub async fn upload_content_to_containerd(
    container_client: Arc<LeasedClient>,
    digest: &str,
    data: Vec<u8>,
    labels: HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let upload_request = WriteContentRequest {
        action: WriteAction::Commit as i32,
        r#ref: digest.to_string(),
        total: data.len() as i64,
        expected: "".to_string(),
        offset: 0,
        data,
        labels,
    };

    let request_stream = with_client!(
        futures_util::stream::iter(vec![upload_request]),
        container_client
    );
    let content = match container_client
        .client()
        .content()
        .write(request_stream)
        .await
    {
        Ok(response) => response,
        Err(status) => {
            if status.code() == Code::AlreadyExists {
                println!(
                    "Content with digest {} already exists, skipping upload.",
                    digest
                );
                return Ok(());
            }

            eprintln!("Failed to upload content: {}", status);
            return Err(Box::new(status));
        }
    };

    let mut stream = content.into_inner();
    if let Ok(Some(response)) = stream.message().await {
        println!(
            "Successfully uploaded content with digest: {}",
            response.digest
        );
    }

    println!("Content upload completed successfully.");

    Ok(())
}

pub async fn create_image_in_containerd(
    container_client: Arc<LeasedClient>,
    full_image: &FullImageWithTag,
    index_digest: String,
    index_length: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match container_client
        .client()
        .images()
        .create(with_client!(
            CreateImageRequest {
                image: Some(Image {
                    name: format!(
                        "docker.io/{}:{}",
                        full_image.image.library_name, full_image.tag
                    ),
                    labels: HashMap::new(),
                    target: Some(types::Descriptor {
                        media_type: "application/vnd.oci.image.index.v1+json".to_string(),
                        digest: index_digest.clone(),
                        size: index_length,
                        annotations: HashMap::new(),
                    }),
                    created_at: Some(Timestamp::default()),
                    updated_at: Some(Timestamp::default())
                }),
                source_date_epoch: None,
            },
            container_client
        ))
        .await
    {
        Ok(response) => {
            println!(
                "Image created successfully: {} with digest {}",
                full_image.image.library_name, index_digest
            );
            Ok(())
        }
        Err(status) => {
            if status.code() == Code::AlreadyExists {
                return match container_client
                    .client()
                    .images()
                    .update(with_client!(
                        UpdateImageRequest {
                            image: Some(Image {
                                name: format!(
                                    "docker.io/{}:{}",
                                    full_image.image.library_name, full_image.tag
                                ),
                                labels: HashMap::new(),
                                target: Some(types::Descriptor {
                                    media_type: "application/vnd.oci.image.index.v1+json"
                                        .to_string(),
                                    digest: index_digest.clone(),
                                    size: index_length,
                                    annotations: HashMap::new(),
                                }),
                                created_at: Some(Timestamp::default()),
                                updated_at: Some(Timestamp::default())
                            }),
                            source_date_epoch: None,
                            update_mask: None,
                        },
                        container_client
                    ))
                    .await
                {
                    Ok(response) => {
                        println!(
                            "Image updated successfully: {} with digest {}",
                            full_image.image.library_name, index_digest
                        );
                        println!("Image response: {:?}", response);
                        Ok(())
                    }
                    Err(status) => {
                        eprintln!("Failed to update image: {}", status);
                        Err(Box::new(status))
                    }
                };
            }

            eprintln!("Failed to upload content: {}", status);
            return Err(Box::new(status));
        }
    }
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

    let downloader = Arc::new(OciDownloader::new(client.clone(), true));
    let mut tasks = vec![];

    for i in 0..8 {
        let downloader = downloader.clone();
        let download_queue = pull_instance.download_queue.clone();
        let existing_digests = pull_instance.existing_digests.clone();
        let container_client = pull_instance.container_client.clone();

        let task = tokio::spawn(async move {
            let platform_matcher = PlatformMatcher::new();

            let queue_if_not_download = async |digest: &str, something| {
                let mut existing_digests = existing_digests.lock().await;

                if !existing_digests.contains(digest) {
                    let mut queue = download_queue.lock().await;
                    queue.push(something);
                    existing_digests.insert(digest.to_string());
                }
            };

            while let Some(downloadable) = {
                let mut queue = download_queue.lock().await;
                queue.pop()
            } {
                match downloadable {
                    Downloadable::Index(index_to_download) => {
                        println!(
                            "Thread {} downloading index for {:?}",
                            i, index_to_download.full_image
                        );
                        match downloader
                            .download_index(index_to_download.full_image.clone())
                            .await
                        {
                            Ok((image_index, image_json)) => {
                                let image_json_len = image_json.len();
                                let image_digest = format!("sha256:{}", digest(&image_json));
                                println!("Image digest: {}", image_digest);
                                println!(
                                    "Thread {} downloaded index for {}",
                                    i, index_to_download.full_image.image.library_name
                                );
                                upload_content_to_containerd(
                                    container_client.clone(),
                                    &image_digest,
                                    image_json.into_bytes(),
                                    {
                                        let mut labels = HashMap::new();
                                        labels.insert(
                                            "containerd.io/distribution.source.docker.io"
                                                .to_string(),
                                            index_to_download.full_image.image.library_name.clone(),
                                        );
                                        for (idx, manifest) in
                                            image_index.manifests.iter().enumerate()
                                        {
                                            labels.insert(
                                                format!("containerd.io/gc.ref.content.m.{}", idx),
                                                manifest.digest.clone(),
                                            );
                                        }
                                        labels
                                    },
                                )
                                .await
                                .expect("Failed to upload index to containerd");

                                create_image_in_containerd(
                                    container_client.clone(),
                                    &index_to_download.full_image,
                                    image_digest.clone(),
                                    image_json_len as i64,
                                )
                                .await
                                .expect("msg: Failed to create image in containerd");

                                println!(
                                    "Thread {} downloaded index, found {} manifests.",
                                    i,
                                    image_index.manifests.len()
                                );
                                let manifest =
                                    platform_matcher.find_manifest(&image_index.manifests);
                                if let Some(manifest) = manifest {
                                    println!(
                                        "Thread {} found manifest for platform: {:?}",
                                        i, manifest.platform
                                    );

                                    // Check if the manifest digest is already in the download queue
                                    queue_if_not_download(
                                        &manifest.digest,
                                        Downloadable::Manifest(DownloadableManifest {
                                            digest: manifest.digest.clone(),
                                            full_image: index_to_download.full_image.clone(),
                                        }),
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                eprintln!("Thread {} failed to download index: {}", i, e);
                            }
                        }
                    }
                    Downloadable::Manifest(manifest_to_download) => {
                        println!(
                            "Thread {} downloading manifest for {:?} with digest {}",
                            i, manifest_to_download.full_image, manifest_to_download.digest
                        );
                        match downloader
                            .download_manifest(
                                manifest_to_download.full_image.image.clone(),
                                &manifest_to_download.digest,
                            )
                            .await
                        {
                            Ok((manifest, manifest_json)) => {
                                println!("Thread {} downloaded manifest: {:?}", i, manifest_json);

                                // UPLOADING A MANIFEST //
                                upload_content_to_containerd(
                                    container_client.clone(),
                                    &manifest_to_download.digest,
                                    manifest_json.into(),
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

                                queue_if_not_download(
                                    &manifest.config.digest,
                                    Downloadable::Config(DownloadableConfig {
                                        full_image: manifest_to_download.full_image.clone(),
                                        layers: manifest.layers.clone(),
                                        digest: manifest.config.digest.clone(),
                                    }),
                                )
                                .await;
                            }
                            Err(e) => {
                                eprintln!("Thread {} failed to download manifest: {}", i, e);
                            }
                        }
                    }
                    Downloadable::Config(config_to_download) => {
                        println!(
                            "Thread {} downloading config for {:?} with digest {}",
                            i, config_to_download.full_image, config_to_download.digest
                        );
                        match downloader
                            .download_config(
                                config_to_download.full_image.image.clone(),
                                &config_to_download.digest,
                            )
                            .await
                        {
                            Ok((config, config_bytes)) => {
                                println!("Thread {} downloaded config: {:?}", i, config_bytes);

                                // UPLOADING A CONFIG //
                                upload_content_to_containerd(
                                    container_client.clone(),
                                    &config_to_download.digest,
                                    config_bytes.into(),
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
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                eprintln!("Thread {} failed to download config: {}", i, e);
                            }
                        }
                    }
                    Downloadable::Layer(layer_to_download) => {
                        println!(
                            "Thread {} downloading layer for {:?} with digest {}, uncompressed digest {}",
                            i, layer_to_download.full_image, layer_to_download.digest, layer_to_download.uncompressed_digest
                        );

                        match downloader
                            .download_layer_to_containerd(
                                container_client.clone(),
                                layer_to_download.full_image.image.clone(),
                                &layer_to_download.digest,
                                &layer_to_download.uncompressed_digest,
                            )
                            .await
                        {
                            Ok(()) => {
                                println!(
                                    "Thread {} downloaded layer: {}",
                                    i, layer_to_download.digest
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "\x1b[31mThread {} failed to download layer: {}\x1b[0m",
                                    i, e
                                );
                            }
                        }
                    }
                    _ => {
                        eprintln!(
                            "\x1b[31mThread {} encountered an unsupported downloadable type: {:?}\x1b[0m",
                            i, downloadable
                        );
                    }
                }
            }
        });
        tasks.push(task);
    }

    futures::future::join_all(tasks).await;
    Ok(())
}

pub async fn pull_command(
    compose_settings: &Compose,
    pull_settings: &Pull,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let mut images_to_pull = HashSet::<String>::new();

    for compose in composes {
        println!("Pulling images for {}", compose.compose_path.display());

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
    for image in &full_images {
        println!("Would pull image: {:?}", image);
    }

    println!("\nAttempting to connect to containerd...");
    let container_client = Client::from_path("/run/containerd/containerd.sock").await?;
    let leased_client =
        Arc::new(LeasedClient::new(Arc::new(container_client), "default".to_string()).await?);

    println!(
        "Leased client created with namespace: {} and lease id {}",
        leased_client.namespace(),
        leased_client.lease_id()
    );
    //let version = container_client.version().version(()).await?;
    //    container_client.content().get_content_store().await?;
    //println!("Containerd Version: {:?}", version);

    let existing_digests = get_existing_digests_from_containerd(leased_client.clone()).await?;
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
    };

    match run_pull(&pull_instance).await {
        Ok(_) => {
            println!("Pull completed successfully.");
            pull_instance.container_client.delete_lease().await;
            Ok(())
        }
        Err(e) => {
            eprintln!("Error during pull: {}", e);
            pull_instance.container_client.delete_lease().await;
            Err(e)
        }
    }

    //Ok(())
}
