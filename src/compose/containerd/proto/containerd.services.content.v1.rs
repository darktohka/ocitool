// This file is @generated by prost-build.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Info {
    /// Digest is the hash identity of the blob.
    #[prost(string, tag = "1")]
    pub digest: ::prost::alloc::string::String,
    /// Size is the total number of bytes in the blob.
    #[prost(int64, tag = "2")]
    pub size: i64,
    /// CreatedAt provides the time at which the blob was committed.
    #[prost(message, optional, tag = "3")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
    /// UpdatedAt provides the time the info was last updated.
    #[prost(message, optional, tag = "4")]
    pub updated_at: ::core::option::Option<::prost_types::Timestamp>,
    /// Labels are arbitrary data on snapshots.
    ///
    /// The combined size of a key/value pair cannot exceed 4096 bytes.
    #[prost(map = "string, string", tag = "5")]
    pub labels: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
impl ::prost::Name for Info {
    const NAME: &'static str = "Info";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.Info".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.Info".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InfoRequest {
    #[prost(string, tag = "1")]
    pub digest: ::prost::alloc::string::String,
}
impl ::prost::Name for InfoRequest {
    const NAME: &'static str = "InfoRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.InfoRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.InfoRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InfoResponse {
    #[prost(message, optional, tag = "1")]
    pub info: ::core::option::Option<Info>,
}
impl ::prost::Name for InfoResponse {
    const NAME: &'static str = "InfoResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.InfoResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.InfoResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRequest {
    #[prost(message, optional, tag = "1")]
    pub info: ::core::option::Option<Info>,
    /// UpdateMask specifies which fields to perform the update on. If empty,
    /// the operation applies to all fields.
    ///
    /// In info, Digest, Size, and CreatedAt are immutable,
    /// other field may be updated using this mask.
    /// If no mask is provided, all mutable field are updated.
    #[prost(message, optional, tag = "2")]
    pub update_mask: ::core::option::Option<::prost_types::FieldMask>,
}
impl ::prost::Name for UpdateRequest {
    const NAME: &'static str = "UpdateRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.UpdateRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.UpdateRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateResponse {
    #[prost(message, optional, tag = "1")]
    pub info: ::core::option::Option<Info>,
}
impl ::prost::Name for UpdateResponse {
    const NAME: &'static str = "UpdateResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.UpdateResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.UpdateResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListContentRequest {
    /// Filters contains one or more filters using the syntax defined in the
    /// containerd filter package.
    ///
    /// The returned result will be those that match any of the provided
    /// filters. Expanded, containers that match the following will be
    /// returned:
    ///
    
            /// ```notrust
            /// 	filters[0] or filters[1] or ... or filters[n-1] or filters[n]
            /// ```
    ///
    /// If filters is zero-length or nil, all items will be returned.
    #[prost(string, repeated, tag = "1")]
    pub filters: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
impl ::prost::Name for ListContentRequest {
    const NAME: &'static str = "ListContentRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.ListContentRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.ListContentRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListContentResponse {
    #[prost(message, repeated, tag = "1")]
    pub info: ::prost::alloc::vec::Vec<Info>,
}
impl ::prost::Name for ListContentResponse {
    const NAME: &'static str = "ListContentResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.ListContentResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.ListContentResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteContentRequest {
    /// Digest specifies which content to delete.
    #[prost(string, tag = "1")]
    pub digest: ::prost::alloc::string::String,
}
impl ::prost::Name for DeleteContentRequest {
    const NAME: &'static str = "DeleteContentRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.DeleteContentRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.DeleteContentRequest".into()
    }
}
/// ReadContentRequest defines the fields that make up a request to read a portion of
/// data from a stored object.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadContentRequest {
    /// Digest is the hash identity to read.
    #[prost(string, tag = "1")]
    pub digest: ::prost::alloc::string::String,
    /// Offset specifies the number of bytes from the start at which to begin
    /// the read. If zero or less, the read will be from the start. This uses
    /// standard zero-indexed semantics.
    #[prost(int64, tag = "2")]
    pub offset: i64,
    /// size is the total size of the read. If zero, the entire blob will be
    /// returned by the service.
    #[prost(int64, tag = "3")]
    pub size: i64,
}
impl ::prost::Name for ReadContentRequest {
    const NAME: &'static str = "ReadContentRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.ReadContentRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.ReadContentRequest".into()
    }
}
/// ReadContentResponse carries byte data for a read request.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadContentResponse {
    /// offset of the returned data
    #[prost(int64, tag = "1")]
    pub offset: i64,
    /// actual data
    #[prost(bytes = "vec", tag = "2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
