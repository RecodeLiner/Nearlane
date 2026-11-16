# Private discovery stage - 2026-11-15

Identity API success was confirmed: registration, 9 own-device credentials,
6 active, 0 malformed. Counts are credentials, not a device count: rotating
credentials and overlapping validity windows prevent that interpretation.

Original Rust discovery adapters follow the pinned public Google Nearby source
`770a030be8fbebbcda5bf71ba718c35f5350c27c`:

- connections/implementation/mediums/utils.cc: service type is six SHA-256 bytes
  of NearbySharing, uppercase hex, producing `_FC9F5ED42C8A._tcp.local.`.
- connections/implementation/wifi_lan_service_info.cc: URL-safe base64 instance
  carries version/PCP, four-byte endpoint ID, three-byte service hash, optional
  UWB/WebRTC fields. TXT `n` carries base64 endpoint info (maximum 131 bytes).
- sharing/advertisement.cc: versions 0/1 share the salt/encrypted metadata-key
  layout, followed by optional public name and TLVs. Public names are ignored.
- sharing/paired_key_verification_runner.cc and sharing/proto/wire_format.proto:
  paired-key signature is role-prefixed UKEY2 auth material; secret_id_hash proves
  knowledge of the other side's discovery credential. Both proofs and explicit
  peer SUCCESS are required; UNABLE does not grant access.

Source root: https://github.com/google/nearby/tree/770a030be8fbebbcda5bf71ba718c35f5350c27c

The independent mDNS transport uses mdns-sd with its locked dependency and
declared MIT/Apache license. It performs only an explicit 20-second browse, not
service publication or TCP probing. Its current candidates require usable IPv4
private/link-local addresses and a current API-delivered own-device certificate
that authenticates the advertised encryption key and metadata. The service owns
cancellation and clears candidate references before account sign-out.

Local tests cover private advertisement parsing, authenticated names, absent
trusted credentials, case-insensitive DNS service types, malformed lengths,
cancellation and paired-key role/token/certificate mismatch. The file harness
exchanges actual paired-key frame shapes using process-owned synthetic keys;
real Google certificate distribution is not simulated as proof of compatibility.

Subsequent testing confirmed discovery of the authorized Android phone. Other Android
states may require Bluetooth-assisted discovery or announce differently; an empty Wi-Fi
LAN search is not evidence that private sharing is impossible. A full Nearby connection
and outgoing private transfer were subsequently implemented and verified on Android; see
[native-outgoing.md](native-outgoing.md). Discovery alone still does not authorize a
transfer. IPv6 remains unsupported. Google account tokens are not used by the discovery
worker, and the automated tests do not scan the LAN.

Private BLE discovery is now implemented; see [bluetooth.md](bluetooth.md).
