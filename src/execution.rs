use crate::{
    client::{ImagePermission, ImagePermissions, OciClient},
    digest::sha256_digest,
    downloader::OciDownloader,
    parser::{FullImage, FullImageWithTag},
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

use crate::spec::plan::{ImagePlan, ImagePlanLayerType};
use std::{collections::HashSet, io::Write, sync::Arc};
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

pub struct Digest {
    pub compressed_digest: String,
    pub uncompressed_digest: String,
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
        client: Arc<OciClient>,
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

    async fn compress_tar(&self, tar_buffer: &Vec<u8>) -> (Vec<u8>, Digest) {
        let uncompressed_digest = sha256_digest(&tar_buffer);
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

        return (
            compressed_data,
            Digest {
                compressed_digest,
                uncompressed_digest,
            },
        );
    }

    fn build_layer(&self, data: Vec<u8>, digest: Digest, comment: &str) -> (Blob, Layer) {
        let blob = Blob {
            digest: digest.compressed_digest.clone(),
            data,
        };

        let layer = Layer {
            uncompressed_digest: digest.uncompressed_digest,
            digest: digest.compressed_digest,
            size: blob.data.len() as u64,
            comment: comment.to_string(),
        };

        (blob, layer)
    }

    pub async fn execute(&mut self) -> Result<(), OciUploaderError> {
        let mut manifests: Vec<Manifest> = vec![];
        let full_image = FullImage::from_image_name(&self.plan.name);

        // First things first, log into every registry necessary
        let mut image_permissions = HashSet::<ImagePermission>::new();

        image_permissions.insert(ImagePermission {
            full_image: full_image.clone(),
            permissions: ImagePermissions::Push,
        });

        for platform in &self.plan.platforms {
            for layer in &platform.layers {
                if let ImagePlanLayerType::Image = layer.layer_type {
                    let image_name = layer.source.clone();
                    let image = FullImageWithTag::from_image_name(&image_name);

                    image_permissions.insert(ImagePermission {
                        full_image: image.image.clone(),
                        permissions: ImagePermissions::Pull,
                    });
                }
            }
        }

        let image_permissions_vec: Vec<ImagePermission> = image_permissions.into_iter().collect();
        self.downloader.client.login(&image_permissions_vec).await?;

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

                        let (compressed_tar_buffer, digest) = self.compress_tar(&tar_buffer).await;

                        vec![(compressed_tar_buffer, digest)]
                    }
                    ImagePlanLayerType::Layer => {
                        let layer_data = fs::read(&layer.source).unwrap();
                        let digest = sha256_digest(&layer_data);
                        vec![(
                            layer_data,
                            Digest {
                                compressed_digest: digest.clone(),
                                uncompressed_digest: digest,
                            },
                        )]
                    }
                    ImagePlanLayerType::Image => {
                        let image_name = layer.source.clone();
                        let image = FullImageWithTag::from_image_name(&image_name);

                        let index = self
                            .downloader
                            .download_index(image.clone())
                            .await
                            .unwrap()
                            .0;

                        let platform_matcher =
                            PlatformMatcher::match_architecture(platform.architecture.clone());

                        let manifest = platform_matcher
                            .find_manifest(&index.manifests)
                            .ok_or(OciUploaderError("No matching platform found".to_string()))
                            .unwrap();

                        let downloaded_manifest: ImageManifest = self
                            .downloader
                            .download_manifest(image.image.clone(), &manifest.digest)
                            .await
                            .unwrap()
                            .0;

                        let downloaded_config: ImageConfig = self
                            .downloader
                            .download_config(
                                image.image.clone(),
                                &downloaded_manifest.config.digest,
                            )
                            .await
                            .unwrap()
                            .0;

                        let mut tar_layers: Vec<(Vec<u8>, Digest)> = vec![];

                        for (index, layer) in downloaded_manifest.layers.iter().enumerate() {
                            let layer_data = self
                                .downloader
                                .download_layer(image.image.clone(), &layer.digest)
                                .await
                                .unwrap();

                            tar_layers.push((
                                layer_data,
                                Digest {
                                    compressed_digest: layer.digest.clone(),
                                    uncompressed_digest: downloaded_config.rootfs.diff_ids[index]
                                        .clone(),
                                },
                            ));
                        }

                        tar_layers
                    }
                };

                for (tar_buffer, digest) in tar_buffers {
                    let layer_comment = layer.comment.clone();
                    let (blob, new_layer) = self.build_layer(tar_buffer, digest, &layer_comment);
                    self.uploader.upload_blob(full_image.clone(), &blob).await?;
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
                .upload_blob(full_image.clone(), &config_blob)
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
                        FullImageWithTag {
                            image: full_image.clone(),
                            tag: tag.to_string(),
                        },
                        manifest_data.clone(),
                        "application/vnd.oci.image.manifest.v1+json",
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
                    FullImageWithTag {
                        image: full_image.clone(),
                        tag: tag.to_string(),
                    },
                    index_data.clone(),
                    "application/vnd.oci.image.index.v1+json",
                )
                .await?;
        }

        Ok(())
    }
}
