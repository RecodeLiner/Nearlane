# Nearlane development conventions

- Use a Cargo workspace. Keep the Slint desktop interface in apps/slint.
  It renders snapshots and dispatches typed commands only. Shared application
  orchestration belongs in crates/application, OS integration in crates/platform,
  and protocol/account policy in the existing service and domain crates.
  No GUI toolkit dependency may enter those shared crates.
- Follow Rust API Guidelines: clear ownership, typed errors, bounded input,
  documented public APIs. Run rustfmt, clippy and relevant tests.
- Prefer flat module files (auth/src/callback.rs) grouped into crate directories.
  Add a crate only for a real dependency/build boundary, not one per tiny type.
- No implicit public fallback. Never report Contacts/OwnDevices as available
  before certificate exchange is verified. Do not add public visibility to work
  around missing authentication.
- Never log secrets, raw OAuth callbacks, token responses or account cookies.
  Never store credentials in Git, Nix expressions, fixtures or compiled resources.
- Keep downloaded proprietary binaries, captures, string dumps and decompilation
  in ignored work/. Commit only original summaries, hashes and safe indicators.
- No copying code from GPL-only projects into MIT/Apache code. Record provenance
  and preserve licenses/NOTICE for any licensed source incorporated later.
- Local-only examination and reuse of the desktop OAuth configuration from the
  downloaded official Google binary is explicitly authorized for personal
  interoperability research. Keep it exclusively in a private runtime application
  configuration outside Git/Nix outputs (or ignored work/private/ for research).
  Never embed values in code, Nix expressions, fixtures or releases.
  This exception does not authorize collecting cookies or anyone else's tokens,
  impersonating account holders, bypassing consent, or publishing Google's
  configuration.
- Separate observed facts from static inference and untested hypotheses.
- Do not use the primary Google account for exploratory authorization.
- Test only authorized devices/accounts. No scanning or probing devices/accounts
  without ownership or explicit testing permission.
- Before committing, inspect staged files for credentials and binary artifacts.
