# Security boundary

This is a research prototype, not a security-audited sharing application.

Browser OAuth is opt-in; no file-transfer listener is currently exposed. A login
binds only IPv4 loopback on an OS-assigned port before opening the system browser.
It has a five-minute deadline, bounded HTTP requests, a short per-connection
deadline, exact Host/path validation and no Origin/body/transfer-encoding support.
Independent random state and S256 PKCE bind the callback to this attempt. Stray
requests do not consume it; provider denial requires matching state as well.

Codes go only to the fixed Google HTTPS token endpoint, with TLS validation,
redirects disabled, connection/request timeouts and a 64 KiB response limit.
The session requires the exact sharing scope and expires locally. Cancellation
discards late results. The worker may finish an already-sent HTTPS request within
its 20-second timeout, but cannot restore the cancelled session.

QML sees status and capability flags, not configuration, authorization URLs,
codes or tokens. Bearer and refresh tokens are not written to disk. Closing the
app or clearing the local session drops them; clearing does not revoke provider
consent. Secrets redact Debug and use zeroizing owned strings. This does not
guarantee secrets are absent from all allocator, parser, TLS or OS copies.

After sign-in, a separate worker prepares three temporary SELF
certificates and exchanges them with the fixed TLS origin nearby.googleapis.com
using gRPC with validated roots and real trailer/status handling. No configurable
API origin, proxy callback, account cookies or plaintext transport is accepted.
Only numeric gRPC failure codes reach the UI; provider error text is discarded.
Queries are limited to eight pages, 256 records, 4 MiB of credential data and a
1 MiB decoded RPC message. Only own-device certificates enter this cache. No
remote record alone enables discovery, trust or file transfer.

The new cryptographic module follows Google's public Nearby certificate code:
P-256 signatures, HKDF-SHA256, AES-CTR discovery keys and AES-GCM metadata.
Private signing keys are never published. Symmetric discovery keys are included
in the sharing credential sent to Google, as the protocol requires, and must
also stay out of logs and untrusted application surfaces. All local keys, salts
and downloaded certificates are session-only. Certificate keys/metadata are
immutable to avoid AES-GCM nonce reuse with changed plaintext.

Sign-out first removes local visibility/trust, then asks the server to replace
this process-owned resource's SELF credentials with an empty list. This is an
experimental withdrawal request, not a verified server deletion guarantee.
Keep the process running while cleanup finishes. Without confirmed cleanup,
credentials may remain at Google for up to nine days plus two hours. Signing out
never changes another device's resource or revokes the shared OAuth project.

Local-only examination/reuse of the official desktop application configuration is
explicitly authorized for this research. Only the previously hashed artifact is accepted
by the importer. Its output stays in ignored work/private/ (directory 0700, file 0600),
outside Nix sources/resources. This is an interoperability experiment, not a claim of
provider authorization. Never distribute this local configuration.

The core must stay hidden when an authentication feature is unavailable. A
contact is a cryptographic account identity, not a friendly name, MAC address or
Bluetooth pairing. Receiving a signed-in account identifier does not prove
contact certificate distribution works.

Session persistence will require OS credential storage (Secret Service/Keychain/
Windows), refresh/revocation handling and corresponding tests. Sign-out resets
sharing visibility/trust before dropping the session and prevents late callbacks
from restoring it. Provider-wide revocation is not performed as ordinary logout.

The file engine uses pinned public Google UKEY2/D2D sources with RustCrypto.
Framing bounds apply before allocation; a failed integrity/sequence check makes
the channel unusable. This does not authenticate a Google contact: the production
certificate/paired-key verifier is still missing, so only owned loopback test
peers are connected. No file listener or discovery is advertised on the LAN.

The initial receiver accepts one approved file, enforces size/ID/type/offset/EOF
consistency, rejects remote paths/device names and stages into a private random
file. Completion publishes without replacing existing paths, including symlinks.
Earlier errors/cancellation drop staged files. A lost ACK can leave a complete
file saved without confirmed delivery at the sender. Caller IO deadlines bound
stalled reads/writes; cancellation is checked between frames. Resume, folders,
multi-file transactions and production consent/space-management are not ready.

Never upload account tokens or full traffic captures to public issues. This
repository has no public remote or private reporting channel configured yet.
