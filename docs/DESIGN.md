# Desktop UI contract

Nearlane uses the stable Qt Quick Controls Material style in the existing Qt
6.12 runtime. No web layer, new UI dependency or experimental Expressive API.
The requested [material-skill](https://github.com/gdlbo/material-skill) was read
at `6ab265267c0a5512c43e2d1c27abd6bc2d5de785`; its instructions are not vendored.

- Primary task: select a file, select a nearby own device, send. Device rows are
  a list, not a grid of outlined cards. A changed discovery list clears selection.
- Receive: show effective availability, destination, a pending decision when
  needed and the receiving outcome. Saved defaults and consent policy stay intact.
- Settings: receiving, confirmation, continuous discovery, destination, account;
  raw protocol/account status and certificates are progressively disclosed.
- Navigation: labeled rail at 760 logical pixels and above; labeled bottom
  navigation below. All three destinations and their state survive resizing.
- Semantic light/dark colors, system font scaling, 8-pixel spacing rhythm,
  normal-size native Qt controls, visible keyboard focus and plain-text metadata.
- Quiet surfaces; a tonal file-selection region and receiving-status region have
  distinct ownership. No decorative outlines, gradients, shadows or nested cards.
- Native file/folder dialogs, tray, notifications, cancellation and private-mode
  gates continue through the existing backend. No redesign action widens trust.

Qt's Material controls implement the available component states; custom text,
navigation and setting compositions keep labels and keyboard semantics. This is
a desktop adaptation of Material principles, not Android Compose API parity or
a claim of full Material 3 Expressive support. Dynamic wallpaper colors are not
implemented. Visual validation and remaining limits are recorded in VALIDATION.md.
