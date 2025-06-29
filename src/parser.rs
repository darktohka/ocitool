#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct FullImage {
    // The registry URL, e.g., "registry-1.docker.io"
    pub registry: String,

    // The full image name, e.g., "ubuntu"
    pub image_name: String,

    // The library name, e.g., "library/ubuntu"
    pub library_name: String,

    // The service name, e.g., "docker.io" or "ghcr.io"
    pub service: String,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct FullImageWithTag {
    pub image: FullImage,
    pub tag: String,
}

impl FullImage {
    pub fn get_auth_url(&self) -> String {
        if self.registry.contains("registry-1.docker.io")
            || self.registry.contains("registry.docker.io")
        {
            "https://auth.docker.io/token".to_string()
        } else {
            format!("{}/auth", self.registry)
        }
    }

    pub fn get_image_url(&self) -> String {
        format!("{}/v2/{}", self.registry, self.image_name)
    }

    pub fn is_github_registry(&self) -> bool {
        self.registry.contains("ghcr.io")
    }
}

impl FullImageWithTag {
    pub fn from_image_name(image_name: &str) -> Self {
        let parts: Vec<&str> = image_name.split('/').collect();
        let registry = if parts.len() > 2 {
            format!("https://{}", parts[0])
        } else {
            "https://registry-1.docker.io".to_string()
        };

        let full_name = if parts.len() == 3 {
            parts[1..].join("/")
        } else {
            image_name.to_owned()
        };

        let name = full_name.split(':').nth(0).unwrap().to_string();
        let tag = full_name.split(':').nth(1).unwrap_or("latest").to_string();

        let library_name = if image_name.contains('/') {
            name.to_string()
        } else {
            format!("library/{}", name)
        };

        let service = if parts.len() > 2 {
            parts[0].to_string()
        } else {
            "registry.docker.io".to_string()
        };

        FullImageWithTag {
            image: FullImage {
                registry,
                image_name: name,
                library_name,
                service,
            },
            tag,
        }
    }
}

impl FullImage {
    pub fn from_image_name(image_name: &str) -> Self {
        FullImageWithTag::from_image_name(image_name).image
    }
}
