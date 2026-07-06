# Functions reference

*[Leggi in italiano](FUNCTIONS.it.md)*

What Symbinux can do today, across the GUI and the `symbinux-fbus` CLI. Each
function lists the icon used in the GUI, what it needs to be enabled, and its
safety class (see `docs/PROTOCOL_NOTES.md` §6).

## Connection channels (GUI)

The channel selector picks how Symbinux looks for a phone. Each channel runs a
real scan with a spinner and an honest empty/error state — never a fake loader.

| Channel | Icon | Behaviour |
|---|---|---|
| USB | `drive-harddisk-usb-symbolic` | Multi-platform device detection + serial FBUS/2 I/O (Nokia). |
| Bluetooth | `bluetooth-symbolic` | Real device inquiry via BlueZ (`bluetoothctl`); needs an adapter. |
| Wi-Fi | `network-wireless-symbolic` | Real network scan via NetworkManager (`nmcli`). |

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
| `devices [--all] [--json]` | Advanced device enumeration. Without `--all`, shows only phones and known cable bridges. `--json` emits a stable machine format. | Confirmed |
| `detect [--progress] [--json]` | Auto-detect a connected phone's platform and capabilities. `--json` for scripting. | Confirmed |
| `ports [--json]` | List the serial ports the OS exposes (with USB ids). | Confirmed |
| `identify [--port <p>] [--usb] [--json]` | HW/SW version query. `--usb` claims the Nokia device directly via libusb (no serial driver needed); `--json` prints the decoded model/firmware/date. | Confirmed |
| `getphonebook --port <p> --mem <me\|sim\|…> --location <n>` | Read a phonebook entry. | Confirmed |
| `netmon --port <p> [--screen <n>]` | Netmonitor screen / control. | Confirmed |
| `raw --port <p> --msg-type <hex> --block "<hex …>" --i-understand-risk` | Send an arbitrary FBUS/2 frame (reverse-engineering). | Experimental |
| `completions <bash\|zsh\|fish\|…>` | Print a shell completion script to stdout. | Confirmed |
| `man` | Print the roff man page to stdout. | Confirmed |
| `decode-frame <hex>` | Decode a captured FBUS/2 frame offline (no device). | Confirmed |
| `decode-sms <hex>` | Decode a captured SMS-DELIVER PDU offline (no device). | Confirmed |

In the GUI, the **Identify** button shows the decoded identity as a card
(model / firmware / date) rather than raw text, resolving the phone's serial port
automatically. See `docs/CONNECTION_MODEL.md` for the app-owned USB/Bluetooth
paths.

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
