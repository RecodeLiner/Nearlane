# Architecture and build boundaries

The shared core and service own policy. Platform adapters own OS services. The
Qt application presents state and sends commands; it must not mint or persist
credentials. An account plugin from Scotty is neither used nor required.

Dependency graph:

```text
desktop (Qt/QML) ──> service ──> core
                      └─────> platform
cli ──────────────> service
 └────────────────> auth
protocol (independent; not connected to service yet)
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
2. Authorized third-party browser OAuth access to a suitable Nearby scope.
3. Certificate publication/download, then real Contacts/OwnDevices handshake.
4. One small file exchanged with Android while public visibility remains off.
5. Background lifecycle, tray, OS secret stores, recovery and packaging.

Milestone 2 is not solved by discovering a string in a binary. Client allowlists,
API provisioning or Google review may prevent it; no such restriction is assumed
proven until observed. macOS/Windows compile and hardware behavior must be tested
on those operating systems. Rust portability does not abstract away their radios.
