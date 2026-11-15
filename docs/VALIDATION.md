# Validation baseline - 2026-11-15

Local environment: NixOS x86_64, locked nixpkgs
`d4f25a3e562008648d45bcf2d70bcc7422cad84e`, Rust 1.99.1, Qt 6.12.0.
Rust 1.85 is the declared minimum; it has not been tested locally.

## Hardware verification

- A 94-byte file was sent privately to a real Android phone, saved and opened.
- The same file was sent from Android to the PC and saved successfully.
- These tests verify the own-device transfer path. Contacts mode is unavailable.

Large-file Android delivery and screen-off Wi-Fi bootstrap are not verified.
The checks and earlier failure/correction sequence are recorded below.

Material desktop redesign:

- Stable Qt Quick Controls Material; no new Cargo/Qt library or experimental API.
- Real Qt screenshots inspected at 1040×760, 1040×900, 540×760 and 540×900.
  Sixteen synthetic cases cover both themes, selected/unselected file/device,
  200% text, a long device name, incoming consent, empty/loading/signed-out/error
  states and scrolled content. These are UI fixtures, not fake backend transfers.
- Rail and bottom-navigation control activation, resize continuity, retained
  device selection, Tab-reason focus and focus-chain availability passed.
  Full hardware keyboard/screen-reader and other operating systems are untested.
- Nominal foreground/background token pairs have at least 6.16:1 text contrast
  in light and 7.33:1 in dark. Disabled controls are not included in that claim.
- Native session/KDE environment smoke runs passed QML loading, private-mode
  gating, saved preferences, tray availability and close/show behavior. Qt style
  is now intentionally Material; the KDE environment still supplies native dialogs.
- `tools/preview_desktop.py` requires smoke mode with synthetic client config;
  the preview path never restores the saved account grant. No report filename or
  test-device list is supplied in normal startup. Captures remain ignored in work/.
- Dynamic wallpaper colors, high-contrast themes and full Material 3 Expressive
  component parity are not implemented. See DESIGN.md for the UI contract.

Verified:

- `cargo fmt --all -- --check`
- `cargo test --locked --workspace --exclude nearlane-desktop`: 95 tests pass on Linux,
  including local OAuth callbacks, sign-out, encrypted file streaming,
  tampering/replay, size/offset/path checks and non-overwrite behavior, plus
  public Google certificate crypto vectors, own-device-only publication,
  bounded pagination/cache replacement and late setup/cleanup completion,
  synthetic private advertisements and role/session-bound paired-key proof frames,
  restart/refresh-token rotation, stable registration with fresh keys, saved
  grant cancellation/deletion ordering and transient-versus-revoked failures,
  full outgoing setup with concurrent proofs, keep-alives, accepted file save
  and reread, wrong signer, rejection, cancellation, absent ACK, source symlinks
  and an absolute partial-frame deadline, plus exact/unambiguous CLI recipient
  selection, legacy version-4 encrypted acceptance and a reference EOF fixture;
  private receiver publication, unauthenticated receive gating, explicit consent,
  decline, unmatched senders, file content before consent, non-overwrite and
  cleanup, plus Android sender acceptance/repeated offers/capability queries
  interleaved before and after local consent.
  Background-specific checks cover unauthenticated watch gating, stop/reset
  behavior and destination retention across account-state changes.
  Notification checks cover bounded/redacted metadata, markup escaping and scoped
  action IDs. Receiving policy checks cover saved intent across sign-out,
  unauthenticated gating and switching back to explicit confirmation.
- The opt-in session-bus notification test passed for Accept, Decline and
  cancellation using its own mock service. It ignores wrong IDs/stale keys,
  handles an action arriving before the Notify reply and closes only its own ID.
  It does not replace the real desktop notification service or access accounts.
- The opt-in `system_store_roundtrip` test passed against the actual Linux
  Secret Service: created, read and deleted a unique synthetic entry; no account
  tokens or other vault entries were read. It is ignored in normal/Nix test runs.
- The opt-in native mDNS publication test passed: a synthetic private service
  was announced and resolved on the local PC in 0.97 seconds. It uses no account
  or real account credentials and is also ignored in normal/Nix test runs.
