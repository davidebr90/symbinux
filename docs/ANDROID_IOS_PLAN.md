# Android & iOS transfer plan (`adb_client` / `idevice`) — item D

> Status: **plan / design document — not started.** Execution is (at least)
> **two** independent sub-sessions (Android, then iOS) per §8. Crate versions and
> APIs below are verified against current docs (`adb_client` 3.2.2, `idevice`
> 0.1.64, `mtp-rs` for MTP); each Step 0 spike stands the chosen crate up against
> real hardware before the full build.

## 1. Goal & current gap

Symbinux already **detects and classifies** Android and Apple devices and
advertises per-mode capabilities, but performs **no real transfer**. Today:

- `crates/symbinux-devices/src/fingerprint.rs` classifies Android modes
  (`AndroidMode::{Adb, Fastboot, Mtp, Ptp, Aoa}`) and Apple iOS.
- `crates/symbinux-devices/src/handlers.rs` — `AndroidHandler` / `AppleHandler`
  advertise `Capability` values (identify, file-transfer, app-install, backup,
  screenshot) but the handlers don't *do* the transfer.
- `crates/symbinux-devices/src/manager.rs` — `DeviceManager` tracks devices by
  physical `PortKey` across re-enumeration, which is exactly what an Android AOA
  accessory switch and an iOS trust-dialog re-probe trigger (documented in
  `docs/DEVICE_DETECTION.md`).

This plan fills that gap by embedding two pure-Rust backends:
**`adb_client`** (Android) and **`idevice`** (iOS).

## 2. `adb_client` at a glance (verified: v3.2.2)

- **Version:** `adb_client` 3.2.2 (2026-05-31), MIT, pure-Rust ADB protocol.
- **Two transports** behind a common `ADBDeviceExt` trait:
  - `ADBServerDevice` — talks to a locally running `adb` server on TCP `5037`
    (like the stock CLI); no USB code.
  - `ADBUSBDevice` (feature `usb`, **off by default**) — speaks ADB **directly
    over USB**; discovery via `find_all_connected_adb_devices()`; constructors
    `ADBUSBDevice::new(vid, pid)`, `::autodetect()`,
    `::new_with_custom_private_key(vid, pid, key_path)`,
    `::new_from_transport(USBTransport, key_path)`.
- **⚠️ CONFIRMED: `ADBUSBDevice`'s USB backend is `rusb` 0.9.4 with the
  `vendored` feature — it builds libusb from C source.** There is no nusb-based
  or native-OS transport. Enabling `adb_client/usb` therefore **re-introduces
  exactly the libusb C dependency the nusb migration removed** (vendoring only
  avoids a *pre-installed* system libusb; it still needs a C toolchain and links
  libusb).
- **API** (`ADBDeviceExt`): `shell_command(cmd, stdout, stderr)`, interactive
  `shell(reader, writer)`, `push(reader, path)`, `pull(src, writer)`,
  `install(apk, user)`, `uninstall(pkg, user)`, `framebuffer*/framebuffer_bytes()`
  (screenshot; feature `framebuffer`, default-on).
- **Auth:** standard ADB RSA (SHA1withRSA); the device shows *"Allow USB
  debugging?"* and pins the host public key. Stock key at `~/.android/adbkey`;
  the crate accepts a **custom private-key path**, so Symbinux can carry its own
  app identity instead of reusing the user's adbkey.
- **Cross-platform:** protocol logic is portable; the only constraint is the
  `usb` feature's `rusb`/libusb build requirement.

### 2.1 The libusb decision (settle this before any code)

`adb_client` offers no pure-Rust USB path today. Options, best-first:

1. **Feature-flag isolation (recommended)** — put the `adb_client/usb` path
   behind a Symbinux cargo feature (e.g. `android-usb-adb`, off by default). The
   default build stays pure-Rust (iOS via `idevice`, MTP via `mtp-rs`, Nokia via
   `nusb`); only users who opt into Android-over-USB pull in libusb, with a
   documented C-toolchain build cost.
2. **Server/TCP transport** — use `ADBServerDevice` (or wireless ADB) so nothing
   from `adb_client` pulls in `rusb`. Cost: the user needs the `adb` binary +
   server (or wireless debugging) — a heavier "just plug in" UX.
3. **Custom nusb ADB transport** — the ADB-over-USB wire protocol (bulk endpoints
   + simple framing) is small, but `new_from_transport` takes the concrete
   `USBTransport` struct (rusb-tied), **not a trait**, so this means
   forking/patching `adb_client`, not writing an adapter. Verify against source
   before counting on it.
4. **Upstream** — request/contribute an nusb backend upstream; not available now.

Trading away the "no libusb" property must be a **conscious, documented** choice
(CHANGELOG), never a silent regression — same discipline as the nusb migration.

