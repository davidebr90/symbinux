# Cross-platform compatibility

Symbinux targets Linux today, but the split matters: the **Rust core is already
essentially portable**, while the **GUI layer is the Linux wall**. This is
developer-facing reference material (English only).

## Compatibility matrix

| Component | Linux | Windows | macOS | Notes |
|---|---|---|---|---|
| `symbinux-protocol` (framing) | ✅ | ✅ | ✅ | Pure byte logic, no OS calls. |
| `symbinux-transport` serial | ✅ | ✅ | ✅ | `serialport` crate is cross-platform; `--port` takes any string (`COM3` works). Enumeration uses `libudev` on Linux only. |
| `symbinux-transport` raw USB | ✅ | ⚠️ | ✅ | `rusb`/libusb is cross-platform; one `#[cfg(target_os="linux")]` guard (`set_auto_detach_kernel_driver`) is correctly scoped. Windows raw-USB needs a WinUSB driver bound (Zadig) — **only** for the raw-USB/BB5 path, not the serial cable path. |
| `symbinux-devices` (detection) | ✅ | ✅ | ✅ | Pure descriptor/interface logic via `rusb`. The `PortKey::path()` `"1-1.3"` string is a display label, not used to open devices. |
| `symbinux-fbus` CLI | ✅ | ✅ | ✅ | All subcommands portable given a libusb backend and a valid `COM`/tty path. |
| Python GTK4 + **libadwaita** GUI | ✅ | ❌ | ❌ | libadwaita is GNOME/Linux-first and portal-dependent. `pyproject.toml` already gates `PyGObject` to `sys_platform == 'linux'`. |
| GUI Bluetooth scan | ✅ | ❌ | ❌ | Shells `bluetoothctl` (BlueZ, Linux-only). |
| GUI Wi-Fi scan | ✅ | ❌ | ❌ | Shells `nmcli` (NetworkManager, Linux-only). |
| Desktop notifications | ✅ | ⚠️ | ⚠️ | `Gio.Notification` → freedesktop spec; weak/absent off Linux. |
| Flatpak packaging | ✅ | ❌ | ❌ | Linux-only by design. |
| udev rules | ✅ | ❌ | ❌ | Linux subsystem; Windows uses driver install, macOS needs no grant for the serial path. |
| iOS (usbmuxd) | ✅ | ⚠️ | ✅ | Linux via usbmuxd daemon; Windows via Apple's own service; native on macOS. |

## Strategy

**Ships on Windows/macOS with modest effort — the CLI + core.** `symbinux-fbus`
builds for `x86_64-pc-windows-msvc` / `aarch64-apple-darwin` with close to zero
code changes. Only follow-ups:
- Windows: bundle `libusb-1.0.dll`, **or migrate the USB layer to
  [`nusb`](https://github.com/kevinmehall/nusb)** (pure Rust, no libusb C
  dependency) to drop the native dependency entirely. Document the Zadig/WinUSB
  step only for raw-USB/BB5 users; the DKU-2/CA-42 serial path just needs a
  `COMn` port Windows already exposes.
- macOS: ship a signed/notarised universal binary or a Homebrew formula.
- Cosmetic: the `--port` help text example (`/dev/ttyUSB0`) could be
  OS-conditional.

**Linux-bound:** udev rules, Flatpak, and the current libadwaita GUI (plus the
`bluetoothctl`/`nmcli` shell-outs). `pyproject.toml` already treats the GUI as
Linux-only.

**A genuinely cross-platform GUI** would need: dropping libadwaita (GTK4 alone
is portable) or a Rust-native toolkit (Slint/iced/egui) that could link the core
directly instead of shelling to the CLI; per-OS Bluetooth/Wi-Fi backends (WinRT
/ CoreBluetooth+CoreWLAN); and native notifications. That is a separate, larger
effort from shipping the CLI cross-platform.

## Packaging per platform

- **Linux** — Flatpak for the GUI (already fits); a plain binary / `.deb` /
  `.rpm` / AUR for the CLI alone.
- **Windows** — zipped `symbinux-fbus.exe` (+ optional `libusb-1.0.dll`, or
  `nusb` build); optional winget/Scoop manifest. GUI is CLI-only until the
  toolkit question is resolved.
- **macOS** — Homebrew formula and a notarised universal binary; GUI CLI-only
  for now.

**Bottom line:** the cross-platform ceiling is entirely in the GUI layer, not
the core.
