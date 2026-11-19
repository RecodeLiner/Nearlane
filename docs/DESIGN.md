# Desktop UI contract

Nearlane uses Slint Material Components for its desktop window and system tray.
The requested [material-skill](https://github.com/gdlbo/material-skill) was read
at `6ab265267c0a5512c43e2d1c27abd6bc2d5de785`; its instructions are not vendored.

- Choose a file and press Send in the row of a discovered own device. Each send
  carries the generation of the displayed list to guard against stale selections.
- Receive: show effective availability, destination, a pending decision when
  needed and the receiving outcome. Saved defaults and consent policy stay intact.
- Settings: receiving, confirmation, continuous discovery, destination, account;
  raw protocol/account status and certificates are progressively disclosed.
- Navigation: labeled rail at 720 logical pixels and above; labeled bottom
  navigation below. All three destinations and their state survive resizing.
- Material color roles provide light and dark palettes. Device and file names
  render as plain text.
- Quiet surfaces; a tonal file-selection region and receiving-status region have
  distinct ownership. No decorative outlines, gradients, shadows or nested cards.
- Native file/folder dialogs, tray, notifications, cancellation and private-mode
  gates continue through the existing backend. No redesign action widens trust.

The settings page and tray menu open the About Nearlane dialog with its Slint
attribution. Headless previews and smoke tests are described in LOCALIZATION.md.
