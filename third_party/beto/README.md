# Better Together Core: selected public Rust components

Source: https://beto-core.googlesource.com/beto-core/
Revision: `479289ef072b0880c0347d36937265e44f00f4ee`
License: Apache-2.0; original copyright notices are preserved in every source.

Included: crypto_provider, crypto_provider_rustcrypto, crypto_provider_default,
ukey2_rs, ukey2_connections, ukey2_proto. Original Rust sources and proto files
are copied without changes. Only Cargo packaging manifests are adapted:

- Select the public RustCrypto path; no BoringSSL/private workspace paths.
- Use the upstream checked-in protobuf 3.7.2 bindings without protoc/build scripts.
- Keep upstream UKEY2 unit tests; omit unrelated provider/benchmark test dependencies.
- Keep this dependency workspace outside the Nearlane workspace's formatting/lints.

Run the upstream UKEY2 tests, using the same CARGO_HOME/target cache as Nearlane:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --locked --manifest-path third_party/beto/Cargo.toml -p ukey2_rs -p ukey2_connections --lib
```

The crypto provider's own full vector/benchmark suites are not configured in this
minimal snapshot. The `boringssl` selector is unsupported here. Do not enable it.
Future upstream updates require source/provenance and interoperability review.

This is open-source Google code, separate from the proprietary Windows artifacts
and local OAuth application configuration. Neither of those is included here.
