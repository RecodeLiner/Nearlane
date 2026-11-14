use std::time::Duration;

use nearlane_auth::TokenGrant;
use prost::Message;
use tonic::{
    Request,
    client::Grpc,
    metadata::{Ascii, MetadataValue},
    transport::{Channel, ClientTlsConfig},
};

use super::{IdentityError, proto};

pub(super) trait IdentityTransport {
    async fn publish(
        &mut self,
        request: proto::PublishRequest,
    ) -> Result<proto::PublishResponse, IdentityError>;
    async fn query(
        &mut self,
        request: proto::QueryRequest,
    ) -> Result<proto::QueryResponse, IdentityError>;
}

pub(super) struct GoogleIdentityClient {
    grpc: Grpc<Channel>,
    authorization: MetadataValue<Ascii>,
}

impl GoogleIdentityClient {
    pub(super) async fn connect(grant: &TokenGrant) -> Result<Self, IdentityError> {
        let tls = ClientTlsConfig::new()
            .domain_name("nearby.googleapis.com")
            .with_webpki_roots();
        let channel = Channel::from_static("https://nearby.googleapis.com")
            .tls_config(tls)
            .map_err(|_| IdentityError::Network)?
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .connect()
            .await
            .map_err(|_| IdentityError::Network)?;
        let mut authorization = MetadataValue::try_from(format!("Bearer {}", grant.access_token()))
            .map_err(|_| IdentityError::Encoding)?;
        authorization.set_sensitive(true);
        Ok(Self {
            grpc: Grpc::new(channel)
                .max_decoding_message_size(1024 * 1024)
                .max_encoding_message_size(256 * 1024),
            authorization,
        })
    }

    async fn unary<Q, R>(&mut self, body: Q, path: &'static str) -> Result<R, IdentityError>
    where
        Q: Message + Send + 'static,
        R: Message + Default + Send + 'static,
    {
        self.grpc
            .ready()
            .await
            .map_err(|_| IdentityError::Network)?;
        let mut request = Request::new(body);
        request
            .metadata_mut()
            .insert("authorization", self.authorization.clone());
        request.set_timeout(Duration::from_secs(15));
        // Real gRPC status/trailers are handled by tonic; HTTP 200 alone must not
        // be mistaken for a successful certificate response. Never retain/log a
        // Status message, metadata, provider body, bearer header or full request.
        let response: tonic::Response<R> = self
            .grpc
            .unary(
                request,
                tonic::codegen::http::uri::PathAndQuery::from_static(path),
                tonic_prost::ProstCodec::default(),
            )
            .await
            .map_err(|status| IdentityError::RpcStatus(status.code() as i32))?;
        Ok(response.into_inner())
    }
}

impl IdentityTransport for GoogleIdentityClient {
    async fn publish(
        &mut self,
        request: proto::PublishRequest,
    ) -> Result<proto::PublishResponse, IdentityError> {
        self.unary(
            request,
            "/google.nearby.identity.v1.NearbyService/PublishDevice",
        )
        .await
    }
    async fn query(
        &mut self,
        request: proto::QueryRequest,
    ) -> Result<proto::QueryResponse, IdentityError> {
        self.unary(
            request,
            "/google.nearby.identity.v1.NearbyService/QuerySharedCredentials",
        )
        .await
    }
}
