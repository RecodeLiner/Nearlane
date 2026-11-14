# Own-device identity and certificates - 2026-11-15

## Evidence

Google's public Nearby source, revision
`770a030be8fbebbcda5bf71ba718c35f5350c27c`, uses PublishDevice and
QuerySharedCredentials in its certificate manager. It groups sharing certificates
by SELF/CONTACTS visibility, creates three consecutive 72-hour certificates and
adds up to two hours of public timestamp obfuscation. Its public/decrypted/private
certificate implementations and synthetic test_util.cc vectors are the reference
for Nearlane's original Rust certificate module.

Primary sources:

- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/certificates/nearby_share_certificate_manager_impl.cc
- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/certificates/nearby_share_private_certificate.cc
- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/certificates/nearby_share_decrypted_public_certificate.cc
- https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/certificates/test_util.cc

The examined official NearbyShareExe is SHA-256
`4364301891d44e7b1ee84685874f559cda405ccdfcb0637df5f7ec223cb806c4`.
Its embedded interface descriptors identify the gRPC service
`google.nearby.identity.v1.NearbyService`, PublishDevice/QuerySharedCredentials
methods, resource field numbers and SELF enum values. The binary contains the
host nearby.googleapis.com. Interface adapters are original minimal Rust;
proprietary descriptor dumps and binary artifacts remain in ignored work/.

A five-byte empty gRPC request to GetAccountInfo at that HTTPS origin, with
no bearer token, cookies or real account, negotiated HTTP/2 and returned
HTTP 200, application/grpc, grpc-status 16 UNAUTHENTICATED. This confirms a gRPC
transport endpoint, **not** authenticated access or successful registration.
The actual account permission was confirmed in the previous build;
raw account tokens were not directly inspected or manipulated.

## Implemented, locally validated

- A separate certificates crate with no network, Qt or persistent storage.
  Public Google vectors match HMAC, HKDF, AES-CTR, AES-GCM and P-256 verification.
  Generated credentials round-trip, rotate advertisement salts and reject
  modified metadata, wrong role signatures, malformed keys and expired windows.
- A session-owned ten-character random device ID; three certificates for SELF
  only. No contact-list refresh or CONTACTS credential distribution is requested.
- Fixed-origin TLS gRPC with a sensitive bearer metadata field, real gRPC
  trailer handling, bounded replies/pages and deadlines. Status text and remote
  metadata never reach QML/logs. Failed pagination preserves the previous cache.
- Idempotent retries reuse the same name and keys within the account session.
  Query results retain only non-expired own-device records, including future
  rotation certificates. API-delivered data is necessary for a future verifier;
  the self-share flag in arbitrary peer-supplied data is not identity proof.
- Sign-out clears local trust and asks to publish an empty SELF credential list
  for this owned resource. If cleanup fails, local keys are still discarded and
  the UI reports that remote cleanup was not confirmed. Late replies cannot
  reauthorize the signed-out account.

## Inference and remaining checks

Authenticated publication/query, the acceptability of CONTACT_SELF with the
existing sharing-pa token, and remote empty-publication semantics are untested.
An empty SELF group is a withdrawal request inferred from the publication model;
no undocumented Delete RPC or project-wide OAuth revocation is attempted.

Google's public implementation computes credential wrapper IDs using its
HighwayFingerprint64 helper. Nearlane currently uses the first eight bytes of
the cryptographically random certificate identifier as a stable opaque int64.
No cryptographic check relies on that wrapper ID. Whether the backend requires
the exact Google fingerprint remains an interoperability question; do not claim
byte-for-byte equivalence or attribute a server rejection to an allowlist without
further evidence.

Keys are not persisted or refreshed. App exit can interrupt cleanup, and remote
certificates can remain until expiry (maximum nine days plus two hours). Normal
sign-out does not remove browser cookies or revoke the OAuth application grant.

This build stays hidden even after a successful server exchange. Real nearby
discovery, connection setup, paired-key exchange over the encrypted channel and
Android file transfer remain to be wired and checked with an owned phone.
