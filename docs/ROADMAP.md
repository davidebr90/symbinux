# Roadmap

Status of the Symbinux stack and the planned path to broader phone support.

## Done (v0.2.0)

- **Protocol core (`symbinux-protocol`)** — FBUS/2 and MBUS v1 frame codecs with
  dual/single checksums, incremental frame reader, named command builders with
  safety classification. Validated against real-capture checksum oracles.
- **Transport (`symbinux-transport`)** — serial (termios, 115200 8N1) backend,
  raw USB (libusb) backend skeleton, lsusb-style device enumeration, FBUS/2
  request/response exchange.
- **CLI (`symbinux-fbus`)** — `devices`, `identify`, `getphonebook`, `netmon`,
  `raw` (guarded). gnokii-style flags.
- **GUI** — GTK4/libadwaita front-end: channel selector, capability-aware
  (greyed) function buttons, contextual empty states, native desktop
  notifications, advanced device view.
- **Packaging** — Flatpak manifest, udev rules, `devices.json`.

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
6. **Bluetooth channel** — FBUS/MBUS over RFCOMM via BlueZ (`org.bluez` D-Bus),
   wiring the GUI's Bluetooth channel to a real scan.

## Explicitly out of scope

- Firmware flashing / write operations (brick risk on unsupported hardware).
- Any dependency on proprietary Nokia software or reverse-engineered binaries.

## How to help

Real captures are the bottleneck. See `docs/PROTOCOL_NOTES.md` §7 for the open
questions and the capture methodology.