impl ::prost::Name for ReadContentResponse {
    const NAME: &'static str = "ReadContentResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.ReadContentResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.ReadContentResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(message, optional, tag = "1")]
    pub started_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag = "2")]
    pub updated_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag = "3")]
    pub r#ref: ::prost::alloc::string::String,
    #[prost(int64, tag = "4")]
    pub offset: i64,
    #[prost(int64, tag = "5")]
    pub total: i64,
    #[prost(string, tag = "6")]
    pub expected: ::prost::alloc::string::String,
}
impl ::prost::Name for Status {
    const NAME: &'static str = "Status";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.Status".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.Status".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StatusRequest {
    #[prost(string, tag = "1")]
    pub r#ref: ::prost::alloc::string::String,
}
impl ::prost::Name for StatusRequest {
    const NAME: &'static str = "StatusRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.StatusRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.StatusRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StatusResponse {
    #[prost(message, optional, tag = "1")]
    pub status: ::core::option::Option<Status>,
}
impl ::prost::Name for StatusResponse {
    const NAME: &'static str = "StatusResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.StatusResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.StatusResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListStatusesRequest {
    #[prost(string, repeated, tag = "1")]
    pub filters: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
impl ::prost::Name for ListStatusesRequest {
    const NAME: &'static str = "ListStatusesRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.ListStatusesRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.ListStatusesRequest".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListStatusesResponse {
    #[prost(message, repeated, tag = "1")]
    pub statuses: ::prost::alloc::vec::Vec<Status>,
}
impl ::prost::Name for ListStatusesResponse {
    const NAME: &'static str = "ListStatusesResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.ListStatusesResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.ListStatusesResponse".into()
    }
}
/// WriteContentRequest writes data to the request ref at offset.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteContentRequest {
    /// Action sets the behavior of the write.
    ///
    /// When this is a write and the ref is not yet allocated, the ref will be
    /// allocated and the data will be written at offset.
    ///
    /// If the action is write and the ref is allocated, it will accept data to
    /// an offset that has not yet been written.
    ///
    /// If the action is write and there is no data, the current write status
    /// will be returned. This works differently from status because the stream
    /// holds a lock.
    #[prost(enumeration = "WriteAction", tag = "1")]
    pub action: i32,
    /// Ref identifies the pre-commit object to write to.
    #[prost(string, tag = "2")]
    pub r#ref: ::prost::alloc::string::String,
    /// Total can be set to have the service validate the total size of the
    /// committed content.
    ///
    /// The latest value before or with the commit action message will be use to
    /// validate the content. If the offset overflows total, the service may
    /// report an error. It is only required on one message for the write.
    ///
    /// If the value is zero or less, no validation of the final content will be
    /// performed.
    #[prost(int64, tag = "3")]
    pub total: i64,
    /// Expected can be set to have the service validate the final content against
    /// the provided digest.
    ///
    /// If the digest is already present in the object store, an AlreadyExists
    /// error will be returned.
    ///
    /// Only the latest version will be used to check the content against the
    /// digest. It is only required to include it on a single message, before or
    /// with the commit action message.
    #[prost(string, tag = "4")]
    pub expected: ::prost::alloc::string::String,
    /// Offset specifies the number of bytes from the start at which to begin
    /// the write. For most implementations, this means from the start of the
    /// file. This uses standard, zero-indexed semantics.
    ///
    /// If the action is write, the remote may remove all previously written
    /// data after the offset. Implementations may support arbitrary offsets but
    /// MUST support reseting this value to zero with a write. If an
    /// implementation does not support a write at a particular offset, an
    /// OutOfRange error must be returned.
    #[prost(int64, tag = "5")]
    pub offset: i64,
    /// Data is the actual bytes to be written.
    ///
    /// If this is empty and the message is not a commit, a response will be
    /// returned with the current write state.
    #[prost(bytes = "vec", tag = "6")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    /// Labels are arbitrary data on snapshots.
    ///
    /// The combined size of a key/value pair cannot exceed 4096 bytes.
    #[prost(map = "string, string", tag = "7")]
    pub labels: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
