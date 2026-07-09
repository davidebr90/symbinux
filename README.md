# Symbinux

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo/symbinux_logo_transparent_dark.png">
  <img alt="Symbinux logo" src="assets/logo/symbinux_logo_transparent_light.png" width="320">
</picture>

*[Leggi questo documento in italiano](README.it.md)*

Talk to legacy Nokia phones from a modern Linux, Windows or macOS desktop.
Symbinux is a clean-room implementation of the Nokia **FBUS/MBUS** serial
protocols over USB (with a serial-cable path today and raw-USB/BB5 on the
roadmap), packaged as a Rust core + CLI with a cross-platform **GTK4
(`gtk4-rs`) GUI**. A legacy libadwaita GUI in Python stays usable until the
Rust GUI fully retires it.

**Symbinux is a declared fork of [Nokinux](https://launchpad.net/nokinux)**
(2008-2010), a Bash/Python project born in the Italian Ubuntu community to
configure Nokia phones from Linux, of which Davide Pica (davidebr90) was one of
the original authors alongside other contributors. Symbinux carries its name and
spirit forward, rewritten from scratch to today's standards.

The protocol is reconstructed from the open-source **gnokii** and **gammu**
projects and validated against documented real captures. It uses **no
proprietary Nokia code, libraries or binaries**.

## What it does

- **Identify** a phone (model, IMEI, hardware and firmware version).
- **Phonebook** read (and experimental write) across ME/SIM memory.
- **Netmonitor** diagnostics.
- **SMS** read/send (experimental).
- **Auto-detect** the connected phone and its platform (Nokia legacy / Android /
  Apple iOS), each exposing its own capability set so the UI adapts.
- **Wireless discovery** — Bluetooth and Wi-Fi scanning, plus PBAP contact pulls
  over Bluetooth on Linux, behind one portable `symbinux-wireless` API (BLE on
  Windows/macOS via `btleplug`, honest unavailable states elsewhere).
- **Device classification** — scanned Bluetooth devices are tagged by vendor and
  form factor (Apple watch, Android phone, TV, headphones…) from vanilla
  identification signals, shown as combined badges.
- **Data recovery / export** — recovered phonebook, messages and calendar
  normalise to portable **vCard / vMessage / iCalendar** records regardless of
  transport.
- **Advanced device inventory** — an lsusb-style view of everything connected
  (VID:PID, extended names, classification) to debug detection issues.
- **Raw frame mode** for protocol reverse-engineering.

See [docs/FUNCTIONS.md](docs/FUNCTIONS.md) for the full CLI reference and safety
classes, and [docs/NOKIA_SERVICE_MODES.md](docs/NOKIA_SERVICE_MODES.md) /
[docs/VANILLA_CONNECTIVITY.md](docs/VANILLA_CONNECTIVITY.md) for how the phone is
reached with no software installed on it.

## Architecture

```
symbinux/
├── crates/                     # Rust workspace (the core)
│   ├── symbinux-protocol/      # FBUS/MBUS framing + typed decoders/export — pure, no I/O, fully tested
│   ├── symbinux-transport/     # serial (termios) + raw USB (nusb, pure Rust), enumeration
│   ├── symbinux-devices/       # USB fingerprinting + per-platform dispatch
│   ├── symbinux-wireless/      # portable Bluetooth/Wi-Fi/PBAP/notifications
│   ├── symbinux-cli/           # `symbinux-fbus` gnokii-style command-line tool
│   └── symbinux-gui/           # gtk4-rs desktop GUI — links the core directly
├── src/symbinux/               # legacy GTK4 + libadwaita GUI (Python), shells out to the CLI
├── udev/                       # unprivileged-access rules
├── data/devices.json           # known VID/PID table (community-maintained)
├── docs/                       # PROTOCOL_NOTES / FUNCTIONS / ROADMAP / SETUP / …
└── packaging/                  # flatpak/ (Linux) + windows/ (installer)
```

Layers are strictly separated: framing (no I/O) → transport (I/O) → CLI / GUI.
The Rust GUI (`symbinux-gui`) **links the core crates directly** — no subprocess
bridge; the legacy Python GUI holds no protocol logic and shells out to
`symbinux-fbus`.

## Quick start

```bash
# Core + CLI
cargo build --release

# What is connected right now (no phone required):
target/release/symbinux-fbus devices --all

# Identify a phone over a DKU-2/CA-42 cable (or --usb to claim it directly):
target/release/symbinux-fbus identify --port /dev/nokia_fbus

# Shell completions (bash/zsh/fish/…):
target/release/symbinux-fbus completions bash > ~/.local/share/bash-completion/completions/symbinux-fbus

# GUI (Rust · GTK4, Linux/Windows/macOS)
cargo build --release -p symbinux-gui
target/release/symbinux-gui

# Legacy Python GUI (Linux, until the Rust GUI fully replaces it)
pip install -e ".[gui]"
symbinux
```

On Windows the GUI ships as a double-clickable per-user installer (GTK runtime
bundled) — see [packaging/windows/README.md](packaging/windows/README.md).

Unprivileged access (no `sudo` in normal use) is a one-time udev install — see
[docs/SETUP.md](docs/SETUP.md). How the app owns the connection (claiming USB
directly, forcing a Bluetooth pair) is described in
[docs/CONNECTION_MODEL.md](docs/CONNECTION_MODEL.md).

## Requirements

- Rust ≥ 1.89. On Linux, `libudev` + `pkg-config` (for serial-port enumeration).
  No libusb — raw USB access is pure-Rust via [`nusb`](https://docs.rs/nusb).
- For the Rust GUI: the GTK4 development libraries (`libgtk-4-dev` on
  Debian/Ubuntu, MSYS2 `mingw-w64-x86_64-gtk4` on Windows, `brew install gtk4`
  on macOS).
- For the legacy Python GUI: Python ≥ 3.11, GTK4 and libadwaita
  (`gir1.2-gtk-4.0`, `gir1.2-adw-1` on Debian/Ubuntu).
- A real Linux machine, or WSL2 with USB passthrough for hardware tests. The
  protocol codec is fully testable with no hardware (`cargo test`).

## Tests

```bash
cargo test        # protocol codec against real-capture fixtures + transport
pytest            # Python GUI/backend
```

## Safety

Firmware/flash writes are **not implemented** and are refused. Only read commands
run by default; anything that modifies the phone is opt-in, and raw-frame mode is
gated behind an explicit flag. Details in
[docs/PROTOCOL_NOTES.md](docs/PROTOCOL_NOTES.md).

## License

**GNU AGPLv3** (or later). See [LICENSE](LICENSE). This is compatible with the
GPL/LGPL of the gnokii/gammu documentation the protocol notes draw on.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).
