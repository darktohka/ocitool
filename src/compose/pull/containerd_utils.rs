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
    media_type: String,
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
                        media_type: media_type.clone(),
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
                                    media_type,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::error::Error;

    use crate::parser::{FullImage, FullImageWithTag};
    use crate::test::tests::{create_test_client, ContainerdTestEnv};

    #[tokio::test]
    async fn test_get_existing_digests_from_containerd() -> Result<(), Box<dyn Error>> {
        let env = ContainerdTestEnv::new().await?;
        let client = create_test_client(&env.socket_path).await?;

        let digests = get_existing_digests_from_containerd(client.clone()).await?;
        assert!(digests.is_empty());

        let test_digest = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let test_data = b"".to_vec();
        upload_content_to_containerd(client.clone(), test_digest, test_data, HashMap::new())
            .await?;

        let digests = get_existing_digests_from_containerd(client.clone()).await?;
        assert!(digests.contains(test_digest));

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_content_to_containerd() -> Result<(), Box<dyn Error>> {
        let env = ContainerdTestEnv::new().await?;
        let client = create_test_client(&env.socket_path).await?;

        let test_digest = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let test_data = b"".to_vec();

        let result =
            upload_content_to_containerd(client.clone(), test_digest, test_data, HashMap::new())
                .await;
        assert!(result.is_ok());

        // Verify that the content is there
        let digests = get_existing_digests_from_containerd(client.clone()).await?;
        assert!(digests.contains(test_digest));

        Ok(())
    }

    #[tokio::test]
    async fn test_create_image_in_containerd() -> Result<(), Box<dyn Error>> {
        let env = ContainerdTestEnv::new().await?;
        let client = create_test_client(&env.socket_path).await?;

        let full_image = FullImageWithTag {
            image: FullImage {
                registry: "registry-1.docker.io".to_string(),
                image_name: "hello-world".to_string(),
                library_name: "library/hello-world".to_string(),
                service: "registry.docker.io".to_string(),
            },
            tag: "latest".into(),
        };
        let dummy_manifest = r#"{"schemaVersion":2,"mediaType":"application/vnd.docker.distribution.manifest.v2+json","config":{"mediaType":"application/vnd.docker.container.image.v1+json","size":1,"digest":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"},"layers":[]}"#;
        let test_data = dummy_manifest.as_bytes().to_vec();
        let index_length = test_data.len() as i64;
        let index_digest = format!("sha256:{}", sha256::digest(test_data.as_slice()));
        let media_type = "application/vnd.docker.distribution.manifest.v2+json".to_string();

        // We need to upload the content first
        upload_content_to_containerd(client.clone(), &index_digest, test_data, HashMap::new())
            .await?;

        let result = create_image_in_containerd(
            client.clone(),
            &full_image,
            index_digest.to_string(),
            index_length,
            media_type,
        )
        .await;

        assert!(result.is_ok());

        Ok(())
    }
}
