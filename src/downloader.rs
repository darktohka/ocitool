use flate2::read::GzDecoder;
use indicatif::ProgressBar;
use tokio::fs;

use crate::{
    client::{ImagePermission, ImagePermissions, OciClient, OciClientError},
    compose::{
        containerd::client::services::v1::{WriteAction, WriteContentRequest},
        lease::LeasedClient,
    },
    macros::{impl_error, impl_from_error},
    parser::{FullImage, FullImageWithTag},
    spec::{config::ImageConfig, enums::MediaType, index::ImageIndex, manifest::ImageManifest},
    whiteout::extract_tar,
    with_client,
};
use bytes::Bytes;
use futures::StreamExt;
use std::{collections::HashMap, io::Read, path::PathBuf, sync::Arc};
use tonic::Request;

impl_error!(OciDownloaderError);
impl_from_error!(OciClientError, OciDownloaderError);
impl_from_error!(reqwest::Error, OciDownloaderError);
impl_from_error!(serde_json::Error, OciDownloaderError);
impl_from_error!(std::io::Error, OciDownloaderError);
impl_from_error!(tonic::Status, OciDownloaderError);

pub struct OciDownloader {
    pub client: Arc<OciClient>,
    blob_dir: PathBuf,
    no_cache: bool,
}

impl OciDownloader {
    pub fn new(client: Arc<OciClient>, no_cache: bool) -> Self {
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir.join("ocitool"),
            None => PathBuf::from("/tmp/ocitool"),
        };
        let blob_dir = cache_dir.join("blobs");

