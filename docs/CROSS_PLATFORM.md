# Cross-platform compatibility

The **Rust core is portable** and the **gtk4-rs GUI now builds and runs on
Linux and Windows** (macOS builds in CI); the remaining Linux-only pieces are
the classic-Bluetooth/PBAP and Wi-Fi backends. This is developer-facing
reference material (English only).

## Compatibility matrix

| Component | Linux | Windows | macOS | Notes |
|---|---|---|---|---|
| `symbinux-protocol` (framing) | ✅ | ✅ | ✅ | Pure byte logic, no OS calls. |
| `symbinux-transport` serial | ✅ | ✅ | ✅ | `serialport` crate is cross-platform; `--port` takes any string (`COM3` works). Enumeration uses `libudev` on Linux only. |
| `symbinux-transport` raw USB | ✅ | ⚠️ | ✅ | `nusb` (pure Rust) is cross-platform; kernel-driver detach is folded into `detach_and_claim_interface` (Linux). Windows raw-USB needs a WinUSB driver bound (Zadig) — a WinUSB limitation shared with libusb, **only** for the raw-USB/BB5 path, not the serial cable path. |
| `symbinux-devices` (detection) | ✅ | ✅ | ✅ | Pure descriptor/interface logic via `nusb` (cached strings, no device open). The `PortKey::path()` `"1-1.3"` string is a display label, not used to open devices. |
| `symbinux-fbus` CLI | ✅ | ✅ | ✅ | All subcommands portable given a valid `COM`/tty path; USB access is pure-Rust `nusb`, no native backend to install. |
| `symbinux-gui` (gtk4-rs) | ✅ | ✅ | ⚠️ | GTK4 without libadwaita; links the core directly. Windows: MSYS2 GTK4 runtime, portable dist + installer (`packaging/windows/`). macOS: builds in CI against Homebrew GTK4, not yet run-verified on real hardware. |
| Python GTK4 + **libadwaita** GUI | ✅ | ❌ | ❌ | libadwaita is GNOME/Linux-first and portal-dependent. Stays usable until the Rust GUI fully retires it (Phase 5). |
| Bluetooth scan (`symbinux-wireless`) | ✅ | ⚠️ | ⚠️ | Linux: BlueZ (`bluetoothctl`), classic + LE. Windows/macOS: **BLE only** via `btleplug` — legacy Nokia phones are Bluetooth classic and need the Phase 4 per-OS RFCOMM work. |
| Wi-Fi scan (`symbinux-wireless`) | ✅ | ❌ | ❌ | `nmcli` (NetworkManager) on Linux; honest unavailable error elsewhere (low value for legacy Nokia). |
| PBAP contacts (`symbinux-wireless`) | ✅ | ❌ | ❌ | BlueZ + obexd over D-Bus; Windows/macOS arrive with Phase 4. |
| Desktop notifications (`symbinux-wireless`) | ✅ | ✅ | ✅ | `notify-rust`: freedesktop / Windows toast / macOS notification centre. |
| Flatpak packaging | ✅ | ❌ | ❌ | Linux-only by design. |
| udev rules | ✅ | ❌ | ❌ | Linux subsystem; Windows uses driver install, macOS needs no grant for the serial path. |
| iOS (usbmuxd) | ✅ | ⚠️ | ✅ | Linux via usbmuxd daemon; Windows via Apple's own service; native on macOS. |

## Strategy

**Ships on Windows/macOS with modest effort — the CLI + core.** `symbinux-fbus`
builds for `x86_64-pc-windows-msvc` / `aarch64-apple-darwin` with close to zero
code changes. Only follow-ups:
- Windows: the USB layer uses [`nusb`](https://github.com/kevinmehall/nusb)
  (pure Rust) — no `libusb-1.0.dll` to bundle. Document the Zadig/WinUSB step
  only for raw-USB/BB5 users; the DKU-2/CA-42 serial path just needs a `COMn`
  port Windows already exposes.
- macOS: ship a signed/notarised universal binary or a Homebrew formula.
- Cosmetic: the `--port` help text example (`/dev/ttyUSB0`) could be
  OS-conditional.

**Linux-bound:** udev rules, Flatpak, the legacy libadwaita GUI, and the
classic-Bluetooth/Wi-Fi/PBAP backends inside `symbinux-wireless`.

**The cross-platform GUI exists**: `crates/symbinux-gui` (gtk4-rs, no
libadwaita) links the core directly and runs on Linux and Windows; wireless
platform details live behind `symbinux-wireless`. What remains for full parity
off Linux is **classic Bluetooth (RFCOMM) + OBEX/PBAP per OS** — the Phase 4
work in `docs/CROSS_PLATFORM_GUI_PLAN.md`.

## Packaging per platform

- **Linux** — Flatpak for the GUI (already fits); a plain binary / `.deb` /
  `.rpm` / AUR for the CLI alone.
- **Windows** — GUI: portable folder or per-user Inno Setup installer built by
  `packaging/windows/` (MSYS2 GTK4 runtime bundled, no environment setup
  needed). CLI: zipped `symbinux-fbus.exe`, fully self-contained (pure-Rust
  `nusb`, no DLL to bundle).
- **macOS** — Homebrew formula and a notarised universal binary; the GUI
  builds in CI against Homebrew GTK4, app-bundle packaging still to be done.

**Bottom line:** the remaining cross-platform ceiling is classic-Bluetooth
OBEX/PBAP, not the GUI toolkit or the core.

## Building for Windows and macOS

The CLI cross-compiles per target (`-p symbinux-cli` builds the `symbinux-fbus`
binary). CI does this automatically on a `v*` tag (see `.github/workflows/`).

```sh
# Linux
cargo build --release --target x86_64-unknown-linux-gnu -p symbinux-cli
# Windows
cargo build --release --target x86_64-pc-windows-msvc -p symbinux-cli
# macOS (Apple Silicon)
cargo build --release --target aarch64-apple-darwin -p symbinux-cli
```

The GUI builds per platform as follows:

```sh
# Linux (needs libgtk-4-dev)
cargo build --release -p symbinux-gui
# Windows: MSYS2 GTK4 + gnu target — see packaging/windows/README.md
packaging\windows\build-gui.bat
# macOS (needs Homebrew gtk4 + pkgconf; same recipe CI uses)
brew install gtk4 pkgconf && cargo build --release -p symbinux-gui
```

- **Windows serial:** a DKU-2/CA-42 cable enumerates as a COM port with inbox or
  vendor serial drivers — no extra driver needed. Only the raw-USB/BB5 path needs
  a WinUSB driver bound via [Zadig](https://zadig.akeo.ie/).
- **Self-contained USB:** the USB layer uses
  [`nusb`](https://github.com/kevinmehall/nusb) (pure Rust), so there is no
  libusb C dependency on any platform — a single self-contained binary, no
  `libusb-1.0.dll` to bundle. On Linux the only native build input is `libudev`
  (for serial-port enumeration).
