pub struct ParsedImage {
    pub registry: String,
    pub library_name: String,
    pub tag: String,
    pub service: String,
}

impl ParsedImage {
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

        ParsedImage {
            registry,
            library_name,
            tag,
            service,
        }
    }
}