impl ::prost::Name for WriteContentRequest {
    const NAME: &'static str = "WriteContentRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.WriteContentRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.WriteContentRequest".into()
    }
}
/// WriteContentResponse is returned on the culmination of a write call.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WriteContentResponse {
    /// Action contains the action for the final message of the stream. A writer
    /// should confirm that they match the intended result.
    #[prost(enumeration = "WriteAction", tag = "1")]
    pub action: i32,
    /// StartedAt provides the time at which the write began.
    ///
    /// This must be set for stat and commit write actions. All other write
    /// actions may omit this.
    #[prost(message, optional, tag = "2")]
    pub started_at: ::core::option::Option<::prost_types::Timestamp>,
    /// UpdatedAt provides the last time of a successful write.
    ///
    /// This must be set for stat and commit write actions. All other write
    /// actions may omit this.
    #[prost(message, optional, tag = "3")]
    pub updated_at: ::core::option::Option<::prost_types::Timestamp>,
    /// Offset is the current committed size for the write.
    #[prost(int64, tag = "4")]
    pub offset: i64,
    /// Total provides the current, expected total size of the write.
    ///
    /// We include this to provide consistency with the Status structure on the
    /// client writer.
    ///
    /// This is only valid on the Stat and Commit response.
    #[prost(int64, tag = "5")]
    pub total: i64,
    /// Digest, if present, includes the digest up to the currently committed
    /// bytes. If action is commit, this field will be set. It is implementation
    /// defined if this is set for other actions.
    #[prost(string, tag = "6")]
    pub digest: ::prost::alloc::string::String,
}
impl ::prost::Name for WriteContentResponse {
    const NAME: &'static str = "WriteContentResponse";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.WriteContentResponse".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.WriteContentResponse".into()
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AbortRequest {
    #[prost(string, tag = "1")]
    pub r#ref: ::prost::alloc::string::String,
}
impl ::prost::Name for AbortRequest {
    const NAME: &'static str = "AbortRequest";
    const PACKAGE: &'static str = "containerd.services.content.v1";
    fn full_name() -> ::prost::alloc::string::String {
        "containerd.services.content.v1.AbortRequest".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/containerd.services.content.v1.AbortRequest".into()
    }
}
/// WriteAction defines the behavior of a WriteRequest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum WriteAction {
    /// WriteActionStat instructs the writer to return the current status while
    /// holding the lock on the write.
    Stat = 0,
    /// WriteActionWrite sets the action for the write request to write data.
    ///
    /// Any data included will be written at the provided offset. The
    /// transaction will be left open for further writes.
    ///
    /// This is the default.
    Write = 1,
    /// WriteActionCommit will write any outstanding data in the message and
    /// commit the write, storing it under the digest.
    ///
    /// This can be used in a single message to send the data, verify it and
    /// commit it.
    ///
    /// This action will always terminate the write.
    Commit = 2,
}
impl WriteAction {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Stat => "STAT",
            Self::Write => "WRITE",
            Self::Commit => "COMMIT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "STAT" => Some(Self::Stat),
            "WRITE" => Some(Self::Write),
            "COMMIT" => Some(Self::Commit),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod content_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Content provides access to a content addressable storage system.
    #[derive(Debug, Clone)]
    pub struct ContentClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ContentClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> ContentClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::Body>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> ContentClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::Body>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::Body>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::Body>,
            >>::Error: Into<StdError> + std::marker::Send + std::marker::Sync,
        {
            ContentClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        /// Info returns information about a committed object.
        ///
        /// This call can be used for getting the size of content and checking for
        /// existence.
        pub async fn info(
            &mut self,
            request: impl tonic::IntoRequest<super::InfoRequest>,
        ) -> std::result::Result<tonic::Response<super::InfoResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Info",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Info"),
                );
            self.inner.unary(req, path, codec).await
        }
        /// Update updates content metadata.
        ///
        /// This call can be used to manage the mutable content labels. The
        /// immutable metadata such as digest, size, and committed at cannot
        /// be updated.
        pub async fn update(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateRequest>,
        ) -> std::result::Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Update",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Update"),
                );
            self.inner.unary(req, path, codec).await
        }
        /// List streams the entire set of content as Info objects and closes the
        /// stream.
        ///
        /// Typically, this will yield a large response, chunked into messages.
        /// Clients should make provisions to ensure they can handle the entire data
        /// set.
        pub async fn list(
            &mut self,
            request: impl tonic::IntoRequest<super::ListContentRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ListContentResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/List",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "List"),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        /// Delete will delete the referenced object.
        pub async fn delete(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteContentRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Delete",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Delete"),
                );
            self.inner.unary(req, path, codec).await
        }
        /// Read allows one to read an object based on the offset into the content.
        ///
        /// The requested data may be returned in one or more messages.
        pub async fn read(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadContentRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ReadContentResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Read",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Read"),
                );
            self.inner.server_streaming(req, path, codec).await
        }
        /// Status returns the status for a single reference.
        pub async fn status(
            &mut self,
            request: impl tonic::IntoRequest<super::StatusRequest>,
        ) -> std::result::Result<tonic::Response<super::StatusResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Status",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Status"),
                );
            self.inner.unary(req, path, codec).await
        }
        /// ListStatuses returns the status of ongoing object ingestions, started via
        /// Write.
        ///
        /// Only those matching the regular expression will be provided in the
        /// response. If the provided regular expression is empty, all ingestions
        /// will be provided.
        pub async fn list_statuses(
            &mut self,
            request: impl tonic::IntoRequest<super::ListStatusesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListStatusesResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/ListStatuses",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "containerd.services.content.v1.Content",
                        "ListStatuses",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        /// Write begins or resumes writes to a resource identified by a unique ref.
        /// Only one active stream may exist at a time for each ref.
        ///
        /// Once a write stream has started, it may only write to a single ref, thus
        /// once a stream is started, the ref may be omitted on subsequent writes.
        ///
        /// For any write transaction represented by a ref, only a single write may
        /// be made to a given offset. If overlapping writes occur, it is an error.
        /// Writes should be sequential and implementations may throw an error if
        /// this is required.
        ///
        /// If expected_digest is set and already part of the content store, the
        /// write will fail.
        ///
        /// When completed, the commit flag should be set to true. If expected size
        /// or digest is set, the content will be validated against those values.
        pub async fn write(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::WriteContentRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::WriteContentResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Write",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Write"),
                );
            self.inner.streaming(req, path, codec).await
        }
        /// Abort cancels the ongoing write named in the request. Any resources
        /// associated with the write will be collected.
        pub async fn abort(
            &mut self,
            request: impl tonic::IntoRequest<super::AbortRequest>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/containerd.services.content.v1.Content/Abort",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("containerd.services.content.v1.Content", "Abort"),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
