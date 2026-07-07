# Android & iOS transfer plan (`adb_client` / `idevice`) — item D

> Status: **plan / design document — not started.** Execution is a dedicated
> session (in fact **two** independent sub-sessions: Android, then iOS) per §7.
> Crate versions and exact API names marked *(verify in spike)* could not be
> web-verified while writing this (account session limit); each Step 0 spike
> confirms them against the real crate before code is written.

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

## 2. `adb_client` at a glance *(verify in spike)*

- Speaks the ADB protocol. Two transports of interest:
  - **USB transport** (direct to the device) — the self-contained option.
  - **Server transport** (connect to a running `adb` server on TCP `5037`) —
    no USB code, but requires the platform-tools `adb` binary installed.
- **Auth**: ADB uses an RSA-2048 keypair; the device shows the *"Allow USB
  debugging?"* dialog on first connect and pins the host public key. The crate
  generates/loads a key (typically `~/.android/adbkey`).
- **API surface** *(verify)*: enumerate/connect a device, `shell(cmd)`,
  `push(local, remote)`, `pull(remote, local)`, install/uninstall, framebuffer
  (screenshot). Enough to back identify + file-transfer + app-install +
  screenshot capabilities.
- **⚠️ libusb-reintroduction risk (key decision):** the USB transport may pull in
  `rusb`/libusb, which the project just removed via the nusb migration. Three
  options to settle in the spike, in order of preference:
  1. Use a build/version of `adb_client` whose USB backend is `nusb` (pure Rust)
     if available;
  2. Feed `adb_client` an **already-open USB handle from our `nusb` claim**, if
     the API allows a custom transport — keeps one USB stack;
  3. Fall back to **server transport** (require system `adb`) so no USB C
     dependency is added at all.
  Losing the "no libusb" property silently would be a regression — this must be a
  conscious choice, logged in the CHANGELOG.
- Cross-platform: ADB itself works on Linux/Windows/macOS.

## 3. `idevice` at a glance *(verify in spike)*

- Pure-Rust alternative to libimobiledevice; **async (tokio)**.
- Talks to **usbmuxd** — the USB multiplexing daemon — over its socket:
  - **Linux:** the `usbmuxd` package (ships its own udev rule + systemd
    activation; already noted in `udev/README.md`).
  - **macOS:** usbmuxd is built into the OS.
  - **Windows:** provided by Apple's *Apple Mobile Device Service* (installed
    with iTunes / the Apple Devices app).
- **Pairing / trust:** first contact creates a pairing record and the iPhone
  shows the *"Trust This Computer?"* dialog; subsequent sessions reuse the
  record. A TLS **lockdown** session wraps service access.
- **Service clients** *(verify names/versions)*: `lockdownd` (device info →
  identify), **AFC** (filesystem → file-transfer), `installation_proxy`
  (app-install/list), `mobilebackup2` (backup), screenshotr (screenshot).
- The mux + trust + TLS stack is exactly why we embed a crate instead of
  reimplementing it (per `docs/DEVICE_DETECTION.md`).

## 4. MTP / PTP / AOA fallback (scope note)

- **MTP** (Android with USB debugging off) — no strong pure-Rust crate; would
  mean `libmtp` (C) or a from-scratch MTP/PTP implementation. **Out of initial
  scope**; file-transfer for such phones is a later phase.
- **PTP** (camera/photos) — narrow; defer.
- **AOA** (Android Open Accessory) — vendor control requests `51/52/53` on
  endpoint 0; our detection already recognises AOA. Driving it is niche; defer.

Ship ADB + iOS-over-usbmuxd first (the 90% cases); treat MTP/PTP/AOA as optional
follow-ups so the initial scope stays bounded.

## 5. Mapping to the existing architecture

- Introduce a **`TransferBackend` trait** (in `symbinux-devices` or a new
  `symbinux-mobile` crate) that `AndroidHandler`/`AppleHandler` hold, turning the
  currently-advertised `Capability` values into real calls:

  | Capability | Android (`adb_client`) | iOS (`idevice`) |
  |---|---|---|
  | identify | `shell getprop` / device info | `lockdownd` values |
  | file-transfer | `push`/`pull` | AFC |
  | app-install | install/uninstall | `installation_proxy` |
  | backup | `adb backup` (or per-item) | `mobilebackup2` |
  | screenshot | framebuffer | screenshotr |

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
5. **libusb re-introduction (§2)** — the single biggest architectural risk;
   decide the transport before writing code.
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
| 0 | **Spike:** confirm `adb_client` API + **the USB-backend/libusb decision (§2)**; confirm auth + `shell`/`push`/`pull` signatures | build-only |
| 1 | `TransferBackend` trait + `AndroidHandler` wiring for `identify` (getprop) | unit + on-device |
| 2 | file-transfer (`push`/`pull`), then app-install, then screenshot | on-device |
| 3 | CLI subcommands + GUI actions gated by capability; honest auth-wait UX | pytest (mock) |

**Phase B — iOS (`idevice`)** *(separate session)*

| # | Step | Testable? |
|---|---|---|
| 0 | **Spike:** confirm `idevice` API + usbmuxd availability + pairing/trust flow | build-only |
| 1 | lockdownd identify (device info) with pairing/trust handling | on-device |
| 2 | AFC file-transfer, then installation_proxy, then backup | on-device |
| 3 | usbmuxd-missing detection + honest messaging; CLI/GUI wiring | pytest (mock) |

Commit each phase only when the workspace is green (build/test/clippy/fmt).
Keep the `nusb` "no libusb" property unless §2 consciously trades it away.

## 9. Effort / risk

- **Android** — medium. `adb_client` is high-level; the real risk is the USB
  backend choice (libusb vs nusb vs server mode) and the auth UX.
- **iOS** — medium-to-large. `idevice` + usbmuxd + pairing/TLS is more moving
  parts and harder to test (needs usbmuxd and a physical iPhone).
- Both are **larger than they look** precisely because the value is on-device and
  untestable in CI — budget for hardware sessions, and do not treat "compiles +
  unit tests pass" as "works on the phone" (the same discipline the nusb
  migration used).

## 10. Open questions for the spikes

1. Does `adb_client` support a nusb backend or a caller-supplied USB handle, or
   must we accept libusb / use server mode? (§2 — decide first.)
2. `idevice`'s exact service-client set and their API stability across versions.
3. Where CLI subcommands for mobile transfer live (extend `symbinux-fbus`, or a
   sibling `symbinux-mobile` binary) to avoid bloating the Nokia-focused CLI.
4. How much backup/restore scope to take on (full `mobilebackup2` is large).

---

*Plan only. No production code, dependency, or git history has been modified.
Execution is (at least) two dedicated sessions per §8, each starting with its
Step 0 spike.*
