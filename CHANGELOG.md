# Changelog

*[Leggi questo changelog in italiano](CHANGELOG.it.md)*

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Real Bluetooth and Wi-Fi scanning**: the Bluetooth channel discovers devices
  via BlueZ (`bluetoothctl`) and the Wi-Fi channel lists networks via
  NetworkManager (`nmcli`), each with a real spinner and honest empty/error
  states (no fake loader). Replaces the previous "not available" placeholder.
- **Typed response decoding** (`symbinux-protocol::decode`): the HW/SW version
  reply is parsed into a struct (`model`/`firmware`/`date`), validated against
  the real Nokia 3310 capture. `identify` now prints decoded fields.
- **Stable JSON output** (`--json`) on `devices` and `detect`, so the GUI and
  scripts consume structured data instead of scraped text; the GUI now uses it
  for device enumeration (removing a class of column-parsing bugs).
- **Structured logging** via the `log`/`env_logger` crates (previously declared
  but unused): `RUST_LOG=debug` gives frame traces on stderr, keeping stdout
  clean for machine parsing.
- New reference docs from a multi-project review: `docs/COMPARISON.md` (prior art
  + prioritised feature backlog) and `docs/CROSS_PLATFORM.md` (Linux/Windows/macOS
  compatibility matrix + strategy). The roadmap is updated with the backlog.
- **Serial-port resolver** (`symbinux-transport::ports`) + `ports` CLI command:
  maps a detected phone/cable to its serial port cross-platform. The GUI's
  **Identify button now runs a real identify** end-to-end and shows the decoded
  model/firmware, resolving the port automatically (or an honest "no port" note).
- **Bluetooth contacts (PBAP)**: paired phones expose a "Contacts" action that
  pulls the phonebook as vCard over BlueZ obexd. Implemented to the standard
  `org.bluez.obex` API; needs real Bluetooth hardware + obexd to validate.
- **SMS/vCard decoding** (`symbinux-protocol::decode`): GSM 7-bit unpacking,
  BCD number decoding, an SMS-DELIVER PDU parser, and vCard 3.0 export — the
  building blocks for contacts/SMS features (unit-tested).
- **CI/release workflows** (`.github/workflows/`): fmt/clippy/test on push, and
  cross-compiled `symbinux-fbus` binaries for Linux/Windows/macOS on tag.
- **Multi-frame reply reassembly**: `exchange_fbus2` reads a fragmented reply to
  the last frame, and `reassemble_fbus2` concatenates the fragments into a single
  payload (unit-tested), so long phone responses aren't truncated.
- **Retransmission window** (`ExchangeConfig` / `exchange_fbus2_with`): the
  command is resent when the phone stays silent past a per-attempt timeout
  (gnokii-style), up to N retries, before failing — tested with mock transports.
- **GUI identity card**: the Identify button now shows the decoded model /
  firmware / date as a tidy card (via `identify --json`) instead of raw text.
- **CLI `completions`** subcommand (bash/zsh/fish/…) and `identify --json`.
- **Flatpak packaging**: a `.desktop` launcher, AppStream `metainfo.xml`, a real
  square **app icon** (`assets/logo/symbinux_icon.png`), and installed shell
  completions + man page; the manifest opens the D-Bus names needed for Bluetooth
  (BlueZ/obexd) and Wi-Fi (NetworkManager). CI now validates the desktop and
  AppStream metadata.
- **Offline decode commands**: `decode-frame <hex>` and `decode-sms <hex>` decode
  captured FBUS/2 frames and SMS-DELIVER PDUs without a device — handy for
  reverse-engineering. Plus a `man` subcommand (roff man page).
- **Four more UI languages**: Polish, Russian, Simplified Chinese and Japanese
  (**11 languages** total).
- **Actionable GUI errors**: common failures (permission denied, missing serial
  port, timeout) now carry a hint pointing at the fix.
- **CLI config file** (`~/.config/symbinux/config.toml`, `%APPDATA%` on Windows):
  optional `default_port`, `ack_timeout_ms`, `retries` and `log_level`; commands
  fall back to `default_port` when `--port` is omitted.
- **Cancel button** on the progress panel: dismisses a running scan (and kills
  the `detect` subprocess). "Cancel" translated in all languages.

### Fixed
- **CPU busy-loop in `exchange_fbus2`**: a short back-off is added between empty
  reads so waiting for a reply no longer spins a core.
- **`Fbus2Reader` line-noise wedge**: a false `0x1E` marker declaring an
  implausible length no longer stalls the reader forever — it resyncs and the
  buffer stays bounded (covered by a pseudo-random fuzz test).
- **`write_phonebook` silent truncation**: it now returns an error instead of
  wrapping a name/number longer than the protocol's single-byte length field.

### Changed
- English is now the documentation standard; the GitHub repository description is
  in English, and the changelog is English-primary with an Italian variant
  (`CHANGELOG.it.md`), matching the README pattern. Italian variants are provided
  for the user-facing docs (README, CHANGELOG, FUNCTIONS, SETUP, ROADMAP).

## [0.4.0] - 2026-07-05

