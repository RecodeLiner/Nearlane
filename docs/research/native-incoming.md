# Private incoming Wi-Fi LAN stage - 2026-11-15

## Android image chunk interoperability - 2026-11-18

An authorized phone reported Sent while no image was saved. Sanitized diagnostics
showed successful mutual private verification, one valid file introduction and
local acceptance, followed by one FILE DATA block of 91,458 bytes. The receiver's
wire decoder rejected it as Size because it reused the 64 KiB outgoing chunk
size as an incoming limit. After separating the receive limit, the same phone
sent the image again. It appeared in Downloads/Nearlane and opened successfully.

The published Google Nearby implementation uses a 64 KiB transmit buffer and a
1 MiB maximum for the entire frame following the four-byte length prefix,
including protobuf and encryption overhead. The receiver checks the prefix
before reading the body. Both sizes are configurable defaults. Runtime values
in a specific Windows release have not been measured.
[Feature flags](https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/connections/implementation/flags/nearby_connections_feature_flags.h#L124-L129)
and [receiver](https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/connections/implementation/base_endpoint_channel.cc#L107-L136).

Nearlane applies that wire limit to approved file traffic. The handshake and
control messages keep their smaller budgets. Private-session tests reproduce
the 91,458-byte block and a multi-block file with a near-limit encrypted frame,
then verify exact EOF, persisted bytes and SHA-256. A framing test exercises
the inclusive 1 MiB boundary and rejects a declared length one byte above it.

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

The destination defaults to Downloads/Nearlane and can be changed in settings. Only
after mutual verification are the sender's authenticated name, safe filename and size
exposed to plain-text QML. Verified own-device offers are approved automatically by
default. Optional per-file confirmation uses Accept or Decline and expires after sixty
seconds. Keep-alives are serviced while waiting, and file content sent before consent is
rejected without creating a temporary file. One regular file up to 64 GiB is supported;
other attachment families and folders are rejected. Staging uses the existing private
temporary-file sink, with exact ID/type/length/offset/EOF checks, sync and
non-overwriting publication. A receipt is sent only after persistence and only when
negotiated. Closing or a lost receipt cannot undo an already saved file.

Stop receiving, sign-out, session loss and application exit cancel the listener,
withdraw its publication as the worker finishes and interrupt established IO.
Incomplete staging is removed; completed files remain. Cancellation cannot bound
slow filesystem operations. Outgoing sends and certificate regeneration are
coordinated while the receiver owns its private signing handle. Each worker
handles one file; the desktop restarts reception while receiving stays enabled.
Reception and automatic acceptance of verified own-device offers are on by default.

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

This paragraph records the earlier radio-less baseline. The current portable
btleplug scanner, private BLE hint matching and optional Linux silent-initiation lease
are documented in [bluetooth.md](bluetooth.md). Bluetooth file sockets and full
BLE/GATT connection handling remain outside this Wi-Fi transfer stage.

The public sharing manager uses NearbySharing, point-to-point strategy and fast
advertisement UUID 0000fef3-0000-1000-8000-00805f9b34fb. Discovery/advertising
involves a connections advertisement inside a BLE-medium envelope, with optional
GATT advertisement slots and BLE socket versioning. A UUID scan or a dummy beacon
alone is not a complete compatible background connection. BLE discovery and
the Linux advertisement lease are implemented. Bluetooth socket/GATT connection
bootstrap and screen-off Wi-Fi negotiation remain separate work.

## Background behavior

The desktop defaults to the system Downloads directory with a Nearlane subfolder and creates it
when receiving starts. Receiving is enabled after login/private setup by default,
and verified own devices are automatically approved. Persistent settings can
disable reception or require confirmation for each file; the selected destination
is also saved. A restart delay releases the prior certificate handle and socket;
a storage/listener/identity failure disables retries until an explicit retry or
app restart. Sign-out/session loss/quit cancel workers without erasing preferences.
Unconfigured service/CLI construction stays offline and requires explicit consent.

A bounded continuous DNS-SD watch replaces snapshots instead of queuing events;
it refreshes matching endpoints, handles removal and expires stale observations.
The desktop enables it after login. CLI/service construction remains offline,
and its own local certificates are filtered from outgoing discovery.

The Qt frontend used in the original test provided tray show/hide, receiving/discovery
switches, Quit and generic saved-file notifications. It used QApplication/QtWidgets for native fallback
and a project-owned icon. Closing hides only when a tray is available. Quit stops
workers and preserves the saved account grant. No login autostart is installed.
The new code compiled, passed the 95 root tests and strict Rust clippy checks.
Native KDE checks verified default-folder retention, tray availability, window
close-to-hide/show, and notification invocation in session and KDE styles. A
continuous-watch native test found its own synthetic advertisement, then removed
it on withdrawal. These opt-in checks do not access the saved Google grant.
Automatic acceptance on real Android and Windows/macOS tray behavior remain
unverified. Pending offers never survive login restoration/sign-out.

Google's [Windows documentation](https://support.google.com/android/answer/13801258?hl=en)
states that Your devices visibility is for the same Google account, same-account
transfers are accepted automatically, and the app receives while running in the
background. Closing its window keeps it running; Quit exits. This establishes the
behavior being implemented, not a claim about the original installation's default
visibility or complete radio compatibility. Nearlane still has no BLE transport
or OS-login autostart installation.

## Native notification actions

When confirmation is enabled, Linux uses the [freedesktop notification
protocol](https://specifications.freedesktop.org/notification/latest/protocol.html). It
checks the server's actions capability, subscribes before Notify, and uses random scoped
Accept/Decline keys plus the returned notification ID. The desktop routes a decision
through the service's monotonic current-offer generation. It closes its own notification
on completion/cancel/expiry and falls back to the window on an unavailable server. Only
verified sender/file metadata is displayed, with escaped markup and bounded input.
Nothing includes account tokens or paths. An owned mock session-bus service tested
immediate actions, wrong/stale IDs and cancellation without replacing the real
notification server. The buttons were confirmed working with Android. A transient
Sending delay did not recur; no speculative transport change was made for it.
