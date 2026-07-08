# Roadmap

*[Leggi in italiano](ROADMAP.it.md)*

Status of the Symbinux stack and the planned path to broader phone support.

## Done (through v0.4.0)

- **Protocol core (`symbinux-protocol`)** — FBUS/2 and MBUS v1 frame codecs with
  dual/single checksums, incremental frame reader, named command builders with
  safety classification. Validated against real-capture checksum oracles.
- **Transport (`symbinux-transport`)** — serial (termios, 115200 8N1) backend,
  raw USB backend via `nusb` (pure Rust, no libusb), lsusb-style device
  enumeration, FBUS/2 request/response exchange.
- **Device detection (`symbinux-devices`)** — cascade fingerprinting
  (Nokia/Android/Apple iOS/unknown), `DeviceHandler` strategy with per-platform
  capabilities, port-based tracking across AOA/iOS mode switches.
- **CLI (`symbinux-fbus`)** — `devices`, `detect`, `identify`, `getphonebook`,
  `netmon`, `raw` (guarded). gnokii-style flags.
- **GUI (Rust, `symbinux-gui`)** — gtk4-rs without libadwaita, linking the core
  directly (no subprocess): channel selector with real USB detection, real
  Bluetooth/Wi-Fi scans, capability-aware function buttons, direct Identify
  card, real percentage progress with Cancel, theme switcher (Automatic follows
  the desktop via the XDG portal), 11-language localisation from the `.po`
  files. Runs on Linux and Windows; builds and tests on macOS in CI. The
  Python GTK4/libadwaita GUI stays usable until parity is hardware-validated
  (PBAP) and Phase 5 of `docs/CROSS_PLATFORM_GUI_PLAN.md` retires it.
- **Wireless core (`symbinux-wireless`)** — Bluetooth/Wi-Fi scanning, PBAP
  contact pulls and desktop notifications behind one portable API: BlueZ /
  NetworkManager / obexd on Linux, BLE-only `btleplug` scanning on
  Windows/macOS (verified live on Windows), `notify-rust` notifications
  everywhere.
- **Packaging** — Flatpak manifest, per-category udev rules, `devices.json`,
  and a Windows portable dist + per-user Inno Setup installer
  (`packaging/windows/`, GTK runtime bundled, verified end to end).
- **Typed decoding (started)** — `symbinux-protocol::decode` turns the HW/SW
  version reply into a struct (validated against the real 3310 capture); stable
  `--json` output on `devices`/`detect`; structured logging (`RUST_LOG`).

- **App-owned USB link (started)** — `symbinux-fbus identify --usb` claims the
  Nokia device directly via `nusb` (kernel driver detached, FBUS bulk endpoints
  auto-discovered), so a phone can be reached without any OS serial driver. See
  `docs/CONNECTION_MODEL.md` — the app owns the connection and forces the link,
  rather than depending on OS drivers/daemons.

The backlog below is prioritised from a multi-project review; see
`docs/COMPARISON.md` for prior art and `docs/CROSS_PLATFORM.md` for portability.

## Near term (P0/P1)

1. **Wire the remaining GUI functions end-to-end.** The serial-port resolver
   exists and Identify already calls the core directly; the Phonebook, SMS and
   Netmonitor buttons still show an honest "not wired up" state — they are
   blocked on the response decoders below (item 4), not on plumbing.
2. **Typed decoding → PIM formats.** Extend `decode` to phonebook entries
   (→ vCard `.vcf`) and SMS PDU (7-bit/UCS2, 3GPP TS 23.040), then add `--json`
   to every command. Unblocks contacts and SMS features.
3. **SMS list/read/send end-to-end** and **phonebook read/write** wired through
   CLI + GUI, with an explicit confirmation gate for `Experimental` writes.
4. **Response decoders on hardware** — the transport is now robust (multi-frame
   reassembly, retransmission window, reader resync all done); the remaining
   near-term work is decoding real phonebook/SMS *responses* into typed structs,
   which needs a real capture or a phone to validate the byte layout.
5. **MBUS v1 on hardware** — call `drain_echo` in an MBUS exchange loop, validate
   against a real phone, replace the synthetic fixture with a real capture.
6. **Robustness** — differentiate GUI errors (missing binary vs permission vs
   timeout vs no device) with actionable text; add a fuzz/property test for
   `Fbus2Reader`; cancellation for long scans.

## Medium term

7. **Backup/restore bundle** — one command dumping phonebook + SMS (+ calendar)
   to `.vcf`/`.ics`/`.json`; add `Capability::Backup` to the Nokia handler.
8. **Call log, calendar/todo, ringtones, logos** — reuse the `0x03`/security
   framing generalised (gnokii/gammu wire these on the same families).
9. **Bluetooth phone comms** — PBAP (contacts) and MAP (SMS) over Bluetooth via
   BlueZ `obexd` D-Bus, to reach cable-dead Nokias (see `docs/COMPARISON.md`).
10. **FBUS/2 over raw USB (DKU-2 native)** and **BB5** — auto-discover the FBUS
    bulk endpoints / PhoNet interface; per-model table in `devices.json`.
10b. **Deeper native access (read-only recovery)** — from the NSS study
    (`docs/NOKIA_SERVICE_MODES.md`): the PhoNet-over-USB "PC Suite" service
    channel, normal/local mode detection, filesystem browse & pull, and
    read-only Permanent Memory dumps for identity/settings recovery. Writes to
    PM / locks / product code / flash stay out of scope.

## Infrastructure & cross-platform

11. **CLI on Windows/macOS** — the core is already portable and the USB layer
    already uses `nusb` (pure Rust, no libusb), so binaries are self-contained;
    ship `symbinux-fbus` cross-platform. See `docs/CROSS_PLATFORM.md`.
12. **D-Bus service** (zbus) exposing device state + hotplug events to the GUI
    and other apps, KDE-Connect style — replaces subprocess scraping long-term.
13. **Android/iOS transfer** — embed `adb_client` (Rust) and `idevice` (Rust)
    rather than shelling out, when the product scope expands.

## Explicitly out of scope

- Firmware flashing / write operations (brick risk on unsupported hardware).
- Permanent Memory / SIM-lock / product-code / IMEI **writes** — the NSS-style
  service-tool features that change device identity or locks
  (`docs/NOKIA_SERVICE_MODES.md`). PM is a **read-only** recovery target.
- Any dependency on proprietary Nokia software or reverse-engineered binaries.

## How to help

Real captures are the bottleneck. See `docs/PROTOCOL_NOTES.md` §7 for the open
questions and the capture methodology.
