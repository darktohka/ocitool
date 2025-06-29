pub use tonic;

/// Generated `containerd.types` types.
pub mod types {
    include!("proto/containerd.types.rs");

    pub mod v1 {
        include!("proto/containerd.v1.types.rs");
    }
    pub mod transfer {
        include!("proto/containerd.types.transfer.rs");
    }
}

/// Generated `google.rpc` types, containerd services typically use some of these types.
pub mod google {
    pub mod rpc {
        include!("proto/google.rpc.rs");
    }
}

/// Generated `containerd.services.*` services.
pub mod services {
    pub mod v1 {
        include!("proto/containerd.services.containers.v1.rs");
        include!("proto/containerd.services.content.v1.rs");
        include!("proto/containerd.services.diff.v1.rs");
        include!("proto/containerd.services.events.v1.rs");
        include!("proto/containerd.services.images.v1.rs");
        include!("proto/containerd.services.introspection.v1.rs");
        include!("proto/containerd.services.leases.v1.rs");
        include!("proto/containerd.services.namespaces.v1.rs");
        include!("proto/containerd.services.streaming.v1.rs");
        include!("proto/containerd.services.tasks.v1.rs");
        include!("proto/containerd.services.transfer.v1.rs");

        // Sandbox services (Controller and Store) don't make it clear that they are for sandboxes.
        // Wrap these into a sub module to make the names more clear.
        pub mod sandbox {
            include!("proto/containerd.services.sandbox.v1.rs");
        }

        // Snapshot's `Info` conflicts with Content's `Info`, so wrap it into a separate sub module.
        pub mod snapshots {
            include!("proto/containerd.services.snapshots.v1.rs");
        }

        include!("proto/containerd.services.version.v1.rs");
    }
}

/// Generated event types.
pub mod events {
    include!("proto/containerd.events.rs");
}

/// Connect creates a unix channel to containerd GRPC socket.
pub async fn connect(
    path: impl AsRef<std::path::Path>,
) -> Result<tonic::transport::Channel, tonic::transport::Error> {
    use tonic::transport::Endpoint;

    let path = path.as_ref().to_path_buf();

    let channel = Endpoint::try_from("http://[::]")?
        .connect_with_connector(tower::service_fn(move |_| {
            let path = path.clone();

            async move {
                {
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(
                        tokio::net::UnixStream::connect(path).await?,
                    ))
                }
            }
        }))
        .await?;

    Ok(channel)
}

/// Help to inject namespace into request.
///
/// To use this macro, the `tonic::Request` is needed.
#[macro_export]
macro_rules! with_namespace {
    ($req:expr, $ns:expr) => {{
        let mut req = Request::new($req);
        let md = req.metadata_mut();

        // https://github.com/containerd/containerd/blob/main/pkg/namespaces/grpc.go#L27
        md.insert("containerd-namespace", $ns.parse().unwrap());
        req
    }};
}

use services::v1::{
    containers_client::ContainersClient,
    content_client::ContentClient,
    diff_client::DiffClient,
    events_client::EventsClient,
    images_client::ImagesClient,
    introspection_client::IntrospectionClient,
    leases_client::LeasesClient,
    namespaces_client::NamespacesClient,
    sandbox::{controller_client::ControllerClient, store_client::StoreClient},
    snapshots::snapshots_client::SnapshotsClient,
    streaming_client::StreamingClient,
    tasks_client::TasksClient,
    transfer_client::TransferClient,
    version_client::VersionClient,
};
use tonic::transport::{Channel, Error};

/// Client to containerd's APIs.
pub struct Client {
    channel: Channel,
}

impl From<Channel> for Client {
    fn from(value: Channel) -> Self {
        Self { channel: value }
    }
}

#[allow(dead_code)]
impl Client {
    /// Create a new client from UDS socket.
    pub async fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let channel = connect(path).await?;
        Ok(Self { channel })
    }

    /// Access to the underlying Tonic channel.
    #[inline]
    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    /// Version service.
    #[inline]
    pub fn version(&self) -> VersionClient<Channel> {
        VersionClient::new(self.channel())
    }

    /// Task service client.
    #[inline]
    pub fn tasks(&self) -> TasksClient<Channel> {
        println!("log: tasks client created");
        TasksClient::new(self.channel())
    }

    /// Transfer service client.
    #[inline]
    pub fn transfer(&self) -> TransferClient<Channel> {
        TransferClient::new(self.channel())
    }

    /// Sandbox store client.
    #[inline]
    pub fn sandbox_store(&self) -> StoreClient<Channel> {
        StoreClient::new(self.channel())
    }

    /// Streaming services client.
    #[inline]
    pub fn streaming(&self) -> StreamingClient<Channel> {
        StreamingClient::new(self.channel())
    }

    /// Sandbox controller client.
    #[inline]
    pub fn sandbox_controller(&self) -> ControllerClient<Channel> {
        ControllerClient::new(self.channel())
    }

    /// Snapshots service.
    #[inline]
    pub fn snapshots(&self) -> SnapshotsClient<Channel> {
        SnapshotsClient::new(self.channel())
    }

    /// Namespaces service.
    #[inline]
    pub fn namespaces(&self) -> NamespacesClient<Channel> {
        NamespacesClient::new(self.channel())
    }

    /// Leases service.
    #[inline]
    pub fn leases(&self) -> LeasesClient<Channel> {
        LeasesClient::new(self.channel())
    }

    /// Intropection service.
    #[inline]
    pub fn introspection(&self) -> IntrospectionClient<Channel> {
        IntrospectionClient::new(self.channel())
    }

    /// Image service.
    #[inline]
    pub fn images(&self) -> ImagesClient<Channel> {
        ImagesClient::new(self.channel())
    }

    /// Event service.
    #[inline]
    pub fn events(&self) -> EventsClient<Channel> {
        EventsClient::new(self.channel())
    }

    /// Diff service.
    #[inline]
    pub fn diff(&self) -> DiffClient<Channel> {
        DiffClient::new(self.channel())
    }

    /// Content service.
    #[inline]
    pub fn content(&self) -> ContentClient<Channel> {
        ContentClient::new(self.channel())
    }

    /// Container service.
    #[inline]
    pub fn containers(&self) -> ContainersClient<Channel> {
        ContainersClient::new(self.channel())
    }
}

mod tests {
    #[test]
    fn any_roundtrip() {
        use crate::compose::containerd::client::events::ContainerCreate;
        use prost_types::Any;

        let original = ContainerCreate {
            id: "test".to_string(),
            image: "test".to_string(),
            runtime: None,
        };

        let any = Any::from_msg(&original).expect("should not fail to encode");
        let decoded: ContainerCreate = any.to_msg().expect("should not fail to decode");

        assert_eq!(original, decoded)
    }
}
