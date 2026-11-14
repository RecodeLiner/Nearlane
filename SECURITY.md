# Security boundary

This is a research prototype, not a security-audited sharing application.

No Google login or file-transfer listener is currently exposed. OAuth primitives
generate random state and S256 PKCE; the callback verifier checks the exact
loopback origin, port and path, rejects duplicate parameters, and consumes each
attempt. Secrets redact Debug and use zeroizing owned strings. This does not
guarantee secrets are absent from all allocator, parser or OS copies.

The core must stay hidden when an authentication feature is unavailable. A
contact is a cryptographic account identity, not a friendly name, MAC address or
Bluetooth pairing. Receiving a signed-in account identifier does not prove
contact certificate distribution works.

Before enabling real login: implement a bounded loopback listener; expiry and
cancellation; narrowly scoped HTTPS token exchange; validated granted scopes;
OS credential storage (Secret Service/Keychain/Windows); provider-side revocation;
and tests that QML cannot access tokens. Real transfer additionally needs
reviewed UKEY2, certificate validation, filename/path containment and limits.

Never upload account tokens or full traffic captures to public issues. This
repository has no public remote or private reporting channel configured yet.
