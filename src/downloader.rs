use flate2::read::GzDecoder;
use tokio::fs;

use crate::{
    client::{ImagePermission, ImagePermissions, OciClient, OciClientError},
    macros::{impl_error, impl_from_error},
    parser::{FullImage, FullImageWithTag},
    spec::{config::ImageConfig, enums::MediaType, index::ImageIndex, manifest::ImageManifest},
    whiteout::extract_tar,
};
use std::{io::Read, path::PathBuf, sync::Arc};

impl_error!(OciDownloaderError);
impl_from_error!(OciClientError, OciDownloaderError);
impl_from_error!(reqwest::Error, OciDownloaderError);
impl_from_error!(serde_json::Error, OciDownloaderError);
impl_from_error!(std::io::Error, OciDownloaderError);

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
    ) -> Result<ImageIndex, OciDownloaderError> {
        let url = format!("{}/manifests/{}", image.image.get_image_url(), image.tag);
        println!("Downloading {}:{}...", image.image.image_name, image.tag);

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
        Ok(image_index)
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
    ) -> Result<ImageManifest, OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            if let Ok(manifest) = serde_json::from_slice(&blob) {
                return Ok(manifest);
            }
        }

        let url = format!("{}/manifests/{}", image.get_image_url(), digest);

        println!("Downloading manifest {}:{}...", image.image_name, digest);

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
        Ok(result)
    }

    pub async fn download_config(
        &self,
        image: FullImage,
        digest: &str,
    ) -> Result<ImageConfig, OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            if let Ok(config) = serde_json::from_slice(&blob) {
                return Ok(config);
            }
        }

        let url = format!("{}/blobs/{}", image.get_image_url(), digest);

        println!("Downloading config {}:{}...", image.image_name, digest);

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
        Ok(result)
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
        println!("Downloading layer {}:{}...", image.image_name, digest);

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
        println!("Downloading layer {}:{}...", image.image_name, digest);

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
}
