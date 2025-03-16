use crate::{
    client::OciClient,
    digest::sha256_digest,
    downloader::OciDownloader,
    parser::ParsedImage,
    platform::PlatformMatcher,
    spec::{
        config::{History, ImageConfig, RootFs},
        enums::{MediaType, PlatformOS},
        index::{ImageIndex, Manifest, Platform},
        manifest::{Descriptor, ImageManifest},
        plan::merge_image_plan_configs,
    },
    uploader::OciUploaderError,
    walk::walk_with_filters,
};
use regex_lite::Regex;
use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::spec::plan::{ImagePlan, ImagePlanLayerType};
use std::{io::Write, sync::Arc};
use tar::Builder;
use zstd::stream::write::Encoder;

use crate::uploader::OciUploader;
use std::fs;

pub struct PlanExecution {
    pub plan: ImagePlan,
    pub downloader: OciDownloader,
    pub uploader: OciUploader,
    pub compression_level: i32,
}

pub struct Blob {
    pub digest: String,
    pub data: Vec<u8>,
}

pub struct Layer {
    pub uncompressed_digest: String,
    pub digest: String,
    pub size: u64,
    pub comment: String,
}

impl Layer {
    pub fn to_descriptor(&self) -> Descriptor {
        Descriptor {
            media_type: MediaType::OciImageLayerV1TarZstd,
            digest: self.digest.clone(),
            size: self.size,
            data: None,
        }
    }

    pub fn to_history(&self) -> History {
        History {
            created: Some(OffsetDateTime::now_utc()),
            created_by: Some(self.comment.clone()),
            author: None,
            comment: None,
            empty_layer: None,
        }
    }
}

impl PlanExecution {
    pub fn new(
        plan: ImagePlan,
        client: Arc<Mutex<OciClient>>,
        no_cache: bool,
        compression_level: i32,
    ) -> Self {
        PlanExecution {
            plan,
            downloader: OciDownloader::new(client.clone(), no_cache),
            uploader: OciUploader::new(client),
            compression_level,
        }
    }

    async fn compress_and_upload_tar(
        &self,
        tar_buffer: &Vec<u8>,
        comment: &str,
        compress: bool,
    ) -> (Blob, Layer) {
        let uncompressed_digest = sha256_digest(&tar_buffer);

        if !compress {
            return self
                .upload_tar(
                    tar_buffer.clone(),
                    uncompressed_digest.clone(),
                    uncompressed_digest,
                    comment,
                )
                .await;
        }

        let mut encoder = Encoder::new(Vec::new(), self.compression_level).unwrap();

        // Enable multithreading
        encoder.multithread(num_cpus::get() as u32).unwrap();

        encoder.write_all(&tar_buffer).unwrap();
        let compressed_data = encoder.finish().unwrap();
        let compressed_digest = sha256_digest(&compressed_data);

        println!(
            "Compressing layer: {}, original size: {}, compressed size: {} ({:.2}% of original size)",
            compressed_digest,
            tar_buffer.len(),
            compressed_data.len(),
            (compressed_data.len() as f64 / tar_buffer.len() as f64) * 100.0
        );

        return self
            .upload_tar(
                compressed_data,
                uncompressed_digest,
                compressed_digest,
                comment,
            )
            .await;
    }

    async fn upload_tar(
        &self,
        compressed_data: Vec<u8>,
        uncompressed_digest: String,
        compressed_digest: String,
        comment: &str,
    ) -> (Blob, Layer) {
        let blob = Blob {
            digest: compressed_digest.clone(),
            data: compressed_data,
        };

        let layer = Layer {
            uncompressed_digest,
            digest: compressed_digest,
            size: blob.data.len() as u64,
            comment: comment.to_string(),
        };

        (blob, layer)
    }

