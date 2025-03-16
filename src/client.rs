use std::{collections::HashMap, error::Error};

use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Client, StatusCode,
};

pub struct ImageToken {
    token: String,
    permissions: ImagePermissions,
}

pub struct OciClient {
    pub client: Client,
    pub registry: String,
    username: Option<String>,
    password: Option<String>,
    service: String,
    image_bearer_map: HashMap<String, ImageToken>,
}

#[derive(PartialEq, Clone)]
pub enum ImagePermissions {
    Pull,
    Push,
}

#[derive(Debug, Clone)]

pub struct OciClientError(String);

impl<'a> Error for OciClientError {}

impl<'a> std::fmt::Display for OciClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl OciClient {
    pub fn new(
        registry: String,
        username: Option<String>,
        password: Option<String>,
        service: String,
    ) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to build HTTP client");

        OciClient {
            client,
            registry,
            username,
            password,
            service,
            image_bearer_map: HashMap::new(),
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

    pub fn get_image_url(&self, image_name: &str) -> String {
        format!("{}/v2/{}", self.registry, image_name)
    }

    pub fn is_github_registry(&self) -> bool {
        self.registry.contains("ghcr.io")
    }

    pub fn get_bearer(&self, token: &str) -> String {
        format!("Bearer {}", token)
    }

    pub fn get_base64_bearer(&self, token: &str) -> String {
        self.get_bearer(&BASE64_STANDARD.encode(token.as_bytes()))
    }

    pub fn login_to_github_registry(&self) -> Result<String, OciClientError> {
        // On GitHub, we do not need to login again
        let password = match &self.password {
            Some(pass) => pass,
            None => &match std::env::var("GITHUB_TOKEN") {
                Ok(token) => token,
                Err(_) => {
                    return Err(OciClientError(
                        "GITHUB_TOKEN environment variable not set".to_string(),
                    ))
                }
            },
        };

        Ok(self.get_base64_bearer(&password))
    }

    pub async fn login_to_container_registry(
        &self,
        image_name: &str,
        image_permissions: ImagePermissions,
    ) -> Result<String, OciClientError> {
        let permissions = if image_permissions == ImagePermissions::Pull {
            "pull"
        } else {
            "pull,push"
        };
        let scope = format!("repository:{}:{}", image_name, permissions,);
        let url = format!(
            "{}?service={}&scope={}",
            self.get_auth_url(),
            self.service,
            scope
        );

        let mut request = self.client.get(&url);

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            request = request.basic_auth(username, Some(password));
            println!("Logging in as {} for {}...", username, scope);
        } else {
            println!("Logging in anonymously...");
        }

        let response = request.send().await;

        match response {
            Ok(response) => match response.status() {
                StatusCode::OK => match response.text().await {
                    Ok(response_text) => {
                        match serde_json::from_str::<serde_json::Value>(&response_text) {
                            Ok(json) => ["access_token", "token"]
                                .iter()
                                .find_map(|key| json.get(key).and_then(|v| v.as_str()))
                                .map_or_else(
                                    || {
                                        Err(OciClientError(format!(
                                            "Could not get token from JSON response: {}",
                                            response_text
                                        )))
                                    },
                                    |token| Ok(self.get_bearer(token)),
                                ),
                            _ => Ok(self.get_bearer(&response_text)),
                        }
                    }
                    Err(e) => Err(OciClientError(format!(
                        "Failed to get text response: {}",
                        e
                    ))),
                },
                code => Err(OciClientError(format!(
                    "Login status code not OK: {}",
                    code
                ))),
            },
            Err(e) => Err(OciClientError(format!(
                "Failed to send login request: {}",
                e
            ))),
        }
    }

    pub async fn login_to_registry(
        &mut self,
        image_name: &str,
        image_permissions: ImagePermissions,
    ) -> Result<String, OciClientError> {
        let new_bearer = if self.is_github_registry() {
            self.login_to_github_registry()
        } else {
            self.login_to_container_registry(image_name, image_permissions.clone())
                .await
        };

        if let Ok(bearer) = &new_bearer {
            self.image_bearer_map.insert(
                image_name.to_string(),
                ImageToken {
                    token: bearer.clone(),
                    permissions: image_permissions,
                },
            );
        }

        new_bearer
    }

    pub async fn auth_headers(
        &mut self,
        image_name: &str,
        image_permissions: ImagePermissions,
    ) -> Result<HeaderMap, OciClientError> {
        let parts = image_name.split('/').collect::<Vec<&str>>();
        let only_image_name = if parts.len() == 3 {
            parts[1..].join("/")
        } else {
            image_name.to_owned()
        };
        let actual_bearer = match self.image_bearer_map.get(&only_image_name) {
            Some(bearer) => {
                if bearer.permissions == ImagePermissions::Pull
                    && image_permissions == ImagePermissions::Push
                {
                    self.login_to_registry(&only_image_name, image_permissions)
                        .await
                } else {
                    Ok(bearer.token.clone())
                }
            }
            None => {
                self.login_to_registry(&only_image_name, image_permissions)
                    .await
            }
        };

        match actual_bearer {
            Ok(bearer) => {
                let mut headers = HeaderMap::with_capacity(1);
                headers.insert(AUTHORIZATION, HeaderValue::from_str(&bearer).unwrap());

                Ok(headers)
            }
            Err(e) => Err(e),
        }
    }
}