- The opt-in continuous-watch test passed: it found its own synthetic private
  advertisement, then removed it on withdrawal. No account access or TCP probe.
- `tools/smoke_desktop.py ... --kde --native --tray`: both session and KDE styles
  passed default-folder retention, tray availability, close-to-hide/show and a
  notification request with synthetic local authorization only.
  Isolated preference files verify enabled/autoaccept defaults, disabled/confirm
  settings and destination persistence across two desktop processes. They never
  modify personal desktop settings, including on Windows/macOS.
- 22 upstream UKEY2 unit tests pass with the adapted public RustCrypto package.
- `cargo run --locked -p nearlane-cli -- test-transfer`: 3,145,851 bytes transfer
  over owned loopback peers; the reread saved-file SHA-256 matches the source.
- `cargo clippy --locked --workspace --exclude nearlane-desktop --all-targets -- -D warnings`
- `cargo build --locked -p nearlane-desktop`
- Offscreen Qt smoke run: QML loads, the timer invokes Rust's private-mode check,
  a synthetic-config browser attempt binds/cancels without opening the browser,
  sign-out resets state, the service remains hidden, and the application exits
  successfully. Tests that bind loopback need local-network permission.
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
interactive desktop session. Windows/macOS execution has not been validated locally. Bluetooth discovery remains unimplemented.
Incoming and outgoing single-file stacks and UI are implemented and verified on real
Android hardware. Saving/opening a 94-byte private transfer to Android was confirmed,
followed by successful saving of the same file from Android to the PC. Bluetooth
background discovery and large files remain unverified. Follow-up
tray/continuous-discovery changes compile and pass the checks above. Windows/macOS tray
behavior and automatic acceptance on a real phone remain unverified. Native notification
buttons were confirmed working and a subsequent incoming transfer was instant; an
earlier transient Sending delay was not reproduced or attributed to a cause. Simulated
peer tests alone do not prove Android interoperability. Wi-Fi LAN discovery and metadata
decryption found the authorized Android phone. On 2026-11-15 an authorized trial used a
harmless 94-byte test file on that phone. `send-own` restored the existing Google grant
without a browser, completed certificate setup and found exactly the requested
certificate-matched phone. Its first connection failed during the encryption prelude,
before an introduction or file content. A second search found no matching advertisement.
Later diagnostics showed accepted Android feature version 4, which lacks the optional
version-6 file ACK; that is no longer a connection rejection. A subsequent test passed
mutual private proofs and successful peer results, receiver acceptance and a 94-byte
upload. The phone displayed receiving and eventually Can't transfer files, so that
attempt did not establish saved delivery. Chunk indices and exact EOF encoding were
corrected. The following real attempt passed mutual private proof/receiver acceptance
again, and the test file was confirmed received and opening successfully. The peer
closed without a file ACK; this stays unconfirmed in the sender's wire-level status
rather than fabricating a receipt. Diagnostics retain only safe protocol checkpoints,
never tokens, handshake bytes or traffic captures. No saved token values were inspected
and no new account was authorized for this check. The local file engine is implemented,
but the self-test starts directly at UKEY2 and is not a complete Nearby connection or
Android/private-mode session. The application displayed “Quick Share permission granted”
after browser login on 2026-11-15. Successful registration and query were subsequently
confirmed: 9 own-device certificates, 6 active, 0 malformed. That establishes
authenticated API exchange in the authorized account session; live Android verification
is recorded separately above. Earlier public-endpoint probes used no account or cookies:
OAuth reached Google's login page, and a separate empty HTTP/2 gRPC request received
UNAUTHENTICATED status 16 from Nearby Identity. The app now implements temporary
own-device publication and queries. Explicit device search starts a bounded LAN search;
it never publishes a receiver or enables file sharing on that basis alone. See [the
login experiment](research/oauth-runtime.md) and [the certificate/API
evidence](research/identity-certificates.md).

Debug C++ builds can warn that FORTIFY needs optimization; the release package
is optimized. Tests do not use real account credentials or proprietary fixtures.
