use crate::{
    archive::detect_media_type,
    client::{ImagePermission, ImagePermissions, OciClient},
    digest::sha256_digest,
    downloader::{IndexResponse, OciDownloader},
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
    pub media_type: MediaType,
}

pub struct Digest {
    pub compressed_digest: String,
    pub uncompressed_digest: String,
}

impl Layer {
    pub fn to_descriptor(&self) -> Descriptor {
        Descriptor {
            media_type: self.media_type.clone(),
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

    fn build_layer(
        &self,
        data: Vec<u8>,
        digest: Digest,
        comment: &str,
        media_type: MediaType,
    ) -> (Blob, Layer) {
        let blob = Blob {
            digest: digest.compressed_digest.clone(),
            data,
        };

        let layer = Layer {
            uncompressed_digest: digest.uncompressed_digest,
            digest: digest.compressed_digest,
            size: blob.data.len() as u64,
            comment: comment.to_string(),
            media_type,
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

                        vec![(compressed_tar_buffer, digest, MediaType::OciImageLayerV1TarZstd)]
                    }
                    ImagePlanLayerType::Layer => {
                        let tar_buffer = fs::read(&layer.source).unwrap();
                        let (compressed_tar_buffer, digest) = self.compress_tar(&tar_buffer).await;

                        vec![(compressed_tar_buffer, digest, MediaType::OciImageLayerV1TarZstd)]
                    }
                    ImagePlanLayerType::Image => {
                        let image_name = layer.source.clone();
                        let image = FullImageWithTag::from_image_name(&image_name);

                        let index = self
                            .downloader
                            .download_index(image.clone())
                            .await
                            .map_err(|e| OciUploaderError(e.to_string()))?
                            .0;

                        let platform_matcher =
                            PlatformMatcher::match_architecture(platform.architecture.clone());

                        let downloaded_manifest = match index {
                            IndexResponse::ImageIndex(index) => {
                                let manifest =
                                    platform_matcher.find_manifest(&index.manifests).ok_or(
                                        OciUploaderError("No matching platform found".to_string()),
                                    )?;

                                let downloaded_manifest = self
                                    .downloader
                                    .download_manifest(image.image.clone(), &manifest.digest)
                                    .await
                                    .map_err(|e| OciUploaderError(e.to_string()))?
                                    .0;

                                Ok::<ImageManifest, OciUploaderError>(downloaded_manifest)
                            }
                            IndexResponse::ImageManifest(index) => Ok(index),
                        }?;

                        let downloaded_config: ImageConfig = self
                            .downloader
                            .download_config(
                                image.image.clone(),
                                &downloaded_manifest.config.digest,
                            )
                            .await
                            .unwrap()
                            .0;

                        let mut tar_layers: Vec<(Vec<u8>, Digest, MediaType)> = vec![];

                        for (index, layer) in downloaded_manifest.layers.iter().enumerate() {
                            let layer_data = self
                                .downloader
                                .download_layer(image.image.clone(), &layer.digest)
                                .await
                                .unwrap();

                            let media_type = detect_media_type(&layer_data)
                                .unwrap_or_else(|_| layer.media_type.clone())
                                .to_oci_layer_media_type();

                            tar_layers.push((
                                layer_data,
                                Digest {
                                    compressed_digest: layer.digest.clone(),
                                    uncompressed_digest: downloaded_config.rootfs.diff_ids[index]
                                        .clone(),
                                },
                                media_type,
                            ));
                        }

                        tar_layers
                    }
                };

                for (tar_buffer, digest, media_type) in tar_buffers {
                    let layer_comment = layer.comment.clone();
                    let (blob, new_layer) =
                        self.build_layer(tar_buffer, digest, &layer_comment, media_type);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_execution() -> PlanExecution {
        let plan = ImagePlan {
            name: "registry.example.com/test".to_string(),
            tags: vec!["latest".to_string()],
            platforms: vec![],
            config: None,
        };

        PlanExecution::new(plan, Arc::new(OciClient::new(HashMap::new(), None)), true, 3)
    }

    fn sample_tar() -> Vec<u8> {
        let mut tar_buffer = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_buffer);
            let mut header = tar::Header::new_gnu();
            header.set_size(4);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "file.txt", &b"test"[..])
                .unwrap();
            builder.finish().unwrap();
        }

        tar_buffer
    }

    #[tokio::test]
    async fn compress_tar_produces_zstd_blob_with_correct_digests() {
        let execution = test_execution();
        let tar_buffer = sample_tar();

        let (compressed, digest) = execution.compress_tar(&tar_buffer).await;

        assert_eq!(&compressed[0..4], &[0x28, 0xB5, 0x2F, 0xFD]);
        assert_eq!(digest.uncompressed_digest, sha256_digest(&tar_buffer));
        assert_eq!(digest.compressed_digest, sha256_digest(&compressed));
        assert_ne!(digest.compressed_digest, digest.uncompressed_digest);
    }

    #[tokio::test]
    async fn tar_layer_blob_is_zstd_and_descriptor_matches() {
        let execution = test_execution();
        let tar_buffer = sample_tar();

        let (compressed, digest) = execution.compress_tar(&tar_buffer).await;
        let (blob, layer) =
            execution.build_layer(compressed, digest, "test", MediaType::OciImageLayerV1TarZstd);
        let descriptor = layer.to_descriptor();

        assert_eq!(
            descriptor.media_type.to_string(),
            "application/vnd.oci.image.layer.v1.tar+zstd"
        );
        assert_eq!(descriptor.digest, blob.digest);
        assert_eq!(descriptor.size, blob.data.len() as u64);
        assert_eq!(
            detect_media_type(&blob.data).unwrap().to_string(),
            "application/vnd.oci.image.layer.v1.tar+zstd"
        );
    }

    #[test]
    fn detects_uncompressed_tar_as_tar_media_type() {
        let tar_buffer = sample_tar();

        assert_eq!(
            detect_media_type(&tar_buffer).unwrap().to_string(),
            "application/vnd.oci.image.layer.v1.tar"
        );
    }

    #[test]
    fn descriptor_uses_layer_media_type() {
        let layer = Layer {
            uncompressed_digest: "sha256:uncompressed".to_string(),
            digest: "sha256:compressed".to_string(),
            size: 42,
            comment: "test".to_string(),
            media_type: MediaType::OciImageLayerV1TarGzip,
        };

        let descriptor = layer.to_descriptor();

        assert_eq!(
            descriptor.media_type.to_string(),
            "application/vnd.oci.image.layer.v1.tar+gzip"
        );
        assert_eq!(descriptor.digest, "sha256:compressed");
        assert_eq!(descriptor.size, 42);
    }

    #[test]
    fn maps_docker_layer_media_types_to_oci() {
        assert_eq!(
            MediaType::DockerImageRootfsDiffTarGzip
                .to_oci_layer_media_type()
                .to_string(),
            "application/vnd.oci.image.layer.v1.tar+gzip"
        );
        assert_eq!(
            MediaType::DockerImageRootfsDiffTarZstd
                .to_oci_layer_media_type()
                .to_string(),
            "application/vnd.oci.image.layer.v1.tar+zstd"
        );
        assert_eq!(
            MediaType::DockerImageRootfsDiffTar
                .to_oci_layer_media_type()
                .to_string(),
            "application/vnd.oci.image.layer.v1.tar"
        );
        assert_eq!(
            MediaType::OciImageLayerV1TarGzip
                .to_oci_layer_media_type()
                .to_string(),
            "application/vnd.oci.image.layer.v1.tar+gzip"
        );
    }
}
