# Roadmap

*[Leggi in italiano](ROADMAP.it.md)*

Status of the Symbinux stack and the planned path to broader phone support.

## Done (through v0.4.0)

- **Protocol core (`symbinux-protocol`)** — FBUS/2 and MBUS v1 frame codecs with
  dual/single checksums, incremental frame reader, named command builders with
  safety classification. Validated against real-capture checksum oracles.
- **Transport (`symbinux-transport`)** — serial (termios, 115200 8N1) backend,
  raw USB (libusb) backend skeleton, lsusb-style device enumeration, FBUS/2
  request/response exchange.
- **Device detection (`symbinux-devices`)** — cascade fingerprinting
  (Nokia/Android/Apple iOS/unknown), `DeviceHandler` strategy with per-platform
  capabilities, port-based tracking across AOA/iOS mode switches.
- **CLI (`symbinux-fbus`)** — `devices`, `detect`, `identify`, `getphonebook`,
  `netmon`, `raw` (guarded). gnokii-style flags.
- **GUI** — GTK4/libadwaita: channel selector with real USB detection plus real
  Bluetooth (BlueZ) and Wi-Fi (NetworkManager) scans, capability-aware function
  buttons, real percentage progress, theme switcher, 7-language localisation.
- **Packaging** — Flatpak manifest, per-category udev rules, `devices.json`.
- **Typed decoding (started)** — `symbinux-protocol::decode` turns the HW/SW
  version reply into a struct (validated against the real 3310 capture); stable
  `--json` output on `devices`/`detect`; structured logging (`RUST_LOG`).

The backlog below is prioritised from a multi-project review; see
`docs/COMPARISON.md` for prior art and `docs/CROSS_PLATFORM.md` for portability.

## Near term (P0/P1)

1. **Wire the GUI functions end-to-end.** The Identify/Phonebook/SMS/Netmonitor
   buttons don't yet call the core — they need a **serial-port resolver** that
   maps a detected USB device (`PortKey`/VID:PID) to a `/dev/ttyUSB*` path, then
   the GUI can run `identify` and show the decoded result. This port-resolution
   step is the current blocker for every real phone operation from the GUI.
2. **Typed decoding → PIM formats.** Extend `decode` to phonebook entries
   (→ vCard `.vcf`) and SMS PDU (7-bit/UCS2, 3GPP TS 23.040), then add `--json`
   to every command. Unblocks contacts and SMS features.
3. **SMS list/read/send end-to-end** and **phonebook read/write** wired through
   CLI + GUI, with an explicit confirmation gate for `Experimental` writes.
4. **Multi-frame reassembly** in `exchange_fbus2` (handle `FramesToGo > 1`
   fragmented replies) and a **retransmission window** (configurable ACK timeout
   200–500 ms + retry per the gnokii sequence scheme).
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

## Infrastructure & cross-platform

11. **CLI on Windows/macOS** — the core is already portable; ship
    `symbinux-fbus` cross-platform, optionally migrating USB to `nusb` to drop
    the libusb dependency. See `docs/CROSS_PLATFORM.md`.
12. **D-Bus service** (zbus) exposing device state + hotplug events to the GUI
    and other apps, KDE-Connect style — replaces subprocess scraping long-term.
13. **Android/iOS transfer** — embed `adb_client` (Rust) and `idevice` (Rust)
    rather than shelling out, when the product scope expands.

## Explicitly out of scope

- Firmware flashing / write operations (brick risk on unsupported hardware).
- Any dependency on proprietary Nokia software or reverse-engineered binaries.

## How to help

Real captures are the bottleneck. See `docs/PROTOCOL_NOTES.md` §7 for the open
questions and the capture methodology.