## 3. `idevice` at a glance (verified: v0.1.64)

- **Version:** `idevice` 0.1.64 (jkcoxson), pure-Rust libimobiledevice
  alternative, **async (tokio)**. **Pre-1.0:** the README warns *breaking changes
  at every point release until 0.2.0* — pin exactly and expect churn.
- **Connection providers:** `UsbmuxdProvider` (talks to the usbmuxd socket to
  discover devices and open a muxed connection) and `TcpProvider` (Wi-Fi/network).
  No C dependency of its own — the only external piece is the usbmuxd service.
- **Pairing / trust:** a `PairingFile` (from `provider.get_pairing_file()`) holds
  the keys; `LockdowndClient::start_session(pairing_file)` negotiates the TLS
  lockdown session. The *"Trust This Computer?"* dialog is triggered by the
  pairing handshake (idevice implements the protocol side, not a new dialog).
  A newer `RemotePairingClient` path exists for RSD/tunnel pairing on modern iOS.
- **Service clients present today** (feature-gated): `LockdowndClient` (core:
  device info/session), `AfcClient` (`afc`: filesystem → file-transfer),
  `InstallationProxyClient` (`installation_proxy`: app install/list),
  `MobileBackup2Client` (`mobilebackup2`: backup), `ScreenshotClient` — plus more
  (CoreDevice/RSD tunnel, HID input, XCUITest, syslog, springboard). All go
  through an `IdeviceService` trait over a provider.
- **Cross-platform:** pure Rust; the platform-variable piece is usbmuxd, a
  *runtime* service (not a build dependency):
  - **Linux** — install the `usbmuxd` package (owns `/var/run/usbmuxd`, pairing
    records under `/var/lib/lockdown`).
  - **macOS** — usbmuxd is built into the OS.
  - **Windows** — the **Apple Mobile Device Service** (iTunes / "Apple Devices").
- **No libusb conflict:** iOS integration is orthogonal to the nusb-vs-libusb
  question — it never touches raw USB, only the usbmuxd socket. This is a
  materially easier story than the Android USB path (§2.1).

## 4. MTP / PTP / AOA fallback (scope note)

