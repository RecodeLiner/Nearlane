# Published Quick Share behavior and initial file engine - 2026-11-15

## Use the same protocol and published implementation

The encrypted adapter now uses Google's public Better Together Core Rust UKEY2
and D2D implementation, with RustCrypto. Revision:
`479289ef072b0880c0347d36937265e44f00f4ee`. Six public components are packaged in
third_party/beto with their Apache-2.0 license/notices. All 45 Rust source files
match upstream byte-for-byte. Only manifests are adapted to select public crates,
use checked-in protobuf bindings and avoid private/missing workspace paths.

Source: https://beto-core.googlesource.com/beto-core/+/479289ef072b0880c0347d36937265e44f00f4ee/nearby/connections/ukey2/

The original wire adapter follows the published google/nearby schemas at
`770a030be8fbebbcda5bf71ba718c35f5350c27c`:

- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/connections/implementation/proto/offline_wire_formats.proto
- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/proto/wire_format.proto

Implemented subset: length framing; three-message UKEY2; AES-256-CBC/HMAC-SHA256
D2D with independent directions/order checks; single-file sharing introduction
and acceptance; Nearby BYTES/FILE chunks; terminal marker, ACK and cancellation.
There is no invented file-transfer protocol or proprietary binary dependency.

The loopback harness transfers 3,145,851 synthetic bytes, rereads the saved file
and compares SHA-256. Empty and chunk-boundary cases also pass. Tests reject
tampering/replay, malformed framing/offsets, conflicting wire subframes, path
traversal, existing destination/symlink overwrites, partial completion and size
policy violations. The 22 upstream UKEY2 unit tests pass in the adapted package.

This is **not** a complete Quick Share session. The harness starts directly at
UKEY2 and authenticates its two owned peers over process-local channels. It does
not implement Android discovery/connection negotiation, certificate publication/
download, sharing paired-key verification, keepalive dispatch, multiplexing,
production consent handling or cross-device recovery. The private-mode capability
flags stay false. No nearby third-party device has been scanned or contacted.

Live Android connection setup and private transfer are implemented outside
this loopback harness; see [native-outgoing.md](native-outgoing.md).
Incoming Android-to-PC saving is also verified; see [native-incoming.md](native-incoming.md).

## Sign-out, based on the published service

Google's NearbySharingServiceImpl::OnLogoutSucceeded forces a new endpoint ID and
calls ResetAllSettings(logout=true). That function stops advertising/scanning,
shuts down Nearby connections, stops certificate tasks, resets account-related
preferences and clears public certificates. It retains a pre-existing permanent
Everyone setting; Nearlane has no Everyone mode and always signs out to Hidden.

Source: https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/nearby_sharing_service_impl.cc

The AccountManager implementation referenced by this public source is not
included in that repository. Thus its exact token/server cleanup behavior has
not been established. Static absence of a literal revocation URL in the Windows
binary is not proof that it never performs revocation. No server-side device
unregistration is claimed without evidence.

Nearlane now routes Sign out through the service: reset visibility/capabilities,
cancel pending browser authorization and discard the in-memory account grant.
Late callbacks cannot restore it. Browser cookies and project-wide consent are
not changed. There are no registered device resources/certificate adapters yet
to clean up; those must join this lifecycle when implemented.

Google documents project-wide token revocation separately, affecting all clients
and scopes of that OAuth project for the authorized account. It is therefore not substituted
for ordinary logout with the imported client identity.
https://developers.google.com/identity/protocols/oauth2/native-app#tokenrevoke

## Independent credentials

Google Cloud supports registering a Desktop OAuth client for Nearlane. Such
application parameters would be issued by Google for Nearlane, not copied
from the official client. Normal use should not require registering a separate OAuth application.
However, nearbysharing-pa and nearbysharingsdk are absent from Google's current
public scope catalog; successful third-party access is not established. Creating
a client ID alone is not proof of access. Credentials from another identity
provider cannot replace Google authorization for Google's certificate service.

https://developers.google.com/identity/protocols/oauth2/native-app
https://developers.google.com/identity/protocols/oauth2/scopes
