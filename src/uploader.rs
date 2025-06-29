use crate::{
    client::{ImagePermission, ImagePermissions, OciClient, OciClientError},
    execution::Blob,
    macros::{impl_error, impl_from_error},
    parser::{FullImage, FullImageWithTag},
};
use reqwest::{
    header::{CONTENT_LENGTH, CONTENT_TYPE},
    StatusCode,
};
use std::{collections::HashSet, sync::Arc};

pub struct OciUploader {
    client: Arc<OciClient>,
    uploaded_blobs: HashSet<String>,
}

impl_error!(OciUploaderError);
impl_from_error!(OciClientError, OciUploaderError);
impl_from_error!(reqwest::Error, OciUploaderError);

impl OciUploader {
    pub fn new(client: Arc<OciClient>) -> Self {
        OciUploader {
            client,
            uploaded_blobs: HashSet::new(),
        }
    }

    async fn blob_exists(
        &mut self,
        image: FullImage,
        blob: &Blob,
    ) -> Result<bool, OciUploaderError> {
        if self.uploaded_blobs.contains(&blob.digest) {
            println!("Blob {} was already uploaded.", blob.digest);
            return Ok(true);
        }

        println!("Checking blob {}...", blob.digest);

        let url = format!("{}/blobs/{}", image.get_image_url(), blob.digest);
        let response = self
            .client
            .client
            .head(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image,
                        permissions: ImagePermissions::Push,
                    })
                    .await?,
            )
            .send()
            .await?;

        let status = response.status();

        if status.is_server_error() {
            return Err(OciUploaderError(format!(
                "Failed to check blob: {}",
                status
            )));
        }

        let exists = status == StatusCode::OK;

        if exists {
            self.uploaded_blobs.insert(blob.digest.clone());
        }

        Ok(exists)
    }

    pub async fn upload_blob(
        &mut self,
        image: FullImage,
        blob: &Blob,
    ) -> Result<(), OciUploaderError> {
        let exists = self.blob_exists(image.clone(), &blob).await?;

        if exists {
            println!("Blob {} already exists.", blob.digest);
            return Ok(());
        }

        let url = format!("{}/blobs/uploads/", image.get_image_url());
        let registry = image.registry.clone();

        let headers = self
            .client
            .auth_headers(ImagePermission {
                full_image: image,
                permissions: ImagePermissions::Push,
            })
            .await?;

        let response = self
            .client
            .client
            .post(&url)
            .headers(headers.clone())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(OciUploaderError(format!(
                "Failed to initiate blob upload: {}",
                response.status()
            )));
        }

        let location = response
            .headers()
            .get("location")
            .ok_or(OciUploaderError("No location header".to_string()))?
            .to_str()
            .map_err(|e| OciUploaderError(e.to_string()))?;

        let location = if location.starts_with('/') {
            format!("{}{}", registry, location)
        } else {
            location.to_string()
        };

        let upload_url = if location.contains('?') {
            format!("{}&digest={}", location, blob.digest)
        } else {
            format!("{}?digest={}", location, blob.digest)
        };

        let request = self
            .client
            .client
            .put(upload_url)
            .headers(headers)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, blob.data.len() as u64)
            .body(blob.data.clone());

        let response = request.send().await?;

        match response.status() {
            StatusCode::CREATED => {
                println!("Blob {} uploaded.", blob.digest);
                self.uploaded_blobs.insert(blob.digest.clone());
                Ok(())
            }
            code => Err(OciUploaderError(format!("Failed to upload blob: {}", code))),
        }
    }

    pub async fn upload_manifest(
        &self,
        image: FullImageWithTag,
        manifest_data: Vec<u8>,
        content_type: &str,
    ) -> Result<(), OciUploaderError> {
        let url = format!("{}/manifests/{}", image.image.get_image_url(), image.tag);

        println!("Uploading {}:{}...", image.image.image_name, image.tag);

        let response = self
            .client
            .client
            .put(&url)
            .headers(
                self.client
                    .auth_headers(ImagePermission {
                        full_image: image.image,
                        permissions: ImagePermissions::Push,
                    })
                    .await?,
            )
            .header("Content-Type", content_type)
            .body(manifest_data)
            .send()
            .await?;

        match response.status() {
            StatusCode::CREATED => {
                println!("Manifest uploaded successfully.");
                Ok(())
            }
            code => Err(OciUploaderError(format!(
                "Failed to upload manifest: {}",
                code
            ))),
        }
    }
}
