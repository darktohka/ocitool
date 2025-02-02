use crate::{
    digest::sha256_digest,
    spec::{
        config::{History, ImageConfig, RootFs},
        enums::{MediaType, PlatformOS},
        index::{ImageIndex, Manifest, Platform},
        manifest::{Descriptor, ImageManifest},
        plan::merge_image_plan_configs,
    },
    walk::walk_with_filters,
};
use regex_lite::Regex;
use time::OffsetDateTime;

use crate::spec::plan::{ImagePlan, ImagePlanLayerType};
use std::io::Write;
use tar::Builder;
use zstd::stream::write::Encoder;

use crate::uploader::OciUploader;
use std::fs;

pub struct PlanExecution {
    pub plan: ImagePlan,
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
        service: Option<String>,
        username: Option<String>,
        password: Option<String>,
        compression_level: i32,
    ) -> Self {
        let uploader = OciUploader::new(
            &plan.get_registry_url(),
            &service.unwrap_or_else(|| plan.get_service_url()),
            &plan.name,
            username,
            password,
        );

        PlanExecution {
            plan,
            uploader,
            compression_level,
        }
    }

    async fn upload_tar(&self, tar_buffer: &Vec<u8>, comment: &str) -> (Blob, Layer) {
        let uncompressed_digest = sha256_digest(&tar_buffer);

        let mut encoder = Encoder::new(Vec::new(), self.compression_level).unwrap();
        encoder.write_all(&tar_buffer).unwrap();
        let compressed_data = encoder.finish().unwrap();
        let digest = sha256_digest(&compressed_data);

        println!(
            "Compressing layer: {}, original size: {}, compressed size: {} ({:.2}% of original size)",
            digest,
            tar_buffer.len(),
            compressed_data.len(),
            (compressed_data.len() as f64 / tar_buffer.len() as f64) * 100.0
        );

        let blob = Blob {
            digest: digest.clone(),
            data: compressed_data,
        };

        let layer = Layer {
            uncompressed_digest,
            digest: digest.clone(),
            size: blob.data.len() as u64,
            comment: comment.to_string(),
        };

        (blob, layer)
    }

    pub async fn execute(&mut self) {
        let mut manifests: Vec<Manifest> = vec![];

        for platform in &self.plan.platforms {
            let mut layers: Vec<Layer> = vec![];

            for layer in &platform.layers {
                match layer.layer_type {
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

                        let layer_comment = layer.comment.clone();
                        let (blob, new_layer) = self.upload_tar(&tar_buffer, &layer_comment).await;
                        self.uploader.upload_blob(&blob).await.unwrap();
                        layers.push(new_layer);
                    }
                    ImagePlanLayerType::Layer => {
                        let tar_buffer = fs::read(&layer.source).unwrap();
                        let layer_comment = layer.comment.clone();
                        let (blob, new_layer) = self.upload_tar(&tar_buffer, &layer_comment).await;
                        self.uploader.upload_blob(&blob).await.unwrap();
                        layers.push(new_layer);
                    }
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
            self.uploader.upload_blob(&config_blob).await.unwrap();

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
                        manifest_data.clone(),
                        "application/vnd.oci.image.manifest.v1+json",
                        tag,
                    )
                    .await
                    .unwrap();
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
                    index_data.clone(),
                    "application/vnd.oci.image.index.v1+json",
                    tag,
                )
                .await
                .unwrap();
        }
    }
}
