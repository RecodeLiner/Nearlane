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
Windows), refresh/revocation handling and corresponding tests. Real transfer
additionally needs reviewed UKEY2, certificate validation, filename/path
containment and limits. Ordinary sign-in does not enable private-mode discovery.

Never upload account tokens or full traffic captures to public issues. This
repository has no public remote or private reporting channel configured yet.
