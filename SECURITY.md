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
codes or tokens. Access tokens stay in memory. Refresh tokens are persisted only
in the OS credential store, scoped to this application and a hash of the local
OAuth client ID. There is no plaintext-file fallback. Closing the app preserves
offline access; Sign out queues deletion of this app's vault entry and does not
revoke provider consent. Secrets redact Debug and use zeroizing owned strings. This does not
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

The desktop explicitly starts saved-login restoration; the service constructor,
Offline CLI diagnostics and normal test suites do not access the vault or
auto-login. The explicit `send-own --device EXACT_NAME FILE` command restores
only this app's saved grant, prepares own-device credentials and performs the
same private discovery and send flow as the desktop. It never starts browser
authorization or picks a similar/ambiguous device name. Run only one app instance
when using the same saved registration.
Linux uses encrypted Secret Service sessions through the pure Rust backend;
macOS/Windows use their native keyring adapters and remain locally untested.
The persisted record is bounded, versioned and requires the exact sharing scope.
It contains only the refresh token and ten-character device registration ID,
never the access token, signing keys, downloaded certificate cache or client secret.
Renewal uses the same fixed HTTPS token endpoint and timeouts as code exchange.
Omitted refresh scope/token may inherit the previous validated values; an
explicitly conflicting scope fails. Transient errors keep offline access;
invalid_grant discards it. A serial worker orders saving, renewal and deletion,
and per-operation generations prevent late completion from restoring sign-out
within this process. Keep one active Nearlane instance while changing accounts.
Keep the app open until vault deletion and remote cleanup finish; neither can
complete after process termination. A locked vault can require desktop interaction.
Sign-out resets
sharing visibility/trust before dropping the session and prevents late callbacks
from restoring it. Provider-wide revocation is not performed as ordinary logout.

The file engine uses pinned public Google UKEY2/D2D sources with RustCrypto.
Framing bounds apply before allocation; a failed integrity/sequence check makes
the channel unusable. The explicit outgoing operation connects only to a recently
certificate-matched own-device candidate and requires mutual role-bound paired-key
verification. It does not authenticate arbitrary Google contacts. No production
file receiver is advertised on the LAN.

Explicit device search browses only the NearbySharing DNS-SD service
for 20 seconds, with limits of 1024 events and 32 candidate endpoints. It does not
make TCP connections, register a service or open a file receiver. The initial
adapter retains private/link-local IPv4 candidates only. Sharing advertisement
metadata must pass current own-device certificate commitment and AES-GCM checks;
publicly advertised names are never used. Candidate names use plain-text QML
rendering. Matching metadata is not live identity proof: an old advertisement
can be replayed, and connection destinations still require paired-key validation.
Stop, sign-out, session expiry and retry clear candidates and cancel discovery;
late results cannot restore them. Certificate references held by a cancelled
worker are released as it finishes, rather than copied to disk or exposed to UI.

The certificate inventory exposes only temporary row numbers, public validity,
locally owned-key labels and names authenticated by discovery. It never exposes
raw IDs, symmetric keys or guesses about unidentified/future rotation certificates.
Google's published schema explicitly says metadata_encryption_key is withheld
from certificate downloads; the full device list cannot be derived from those
records alone. Names and dates are rendered as plain text and discarded at sign-out.

The local file harness now exchanges paired-key proofs bound to UKEY2 token and
sender/receiver roles, plus explicit successful peer results. Credentials are
synthetic and locally distributed by the harness. This tests the new verifier
and frame shapes, not Google-backed peer identity or a full Android connection.

The initial receiver accepts one approved file, enforces size/ID/type/offset/EOF
consistency in the local harness; it is not a production incoming listener.
The outgoing desktop operation opens only an explicitly selected regular file,
up to 64 MiB, on a worker. Unix source opens are nonblocking and refuse symlinks.
Its connection prelude is plain request, UKEY2, then mutual plain acceptance
before encryption. Only after private proof may an introduction be sent; only
after receiver acceptance may file content be sent. Selected destinations expire
after 120 seconds; no peer-supplied address or Wi-Fi upgrade is followed.
Control fragments, payload IDs and counts are bounded, keep-alives are answered
during upload, and an absolute socket deadline bounds partial trickle frames.
Only a matching file ACK after EOF confirms delivery. Legacy acceptance without
this optional feature is supported and never weakens mutual private proof. No
payload ACK is sent when not negotiated. A five-second post-EOF grace period
for such peers prevents keep-alives from prolonging an impossible receipt wait.
The bounded safe-close exchange follows confirmed or unconfirmed EOF; it cannot
undo an ACK or turn an unconfirmed upload into confirmed delivery. Chunks carry
monotonic indices and a separate terminal chunk matching the public sender.
Cancellation/sign-out shut down the established
socket and discard late results. Slow filesystem operations are outside socket
cancellation guarantees. Full outgoing automated tests use simulated owned peers.
An authorized test on a real Android phone confirmed that a
94-byte text file was received and opens after mutual private verification and
receiver acceptance. The phone advertised version 4 and closed without a file
receipt. Background operation and large-file Android behavior are not verified.
See docs/research/native-outgoing.md.

The file sink enforces size/ID/type/offset/EOF
consistency, rejects remote paths/device names and stages into a private random
file. Completion publishes without replacing existing paths, including symlinks.
Earlier errors/cancellation drop staged files. A lost ACK can leave a complete
file saved without confirmed delivery at the sender. Caller IO deadlines bound
stalled reads/writes; cancellation is checked between frames. Resume, folders,
multi-file transactions and production consent/space-management are not ready.

Never upload account tokens or full traffic captures to public issues. This
repository has no public remote or private reporting channel configured yet.
