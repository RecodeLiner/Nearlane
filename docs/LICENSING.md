# Licensing and distribution notes - 2026-11-14

This is a practical provenance review, not a legal opinion or a promise against
DMCA notices. Applicable law depends on jurisdiction and how research and
distribution are performed. The applicable jurisdiction has not been assessed.

## Chosen license

Original Nearlane code is dual-licensed MIT OR Apache-2.0, a common Rust choice.
Apache-2.0 includes an express patent grant from contributors; neither license
grants rights in Google's proprietary client, trademarks, server access, or
third-party patents. No license can guarantee that Google will not submit a
DMCA notice or that GitHub will never restrict a repository.

This is a new repository, not a relicensed fork. No implementation code from
Scotty, rquickshare or Packet has been copied. The auth implementation follows
public OAuth/PKCE specifications; the CLI and frame limits are original.
Qt startup/bridge API usage was informed by CXX-Qt's MIT/Apache documentation;
see NOTICE for acknowledgment.

## Components considered

| Component | License / treatment |
| --- | --- |
| CXX-Qt 0.10 | MIT OR Apache-2.0; preserve its licenses and notices in distributions |
| Qt Core, Gui, QML, Quick, Quick Controls | Open-source LGPL/GPL alternatives vary by module; use the LGPLv3 option for the selected modules, dynamic linking and redistribution compliance |
| Google Better Together Core | Apache-2.0; selected UKEY2/crypto Rust components incorporated in third_party/beto, pinned to 479289ef072b0880c0347d36937265e44f00f4ee. Original sources/notices are unchanged; packaging manifests are adapted and documented |
| Google Nearby schemas | Apache-2.0; original Rust adapters follow published field numbers from revision 770a030be8fbebbcda5bf71ba718c35f5350c27c; provenance is recorded in NOTICE and source comments |
| rquickshare / Packet | GPL-3.0 family; not incorporated. Copying implementation requires revisiting the licensing of the combined work |
| Google Quick Share EXE/MSI/resources | Proprietary; downloaded only into ignored work/ for local examination; do not redistribute or embed |
| Local desktop OAuth configuration | Local-only import into ignored work/private/ is explicitly authorized; never compiled or distributed. Application-identity/API authorization is separate from the license of Nearlane's original code |

MIT/Apache licensing the application does not remove LGPL obligations for Qt.
Before publishing binaries, inventory the exact Qt modules/plugins and transitive
dependencies, include license texts and attributions, provide required source
access, and permit replacement/relinking of LGPL components as applicable. Do not
assume all Qt add-on modules are LGPL. Avoid static Qt linking for this prototype.

## Local OAuth compatibility experiment

The local interoperability experiment is explicitly authorized to read desktop
application parameters from the examined official binary. No account cookies, account tokens
or master tokens are extracted. The importer is original code; the extracted
values remain outside Git, Nix source snapshots and compiled resources.

This limits publication of Google's configuration; it does **not** establish
permission to use Google's registered application identity. Google's OAuth policy
requires an accurately represented app identity and allows suspension/revocation
for misrepresentation. The browser consent page may identify the official Quick
Share client even though Nearlane initiated the request; the experimental UI
discloses that distinction. A redirect to Google's sign-in page does not prove
provider approval, grant issuance or certificate API access. No claim is made
that local-only storage resolves terms-of-service, trademark or legal questions.

Primary policy: https://developers.google.com/identity/protocols/oauth2/policies

## Reducing avoidable disputes

- Independent project name and artwork. Quick Share is mentioned descriptively
  for compatibility; no Google/Samsung logo or claim of endorsement.
- Keep provenance notes for every incorporated source and generated schema.
- Publish original implementation and concise interoperability findings, not
  proprietary binaries, decompiled source or extracted Google client secrets.
- Do not treat knowledge of an endpoint as permission to access it. Provider
  terms and OAuth authorization are distinct from copyright licensing.
- US 17 USC 1201(f) contains a conditional interoperability exception; it is not
  blanket permission, and applicability here has not been legally determined.
- Single-author binary analysis plus implementation is not a clean-room process.
  Do not describe this project as clean-room without an actually separated process.

## Primary references

- https://github.com/google/nearby/blob/main/LICENSE
- https://github.com/KDAB/cxx-qt (MIT/Apache license statement)
- https://doc.qt.io/qt-6/qtquickcontrols-index.html#license-and-attributions
- https://www.qt.io/development/open-source-lgpl-obligations
- https://docs.github.com/en/site-policy/content-removal-policies/dmca-takedown-policy
- https://www.law.cornell.edu/uscode/text/17/1201 (especially subsection f)
- https://policies.google.com/terms

The name Nearlane is a working name; no exhaustive trademark clearance was done.
