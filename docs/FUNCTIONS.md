# Functions reference

What Symbinux can do today, across the GUI and the `symbinux-fbus` CLI. Each
function lists the icon used in the GUI, what it needs to be enabled, and its
safety class (see `docs/PROTOCOL_NOTES.md` §6).

## Connection channels (GUI)

The channel selector picks how Symbinux looks for a phone. Each channel has its
own contextual state rather than a single generic "nothing found" message.

| Channel | Icon | Status |
|---|---|---|
| USB | `drive-harddisk-usb-symbolic` | **Supported.** Enumerates phones and cable bridges; serial FBUS/2 I/O. |
| Bluetooth | `bluetooth-symbolic` | Roadmap. Selectable; shows a "not available yet" state. |
| Wi-Fi | `network-wireless-symbolic` | Roadmap. Selectable; shows a "not available yet" state. |

## Phone functions (GUI)

Function buttons are **disabled (greyed out) until a compatible device is
selected**, so the full capability set is always visible even with nothing
connected. A button enables only when its required capability is present.

| Function | Icon | Enabled when | Safety | What it does |
|---|---|---|---|---|
| Identify | `dialog-information-symbolic` | a Nokia phone is selected | Confirmed | Reads model, IMEI, hardware and firmware version. |
| Phonebook | `contact-new-symbolic` | a Nokia phone is selected | Confirmed (read) / Experimental (write) | Import/export contacts from ME/SIM memory. |
| SMS | `mail-message-new-symbolic` | a Nokia phone is selected | Experimental | Read and send text messages. |
| Netmonitor | `network-cellular-signal-excellent-symbolic` | a Nokia phone is selected | Confirmed | Network engineering / diagnostics screens. |
| Advanced | `utilities-terminal-symbolic` | always | Confirmed | Raw device inventory of everything connected (see below). |

The logo and version are always shown in the header bar and in the About dialog.
Action results are delivered as **native desktop notifications** through the
freedesktop notification spec, so they appear in GNOME, KDE and other desktops
without any extra dependency.

## Advanced mode (diagnostics)

The Advanced function lists **every USB device the host can see** (lsusb-style)
with vendor:product ids, extended manufacturer/product names, bus/address, and a
classification (Nokia phone / known cable bridge / other). This is meant to help
debug "why isn't my phone detected" reports — you can see the raw ids even for
unrecognised cables. It performs no phone I/O.

## CLI commands (`symbinux-fbus`)

| Command | Purpose | Safety |
|---|---|---|
| `devices [--all]` | Advanced device enumeration. Without `--all`, shows only phones and known cable bridges. | Confirmed |
| `identify --port <p>` | HW/SW version query over a serial cable. | Confirmed |
| `getphonebook --port <p> --mem <me\|sim\|…> --location <n>` | Read a phonebook entry. | Confirmed |
| `netmon --port <p> [--screen <n>]` | Netmonitor screen / control. | Confirmed |
| `raw --port <p> --msg-type <hex> --block "<hex …>" --i-understand-risk` | Send an arbitrary FBUS/2 frame (reverse-engineering). | Experimental |

Every phone command first sends the `0x55` init preamble, then the framed
request, and prints the request bytes, the ACK, and the decoded reply (with an
ASCII rendering when the payload is text).

### Safety guarantees

- Firmware/flash writes are **not implemented** and are refused as `Dangerous`.
- `raw` mode requires the explicit `--i-understand-risk` flag.
- Only `Confirmed` commands run by default; `Experimental` ones modify the phone
  and require deliberate opt-in.

## Library API (Rust)

- `symbinux-protocol` — `Fbus2Frame`, `MbusFrame`, `Fbus2Reader`, and the
  `message` module (named command builders + `Safety`). Pure framing, no I/O,
  fully unit-tested.
- `symbinux-transport` — `Transport` trait with `SerialTransport` and
  `UsbTransport`, `list_usb_devices()`, and `exchange_fbus2()`.
