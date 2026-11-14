// Field layout from google/nearby sharing/proto/rpc_resources.proto and
// encrypted_metadata.proto, revision 770a030be8fbebbcda5bf71ba718c35f5350c27c.
use zeroize::Zeroize as _;

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Timestamp {
    #[prost(int64, tag = "1")]
    pub seconds: i64,
    #[prost(int32, tag = "2")]
    pub nanos: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Certificate {
    #[prost(bytes = "vec", tag = "1")]
    pub id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub secret: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub public_key: Vec<u8>,
    #[prost(message, optional, tag = "4")]
    pub start: Option<Timestamp>,
    #[prost(message, optional, tag = "5")]
    pub end: Option<Timestamp>,
    #[prost(bytes = "vec", tag = "7")]
    pub metadata_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub encrypted_metadata: Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub metadata_key_tag: Vec<u8>,
    #[prost(bool, tag = "10")]
    pub self_share: bool,
    #[prost(string, tag = "12")]
    pub binding_id: String,
}
impl Drop for Certificate {
    fn drop(&mut self) {
        self.secret.zeroize();
        self.metadata_key.zeroize();
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Metadata {
    #[prost(string, optional, tag = "1")]
    pub device_name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub full_name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub icon_url: Option<String>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub bluetooth_mac: Option<Vec<u8>>,
    #[prost(string, optional, tag = "5")]
    pub gaia_id: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub account_name: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub model_name: Option<String>,
    #[prost(int32, optional, tag = "8")]
    pub vendor_id: Option<i32>,
    #[prost(string, optional, tag = "9")]
    pub icon_token: Option<String>,
}
impl Drop for Metadata {
    fn drop(&mut self) {
        self.device_name.zeroize();
        self.full_name.zeroize();
        self.icon_url.zeroize();
        self.bluetooth_mac.zeroize();
        self.gaia_id.zeroize();
        self.account_name.zeroize();
        self.model_name.zeroize();
        self.icon_token.zeroize();
    }
}
