# Architecture and build boundaries

The shared core and service own policy. Platform adapters own OS services. The
Qt application presents state and sends commands; it must not mint or persist
credentials. An account plugin from Scotty is neither used nor required.

Dependency graph:

```text
desktop (Qt/QML) ──> service ──> core
                      └─────> platform
                      └─────> account ──> auth
cli ──────────────> service
 └────────────────> auth
cli ──────────────> transfer ──> protocol ──> Google UKEY2 + RustCrypto
```

Each directory under crates/ is a Cargo package with an independent compilation
unit. Cargo shares a target directory across this workspace, caches dependency
artifacts, and builds independent units in parallel. Incremental compilation is
used by the dev profile. No global linker flags or static Qt configuration are
required. Dynamic Qt linking is intentional.

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
2. Browser OAuth access to a suitable Nearby scope: the local runtime is now
   implemented, but a real grant and provider permission remain unverified.
3. Certificate publication/download, then real Contacts/OwnDevices handshake.
4. One small file exchanged with Android while public visibility remains off.
5. Background lifecycle, tray, OS secret stores, recovery and packaging.

The encrypted file engine now streams a single file in bounded Nearby payloads,
with a sharing introduction/response subset and receiver acknowledgement. Its
loopback harness verifies two process-owned peers, saves without overwriting and
checks the reread digest. This is not the Android milestone: production discovery,
Nearby connection setup, certificate/paired-key verification and lifecycle remain
unconnected. The desktop does not advertise a fake receiver or enable Contacts.

Sign-out is owned by the service: reset visibility/trust, then cancel the browser
attempt and drop the account session. Future discovery/connections/certificate
adapters must be shut down there, following Google's published service lifecycle.
Provider-wide OAuth revocation is distinct from ordinary device sign-out.

Milestone 2 is not solved by discovering a string in a binary. Client allowlists,
API provisioning or Google review may prevent it; no such restriction is assumed
proven until observed. macOS/Windows compile and hardware behavior must be tested
on those operating systems. Rust portability does not abstract away their radios.