- **MTP** (Android with USB debugging off) — there **is** now a pure-Rust,
  nusb-based crate: [`mtp-rs`](https://crates.io/crates/mtp-rs) ("no libmtp, no
  libusb, no FFI, async USB", built on `nusb` — it matches our stack). So MTP is
  no longer "out of scope for pure Rust". Caveats: **very young** (first release
  2026-02, unproven), and MTP only works when the phone is unlocked and set to
  "File Transfer/MTP" mode — it is **not** an ADB substitute (no
  shell/install/screenshot), only file-manager transfer. Treat as an
  **experimental phase-2** path, not a v1 dependency — but it fits the no-libusb
  principle and could be the *default* Android file-transfer path that avoids the
  §2.1 libusb decision entirely for users who just want files.
- **PTP** (camera/photos) — narrow; defer.
- **AOA** — vendor control `51/52/53` on endpoint 0; detection already exists,
  driving it is niche; defer.

Ship ADB + iOS-over-usbmuxd first; MTP via `mtp-rs` is an optional nusb-aligned
follow-up; PTP/AOA later.

## 5. Mapping to the existing architecture

- Introduce a **`TransferBackend` trait** (in `symbinux-devices` or a new
  `symbinux-mobile` crate) that `AndroidHandler`/`AppleHandler` hold, turning the
  currently-advertised `Capability` values into real calls:

  | Capability | Android (`adb_client`) | iOS (`idevice`) |
  |---|---|---|
  | identify | `shell_command` getprop | `LockdowndClient` |
  | file-transfer | `push`/`pull` (or `mtp-rs`) | `AfcClient` |
  | app-install | `install`/`uninstall` | `InstallationProxyClient` |
  | backup | per-item `pull` (or `adb backup`) | `MobileBackup2Client` |
  | screenshot | `framebuffer_bytes` | `ScreenshotClient` |

- Capabilities stay **advertised by mode**: an Android device in `Fastboot` or
  `Mtp` mode still exposes only what that mode supports — the handler wires the
  backend only for modes it can actually drive (ADB now; MTP later).
- **Re-enumeration is already handled:** `DeviceManager` keys on `PortKey`
  (bus + port chain), so the Android auth prompt / iOS trust dialog (which make
  the device drop and reappear with a new address, sometimes new vid/pid) surface
  as a `Transition::Switched` on the same port — the handler re-probes with the
  new mode rather than treating it as a new device. This existing design is a
  direct enabler for the auth flows.

## 6. Per-OS reality

- **Android (ADB):** works on Linux/Windows/macOS (subject to the USB-backend
  decision in §2). Linux needs the udev access already granted by
  `51-android.rules`.
- **iOS (usbmuxd):** Linux needs the `usbmuxd` package running; macOS native;
  Windows via Apple Mobile Device Service. If usbmuxd is absent, the iOS path
  must fail with an **honest, actionable** message ("install usbmuxd / Apple
  Devices"), consistent with the project's no-fake-status principle.

## 7. Edge cases & risks

1. **On-device prompts** — Android "Allow USB debugging?" and iOS "Trust This
   Computer?" both require *physical* user action; the UX must wait and retry,
   not error out. Time-box with a clear "confirm on the phone" state.
2. **RSA key storage (Android)** — where to store `adbkey` (respect the standard
   `~/.android/adbkey` so existing authorisations carry over).
3. **Pairing record (iOS)** — persist and reuse; handle "trust revoked".
4. **usbmuxd missing / not running** — detect and message clearly (§6).
5. **libusb re-introduction (§2.1)** — confirmed: `adb_client`'s only USB
   transport is `rusb`/libusb. The single biggest architectural decision; settle
   the transport (feature-flag / server / `mtp-rs`) before writing code.
6. **Mode switches** — rely on `DeviceManager` port tracking; verify a real AOA
   switch and iOS trust re-probe map to `Switched` (needs hardware).
7. **Security / privacy** — these backends can read user data (contacts, photos,
   backups). Reads should be explicit; anything destructive (uninstall, restore)
   gated behind confirmation, consistent with the CLI's `--i-understand-risk`
   pattern for risky ops.
8. **Testability without hardware** — protocol/mapping logic is unit-testable;
   the actual transfer needs a real Android phone and a real iPhone (+ usbmuxd).
   CI cannot cover on-device paths — mark them "pending hardware" honestly.

## 8. Suggested phased plan (two independent sub-sessions)

**Phase A — Android (`adb_client`)**

| # | Step | Testable? |
|---|---|---|
| 0 | **Spike:** take the **libusb decision (§2.1)** concretely (feature-flagged `adb_client/usb` vs server transport vs `mtp-rs` for files), then validate auth + `shell_command`/`push`/`pull` on a real device | build + on-device |
| 1 | `TransferBackend` trait + `AndroidHandler` wiring for `identify` (getprop) | unit + on-device |
| 2 | file-transfer (`push`/`pull`), then app-install, then screenshot | on-device |
| 3 | CLI subcommands + GUI actions gated by capability; honest auth-wait UX | pytest (mock) |

**Phase B — iOS (`idevice`)** *(separate session)*

| # | Step | Testable? |
|---|---|---|
| 0 | **Spike:** pin `idevice` 0.1.64 (pre-1.0 — expect churn), confirm usbmuxd is present, and walk the pairing/trust flow on a real iPhone | build + on-device |
| 1 | lockdownd identify (device info) with pairing/trust handling | on-device |
| 2 | AFC file-transfer, then installation_proxy, then backup | on-device |
| 3 | usbmuxd-missing detection + honest messaging; CLI/GUI wiring | pytest (mock) |

Commit each phase only when the workspace is green (build/test/clippy/fmt).
Keep the `nusb` "no libusb" property unless §2 consciously trades it away.

## 9. Effort / risk

- **Android** — medium. `adb_client` is high-level; the real risk is the USB
  backend choice (libusb vs nusb vs server mode) and the auth UX.
- **iOS** — medium-to-large. `idevice` + usbmuxd + pairing/TLS is more moving
  parts and harder to test (needs usbmuxd and a physical iPhone); it is also
  **pre-1.0** (breaking changes each point release), so pin the version and
  budget for upkeep.
- Both are **larger than they look** precisely because the value is on-device and
  untestable in CI — budget for hardware sessions, and do not treat "compiles +
  unit tests pass" as "works on the phone" (the same discipline the nusb
  migration used).

## 10. Open questions for the spikes

1. Which §2.1 libusb-reconciliation option to take — feature-flagged
   `adb_client/usb` (opt-in libusb) vs server/TCP transport vs `mtp-rs` for plain
   file-transfer. **Decide before any code.** (`adb_client` has no nusb backend
   today — confirmed.)
2. Pinning/upkeep strategy for `idevice` (pre-1.0, breaking each point release).
3. Where CLI subcommands for mobile transfer live (extend `symbinux-fbus`, or a
   sibling `symbinux-mobile` binary) to avoid bloating the Nokia-focused CLI.
4. How much backup/restore scope to take on (full `mobilebackup2` is large).

---

*Plan only. No production code, dependency, or git history has been modified.
Execution is (at least) two dedicated sessions per §8, each starting with its
Step 0 spike.*
