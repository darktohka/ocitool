use crate::{
    client::{OciClient, OciClientError},
    execution::Blob,
};
use reqwest::{
    header::{HeaderValue, CONTENT_LENGTH, CONTENT_TYPE},
    StatusCode,
};
use std::{collections::HashSet, error::Error};

#[derive(Debug, Clone)]

pub struct OciUploaderError(String);

impl<'a> Error for OciUploaderError {}

impl<'a> std::fmt::Display for OciUploaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct OciUploader {
    uploaded_blobs: HashSet<String>,
}

impl From<OciClientError> for OciUploaderError {
    fn from(err: OciClientError) -> Self {
        OciUploaderError(err.to_string())
    }
}

impl From<reqwest::Error> for OciUploaderError {
    fn from(err: reqwest::Error) -> Self {
        OciUploaderError(err.to_string())
    }
}

impl OciUploader {
    pub fn new() -> Self {
        OciUploader {
            uploaded_blobs: HashSet::new(),
        }
    }

    async fn blob_exists(
        &mut self,
        client: &mut OciClient,
        image_name: &str,
        blob: &Blob,
    ) -> Result<bool, OciUploaderError> {
        if self.uploaded_blobs.contains(&blob.digest) {
            println!("Blob {} was already uploaded.", blob.digest);
            return Ok(true);
        }

        println!("Checking blob {}...", blob.digest);

        let url = format!("{}/blobs/{}", client.get_image_url(image_name), blob.digest);
        let response = client
            .client
            .head(&url)
            .headers(client.auth_headers(image_name).await?)
            .send()
            .await?;

        let status = response.status();

        if status.is_server_error() {
            return Err(OciUploaderError(format!(
                "Failed to check blob: {}",
                status
            )));
        }

        let exists = response.status() == StatusCode::OK;

        if exists {
            self.uploaded_blobs.insert(blob.digest.clone());
        }

        Ok(exists)
    }

    pub async fn upload_blob(
        &mut self,
        client: &mut OciClient,
        image_name: &str,
        blob: &Blob,
    ) -> Result<(), OciUploaderError> {
        let exists = self.blob_exists(client, image_name, &blob).await?;

        if exists {
            println!("Blob {} already exists.", blob.digest);
            return Ok(());
        }

        let mut headers = client.auth_headers(image_name).await?;

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

        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        headers.insert(CONTENT_LENGTH, HeaderValue::from(blob.data.len()));

        let request = client
            .client
            .put(upload_url)
            .headers(headers)
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
        client: &mut OciClient,
        image_name: &str,
        manifest_data: Vec<u8>,
        content_type: &str,
        tag: &str,
    ) -> Result<(), OciUploaderError> {
        let url = format!("{}/manifests/{}", client.get_image_url(image_name), tag);

        println!("Uploading {}:{}...", image_name, tag);

        let response = client
            .client
            .put(&url)
            .headers(client.auth_headers(image_name).await?)
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
