# Architecture and build boundaries

The shared core and service own policy. Platform adapters own OS services. The
Qt application presents state and sends commands; it must not mint or persist
credentials. An account plugin from Scotty is neither used nor required.

Dependency graph:

```text
desktop (Qt/QML) ──> service ──> core
                      └─────> platform
                      └─────> account ──> auth
                                  └────> certificates ──> core + RustCrypto
                                  └────> platform ──> OS credential store
                      └─────> discovery ──> certificates + mDNS
                      └─────> transfer ──> protocol + certificates
cli ──────────────> service
 └────────────────> auth
cli ──────────────> transfer ──> protocol ──> Google UKEY2 + RustCrypto
```

Each directory under crates/ is a Cargo package with an independent compilation
unit. Cargo shares a target directory across this workspace, caches dependency
artifacts, and builds independent units in parallel. Incremental compilation is
used by the dev profile. No global linker flags or static Qt configuration are
required. Dynamic Qt linking is intentional.

Bluetooth discovery uses Qt Bluetooth only in the desktop boundary, with native
BlueZ/CoreBluetooth selected by Qt. Bounded BLE packet decoding, trusted hint
matching and all file policy stay in portable Rust. The optional raw wake beacon
is a separate platform adapter; macOS does not falsely inherit Linux capability.
Darwin packaging includes an app bundle and Bluetooth permission metadata.
The radio adapter never owns account grants or authorizes a file from a hint.

The per-file policy is shared in core (64 GiB). The file stack maintains fixed
64 KiB buffers, separate 30-second frame, 90-second file-progress idle and 24-hour
total-transfer bounds. Only validated file progress renews idle time. Control
traffic is limited per minute and cannot extend a stalled transfer. File frames
are no longer charged to the old lifetime control-frame count.

The initial Nix package is a single buildRustPackage derivation: its source changes
rebuild that derivation rather than sharing the development target/ cache. Cargo
workspace caching applies to local development and validation. Splitting immutable third-party
build artifacts into a separate Nix derivation is a later optimization, not a
feature already provided by this flake.

Nix uses an explicit qt6.env union: CXX-Qt asks qmake for module libraries, but
plain qtbase's qmake only sees Qt Core/Gui. Without the union Qml's .prl files
cannot be found on NixOS. The Qt boundary needs generated unsafe FFI code; all
non-UI Rust packages forbid unsafe code.

## Milestones

1. Reproducible offline binary evidence, a buildable UI and isolated auth tests.
2. Browser OAuth access to a suitable Nearby scope: implemented; the actual
   Quick Share permission was confirmed in the UI on 2026-11-15.
3. Certificate publication/download: implemented, real API exchange confirmed
   (9 own-device certificates, 6 active, no malformed records).
   Wi-Fi LAN advertisement matching is confirmed on the authorized Android phone.
   Live account-backed verification is implemented and required for each transfer.
4. A 94-byte file sent to Android and received back on the PC, with public visibility off.
5. Background lifecycle, tray, OS secret stores, recovery and packaging.

The encrypted file engine now streams a single file in bounded Nearby payloads, with a
sharing introduction/response subset and receiver acknowledgement. Its loopback harness
verifies two process-owned peers, saves without overwriting and checks the reread
digest. The explicit outgoing desktop operation now connects discovery to current
account-backed signing credentials, Nearby setup, mutual paired-key proofs, receiver
consent, streaming and payload acknowledgement. Full-stack automated tests use simulated
owned peers. A real Android peer advertised feature version 4: the optional version-6
payload receipt is not a transport acceptance requirement. After chunk-index/EOF
corrections, a 94-byte private outgoing file was confirmed as saved and opening
successfully on that phone. No wire receipt is inferred from that external observation;
legacy EOF cleanup is bounded and missing receipts remain explicit in the sender status.
The legacy local harness uses real paired-key frame shapes and synthetic certificate
signatures instead of process-local auth-token exchange. The desktop explicitly browses
Wi-Fi LAN advertisements and shows only names from certificate-authenticated metadata. A
separate receiving worker publishes private endpoint information after authorized
private setup when desktop receiving is enabled (the desktop default). It requires a
verified own peer and a service policy decision, then reuses the staging sink and duplex
session. The service owns its lifetime, prevents concurrent outgoing/certificate
rotation, and resets receiving on sign-out or session loss. Effective visibility is
OwnDevices only while the receiver is advertised; Contacts remains unavailable. The bare
service/CLI is hidden by default. Desktop settings default to own-device reception and
automatic acceptance; optional per-file consent remains available. Discovery alone never
establishes permission to receive. Qt QSettings holds only local preferences,
independently of account trust and OS-vault credentials. An outgoing operation snapshots
its recipient, cancels an idle receiver and waits asynchronously for its signing handle,
then reception resumes after sending. Native Linux notifications live in the platform
crate; the desktop controller maps their scoped actions through the service's current
verified-offer gate.

Sign-out is owned by the service: reset visibility/trust, then cancel the browser
attempt and drop the account session. Future discovery/connections/certificate
adapters must be shut down there, following Google's published service lifecycle.
Provider-wide OAuth revocation is distinct from ordinary device sign-out.
The saved-login worker serializes OS-vault reads/writes, Google refresh and
deletion. UI startup or explicit CLI send-own starts restore; service construction
and offline CLI diagnostics stay offline.
Generation-checked completions cannot revive a signed-out session. Restored
sessions reuse their registration ID but generate fresh in-memory certificate
keys; do not confuse this with persisted radio/private key state. Tokens refresh
before expiry, transient failures keep the saved grant, and revoked grants fail
closed. Platform-specific storage is isolated from Qt and authentication parsing.
The account worker now publishes only SELF certificates to the fixed Nearby
Identity gRPC service and queries a bounded, memory-only certificate cache.
Signing out requests an empty SELF publication for the same owned resource;
server-side removal semantics remain unverified. Failed cleanup is visible,
and a late reply cannot revive local trust. Nothing enables private visibility
merely because the certificate server exchange succeeds.

An OAuth grant alone does not establish certificate API access. Client allowlists,
API provisioning or Google review may prevent future features; no such restriction
is assumed proven until observed. The current publication/query path has worked
in the authorized account session. macOS/Windows compile and hardware behavior must be tested
on those operating systems. Rust portability does not abstract away their radios.

## Localization

The desktop owns QTranslator and locale preferences. UI text lives in two Qt TS
catalogs (English and Russian), compiled with Linguist lrelease into embedded
resources. QML uses the Ui context. Shared services expose deferred core::Text
messages in the Backend context, alongside English CLI rendering. Templates and
nested messages are translated before opaque literal arguments are inserted;
account names, filenames and paths never enter a translation lookup or a second
placeholder pass. No Qt dependency was introduced into shared crates.

A saved system/ru/en setting is validated before application. Changing language
retranslates existing QML bindings and refreshes statuses without restarting
protocol jobs or account state. A pending notification is withdrawn and recreated
with translated labels, preserving the service offer ID and consent gate.
Platform notification action keys remain random and independent of labels.
Native OS dialogs may follow the OS language; Qt-owned dialogs use available Qt
module translations. macOS usage descriptions have en/ru InfoPlist.strings.