        OciDownloader {
            client,
            blob_dir,
            no_cache,
        }
    }

    pub async fn download_index(
        &self,
        image: FullImageWithTag,
    ) -> Result<(ImageIndex, String), OciDownloaderError> {
        let url = format!("{}/manifests/{}", image.image.get_image_url(), image.tag);
        // println!("Downloading {}:{}...", image.image.image_name, image.tag);

        let response = self
            .client
            .client
            .get(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image.image,
                        permissions: ImagePermissions::Pull,
                    })
                    .await?,
            )
            .header("Accept", "application/vnd.oci.image.index.v1+json")
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            return Err(OciDownloaderError(format!(
                "Failed to download index: {}",
                status
            )));
        }

        let json = response.text().await?;
        let image_index: ImageIndex = serde_json::from_str(&json)?;
        Ok((image_index, json))
    }

    pub async fn load_blob_cache(&self, digest: &str) -> Option<Vec<u8>> {
        if self.no_cache {
            return None;
        }

        if !self.blob_dir.is_dir() {
            fs::create_dir_all(&self.blob_dir).await.ok()?;
        }

        let blob_path = self.blob_dir.join(digest.replace(":", "-"));
        fs::read(blob_path).await.ok()
    }

    pub fn write_blob_cache(&self, digest: &str, blob: &[u8]) -> Result<(), OciDownloaderError> {
        if self.no_cache {
            return Ok(());
        }

        let blob_path = self.blob_dir.join(digest.replace(":", "-"));
        std::fs::write(blob_path, blob)?;
        Ok(())
    }

    pub async fn download_manifest(
        &self,
        image: FullImage,
        digest: &str,
    ) -> Result<(ImageManifest, Bytes), OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            if let Ok(manifest) = serde_json::from_slice(&blob) {
                return Ok((manifest, blob.into()));
            }
        }

        let url = format!("{}/manifests/{}", image.get_image_url(), digest);

        // println!("Downloading manifest {}:{}...", image.image_name, digest);

        let response = self
            .client
            .client
            .get(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image,
                        permissions: ImagePermissions::Pull,
                    })
                    .await?,
            )
            .header("Accept", "application/vnd.oci.image.manifest.v1+json")
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            return Err(OciDownloaderError(format!(
                "Failed to download manifest: {}",
                status
            )));
        }

        let json = response.bytes().await?;
        self.write_blob_cache(digest, &json)?;
        let result = serde_json::from_slice(&json)?;
        Ok((result, json))
    }

    pub async fn download_config(
        &self,
        image: FullImage,
        digest: &str,
    ) -> Result<(ImageConfig, Bytes), OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            if let Ok(config) = serde_json::from_slice(&blob) {
                return Ok((config, blob.into()));
            }
        }

        let url = format!("{}/blobs/{}", image.get_image_url(), digest);

        // println!("Downloading config {}:{}...", image.image_name, digest);

        let response = self
            .client
            .client
            .get(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image,
                        permissions: ImagePermissions::Pull,
                    })
                    .await?,
            )
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            return Err(OciDownloaderError(format!(
                "Failed to download config: {}",
                status
            )));
        }

        let json = response.bytes().await?;
        self.write_blob_cache(digest, &json)?;
        let result = serde_json::from_slice(&json)?;
        Ok((result, json))
    }

    pub async fn extract_layer_bytes_to<T: Read>(
        &self,
        bytes: T,
        media_type: &MediaType,
        dest_dir: &PathBuf,
    ) -> Result<(), OciDownloaderError> {
        match media_type {
            MediaType::OciImageLayerV1Tar => {
                extract_tar(bytes, dest_dir).await?;
                Ok(())
            }
            MediaType::OciImageLayerV1TarGzip => {
                let decoder = GzDecoder::new(bytes);
                extract_tar(decoder, dest_dir).await?;
                Ok(())
            }
            MediaType::OciImageLayerV1TarZstd => {
                let decoder = zstd::stream::Decoder::new(bytes)?;
                extract_tar(decoder, dest_dir).await?;
                Ok(())
            }
            _ => {
                return Err(OciDownloaderError(format!(
                    "Unsupported media type: {:?}",
                    media_type
                )));
            }
        }
    }

    pub async fn extract_layer(
        &self,
        image: FullImage,
        digest: &str,
        media_type: &MediaType,
        dest_dir: &PathBuf,
    ) -> Result<(), OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            self.extract_layer_bytes_to(&blob[..], media_type, &dest_dir)
                .await?;

            return Ok(());
        }

        let url = format!("{}/blobs/{}", image.get_image_url(), digest);
        // println!("Downloading layer {}:{}...", image.image_name, digest);

        let response = self
            .client
            .client
            .get(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image,
                        permissions: ImagePermissions::Pull,
                    })
                    .await?,
            )
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            return Err(OciDownloaderError(format!(
                "Failed to download layer: {}",
                status
            )));
        }

        let bytes = response.bytes().await?;
        self.write_blob_cache(digest, &bytes)?;
        self.extract_layer_bytes_to(bytes.as_ref(), media_type, &dest_dir)
            .await?;

        Ok(())
    }

    pub async fn download_layer(
        &self,
        image: FullImage,
        digest: &str,
    ) -> Result<Vec<u8>, OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            return Ok(blob);
        }

        let url = format!("{}/blobs/{}", image.get_image_url(), digest);
        // println!("Downloading layer {}:{}...", image.image_name, digest);

        let response = self
            .client
            .client
            .get(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image,
                        permissions: ImagePermissions::Pull,
                    })
                    .await?,
            )
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            return Err(OciDownloaderError(format!(
                "Failed to download layer: {}",
                status
            )));
        }

        let bytes = response.bytes().await?;
        self.write_blob_cache(digest, &bytes)?;
        Ok(bytes.to_vec())
    }

    pub async fn download_layer_to_containerd(
        &self,
        container_client: Arc<LeasedClient>,
        image: FullImage,
        digest: &str,
        uncompressed_digest: &str,
        progress_bar: ProgressBar,
        spinner: Option<&ProgressBar>,
        downloaded_bytes: Arc<tokio::sync::Mutex<u64>>,
    ) -> Result<(), OciDownloaderError> {
        let tick = || {
            if let Some(spinner) = spinner {
                spinner.tick();
            }
        };
        let url = format!("{}/blobs/{}", image.get_image_url(), digest);

        let response = self
            .client
            .client
            .get(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image.clone(),
                        permissions: ImagePermissions::Pull,
                    })
                    .await?,
            )
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            return Err(OciDownloaderError(format!(
                "Failed to download layer: {}",
                status
            )));
        }

        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|val| val.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let mut labels = HashMap::new();
        labels.insert(
            "containerd.io/distribution.source.docker.io".to_string(),
            image.library_name.clone(),
        );
        labels.insert(
            "containerd.io/uncompressed".to_string(),
            uncompressed_digest.to_string(),
        );

        // Stream the response in 16MB chunks
        let mut stream = response.bytes_stream();
        const CHUNK_SIZE: usize = 16 * 1000 * 1000;
        let mut buffer = Vec::with_capacity(CHUNK_SIZE);

        let mut offset = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.extend_from_slice(&chunk);

            while buffer.len() >= CHUNK_SIZE {
                let chunk_to_write = buffer.drain(..CHUNK_SIZE).collect::<Vec<u8>>();
                let chunk_length = chunk_to_write.len();

                let upload_request = WriteContentRequest {
                    action: WriteAction::Write as i32,
                    r#ref: digest.to_string(),
                    total: content_length as i64,
                    expected: "".to_string(),
                    offset,
                    data: chunk_to_write.into(),
                    labels: HashMap::new(),
                };

                let request_stream = with_client!(
                    futures_util::stream::iter(vec![upload_request]),
                    container_client
                );

                let content = container_client
                    .client()
                    .content()
                    .write(request_stream)
                    .await?;
                offset += chunk_length as i64;
                *downloaded_bytes.lock().await += chunk_length as u64;
                progress_bar.set_position(*downloaded_bytes.lock().await);
                tick();

                let mut stream = content.into_inner();
                loop {
                    match stream.message().await {
                        Ok(None) => break,
                        Ok(_) => {}
                        Err(e) => return Err(e.into()),
                    }
                    // Wait for the upload to complete
                }
            }
        }

        // Handle any remaining bytes in buffer
        if !buffer.is_empty() {
            let length = buffer.len();
            let upload_request = WriteContentRequest {
                action: WriteAction::Write as i32,
                r#ref: digest.to_string(),
                total: content_length as i64,
                expected: "".to_string(),
                offset,
                data: buffer.into(),
                labels: HashMap::new(),
            };

            let request_stream = with_client!(
                futures_util::stream::iter(vec![upload_request]),
                container_client
            );

            let content = container_client
                .client()
                .content()
                .write(request_stream)
                .await?;

            let mut stream = content.into_inner();
            loop {
                match stream.message().await {
                    Ok(None) => break,
                    Ok(_) => {}
                    Err(e) => return Err(e.into()),
                }
                // Wait for the upload to complete
            }

            // Update the offset after the final write
            offset += length as i64;
            *downloaded_bytes.lock().await += length as u64;
            progress_bar.set_position(*downloaded_bytes.lock().await);
            tick();
        }

        // Finalize with a commit
        let upload_request = WriteContentRequest {
            action: WriteAction::Commit as i32,
            r#ref: digest.to_string(),
            total: content_length as i64,
            expected: "".to_string(),
            offset,
            data: vec![],
            labels,
        };

        let request_stream = with_client!(
            futures_util::stream::iter(vec![upload_request]),
            container_client
        );

        let content = container_client
            .client()
            .content()
            .write(request_stream)
            .await?;
        let mut stream = content.into_inner();

        loop {
            match stream.message().await {
                Ok(None) => break,
                Ok(_) => {}
                Err(e) => return Err(e.into()),
            }
            // Wait for the upload to complete
        }

        Ok(())
    }
}
