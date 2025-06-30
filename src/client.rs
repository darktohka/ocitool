use std::{collections::HashMap, error::Error, sync::Arc};

use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Client, StatusCode,
};
use tokio::sync::Mutex;

use crate::parser::FullImage;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct ImagePermission {
    pub full_image: FullImage,
    pub permissions: ImagePermissions,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
}

pub struct OciClient {
    pub client: Client,
    pub hostname_to_login: HashMap<String, LoginCredentials>,
    pub default_login: Option<LoginCredentials>,
    pub image_bearer_map: Arc<Mutex<HashMap<ImagePermission, String>>>,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
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
        hostname_to_login: HashMap<String, LoginCredentials>,
        default_login: Option<LoginCredentials>,
    ) -> Self {
        let client = Client::builder()
            .http2_prior_knowledge()
            .pool_max_idle_per_host(16)
            .build()
            .expect("Failed to build HTTP client");

        OciClient {
            client,
            hostname_to_login,
            default_login,
            image_bearer_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_bearer(&self, token: &str) -> String {
        format!("Bearer {}", token)
    }

    pub fn get_base64_bearer(&self, token: &str) -> String {
        self.get_bearer(&BASE64_STANDARD.encode(token.as_bytes()))
    }

    pub fn get_credentials(&self, registry_url: &str) -> Result<LoginCredentials, OciClientError> {
        if let Some(credentials) = self.hostname_to_login.get(registry_url) {
            Ok(credentials.clone())
        } else if let Some(default) = &self.default_login {
            Ok(default.clone())
        } else {
            match std::env::var("GITHUB_TOKEN") {
                Ok(token) => Ok(LoginCredentials {
                    username: "github".to_string(),
                    password: token,
                }),
                Err(_) => Err(OciClientError(format!(
                    "No credentials found for registry: {}",
                    registry_url
                ))),
            }
        }
    }

    pub async fn login_to_github_registry(
        &self,
        reference_image: &FullImage,
        image_permissions: &[ImagePermission],
    ) -> Result<String, OciClientError> {
        // On GitHub, we do not need to login again
        match self.get_credentials(&reference_image.registry) {
            Ok(credentials) => Ok(self.get_base64_bearer(&credentials.password)),
            Err(_) => {
                // No credentials found, we can still try the regular login
                self.login_to_regular_registry(reference_image, image_permissions, true)
                    .await
            }
        }
    }

    pub async fn login_to_regular_registry(
        &self,
        reference_image: &FullImage,
        image_permissions: &[ImagePermission],
        use_credentials: bool,
    ) -> Result<String, OciClientError> {
        let scopes = image_permissions
            .iter()
            .map(|perm| {
                let permissions = match perm.permissions {
                    ImagePermissions::Pull => "pull",
                    ImagePermissions::Push => "pull,push",
                };
                format!(
                    "repository:{}:{}",
                    perm.full_image.library_name, permissions
                )
            })
            .collect::<Vec<_>>();

        let all_scopes = scopes
            .iter()
            .map(|scope| format!("scope={}", scope))
            .collect::<Vec<_>>()
            .join("&");

        let url = format!(
            "{}?service={}&{}",
            reference_image.get_auth_url(),
            reference_image.service,
            all_scopes
        );

        let mut request = self.client.get(&url);

        if use_credentials {
            if let Ok(credentials) = self.get_credentials(&reference_image.registry) {
                println!(
                    "Logging in as {} for {} to {}...",
                    credentials.username,
                    scopes.join("; "),
                    reference_image.registry,
                );

                request = request.basic_auth(credentials.username, Some(credentials.password));
            } else {
                println!("Logging in anonymously to {}...", reference_image.registry);
            }
        } else {
            println!(
                "Logging in anonymously to {} (retrying without credentials)",
                reference_image.registry,
            );
        }

        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                return Err(OciClientError(format!(
                    "Failed to send login request: {}",
                    e
                )));
            }
        };

        match response.status() {
            StatusCode::OK => {
                // Status code 200 OK means we got a token,
            }
            code => {
                return Err(OciClientError(format!(
                    "Login status code not OK: {}",
                    code
                )));
            }
        }

        let response_text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Err(OciClientError(format!(
                    "Failed to get text response: {}",
                    e
                )));
            }
        };

        let token = match serde_json::from_str::<serde_json::Value>(&response_text) {
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
        }?;

        Ok(token)
    }

    pub async fn login_to_container_registry(
        &self,
        image_permissions: Vec<ImagePermission>,
    ) -> Result<(), OciClientError> {
        if image_permissions.is_empty() {
            // No image permissions provided, nothing to do
            return Ok(());
        }

        let reference_image = &image_permissions[0].full_image;

        let token = if reference_image.is_github_registry() {
            self.login_to_github_registry(reference_image, &image_permissions)
                .await
        } else {
            match self
                .login_to_regular_registry(reference_image, &image_permissions, true)
                .await
            {
                Ok(token) => Ok(token),
                Err(_) => {
                    // If we fail to login with credentials, we can try again without them
                    self.login_to_regular_registry(reference_image, &image_permissions, false)
                        .await
                }
            }
        };

        if let Ok(new_bearer) = &token {
            let mut map = self.image_bearer_map.lock().await;

            for image_permission in image_permissions {
                map.insert(image_permission.clone(), new_bearer.clone());

                if image_permission.permissions == ImagePermissions::Push {
                    // Pushing requires pull permissions as well
                    // so we insert a separate entry for pull permissions
                    map.insert(
                        ImagePermission {
                            full_image: image_permission.full_image.clone(),
                            permissions: ImagePermissions::Pull,
                        },
                        new_bearer.clone(),
                    );
                }
            }
        }

        return Ok(());
    }

    pub async fn login(&self, image_permissions: &[ImagePermission]) -> Result<(), OciClientError> {
        // There could be both pull and push permissions in the list for a given image
        // Merge them. If an image has both pull and push permissions, we will use the push permissions
        let mut merged_permissions: HashMap<FullImage, ImagePermissions> = HashMap::new();
        for perm in image_permissions {
            merged_permissions
                .entry(perm.full_image.clone())
                .and_modify(|existing| {
                    // Push implies Pull, so Push overrides Pull
                    if *existing == ImagePermissions::Pull
                        && perm.permissions == ImagePermissions::Push
                    {
                        *existing = ImagePermissions::Push;
                    }
                })
                .or_insert(perm.permissions.clone());
        }

        let image_permissions: Vec<ImagePermission> = merged_permissions
            .into_iter()
            .map(|(full_image, permissions)| ImagePermission {
                full_image,
                permissions,
            })
            .collect();

        // We could have multiple images from different registries, so we need to group them by registry
        let image_permissions_by_registry =
            image_permissions
                .iter()
                .fold(HashMap::new(), |mut acc, perm| {
                    acc.entry(perm.full_image.registry.clone())
                        .or_insert_with(Vec::new)
                        .push(perm.clone());
                    acc
                });

        // Run login_to_container_registry in parallel for each registry group
        let futures = image_permissions_by_registry
            .into_iter()
            .map(|(_registry, perms)| self.login_to_container_registry(perms));
        futures::future::try_join_all(futures).await?;

        Ok(())
    }

    pub async fn auth_headers(
        &self,
        image_permission: ImagePermission,
    ) -> Result<HeaderMap, OciClientError> {
        let bearer = {
            let map = self.image_bearer_map.lock().await;

            match map.get(&image_permission) {
                Some(bearer) => bearer.clone(),
                None => {
                    return Err(OciClientError(format!(
                        "No bearer token found for image permission: {:?}",
                        image_permission
                    )));
                }
            }
        };

        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&bearer).unwrap());

        Ok(headers)
    }
}
