# Cross-platform desktop plan: gtk4-rs GUI + native backends

> Status: **Phase 1 under way (Linux).** Phase 0 confirmed the toolkit; the
> `symbinux-gui` crate (gtk4-rs 0.9.7, GTK 4.22, no libadwaita) now lives in the
> workspace — CI-gated with `libgtk-4-dev`, building green, and listing detected
> devices by linking `symbinux-devices` directly (no subprocess, no libusb).
> The Rust window now has the GTK4 header, channel selector, honest progress/cancel
> panel, empty state, capability-aware Nokia action buttons, and direct Identify
> over `symbinux-transport`. Bluetooth and Wi-Fi scans are ported with honest
> host-tool errors, the theme menu persists to `settings.json` (Automatic
> follows the desktop preference via the XDG settings portal, falling back to
> dark, and the empty-state logo tracks the effective scheme), and native
> notifications are wired through Gio. The language menu now loads the existing
> `.po` files in pure Rust. PBAP contacts are wired through BlueZ/obexd and need
> real phone hardware validation before the Python GUI can be retired. The
> **Python GUI stays usable throughout.** The Rust GUI now also **builds and runs
> natively on Windows** (gtk4-rs against MSYS2 GTK4, `x86_64-pc-windows-gnu`,
> `GSK_RENDERER=cairo`); the macOS build and *signed* binaries remain later
> decisions.

## 1. Goal & mission constraint

Make the **full Symbinux experience usable on Linux, Windows and macOS**, not just
the CLI. The **mission is complete compatibility with legacy Nokia / Symbian
devices** — including **contacts over Bluetooth OBEX/PBAP** — so the wireless
Bluetooth path is **in scope on all three OSes**, not deprioritised. (This
corrects the earlier suggestion to defer classic-Bluetooth on Windows/macOS.)

Chosen GUI approach (confirmed): **port the existing GTK4 GUI to `gtk4-rs`
(Rust)**, dropping libadwaita, and link the Rust core directly instead of shelling
out to the CLI.

## 2. Why gtk4-rs, and what it changes

- The current GUI is Python + GTK4 + **libadwaita**. GTK4 itself is
  cross-platform; **libadwaita is the Linux-first blocker**. Porting to `gtk4-rs`
  (the `gtk4` crate) and using plain GTK4 widgets removes that blocker while
  reusing the *same widget structure* we already designed and verified.
- **One language, one binary, no subprocess bridge.** Today the Python GUI runs
  `symbinux-fbus … --json` and scrapes output (`src/symbinux/gui/backend.py`). A
  Rust GUI **links `symbinux-devices` / `symbinux-transport` / `symbinux-protocol`
  directly** — no serialisation round-trip, no Python packaging on Win/macOS.
- **Cross-plan implication (B8):** with the GUI linking the core directly, the
  D-Bus service (`docs/DBUS_SERVICE_PLAN.md`) is **no longer needed by the GUI
  itself**; it stays valuable only as an *external* API for third-party apps
  (KDE-Connect style). B8 therefore drops in priority relative to this track.

## 3. Architecture after the port

```
symbinux-protocol  ─┐
symbinux-transport ─┤  (Rust core crates, cross-platform)
symbinux-devices   ─┤
symbinux-wireless  ─┘  (NEW: BLE + Wi-Fi + classic-BT/OBEX, per-OS behind traits)
        │  (direct function calls — no subprocess, no D-Bus needed)
        ▼
symbinux-gui  (NEW: gtk4-rs, replaces src/symbinux/*.py)
```

The Python GUI and the `symbinux-fbus` CLI both stay usable during the port; the
CLI remains the scriptable/headless entry point on all platforms.

## 4. What moves into the Rust core (cross-platform backends)

Today's Linux-only shell-outs and OS integrations become cross-platform Rust,
exposed as library calls (and optionally CLI subcommands so scripts benefit too):

