# Bluetooth-assisted Wi-Fi discovery - 2026-11-16

Scope: find a background Android device with Bluetooth,
then transfer files over Wi-Fi. No Bluetooth file socket or network upgrade.

## Adapter migration - 2026-11-17

The Qt adapter described below records the original implementation and hardware
observations. Current code uses btleplug in crates/platform, coordinated by the
shared application runtime. Packet size, service UUID,
Apple name fallback, queue/rate bounds and certificate gates are preserved.
The new adapter has parser tests but still needs a new real-phone/OS trial.
The earlier Qt hardware observation does not establish that the Rust adapter
has passed that trial.

## Original platform boundaries

Qt Bluetooth is confined to apps/desktop. Its native discovery adapter selects
BlueZ on Linux and CoreBluetooth on macOS. The shared Rust discovery crate
decodes bounded FEF3 advertisements and matches metadata against current own
certificates. A hint has no IP address and cannot become a file-send target.
Wi-Fi discovery must still supply an endpoint, and live mutual private proofs
and receiver acceptance still precede file content.

The adapter reads only FEF3 service data, or the public Nearby Apple fallback
encoding when the service UUID is present. It never displays a Bluetooth local
name, MAC address or unknown device. Input, deduplication, pending snapshots and
trusted observations are bounded. Search/account cancellation clears state.
Qt may log that CAP_NET_ADMIN is missing for classifying random/public addresses;
no capability is granted or needed for this non-connecting discovery path.

The optional Linux wake adapter uses BlueZ's stable LEAdvertisingManager API,
not bluetoothctl, global discoverability, pairing or a powered-state mutation.
It registers a non-connectable FE2C advertisement with the Nearby model ID and
silent initiation type. It claims no sender-certificate/UWB support, carries no
account or device identifier, and is not proof that a phone was found. It runs
only for an authorized, visible Send surface while not sending; leases expire
after ten minutes and are withdrawn on cancellation. Wi-Fi remains private.

Packet layout facts come from Google's public API, not copied proprietary code:
[FastInitBleBeacon](https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/sharing/internal/api/fast_init_ble_beacon.h),
[Connections BLE](https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/connections/implementation/ble_advertisement.h),
[medium envelope](https://github.com/google/nearby/blob/770a030be8fbebbcda5bf71ba718c35f5350c27c/connections/implementation/mediums/ble/ble_advertisement.h).
The serializers/decoder here are original Rust code. Registration success alone
does not establish that Android accepts the wake packet or advertises Wi-Fi.

## macOS

The same Qt scanner is compiled for macOS; no BlueZ dependency enters that target.
The Darwin Nix package creates Nearlane.app with a Bluetooth usage description.
Running the bare executable without bundle metadata fails closed for Bluetooth
instead of requesting an undeclared permission. Wi-Fi remains usable.

CoreBluetooth's public advertising API only accepts local-name and service-UUID
keys. Consequently the raw FE2C wake adapter is explicitly Linux-only today.
The common scanner/protocol and all Wi-Fi file handling are portable, but the
Linux beacon must not be presented as working on macOS. A real Mac build and
hardware test remain necessary; none was available in this session.

References: [Qt platform matrix](https://doc.qt.io/qt-6/qtbluetooth-index.html),
[Qt Bluetooth permission](https://doc.qt.io/qt-6/qbluetoothpermission.html),
[Apple advertising restrictions](https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager/startadvertising(_:)),
[BlueZ advertising API](https://bluez.readthedocs.io/en/latest/advertising-api/).

## Evidence

Owned synthetic certificate tests accept a valid private two-layer packet and
reject malformed, public and unmatched packets. Unauthenticated service input
cannot create a discovery result or enable visibility. An opt-in session-bus
mock verifies exact properties, silent bytes, registration and unregistration
without replacing the real BlueZ service or using a radio/account.
The actual NixOS adapter reported Powered=true, Discovering=true, one active
advertisement and advertising support. This is OS evidence, not a confirmed
screen-off Android transfer result. Subsequent testing confirmed that the
phone name is authenticated via BLE while locked, but it remains a non-routable
"waiting for Wi-Fi" hint; unlocking makes the Wi-Fi endpoint selectable. The
require_bt_advertising flag (final-byte MSB) did not resolve that gap. Bluetooth
connection bootstrap / Wi-Fi channel negotiation is not implemented yet.
