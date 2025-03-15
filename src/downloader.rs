use flate2::read::GzDecoder;
use tokio::{fs, sync::Mutex};

use crate::{
    client::{ImagePermissions, OciClient, OciClientError},
    macros::{impl_error, impl_from_error},
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
    client: Arc<Mutex<OciClient>>,
    blob_dir: PathBuf,
    no_cache: bool,
}

impl OciDownloader {
    pub fn new(client: Arc<Mutex<OciClient>>, no_cache: bool) -> Self {
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
        image_name: &str,
        tag: &str,
    ) -> Result<ImageIndex, OciDownloaderError> {
        let mut client = self.client.lock().await;
        let url = format!("{}/manifests/{}", client.get_image_url(image_name), tag);

        println!("Downloading {}:{}...", image_name, tag);

        let response = client
            .client
            .get(&url)
            .headers(
                client
                    .auth_headers(image_name, ImagePermissions::Pull)
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
        image_name: &str,
        digest: &str,
    ) -> Result<ImageManifest, OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            if let Ok(manifest) = serde_json::from_slice(&blob) {
                return Ok(manifest);
            }
        }

        let mut client = self.client.lock().await;
        let url = format!("{}/manifests/{}", client.get_image_url(image_name), digest);

        println!("Downloading manifest {}:{}...", image_name, digest);

        let response = client
            .client
            .get(&url)
            .headers(
                client
                    .auth_headers(image_name, ImagePermissions::Pull)
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
        image_name: &str,
        digest: &str,
    ) -> Result<ImageConfig, OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            if let Ok(config) = serde_json::from_slice(&blob) {
                return Ok(config);
            }
        }

        let mut client = self.client.lock().await;
        let url = format!("{}/blobs/{}", client.get_image_url(image_name), digest);

        println!("Downloading config {}:{}...", image_name, digest);

        let response = client
            .client
            .get(&url)
            .headers(
                client
                    .auth_headers(image_name, ImagePermissions::Pull)
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
        image_name: &str,
        digest: &str,
        media_type: &MediaType,
        dest_dir: &PathBuf,
    ) -> Result<(), OciDownloaderError> {
        if let Some(blob) = self.load_blob_cache(digest).await {
            self.extract_layer_bytes_to(&blob[..], media_type, &dest_dir)
                .await?;

            return Ok(());
        }

        let mut client = self.client.lock().await;
        let url = format!("{}/blobs/{}", client.get_image_url(image_name), digest);
        println!("Downloading layer {}:{}...", image_name, digest);

        let response = client
            .client
            .get(&url)
            .headers(
                client
                    .auth_headers(image_name, ImagePermissions::Pull)
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
}
