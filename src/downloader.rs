use tokio::sync::Mutex;

use crate::{
    client::{ImagePermissions, OciClient, OciClientError},
    macros::{impl_error, impl_from_error},
    spec::{config::ImageConfig, index::ImageIndex, manifest::ImageManifest},
};
use std::sync::Arc;

impl_error!(OciDownloaderError);
impl_from_error!(OciClientError, OciDownloaderError);
impl_from_error!(reqwest::Error, OciDownloaderError);
impl_from_error!(serde_json::Error, OciDownloaderError);

pub struct OciDownloader {
    client: Arc<Mutex<OciClient>>,
}

impl OciDownloader {
    pub fn new(client: Arc<Mutex<OciClient>>) -> Self {
        OciDownloader { client }
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

    pub async fn download_manifest(
        &self,
        image_name: &str,
        digest: &str,
    ) -> Result<ImageManifest, OciDownloaderError> {
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

        let json = response.text().await?;
        println!("Manifest: {}", json);
        let manifest: ImageManifest = serde_json::from_str(&json)?;
        Ok(manifest)
    }

    pub async fn download_config(
        &self,
        image_name: &str,
        digest: &str,
    ) -> Result<ImageConfig, OciDownloaderError> {
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

        let json = response.text().await?;
        let config: ImageConfig = serde_json::from_str(&json)?;
        Ok(config)
    }
}
