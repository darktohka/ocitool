use std::{collections::HashMap, error::Error, sync::Arc};

use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, WWW_AUTHENTICATE},
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

/// The token endpoint advertised by a registry through the
/// `WWW-Authenticate: Bearer ...` challenge returned by its `/v2/` endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthChallenge {
    /// The absolute URL of the token endpoint, e.g.
    /// `https://registry.tohka.us/api/auth/token`.
    pub realm: String,

    /// The service name the token should be scoped to, e.g. `registry.tohka.us`.
    pub service: Option<String>,
}

impl AuthChallenge {
    /// Parses a `WWW-Authenticate` header value, returning the challenge when it
    /// is a `Bearer` challenge that advertises a `realm`.
    pub fn parse(header_value: &str) -> Option<Self> {
        let (scheme, params) = header_value.split_once(' ')?;

        if !scheme.eq_ignore_ascii_case("bearer") {
            return None;
        }

        let mut realm = None;
        let mut service = None;

        for (key, value) in parse_auth_params(params) {
            match key.as_str() {
                "realm" => realm = Some(value),
                "service" => service = Some(value),
                _ => {}
            }
        }

        Some(AuthChallenge {
            realm: realm?,
            service,
        })
    }
}

/// Splits an authentication parameter list (e.g. `realm="...",service="..."`)
/// into individual key/value pairs. Values may be quoted or bare.
fn parse_auth_params(input: &str) -> Vec<(String, String)> {
    let bytes = input.as_bytes();
    let mut params = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b',') {
            i += 1;
        }

        if i >= bytes.len() {
            break;
        }

        let key_start = i;
        while i < bytes.len() && !matches!(bytes[i], b'=' | b',') {
            i += 1;
        }

        let key = input[key_start..i].trim().to_ascii_lowercase();

        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;

        let value = if i < bytes.len() && bytes[i] == b'"' {
            i += 1;

            let value_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }

            let value = input[value_start..i].to_string();

            if i < bytes.len() {
                i += 1;
            }

            value
        } else {
            let value_start = i;
            while i < bytes.len() && bytes[i] != b',' {
                i += 1;
            }

            input[value_start..i].trim().to_string()
        };

        if !key.is_empty() {
            params.push((key, value));
        }
    }

    params
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

    pub fn get_bearer(&self, token: &str) -> Result<String, OciClientError> {
        let bearer = format!("Bearer {}", token);

        HeaderValue::from_str(&bearer).map_err(|_| {
            OciClientError("Registry returned an invalid authentication token".to_string())
        })?;

        Ok(bearer)
    }

    pub fn get_base64_bearer(&self, token: &str) -> Result<String, OciClientError> {
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

    /// Discovers the registry's token endpoint from the `WWW-Authenticate`
    /// challenge returned by `/v2/`, returning `None` when unavailable.
    pub async fn discover_auth_challenge(&self, registry_url: &str) -> Option<AuthChallenge> {
        let url = format!("{}/v2/", registry_url.trim_end_matches('/'));

        let response = self.client.get(&url).send().await.ok()?;

        for value in response.headers().get_all(WWW_AUTHENTICATE) {
            if let Ok(value) = value.to_str() {
                if let Some(challenge) = AuthChallenge::parse(value) {
                    return Some(challenge);
                }
            }
        }

        None
    }

    pub async fn login_to_github_registry(
        &self,
        reference_image: &FullImage,
        image_permissions: &[ImagePermission],
    ) -> Result<String, OciClientError> {
        // On GitHub, we do not need to login again
        match self.get_credentials(&reference_image.registry) {
            Ok(credentials) => self.get_base64_bearer(&credentials.password),
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

        let (realm, service) = match self
            .discover_auth_challenge(&reference_image.registry)
            .await
        {
            Some(challenge) => (
                challenge.realm,
                challenge
                    .service
                    .unwrap_or_else(|| reference_image.service.clone()),
            ),
            None => (
                reference_image.get_auth_url(),
                reference_image.service.clone(),
            ),
        };

        let separator = if realm.contains('?') { '&' } else { '?' };
        let url = format!(
            "{}{}service={}&{}",
            realm, separator, service, all_scopes
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
                .map(str::to_string)
                .ok_or_else(|| {
                    OciClientError(format!(
                        "Could not get token from JSON response: {}",
                        response_text
                    ))
                })?,
            _ => response_text.trim().to_string(),
        };

        self.get_bearer(&token)
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

        let value = HeaderValue::from_str(&bearer).map_err(|_| {
            OciClientError("Stored bearer token is not a valid header value".to_string())
        })?;

        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(AUTHORIZATION, value);

        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image(registry: &str) -> FullImage {
        FullImage {
            registry: registry.to_string(),
            image_name: "nirai-panda3d".to_string(),
            library_name: "base/nirai-panda3d".to_string(),
            service: "registry.tohka.us".to_string(),
        }
    }

    #[test]
    fn parses_docker_hub_challenge() {
        let challenge = AuthChallenge::parse(
            r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io""#,
        )
        .expect("Docker Hub challenge should parse");

        assert_eq!(challenge.realm, "https://auth.docker.io/token");
        assert_eq!(challenge.service.as_deref(), Some("registry.docker.io"));
    }

    #[test]
    fn parses_custom_realm_challenge() {
        let challenge = AuthChallenge::parse(
            r#"Bearer realm="https://registry.tohka.us/api/auth/token",service="registry.tohka.us""#,
        )
        .expect("custom realm challenge should parse");

        assert_eq!(
            challenge.realm,
            "https://registry.tohka.us/api/auth/token"
        );
        assert_eq!(challenge.service.as_deref(), Some("registry.tohka.us"));
    }

    #[test]
    fn parses_challenge_without_service() {
        let challenge = AuthChallenge::parse(r#"Bearer realm="https://example.com/token""#)
            .expect("challenge without service should parse");

        assert_eq!(challenge.realm, "https://example.com/token");
        assert_eq!(challenge.service, None);
    }

    #[test]
    fn parses_challenge_with_extra_parameters() {
        let challenge = AuthChallenge::parse(
            r#"Bearer realm="https://example.com/token",service="example.com",scope="repository:foo/bar:pull""#,
        )
        .expect("challenge with extra parameters should parse");

        assert_eq!(challenge.realm, "https://example.com/token");
        assert_eq!(challenge.service.as_deref(), Some("example.com"));
    }

    #[test]
    fn parses_bearer_scheme_case_insensitively() {
        let challenge =
            AuthChallenge::parse(r#"bearer realm="https://example.com/token""#).expect("lowercase");

        assert_eq!(challenge.realm, "https://example.com/token");
    }

    #[test]
    fn ignores_non_bearer_challenges() {
        assert!(AuthChallenge::parse(r#"Basic realm="example.com""#).is_none());
    }

    #[test]
    fn ignores_bearer_challenge_without_realm() {
        assert!(AuthChallenge::parse(r#"Bearer service="example.com""#).is_none());
    }

    #[test]
    fn rejects_invalid_bearer_token() {
        let client = OciClient::new(HashMap::new(), None);

        assert!(client.get_bearer("line1\nline2").is_err());
        assert!(client.get_bearer("line1\r\nline2").is_err());
        assert!(client.get_bearer("valid-token").is_ok());
    }

    #[tokio::test]
    async fn auth_headers_errors_on_invalid_bearer() {
        let client = OciClient::new(HashMap::new(), None);
        let permission = ImagePermission {
            full_image: test_image("https://registry.tohka.us"),
            permissions: ImagePermissions::Pull,
        };

        client
            .image_bearer_map
            .lock()
            .await
            .insert(permission.clone(), "Bearer line1\nline2".to_string());

        assert!(client.auth_headers(permission).await.is_err());
    }
}