    pub async fn execute(&mut self) -> Result<(), OciUploaderError> {
        let mut manifests: Vec<Manifest> = vec![];

        for platform in &self.plan.platforms {
            let mut layers: Vec<Layer> = vec![];

            for layer in &platform.layers {
                let tar_buffers = match layer.layer_type {
                    ImagePlanLayerType::Directory => {
                        let whitelist_regexes: Vec<Regex> =
                            layer.whitelist.clone().map_or_else(Vec::new, |b| {
                                b.iter().map(|s| Regex::new(s).unwrap()).collect::<Vec<_>>()
                            });
                        let blacklist_regexes: Vec<Regex> =
                            layer.blacklist.clone().map_or_else(Vec::new, |b| {
                                b.iter().map(|s| Regex::new(s).unwrap()).collect::<Vec<_>>()
                            });
                        let files = walk_with_filters(
                            &layer.source,
                            &whitelist_regexes,
                            &blacklist_regexes,
                        );

                        println!(
                            "Creating layer from directory: {} (collected {} files)",
                            layer.source,
                            files.len()
                        );

                        let mut tar_buffer = Vec::new();

                        {
                            let mut tar_builder = Builder::new(&mut tar_buffer);
                            tar_builder.follow_symlinks(false);

                            for file_path in files {
                                tar_builder
                                    .append_path_with_name(
                                        &file_path,
                                        file_path.strip_prefix(&layer.source).unwrap(),
                                    )
                                    .unwrap();
                            }

                            tar_builder.finish().unwrap();
                        }

                        vec![(tar_buffer, true)]
                    }
                    ImagePlanLayerType::Layer => vec![(fs::read(&layer.source).unwrap(), true)],
                    ImagePlanLayerType::Image => {
                        let image_name = layer.source.clone();
                        let image = ParsedImage::from_image_name(&image_name);

                        let index = self
                            .downloader
                            .download_index(&image.library_name, &image.tag)
                            .await
                            .unwrap();

                        let platform_matcher =
                            PlatformMatcher::match_architecture(platform.architecture.clone());

                        let manifest = platform_matcher
                            .find_manifest(&index.manifests)
                            .ok_or(OciUploaderError("No matching platform found".to_string()))
                            .unwrap();

                        let downloaded_manifest: ImageManifest = self
                            .downloader
                            .download_manifest(&image.library_name, &manifest.digest)
                            .await
                            .unwrap();

                        let mut tar_layers: Vec<(Vec<u8>, bool)> = vec![];

                        for layer in downloaded_manifest.layers {
                            let layer_data = self
                                .downloader
                                .download_layer(&image.library_name, &layer.digest)
                                .await
                                .unwrap();

                            tar_layers.push((layer_data, false));
                        }

                        tar_layers
                    }
                };

                for (tar_buffer, compress) in tar_buffers {
                    let layer_comment = layer.comment.clone();
                    let (blob, new_layer) = self
                        .compress_and_upload_tar(&tar_buffer, &layer_comment, compress)
                        .await;
                    self.uploader.upload_blob(&self.plan.name, &blob).await?;
                    layers.push(new_layer);
                }
            }

            let platform_config = merge_image_plan_configs(&self.plan.config, &platform.config);
            let image_config = ImageConfig {
                created: Some(OffsetDateTime::now_utc()),
                author: None,
                architecture: platform.architecture.clone(),
                os: PlatformOS::Linux,
                os_version: None,
                os_features: None,
                variant: platform.variant.clone(),
                config: platform_config,
                rootfs: RootFs {
                    fs_type: "layers".to_string(),
                    diff_ids: layers
                        .iter()
                        .map(|d| d.uncompressed_digest.clone())
                        .collect(),
                },
                history: Some(layers.iter().map(|l| l.to_history()).collect()),
            };

            let config_data = image_config.to_json();
            let config_blob = Blob {
                digest: sha256_digest(&config_data),
                data: config_data,
            };

            self.uploader
                .upload_blob(&self.plan.name, &config_blob)
                .await?;

            let manifest = ImageManifest {
                schema_version: 2,
                media_type: MediaType::OciImageManifestV1Json,
                artifact_type: None,
                config: Descriptor {
                    media_type: MediaType::OciImageConfigV1ConfigJson,
                    digest: config_blob.digest.clone(),
                    size: config_blob.data.len() as u64,
                    data: None,
                },
                layers: layers.iter().map(|l| l.to_descriptor()).collect(),
                subject: None,
                annotations: None,
            };

            let manifest_data = manifest.to_json();

            let manifest_blob = Blob {
                digest: sha256_digest(&manifest_data),
                data: manifest_data.clone(),
            };

            manifests.push(Manifest {
                media_type: MediaType::OciImageManifestV1Json,
                size: manifest_blob.data.len() as u64,
                digest: manifest_blob.digest.clone(),
                platform: Some(Platform {
                    architecture: platform.architecture.clone(),
                    os: PlatformOS::Linux,
                    os_version: None,
                    os_features: None,
                    variant: platform.variant.clone(),
                    features: None,
                }),
            });

            for tag in &self.plan.tags {
                self.uploader
                    .upload_manifest(
                        &self.plan.name,
                        manifest_data.clone(),
                        "application/vnd.oci.image.manifest.v1+json",
                        tag,
                    )
                    .await?;
            }
        }

        let index = ImageIndex {
            schema_version: 2,
            media_type: MediaType::OciImageIndexV1Json,
            artifact_type: None,
            manifests,
            annotations: None,
        };
        let index_data = index.to_json();

        for tag in &self.plan.tags {
            self.uploader
                .upload_manifest(
                    &self.plan.name,
                    index_data.clone(),
                    "application/vnd.oci.image.index.v1+json",
                    tag,
                )
                .await?;
        }

        Ok(())
    }
}
