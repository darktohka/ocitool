use crate::{
    compose::containerd::client::{
        services::v1::{CreateRequest, DeleteRequest},
        Client,
    },
    with_namespace,
};
use std::{collections::HashMap, sync::Arc};
use tonic::Request;

pub struct LeasedClient {
    client: Arc<Client>,
    lease_id: String,
    namespace: String,
}

impl LeasedClient {
    pub async fn new(
        client: Arc<Client>,
        namespace: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let lease = client
            .leases()
            .create(with_namespace!(
                CreateRequest {
                    id: "".to_string(),
                    labels: HashMap::new(),
                },
                namespace
            ))
            .await?
            .into_inner();

        match lease.lease {
            None => Err("Failed to create lease".into()),
            Some(lease) => Ok(Self {
                client,
                lease_id: lease.id,
                namespace,
            }),
        }
    }

    pub fn lease_id(&self) -> &str {
        &self.lease_id
    }

    pub fn client(&self) -> Arc<Client> {
        Arc::clone(&self.client)
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub async fn delete_lease(&self) {
        if self.lease_id.is_empty() {
            return;
        }

        let delete_request = with_namespace!(
            DeleteRequest {
                id: self.lease_id.clone(),
                sync: false,
            },
            self.namespace
        );
        let _ = self.client.leases().delete(delete_request).await;
    }
}

impl Drop for LeasedClient {
    fn drop(&mut self) {
        // When the LeasedClient is dropped, we delete the lease asynchronously.
        let lease_id = self.lease_id.clone();

        if self.lease_id.is_empty() {
            return;
        }

        let namespace = self.namespace.clone();
        let client = Arc::clone(&self.client);

        tokio::spawn(async move {
            let delete_request = with_namespace!(
                DeleteRequest {
                    id: lease_id.clone(),
                    sync: false,
                },
                namespace
            );
            let _ = client.leases().delete(delete_request).await;
        });
    }
}

#[macro_export]
macro_rules! with_client {
    ($req:expr, $client:expr) => {{
        let mut req = Request::new($req);
        let md = req.metadata_mut();

        // https://github.com/containerd/containerd/blob/main/pkg/namespaces/grpc.go#L27
        md.insert("containerd-namespace", $client.namespace().parse().unwrap());
        md.insert("containerd-lease", $client.lease_id().parse().unwrap());
        req
    }};
}