| Backend | Today (Linux-only) | Cross-platform Rust plan |
|---|---|---|
| USB claim + FBUS | `nusb` | already cross-platform (done) |
| Serial ports | `serialport` | already cross-platform (done) |
| **BLE scan** | `bluetoothctl` shell-out | [`btleplug`](https://crates.io/crates/btleplug) — Linux (BlueZ) / Windows (WinRT) / macOS (CoreBluetooth). **BLE only** |
| **Wi-Fi scan** | `nmcli` shell-out | Windows WLAN API / macOS CoreWLAN via the `windows`/`objc2` crates; keep `nmcli` on Linux. Low priority (little value for legacy Nokia) |
| **Desktop notifications** | `Gio.Notification` | [`notify-rust`](https://crates.io/crates/notify-rust) — freedesktop / Windows toast / macOS |
| **Classic BT OBEX/PBAP** | BlueZ + obexd (D-Bus) | **§5 — the hard, mission-critical piece** |

New crate **`symbinux-wireless`** holds these behind per-OS `#[cfg]` impls of
shared traits, so the GUI and CLI call one portable API.

## 5. The hard part: classic Bluetooth OBEX/PBAP on all 3 OSes

This is the **highest-risk item** and the one the mission demands. Legacy Nokia
phones expose contacts/SMS over **Bluetooth *classic* (RFCOMM) + OBEX/PBAP**, not
BLE — so `btleplug` (BLE-only) does **not** cover it. There is **no good
cross-platform Rust crate** for classic-BT OBEX; it must be built per-OS on top of
each platform's classic-Bluetooth stack, feeding a **shared, testable OBEX/PBAP +
vCard layer** (the protocol/parse logic is OS-independent and already partly
exists in `symbinux-protocol::decode`).

| OS | Classic-BT / RFCOMM stack | OBEX/PBAP status |
|---|---|---|
| **Linux** | BlueZ + obexd over D-Bus | **already implemented** (`ensure_paired`, `pull_contacts_pbap`) — reuse |
| **Windows** | `windows` crate: `Windows.Devices.Bluetooth.Rfcomm` + `StreamSocket` (or Win32 `BluetoothAPIs`) | **build an OBEX/PBAP client** over the RFCOMM stream *(verify in spike)* |
| **macOS** | IOBluetooth (`IOBluetoothDevice`/`IOBluetoothRFCOMMChannel`, or `OBEXSession`) via `objc2` bindings | **build/bind** — Rust bindings are sparse; highest uncertainty *(verify in spike)* |

Design: a `PhonebookOverBluetooth` trait (pair → open RFCOMM → OBEX CONNECT →
PBAP pull → vCard) with three per-OS transport impls; the OBEX framing + PBAP +
vCard assembly is shared Rust, unit-testable without hardware. Each OS impl needs
a **dedicated spike** to confirm the RFCOMM+OBEX path (and each needs real
hardware to validate).

## 6. Phasing (Linux stays working throughout)

Each phase is independently shippable; the Python GUI remains until Phase 3 proves
parity.

| Phase | Work | Result |
|---|---|---|
| **0** | **Spike:** ✅ *Linux + Windows confirmed* — `gtk4-rs` links `symbinux-devices` directly (no subprocess, no libusb). Windows GTK4 runtime recipe: **MSYS2 mingw64 GTK4 + the `x86_64-pc-windows-gnu` target** (matches rustup gnu/MSVCRT). ⏳ *pending:* the macOS recipe (homebrew gtk4). | de-risk the toolkit |
| **1** | Port the GUI to `gtk4-rs` on **Linux** at feature parity linking the core directly. ✅ *feature port complete:* `symbinux-gui` has the window shell, wordmark/version header, USB/Bluetooth/Wi-Fi selector, theme-aware empty-state logo, real USB detection with progress/cancel, capability-aware Nokia action buttons, a direct Identify card via `symbinux-transport`, Bluetooth/Wi-Fi scans via Linux host tools, PBAP contacts through BlueZ/obexd, persisted theme selection, Gio notifications, and `.po`-based i18n (11 langs). *Pending:* PBAP hardware validation with a real phone | Rust GUI == current GUI, on Linux once PBAP is hardware-validated |
| **2** | New `symbinux-wireless` crate: BLE scan (`btleplug`) + notifications (`notify-rust`), GUI calls them directly; retire the `bluetoothctl`/`nmcli`/`Gio` shell-outs | wireless in the core, portable |
| **3** | Cross-platform **build** of the Rust GUI for Windows + macOS (unsigned). ✅ *Windows done:* built for `x86_64-pc-windows-gnu` against MSYS2 mingw64 GTK4 and **run natively** — single integrated titlebar, theme-aware (follows the OS light/dark), honest empty state, `nusb` USB enumeration working. Requires `GSK_RENDERER=cairo` (the default GL renderer did not realise the window in this environment — a Windows packaging note). ⏳ *macOS pending* (homebrew gtk4). | GUI runs on 3 OSes |
| **4a** | **Classic-BT OBEX/PBAP on Windows** (spike → RFCOMM+OBEX client → PBAP pull) | Nokia contacts over BT on Windows |
| **4b** | **Classic-BT OBEX/PBAP on macOS** (spike → IOBluetooth binding → PBAP pull) | Nokia contacts over BT on macOS |
| **5** | Retire the Python GUI once parity is proven; update docs/packaging | single Rust GUI |

Phase 1 wireless decision: keep Linux parity by porting the existing
`bluetoothctl`, BlueZ/obexd, and `nmcli` flows into the Rust GUI with
`std::process`/D-Bus calls and honest unavailable states when the host stack is
missing. The scan paths use `bluetoothctl`/`nmcli`; PBAP uses `bluetoothctl` for
pair/connect and `busctl --user` against `org.bluez.obex.PhonebookAccess1`.
Phase 2 then moves those paths behind `symbinux-wireless` traits so the GUI stops
owning platform details.

Publishing **signed** Win/macOS binaries (code-signing cert, Apple Developer
account, notarisation) is a **separate later decision** — Phase 3/4 produce
runnable unsigned builds for our own use, which is all that's needed now.

## 7. Per-OS reality after each milestone

- **After Phase 1:** Linux GUI in Rust (parity). Win/macOS: CLI only (as today).
- **After Phase 3:** GUI + USB/serial detection + FBUS on all 3 OSes. Bluetooth
  contacts still Linux-only.
- **After Phase 4:** the mission target — **legacy-Nokia support incl. Bluetooth
  OBEX/PBAP contacts on all 3 OSes.** (Raw USB on Windows still needs WinUSB/Zadig;
  the serial/COM and Bluetooth paths do not.)

## 8. Effort & risk

- **GUI port (Phases 0–3):** **L**, but *bounded and low-uncertainty* — it's a
  structured 1:1 port of a GUI we already designed, into mature `gtk4-rs`. Main
  friction is GTK-runtime packaging on Win/macOS (known, documented path).
- **Classic-BT OBEX/PBAP (Phase 4):** **L and genuinely uncertain**, especially
  macOS (sparse Rust IOBluetooth bindings). This is the real research risk and
  should be spiked *before* committing to a timeline. Linux already works, so the
  shared OBEX/PBAP+vCard layer can be built and tested against the Linux path
  first, leaving only the per-OS RFCOMM transport to prove on Windows/macOS.
- **Wireless (Phase 2):** **M**, low risk (`btleplug`/`notify-rust` are mature).
- No item re-introduces libusb (USB stays `nusb`; BLE via `btleplug`; classic-BT
  via native OS stacks). The pure-Rust / no-C-USB property is preserved.

## 9. Open questions for the spikes

1. GTK4 runtime bundling on Windows (gvsbuild) and macOS (homebrew gtk4 +
   `dylib` relocation / app bundle) — confirm a working packaging recipe in
   Phase 0 before committing to the port.
2. Windows RFCOMM: `windows` crate `Rfcomm`+`StreamSocket` vs Win32
   `BluetoothAPIs` — which gives a usable OBEX transport, and does pairing need a
   user prompt?
3. macOS: are there maintained `objc2` IOBluetooth bindings, or must we bind
   `IOBluetoothRFCOMMChannel`/`OBEXSession` ourselves? (Biggest unknown.)
4. Does the GUI keep any CLI/D-Bus indirection, or link the core purely as a
   library? (Recommendation: pure library link; keep the CLI as a separate
   headless tool.)

---

*Plan only. No production code, dependency, or git history has been modified.
Execution is a sequence of dedicated sessions per §6, each starting with its
spike; the Linux GUI stays working throughout.*
