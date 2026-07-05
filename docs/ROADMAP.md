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

## Near term

1. **FBUS/2 command coverage** — wire `identify` end-to-end response parsing
   (model/IMEI/firmware struct), phonebook read → structured contacts, vCard
   import/export. Currently the framing is complete; the response decoders are
   partial.
2. **MBUS v1 on hardware** — validate the synthetic codec against a real phone,
   add the half-duplex echo-drain to the exchange loop, replace the synthetic
   fixture with a real capture.
3. **Retransmission window** — configurable ACK timeout (200–500 ms) and retry
   for FBUS/2, per the gnokii sequence-number scheme.

## Medium term

4. **FBUS/2 over raw USB (DKU-2 native)** — select the alternate USB
   configuration/interface that exposes the two FBUS bulk endpoints (the default
   config emulates an AT modem), then reuse the existing framing over
   `UsbTransport`.
5. **BB5 phones** — endpoint auto-discovery over the PhoNet bulk interface;
   per-model interface/altsetting table in `devices.json`.
6. **Bluetooth phone comms** — the Bluetooth channel already discovers devices;
   next is FBUS/MBUS (or OBEX) over RFCOMM via BlueZ to actually talk to a phone.

## Explicitly out of scope

- Firmware flashing / write operations (brick risk on unsupported hardware).
- Any dependency on proprietary Nokia software or reverse-engineered binaries.

## How to help

Real captures are the bottleneck. See `docs/PROTOCOL_NOTES.md` §7 for the open
questions and the capture methodology.
