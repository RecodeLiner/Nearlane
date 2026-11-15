# Local browser OAuth experiment

This experiment extends the static findings in 2026-11-14-auth.md through
local-only examination/reuse of desktop parameters authorized for this research.
On 2026-11-15 the application displayed “Quick Share permission granted”
after browser login. Raw account tokens were not inspected as part
of this observation. That validates the granted-scope path for this local setup;
it is not a certificate API result or provider approval of an independent client.

## Static evidence

The previously hashed NearbyShareExe contains one matching desktop client ID
and one GOCSPX-format desktop client secret at file offsets 0x2568880 and
0x25688c8. Their values are intentionally omitted. Code at VA 0x1406c2400 compares
both fields, and neighboring constructors copy them. Those observations support
treating them as paired application parameters, rather than account credentials.
They do not establish any third-party permission to use that application identity.

The original importer accepts only the exact previously recorded SHA-256 and
size. It writes exclusively to work/private/oauth-client.json with 0600 mode
inside a 0700 directory, refuses an existing output, and prints no values.
No proprietary implementation code was incorporated into Nearlane.

## Unauthenticated endpoint check

Two requests to Google's public authorization endpoint used a fresh state, S256
challenge, loopback redirect and the locally imported application ID. They used
no account cookies, credentials, browser profile or signed-in session.

| Requested permission | Observed response |
| --- | --- |
| nearbysharing-pa | HTTP 302 to accounts.google.com/v3/signin/identifier |
| nearbysharingsdk | HTTP 302 to accounts.google.com/signin/oauth/error |

The SDK error was not classified; do not label it invalid_scope without evidence.
The sharing result proves only that the request reached the login page. It does
not prove that signing in will produce a grant or enable access to certificate APIs.
Raw authorization URLs and response headers are not committed or logged.

## Runtime implementation

The new nearlane-account crate handles bounded local config reading, one-shot
loopback callback, cancellation, timeout and fixed-origin HTTPS code exchange.
The Qt app opens the system browser on an explicit local action, displays sanitized
progress, and keeps account credentials out of QML. Only the exact sharing scope
can establish AccountStatus::Authorized. SDK-only grants cannot satisfy it.

The initial runtime kept bearer grants only in memory. Saved grants and automatic
renewal were subsequently implemented; see [saved-login.md](saved-login.md). Access
tokens remain in memory, and provider-wide revocation is not ordinary sign-out. A late
worker result cannot restore a cancelled session. An OAuth grant alone does not enable
private sharing. Own-device certificate exchange is implemented; Contacts credential
distribution remains unavailable.

Tests use synthetic application parameters and local TCP only, with an injected
token exchanger. They exercise stray callback survival, state/Host/request checks,
one-use completion, timeout, cancellation during exchange, wrong-scope rejection
and session expiry. Qt smoke tests additionally bind/cancel a local login without
opening a browser or contacting Google.

## Validation status

1. Browser login and the required Quick Share permission are confirmed. Raw account
   tokens and callback/response captures are not part of the published record.
2. Device registration and certificate publication/query are confirmed: 9 records,
   6 active, no malformed records in the recorded session.
3. A live private handshake and a 94-byte transfer to Android are confirmed.

Legal and provider-policy limitations: ../LICENSING.md.