### Added
- **Multi-platform detection and dispatch layer** (`symbinux-devices`): a
  cascade USB fingerprinter that recognises Nokia legacy / Android
  (ADB/fastboot/MTP/PTP/AOA) / Apple iOS / unknown, with constants confirmed
  against gnokii, AOSP/AOA and libimobiledevice. A common `DeviceHandler`
  strategy interface with `NokiaLegacyHandler`, `AndroidHandler`, `AppleHandler`
  and per-platform capabilities. `DeviceManager` tracks devices by **physical
  port** (bus + port chain) so mode switches (Android AOA, iOS trust) are
  followed, not lost. 15 tests with synthetic fingerprints per category.
- CLI **`detect`** command (with `--progress` for real progress) showing the
  platform and capabilities of connected phones.
- Per-category udev rules (`51-android.rules`), a `udev/README.md` guide, and
  `docs/DEVICE_DETECTION.md` (cascade, capability matrix, integration notes:
  usbmuxd for iOS, adb_client/idevice for Android).
- The GUI now uses multi-platform detection: the list shows each phone's platform
  and capabilities, and the function buttons enable based on the selected
  device's actual capabilities. The **percentage progress bar** is driven by the
  real steps of the `detect` command.

### Changed
- **GUI UX/UI rework**: a minimum window size is enforced (720×600, default
  860×680) so content is never cramped; larger, well-proportioned logo (compact
  wordmark in the header + large logo on the empty state, both theme-aware);
  function buttons in an `Adw.WrapBox` with proper spacing that wraps on narrow
  widths; the version is shown without duplicating the name.
- **Honest wait feedback**: USB scanning moved off the UI thread (no longer
  blocks) with a **spinner** during the wait; added a progress panel with a
  **real percentage bar** (driven by an operation's actual steps, never a fake
  animation).
- The top-left now shows the name "SYMBINUX" in bold uppercase (instead of the
  small logo); the large logo stays on the empty state.
- On launch, the "Automatic" language picks the desktop's language when a
  translation is shipped for it, and otherwise falls back to English.

## [0.3.0] - 2026-07-05

### Added
- **Light/dark theme switcher** (Appearance menu): Automatic / Light / Dark. In
  "Automatic" it follows the desktop's light/dark preference via libadwaita (the
  freedesktop portal); if the desktop exposes no preference, it falls back to
  **Dark**. The logo adapts to the right variant (blue on light, orange on dark).
  The choice is persisted.
- **Internationalisation (gettext)**: the interface is translated into **7
  languages** — English (source), Italian, German, Spanish, French, Dutch,
  Portuguese — selectable from the Language menu (Automatic follows the system
  locale). A complete `po/` workflow (`symbinux.pot`, one `.po` per language,
  `LINGUAS`, `compile.sh`, translator guide) makes adding more languages easy.
- GUI preferences persisted to `~/.config/symbinux/settings.json`.

## [0.2.0] - 2026-07-05

### Added
- **Rust FBUS/MBUS core** (`crates/` workspace): a clean-room implementation of
  the Nokia serial protocols, reconstructed from gnokii/gammu and validated
  against documented real captures (no proprietary Nokia code/binaries).
  - `symbinux-protocol`: FBUS/2 and MBUS v1 codecs with dual/single checksum, an
    incremental reader, command builders with safety classification. Validated by
    the `CF 71` and `72 D5` oracle fixtures.
  - `symbinux-transport`: serial transport (termios 115200 8N1) and raw USB
    (libusb), lsusb-style device enumeration, request/response exchange.
  - `symbinux-cli` (`symbinux-fbus`): `devices`, `identify`, `getphonebook`,
    `netmon`, `raw` (guarded) commands, gnokii-style.
- **Advanced mode** for USB enumeration (VID:PID, extended names,
  classification) to debug device recognition.
- **udev rules** (`udev/69-nokia-legacy.rules`) for unprivileged access and a
  `data/devices.json` table of known VID/PIDs.
- **Documentation**: `docs/PROTOCOL_NOTES.md` (with confidence levels),
  `docs/FUNCTIONS.md`, `docs/ROADMAP.md`, `docs/SETUP.md`.
- **Test suite** with binary fixtures (real FBUS/2 captures + a synthetic MBUS
  fixture, labelled as such).

### Changed
- **Redesigned GUI**: logo and version always visible (header + About), a
  USB/Bluetooth/Wi-Fi channel selector, function buttons with icons disabled
  until a compatible connection is present, contextual empty states (no longer a
  generic message), native desktop notifications integrated with GNOME/KDE. The
  GUI now drives the Rust core (`symbinux-fbus`).

## [0.1.1] - 2026-07-05

### Changed
- **License changed from GPLv3 to AGPLv3**: also covers network/SaaS use of the
  code, not just distribution. No public release had happened under GPLv3.
- README split into English (primary, `README.md`) and Italian (`README.it.md`),
  cross-linked.
- The README logo now uses the transparent-background variants instead of the
  solid ones.

### Added
- Two transparent-background logo variants (`symbinux_logo_transparent_light.png`,
  `symbinux_logo_transparent_dark.png`), derived from the two solid variants
  provided originally.

## [0.1.0] - 2026-07-05

### Added
- Initial project scaffold: Core/GUI/packaging separation.
- `symbinux.core`: USB device detection (`pyudev`) and a stub for Bluetooth
  integration via BlueZ/D-Bus.
- `symbinux.gui`: GTK4 + libadwaita application stub.
- Initial Flatpak manifest for packaging.
- GPLv3 license (later changed to AGPLv3, see above), continuing the original
  Nokinux project (https://launchpad.net/nokinux), whose concept this project
  carries forward, generalised to modern USB/Bluetooth devices.
