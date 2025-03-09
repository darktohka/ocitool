use crate::{
    client::{ImagePermissions, OciClient, OciClientError},
    execution::Blob,
    macros::{impl_error, impl_from_error},
};
use reqwest::{
    header::{CONTENT_LENGTH, CONTENT_TYPE},
    StatusCode,
};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;

pub struct OciUploader {
    client: Arc<Mutex<OciClient>>,
    uploaded_blobs: HashSet<String>,
}

impl_error!(OciUploaderError);
impl_from_error!(OciClientError, OciUploaderError);
impl_from_error!(reqwest::Error, OciUploaderError);

impl OciUploader {
    pub fn new(client: Arc<Mutex<OciClient>>) -> Self {
        OciUploader {
            client,
            uploaded_blobs: HashSet::new(),
        }
    }

    async fn blob_exists(
        &mut self,
        image_name: &str,
        blob: &Blob,
    ) -> Result<bool, OciUploaderError> {
        if self.uploaded_blobs.contains(&blob.digest) {
            println!("Blob {} was already uploaded.", blob.digest);
            return Ok(true);
        }

        println!("Checking blob {}...", blob.digest);

        let mut client = self.client.lock().await;

        let url = format!("{}/blobs/{}", client.get_image_url(image_name), blob.digest);
        let response = client
            .client
            .head(&url)
            .headers(
                client
                    .auth_headers(image_name, ImagePermissions::Push)
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
        image_name: &str,
        blob: &Blob,
    ) -> Result<(), OciUploaderError> {
        let exists = self.blob_exists(image_name, &blob).await?;

        if exists {
            println!("Blob {} already exists.", blob.digest);
            return Ok(());
        }

        let mut client = self.client.lock().await;
        let headers = client
            .auth_headers(image_name, ImagePermissions::Push)
            .await?;

        let url = format!("{}/blobs/uploads/", client.get_image_url(image_name));
        let response = client
            .client
            .post(&url)
            .headers(headers.clone())
            .send()
            .await?;

        let location = response
            .headers()
            .get("location")
            .ok_or(OciUploaderError("No location header".to_string()))?
            .to_str()
            .map_err(|e| OciUploaderError(e.to_string()))?;

        let location = if location.starts_with('/') {
            format!("{}{}", client.registry, location)
        } else {
            location.to_string()
        };

        let upload_url = if location.contains('?') {
            format!("{}&digest={}", location, blob.digest)
        } else {
            format!("{}?digest={}", location, blob.digest)
        };

        let request = client
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
        image_name: &str,
        manifest_data: Vec<u8>,
        content_type: &str,
        tag: &str,
    ) -> Result<(), OciUploaderError> {
        let mut client = self.client.lock().await;
        let url = format!("{}/manifests/{}", client.get_image_url(image_name), tag);

        println!("Uploading {}:{}...", image_name, tag);

        let response = client
            .client
            .put(&url)
            .headers(
                client
                    .auth_headers(image_name, ImagePermissions::Push)
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
