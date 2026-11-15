# Private incoming Wi-Fi LAN stage - 2026-11-15

The original adapters use the same public Google Nearby revision as outgoing:
770a030be8fbebbcda5bf71ba718c35f5350c27c. Reference facts come from
connections/implementation/wifi_lan_service_info.cc, pcp.h,
sharing/advertisement.cc and sharing/paired_key_verification_runner.cc.

Start receiving explicitly binds IPv4 TCP port 7364, then advertises the
NearbySharing DNS-SD service with version 1 / point-to-point PCP 3, a randomized
four-character endpoint and no UWB/WebRTC capability. TXT n contains private
computer endpoint info: salt and encrypted metadata key, no public device name.
Only private/link-local IPv4 interface addresses are published. A daemon
announcement must be observed before the listener is described as ready.
Dropping the publication unregisters it and shuts down its owned daemon.

The worker accepts only private/link-local IPv4 connections and is bounded to
ten minutes waiting, sixteen connection attempts and one approved file. An
unverified connection has a fifteen-second absolute socket budget. Incoming
request endpoint info allows Android form factors, versions 0/1 and bounded TLVs;
any plaintext name is ignored. Metadata must match an API-delivered active
own-device certificate. Both role-bound signatures, certificate-secret knowledge
and a successful peer result are required before reading the file introduction.
There is no UNABLE or public-peer fallback.

A destination folder is selected before reception is enabled. Only
after mutual verification are the sender's authenticated name, safe filename
and size exposed to plain-text QML. Each offer requires Accept file or Decline;
consent expires after sixty seconds. Keep-alives are serviced while waiting,
and file content sent before consent is rejected without creating a temporary
file. One regular file up to 64 MiB is supported; other attachment families and
folders are rejected. Staging uses the existing private temporary-file sink,
with exact ID/type/length/offset/EOF checks, sync and non-overwriting publication.
A receipt is sent only after persistence and only when negotiated. Closing or
a lost receipt cannot undo an already saved file.

Stop receiving, sign-out, session loss and application exit cancel the listener,
withdraw its publication as the worker finishes and interrupt established IO.
Incomplete staging is removed; completed files remain. Cancellation cannot bound
slow filesystem operations. Outgoing sends and certificate regeneration are
blocked while a receiver owns its private signing handle. Reception is off by
default and stops after one file. There is no automatic acceptance.

Tests cover verified consent, multi-chunk save/reread/digest, decline, unmatched
credentials, pre-consent file content and an existing destination. The opt-in
native mDNS check announced and resolved only a process-owned synthetic service
in 0.97 seconds, without account access. The real PC was confirmed
visible on Android after Start receiving. Real diagnostics verified matching
metadata, mutual private proofs, a valid single-file introduction, an additional
sender RESPONSE/ACCEPT and a transport BANDWIDTH_UPGRADE_RETRY query (type 12).
The initial receiver wrongly rejected those interleaved control messages while
waiting for consent. RESPONSE/ACCEPT is now advisory and never substitutes for
local consent; an identical repeated introduction cannot change the approved
metadata. They are handled before and after the UI decision. A capability query
receives a bounded response listing only Wi-Fi LAN, never a new connection or
peer-supplied network credentials. Matching public fields are documented in
connections/implementation/proto/offline_wire_formats.proto.
Subsequent testing confirmed Android-to-PC saving of the same 94-byte
test file. Both directions now have a real small-file confirmation on the authorized
phone; this does not establish large-file or Bluetooth background compatibility.
Opt-in NEARLANE_DIAGNOSTICS logs phase enums, schema-presence flags, public
protocol discriminators/byte counts and sanitized rejection categories;
no account tokens, peer frames, filenames or content are logged.

The test NixOS firewall already allows TCP 7364 and mDNS via Avahi.
No system configuration was changed. On a different host those rules may need
configuration. A running receiver already occupying the port causes a visible
failure; Nearlane does not stop another application or silently change ports.

## Background Bluetooth work

The public sharing manager uses NearbySharing, point-to-point strategy and fast
advertisement UUID 0000fef3-0000-1000-8000-00805f9b34fb. Discovery/advertising
involves a connections advertisement inside a BLE-medium envelope, with optional
GATT advertisement slots and BLE socket versioning. A UUID scan or a dummy beacon
alone is not a complete compatible background implementation. BlueZ integration,
appropriate socket/GATT lifetime, cancellation and Android screen-off tests
remain separate work. No Bluetooth capability is advertised by this stage.

## Background behavior

The desktop defaults to QStandardPaths::DownloadLocation/Nearlane, creates it
on explicit receiving enablement, and keeps accepting one locally approved file
per worker until stopped. A restart delay releases the prior certificate handle
and socket; a storage/listener/identity failure disables automatic restart.
Stop/sign-out/quit clear the restart intent. No automatic file acceptance is added.

A bounded continuous DNS-SD watch replaces snapshots instead of queuing events;
it refreshes matching endpoints, handles removal and expires stale observations.
The desktop enables it after login. CLI/service construction remains offline,
and its own local certificates are filtered from outgoing discovery.

Qt's platform tray has show/hide, receiving/discovery switches, Quit and generic
incoming/saved notifications. It uses QApplication/QtWidgets for native fallback
and a project-owned icon. Closing hides only when a tray is available. Quit stops
workers and preserves the saved account grant. No login autostart is installed.
The new code compiled, passed the 91 root tests and strict Rust clippy checks.
Native KDE checks verified default-folder retention, tray availability, window
close-to-hide/show, and notification invocation in session and KDE styles. A
continuous-watch native test found its own synthetic advertisement, then removed
it on withdrawal. These opt-in checks do not access the saved Google grant.
Sequential real Android files under automatic listener restart and Windows/macOS
tray behavior remain unverified. The default folder is an app preference and
survives login restoration/sign-out; pending offers and restart intent do not.
