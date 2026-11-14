# Validation baseline - 2026-11-14

Local environment: NixOS x86_64, locked nixpkgs
`d4f25a3e562008648d45bcf2d70bcc7422cad84e`, Rust 1.99.1, Qt 6.12.0.
Rust 1.85 is the declared minimum; it has not been tested locally.

Verified:

- `cargo fmt --all -- --check`
- `cargo test --locked --workspace --exclude nearlane-desktop`: 17 tests pass.
- `cargo clippy --locked --workspace --exclude nearlane-desktop --all-targets -- -D warnings`
- `cargo build --locked -p nearlane-desktop`
- Offscreen Qt smoke run: QML loads, the timer invokes Rust's private-mode check,
  the service remains hidden, and the application exits successfully.
- `python3 -m unittest discover -s tools -p 'test_*.py'`: 4 parser-boundary tests.
- `nix build`: a release package was built and its CLI runs from the Nix store.

Example UI smoke invocation inside `nix develop`:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= \
QT_QUICK_CONTROLS_STYLE=Fusion QT_QUICK_BACKEND=software \
NEARLANE_SMOKE_TEST=1 timeout 10s target/debug/nearlane-desktop
```

This checks loading and QML/Rust integration, not a visual screenshot or a real
interactive desktop session. Windows/macOS execution has not been validated locally. Radio, firewall, Google login,
certificate API access and Android file transfers remain untested/unimplemented.

Debug C++ builds can warn that FORTIFY needs optimization; the release package
is optimized. Tests do not use real account credentials or proprietary fixtures.
