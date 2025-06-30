use crate::compose::containerd::client::services::v1::{
    CreateImageRequest, Image, ListContentRequest, UpdateImageRequest, WriteAction,
    WriteContentRequest,
};
use crate::compose::containerd::client::types;
use crate::compose::lease::LeasedClient;
use crate::parser::FullImageWithTag;
use crate::with_client;
use prost_types::Timestamp;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tonic::{Code, Request};

pub async fn get_existing_digests_from_containerd(
    container_client: Arc<LeasedClient>,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let list_content_request =
        with_client!(ListContentRequest { filters: vec![] }, container_client);
    let content = container_client
        .client()
        .content()
        .list(list_content_request)
        .await;

    let mut stream = match content {
        Ok(response) => response.into_inner(),
        Err(e) => {
            eprintln!("Failed to list content: {}", e);
            return Err(Box::new(e));
        }
    };

    let mut existing_digests = HashSet::<String>::new();
    while let Some(item) = stream.message().await? {
        for info in &item.info {
            existing_digests.insert(info.digest.clone());
        }
    }

    Ok(existing_digests)
}

pub async fn upload_content_to_containerd(
    container_client: Arc<LeasedClient>,
    digest: &str,
    data: Vec<u8>,
    labels: HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let upload_request = WriteContentRequest {
        action: WriteAction::Commit as i32,
        r#ref: digest.to_string(),
        total: data.len() as i64,
        expected: "".to_string(),
        offset: 0,
        data,
        labels,
    };

    let request_stream = with_client!(
        futures_util::stream::iter(vec![upload_request]),
        container_client
    );
    let content = match container_client
        .client()
        .content()
        .write(request_stream)
        .await
    {
        Ok(response) => response,
        Err(status) => {
            if status.code() == Code::AlreadyExists {
                println!(
                    "Content with digest {} already exists, skipping upload.",
                    digest
                );
                return Ok(());
            }

            eprintln!("Failed to upload content: {}", status);
            return Err(Box::new(status));
        }
    };

    let mut stream = content.into_inner();
    if let Ok(Some(_response)) = stream.message().await {
        // Wait for the upload to complete
    }

    Ok(())
}

pub async fn create_image_in_containerd(
    container_client: Arc<LeasedClient>,
    full_image: &FullImageWithTag,
    index_digest: String,
    index_length: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match container_client
        .client()
        .images()
        .create(with_client!(
            CreateImageRequest {
                image: Some(Image {
                    name: format!(
                        "docker.io/{}:{}",
                        full_image.image.library_name, full_image.tag
                    ),
                    labels: HashMap::new(),
                    target: Some(types::Descriptor {
                        media_type: "application/vnd.oci.image.index.v1+json".to_string(),
                        digest: index_digest.clone(),
                        size: index_length,
                        annotations: HashMap::new(),
                    }),
                    created_at: Some(Timestamp::default()),
                    updated_at: Some(Timestamp::default())
                }),
                source_date_epoch: None,
            },
            container_client
        ))
        .await
    {
        Ok(_response) => Ok(()),
        Err(status) => {
            if status.code() == Code::AlreadyExists {
                return match container_client
                    .client()
                    .images()
                    .update(with_client!(
                        UpdateImageRequest {
                            image: Some(Image {
                                name: format!(
                                    "docker.io/{}:{}",
                                    full_image.image.library_name, full_image.tag
                                ),
                                labels: HashMap::new(),
                                target: Some(types::Descriptor {
                                    media_type: "application/vnd.oci.image.index.v1+json"
                                        .to_string(),
                                    digest: index_digest.clone(),
                                    size: index_length,
                                    annotations: HashMap::new(),
                                }),
                                created_at: Some(Timestamp::default()),
                                updated_at: Some(Timestamp::default())
                            }),
                            source_date_epoch: None,
                            update_mask: None,
                        },
                        container_client
                    ))
                    .await
                {
                    Ok(_response) => Ok(()),
                    Err(status) => {
                        eprintln!("Failed to update image: {}", status);
                        Err(Box::new(status))
                    }
                };
            }

            eprintln!("Failed to upload content: {}", status);
            return Err(Box::new(status));
        }
    }
}
