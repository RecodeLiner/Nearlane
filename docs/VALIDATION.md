# Validation baseline - 2026-11-14

Local environment: NixOS x86_64, locked nixpkgs
`d4f25a3e562008648d45bcf2d70bcc7422cad84e`, Rust 1.99.1, Qt 6.12.0.
Rust 1.85 is the declared minimum; it has not been tested locally.

Verified:

- `cargo fmt --all -- --check`
- `cargo test --locked --workspace --exclude nearlane-desktop`: 25 tests pass,
  including local TCP callback and cancellation during code exchange.
- `cargo clippy --locked --workspace --exclude nearlane-desktop --all-targets -- -D warnings`
- `cargo build --locked -p nearlane-desktop`
- Offscreen Qt smoke run: QML loads, the timer invokes Rust's private-mode check,
  a synthetic-config browser attempt binds/cancels without opening the browser,
  the service remains hidden, and the application exits successfully.
- `python3 -m unittest discover -s tools -p 'test_*.py'`: 7 parser/import tests.
- `nix build`: a release package was built and its CLI runs from the Nix store.

UI smoke invocation for the installed package (no forced Fusion theme and no
warning suppression):

```sh
python3 tools/smoke_desktop.py result/bin/nearlane-desktop --kde
# To check the actual desktop display as well (the windows close automatically):
python3 tools/smoke_desktop.py result/bin/nearlane-desktop --kde --native
```

The earlier Fusion-only check missed 40 warnings in the KDE desktop style.
Kirigami's StyleSelector scans QML_IMPORT_PATH/QML2_IMPORT_PATH independently of
the QML engine's Nix-patched import paths. In this Plasma session it picked the
partial `/run/current-system/sw` Kirigami tree, containing only styles, and then
registered empty component URLs. The Linux package and development shell now
include the pinned KDE runtime and prepend the complete Kirigami module to
QML_IMPORT_PATH. No Qt warnings are filtered and no style is forced by the app.
The smoke tool forces Qt diagnostics to stderr: otherwise Qt can send them to
the journal, making a captured stderr appear clean.

Upstream reference: [Kirigami StyleSelector, v6.31.0](https://github.com/KDE/kirigami/blob/v6.31.0/src/platform/styleselector.cpp).

This checks loading and QML/Rust integration, not a visual screenshot or a real
interactive desktop session. Windows/macOS execution has not been validated locally. Radio, firewall, Google login,
certificate API access and Android file transfers remain untested/unimplemented.
The browser callback/HTTPS code path is implemented; a real browser-initiated
Google grant has not been tested at this stage. The public-endpoint request used no account
or cookies and reached Google's login page for the sharing scope. See
[the experiment record](research/oauth-runtime.md).

Debug C++ builds can warn that FORTIFY needs optimization; the release package
is optimized. Tests do not use real account credentials or proprietary fixtures.
