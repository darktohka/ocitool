use crate::compose::containerd::client::services::v1::ListContentRequest;
use crate::with_namespace;
use crate::{
    client::{ImagePermission, ImagePermissions, OciClient},
    compose::{containerd::client::Client, docker_compose_finder::find_and_parse_docker_composes},
    parser::FullImageWithTag,
    system_login::get_system_login,
    Compose, Pull,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::Request;

pub struct PullInstance {
    pub existing_digests: Arc<Mutex<HashSet<String>>>,
}

pub async fn pull_command(
    compose_settings: &Compose,
    pull_settings: &Pull,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_dir = compose_settings
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));
    let max_depth = compose_settings.max_depth.unwrap_or(1);

    let composes = find_and_parse_docker_composes(&start_dir, max_depth);

    if composes.is_empty() {
        println!("No docker-compose files found in {}", start_dir.display());
        return Ok(());
    }

    let mut images_to_pull = HashSet::<String>::new();

    for compose in composes {
        println!("Pulling images for {}", compose.compose_path.display());

        for service in compose.compose.services.0.values() {
            if let Some(service) = service {
                if let Some(image) = &service.image {
                    images_to_pull.insert(image.clone());
                }
                // Here you would call the actual pull logic, e.g., using a Docker client
            }
        }
    }

    let mut images: Vec<_> = images_to_pull.into_iter().collect();
    images.sort();

    let full_images: Vec<FullImageWithTag> = images
        .into_iter()
        .map(|image| FullImageWithTag::from_image_name(&image))
        .collect();
    for image in &full_images {
        println!("Would pull image: {:?}", image);
    }

    let client = OciClient::new(get_system_login(), None);

    let image_permissions = full_images
        .iter()
        .map(|image| ImagePermission {
            full_image: image.image.clone(),
            permissions: ImagePermissions::Pull,
        })
        .collect::<Vec<_>>();

    println!("\nAttempting to connect to containerd...");
    let container_client = Client::from_path("/run/containerd/containerd.sock").await?;
    let version = container_client.version().version(()).await?;
    //    container_client.content().get_content_store().await?;
    println!("Containerd Version: {:?}", version);

    let list_content_request = with_namespace!(ListContentRequest { filters: vec![] }, "default");
    let content = container_client.content().list(list_content_request).await;

    let mut stream = match content {
        Ok(response) => response.into_inner(),
        Err(e) => {
            eprintln!("Failed to list content: {}", e);
            return Err(Box::new(e));
        }
    };

    println!("Streaming content items:");
    let mut existing_digests = HashSet::<String>::new();
    while let Some(item) = stream.message().await? {
        for info in &item.info {
            existing_digests.insert(info.digest.clone());
        }
    }

    let pull_instance = PullInstance {
        existing_digests: Arc::new(Mutex::new(existing_digests)),
    };

    println!(
        "Existing content digests: {:?}",
        pull_instance.existing_digests.lock().await
    );

    client.login(&image_permissions).await?;

    Ok(())
}
