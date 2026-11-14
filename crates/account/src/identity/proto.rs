// Original minimal wire adapters. Field numbers/method paths observed in the
// examined official NearbyShareExe (SHA-256 recorded in docs/research).
// Raw descriptors and proprietary implementation remain exclusively in work/.

use zeroize::Zeroize as _;

#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct Timestamp {
    #[prost(int64, tag = "1")]
    pub seconds: i64,
    #[prost(int32, tag = "2")]
    pub nanos: i32,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SharedCredential {
    #[prost(int64, tag = "1")]
    pub id: i64,
    #[prost(int32, tag = "2")]
    pub data_type: i32,
    #[prost(bytes = "vec", tag = "3")]
    pub data: Vec<u8>,
    #[prost(message, optional, tag = "5")]
    pub expiration: Option<Timestamp>,
}
impl Drop for SharedCredential {
    fn drop(&mut self) {
        self.data.zeroize();
    }
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct VisibilityCredentials {
    #[prost(int32, tag = "1")]
    pub visibility: i32,
    #[prost(message, repeated, tag = "2")]
    pub credentials: Vec<SharedCredential>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct Device {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub display_name: String,
    #[prost(int32, tag = "3")]
    pub contact: i32,
    #[prost(message, repeated, tag = "4")]
    pub groups: Vec<VisibilityCredentials>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct PublishRequest {
    #[prost(message, optional, tag = "1")]
    pub device: Option<Device>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct PublishResponse {
    #[prost(int32, repeated, tag = "1")]
    pub contact_updates: Vec<i32>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct QueryRequest {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(int32, tag = "2")]
    pub page_size: i32,
    #[prost(string, tag = "3")]
    pub page_token: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct QueryResponse {
    #[prost(message, repeated, tag = "1")]
    pub credentials: Vec<SharedCredential>,
    #[prost(string, tag = "2")]
    pub next_page_token: String,
}
