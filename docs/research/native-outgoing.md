# Native outgoing connection - 2026-11-15

## Reference and implemented flow

The original Rust adapters follow Google Nearby revision
770a030be8fbebbcda5bf71ba718c35f5350c27c:

- connections/implementation/base_pcp_handler.cc: plain connection request,
  UKEY2, both plain responses, then encrypted transport. Skipping the responses
  and encrypting immediately after UKEY2 is not the reference flow.
- connections/implementation/offline_frames.cc and proto/offline_wire_formats.proto:
  request, acceptance, keep-alive, upgrade-failure, payload-ACK and close fields.
- internal/platform/feature_flags.h: the public default payload-ACK feature
  minimum is safe-disconnect version 6. This is an optional receipt feature,
  not a minimum version for accepting an encrypted connection. Android version-4
  acceptance is supported; no payload ACKs are sent to a peer without support.
  Missing file receipts remain unconfirmed and are never inferred from closing.
- sharing/paired_key_verification_runner.cc: both peers send proofs before
  reading. Role-bound signatures, knowledge of the other certificate's secret
  and successful peer results are required.
- sharing/outgoing_share_session.cc: introduction sets start_transfer and
  NEARBY_SHARE use_case before acceptance and file content.

Primary source root:
https://github.com/google/nearby/tree/770a030be8fbebbcda5bf71ba718c35f5350c27c

## Boundaries and validation

The native file picker selects a local path. Explicit Send file on one discovered
own device issues an immutable private signing handle plus fresh advertisement
material to a worker. No Google token reaches that worker or QML. The candidate
must be recent; live proofs must match the Google-delivered certificate. No
public/unknown-peer fallback exists. Both proofs and peer SUCCESS precede file
metadata, and receiver acceptance precedes file content.

The adapter announces the computer/laptop form factor (ShareTargetType 3), with no
plaintext public device name. Its certificate's encrypted device name is currently
Nearlane. It reassembles bounded BYTES fragments, services keep-alives during upload,
validates IDs/ACKs and declines bandwidth redirection. Only the selected socket is used.
Absolute deadlines also bound partial reads. Cancellation shuts down established IO.
Initial scope: one regular file up to 64 MiB, IPv4 Wi-Fi LAN, no folder/resume support.
Incoming reception is implemented separately; see native-incoming.md. Unix symlinks are
refused.

Chunks include the sequential index in public field 4. EOF is a separate empty
LAST_CHUNK, with the next index and no body field, matching the published sender.
The delivery signal is a matching file ACK after EOF. It is not a remote SHA-256
comparison or a promise of application-level durable storage on Android. Missing
confirmation is reported distinctly even if the file may have arrived. A legacy
peer gets a five-second post-EOF grace period; keep-alives cannot extend it.
Safe close is bounded to two seconds after either confirmed or unconfirmed EOF
and cannot undo an already received file ACK or create delivery proof.

Full-stack tests use independent generated synthetic private credentials for two
owned peers, with original adapters at both ends. They cover the prelude, private
advertisement decryption, concurrent paired proofs, keep-alives, saving/rereading
empty, multi-chunk and 3 MiB files, rejection, wrong signer, cancellation, lost ACK,
and a slow partial read. This is not an independent Android implementation test;
they alone do not establish Android compatibility. The explicit CLI
`send-own --device EXACT_NAME FILE` uses the same Service/SendJob path and only
an existing saved login. Case-insensitive exact names are accepted; absent or
ambiguous matches fail without selecting another recipient. Its exit status uses
the typed worker completion flag. Close the desktop before using the CLI to avoid
two instances rotating the same registration's private keys concurrently.

An authorized LAN test used a real Android phone and a 94-byte harmless text
file. Saved grant restoration, Identity exchange and certificate-filtered exact
discovery succeeded. Diagnostics identified the first prelude failure: a valid
Android acceptance advertises safe-disconnect version 4, while the initial implementation
wrongly required the optional version-6 receipt feature. After removing that
restriction, mutual role-bound paired-key proofs and successful peer results,
receiver acceptance, and 94-byte upload completed. The phone showed an indefinite
receiving screen followed by Can't transfer files; that attempt did not establish
saved delivery. The initial upload omitted chunk indices and encoded an empty body in
EOF; these differences from the reference sender have now been corrected.
The next real test with chunk indices, omitted empty EOF body and computer form
factor again passed private verification and receiver acceptance.
The nearlane-test.txt file was confirmed saved and opened on Android. This establishes
one successful 94-byte outgoing private file transfer on that phone; it does not
establish screen-off delivery or large-file Android compatibility. Incoming
reception is separately verified in [native-incoming.md](native-incoming.md).
The phone closed without a file ACK, consistent with its advertised feature
version 4. Safe connection checkpoints are reported without recording handshake
messages, account tokens or traffic. The UI/CLI continues to describe the missing
receipt honestly; the observed on-device result is recorded here, not fabricated on the wire.

The original Android client supports background reception in Your devices mode,
including a screen-off receiver. Nearlane currently only browses Wi-Fi LAN;
Bluetooth/BLE discovery and wake-up have not been implemented. Opening the
dedicated Receive screen can temporarily expose a phone to everyone; it is not
a requirement for private-mode compatibility and should not be used as a public
fallback. Official documentation:
https://support.google.com/android/answer/15728591
https://support.google.com/android/answer/13801258

## Certificate removal

Nearlane creates three credentials for consecutive 72-hour private validity
periods. All three can be labelled This computer because the current session owns
their keys; that does not mean three duplicate computers.

The examined Identity method set includes PublishDevice and credential query,
but no observed DeleteDevice/DeleteCredential operation. Query records do not
provide an owning resource name usable for deletion. Sign-out requests empty
SELF publication for the known current resource; remote removal semantics remain
unverified. Old ephemeral registrations with lost resource names cannot currently
be targeted safely by display name. They expire with their credentials. No
blanket OAuth revoke, DeleteBinding misuse or guessed delete endpoint is added.
