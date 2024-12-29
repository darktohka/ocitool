use reqwest::{Client, StatusCode};

use crate::execution::Blob;

pub struct OciUploader {
    registry: String,
    service: String,
    image_name: String,
    username: Option<String>,
    password: Option<String>,
    auth_header: Option<String>,
    uploaded_blobs: Vec<String>,
    client: Client,
}

impl OciUploader {
    pub fn get_image_url(name: &str) -> String {
        let parts: Vec<&str> = name.split('/').collect();

        if parts.len() > 2 {
            parts[1..].join("/")
        } else {
            name.to_string()
        }
    }

    pub fn get_auth_url(&self) -> String {
        if self.registry.contains("registry-1.docker.io")
            || self.registry.contains("registry.docker.io")
        {
            "https://auth.docker.io/token".to_string()
        } else {
            format!("{}/auth", self.registry)
        }
    }

    pub fn new(
        registry: &str,
        service: &str,
        image_name: &str,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to build HTTP client");

        OciUploader {
            registry: registry.to_string(),
            service: service.to_string(),
            image_name: OciUploader::get_image_url(image_name),
            username,
            password,
            uploaded_blobs: vec![],
            auth_header: None,
            client,
        }
    }

    async fn blob_exists(&mut self, blob: &Blob) -> Result<bool, Box<dyn std::error::Error>> {
        if self.uploaded_blobs.contains(&blob.digest) {
            println!("Blob {} was already uploaded.", blob.digest);
            return Ok(true);
        }

        println!("Checking blob {}...", blob.digest);
        let url = format!(
            "{}/v2/{}/blobs/{}",
            self.registry, self.image_name, blob.digest
        );
        let response = self
            .client
            .head(&url)
            .headers(self.auth_headers().await)
            .send()
            .await?;

        let exists = response.status() == StatusCode::OK;

        if exists {
            self.uploaded_blobs.push(blob.digest.clone());
        }

        Ok(exists)
    }

    pub async fn upload_blob(&mut self, blob: &Blob) -> Result<(), Box<dyn std::error::Error>> {
        let exists = self.blob_exists(&blob).await?;

        if exists {
            println!("Blob {} already exists.", blob.digest);
            return Ok(());
        }

        println!("Uploading blob {}...", blob.digest);
        let url = format!("{}/v2/{}/blobs/uploads/", self.registry, self.image_name);
        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers().await)
            .send()
            .await?;

        let location = response
            .headers()
            .get("location")
            .ok_or("Missing Location header")?
            .to_str()?;

        let upload_url = format!("{}&digest={}", location, blob.digest);
        let request = self
            .client
            .put(upload_url)
            .headers(self.auth_headers().await)
            .body(blob.data.clone());

        let response = request.send().await?;

        if response.status() == StatusCode::CREATED {
            self.uploaded_blobs.push(blob.digest.clone());
            println!("Blob {} uploaded.", blob.digest.clone());
            Ok(())
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to upload blob: {}", response.status()),
            )))
        }
    }

    pub async fn upload_manifest(
        &mut self,
        manifest_data: Vec<u8>,
        tag: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/v2/{}/manifests/{}", self.registry, self.image_name, tag);

        println!("Uploading {}:{}...", self.image_name, tag);

        let response = self
            .client
            .put(&url)
            .headers(self.auth_headers().await)
            .header("Content-Type", "application/vnd.oci.image.index.v1+json")
            .body(manifest_data)
            .send()
            .await?;

        if response.status() == StatusCode::CREATED {
            println!("Manifest uploaded successfully.");
            Ok(())
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to upload manifest: {}", response.status()),
            )))
        }
    }

    async fn auth_headers(&mut self) -> reqwest::header::HeaderMap {
        if self.auth_header == None {
            let scope = format!("repository:{}:pull,push", self.image_name);
            let url = format!(
                "{}?service={}&scope={}",
                self.get_auth_url(),
                self.service,
                scope
            );

            let mut request = self.client.get(&url);

            if let (Some(username), Some(password)) = (&self.username, &self.password) {
                request = request.basic_auth(username, Some(password));
                println!("Logging in as {}...", username);
            } else {
                println!("Logging in anonymously...");
            }

            if let Some(response) = request.send().await.ok() {
                if response.status() == StatusCode::OK {
                    if let Ok(response_text) = response.text().await {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response_text)
                        {
                            if let Some(token) = json.get("access_token").and_then(|v| v.as_str()) {
                                self.auth_header = Some(format!("Bearer {}", token));
                            } else if let Some(token) = json.get("token").and_then(|v| v.as_str()) {
                                self.auth_header = Some(format!("Bearer {}", token));
                            } else {
                                println!("Could not get token from JSON response");
                            }
                        } else {
                            self.auth_header = Some(format!("Bearer {}", response_text));
                        }
                    } else {
                        println!("Could not get token from text response");
                    }
                } else {
                    println!("Token login status not OK");
                }
            } else {
                println!("Could not send login request");
            }
        }

        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(auth) = &self.auth_header {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(auth).unwrap(),
            );
        }
        headers
    }
}
