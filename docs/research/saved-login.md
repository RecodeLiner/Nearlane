# Saved sign-in and certificate inventory - 2026-11-15

Private Wi-Fi LAN discovery found the authorized Android phone. This
confirms advertisement matching against the account's certificate cache, not a
live paired-key proof or file transfer with Android.

## Saved authorization

Earlier builds deliberately retained tokens only in memory. The new desktop
restores a bounded refresh-token record from the OS credential store and renews
through the fixed Google HTTPS token endpoint. It never asks for a master token
or a separately registered OAuth client. Explicit browser login requests offline consent
when a saved grant is absent; normal restoration does not open the browser.

Primary protocol reference:
https://developers.google.com/identity/protocols/oauth2/native-app#offline

Storage uses keyring 3.6.3 with explicit native platform features, not its mock
fallback: encrypted Secret Service on Linux, Keychain on macOS and Credential
Manager on Windows. Linux's pure Rust Secret Service backend avoids system DBus
development-library requirements. The stable application namespace is combined
with SHA-256 of the configured OAuth client ID; raw application parameters and
account names are not vault attributes. The record contains version, exact
sharing scope, refresh token and a ten-character registration ID. Access tokens
and private device keys are not persisted. There is no file-based fallback.

Library reference: https://docs.rs/keyring/3.6.3/keyring/

The single worker serializes refresh, saving and forgetting. Sign-out queues
deletion after earlier writes; operation generations discard stale authorization
events. Network failures preserve offline access, invalid_grant removes the saved
grant. Locked-vault or failed-deletion status is visible. Process exit can interrupt
cleanup; keep one active application instance when signing out/changing accounts.
No cross-process account-switch coordination is claimed.

Restoration reuses the registration resource name while minting fresh certificate
keys and advertisement salt state. This avoids deliberate GCM/salt/key reuse and
creating another resource ID at every restart, but is not durable private-key
storage. Google-side credential-replacement semantics remain a separate check.

The local OS-vault check created/read/deleted only a uniquely scoped synthetic
entry. Restart, token rotation, network failure, revoked grants and late
completion after sign-out are tested with isolated synthetic stores/HTTP results.
Saved-grant restoration succeeded during the authorized Android LAN test.
Raw refresh tokens were not directly inspected or published.

## Why all nine certificates cannot be assigned phone names

Google's public rpc_resources.proto explicitly documents that
metadata_encryption_key is uploaded but withheld from returned certificates.
Its decrypted-public-certificate implementation obtains this key by decrypting
the rotating metadata-key material in a nearby advertisement, then verifies its
commitment and decrypts device metadata.

References at the pinned public revision:

- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/proto/rpc_resources.proto
- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/certificates/nearby_share_decrypted_public_certificate.cc

The UI inventory shows every cached certificate's validity and temporary row
number. Only currently owned private keys and exact certificate matches from
authenticated discovery receive names. Unknown/future rotating records are not
assigned to an observed phone by inference. No raw certificate IDs/keys are sent
to QML. Names and date strings are rendered as plain text. This is an inventory
of credentials, not a complete account-device directory.
