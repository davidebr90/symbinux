# Connection model: the app owns the link

**The whole point of Symbinux is to be the connection agent** — to talk to a
phone the operating system does not know how to talk to, and to *force* the link
itself, over cable or over the air. It must not depend on the OS having the right
driver, the right daemon, or a prior pairing already set up. This is the model
Nokia PC Suite, Nemesis Service Suite (NSS) and KDE Connect follow: the
application, not the OS, owns the connection and drives the pairing.

That principle shapes every transport.

## Two ways to reach a phone

### 1. OS-mediated (convenient, but limited)
The OS already exposes an endpoint and Symbinux uses it:
- **Serial** (`SerialTransport`): the kernel bound a driver (`ftdi_sio`,
  `cp210x`, `pl2303`, `cdc-acm`) and published a `/dev/ttyUSB*` (or `COMn`); we
  open it. Works only when such a driver exists and loaded.
- **Bluetooth/Wi-Fi scan** via `bluetoothctl`/`nmcli`: reads what BlueZ /
  NetworkManager already know.

This is the easy path, but it fails exactly where the app is supposed to add
value: a phone the OS has no driver for, or that was never paired.

### 2. App-owned (forces the link)
Symbinux takes the device from the OS and speaks the protocol itself:
- **Raw USB** (`UsbTransport::open_fbus_auto`): claims the phone's USB device
  directly via `nusb` (pure Rust), **detaching any kernel driver first**, auto-discovers
  the FBUS bulk endpoints, and drives the protocol — no `/dev/ttyUSB` required.
  This is how a DKU-2 native-USB or BB5 phone is reached on a machine with no
  serial driver. Exposed as `symbinux-fbus identify --usb`.
- **Bluetooth (app-driven pairing)**: rather than requiring the phone to be
  pre-paired in the OS, the app drives BlueZ `Device1.Pair` + `Device1.Connect`
  over D-Bus to *force* the pairing (`ensure_paired`), then pulls contacts over
  obexd PBAP (`pull_contacts_pbap` force-pairs first). Opening a raw RFCOMM
  channel for direct FBUS/OBEX is the next step. Needs a real adapter to
  validate; pairing may require confirming a code on the phone.

> On Linux the app still rides the kernel USB/Bluetooth stacks (via usbfs /
> BlueZ) — you cannot bypass the kernel entirely. What the app *does* bypass is
> the OS's **default drivers, pairing UI and daemons**: it claims the device and
> owns the protocol, which is the meaningful "force the connection" the tool
> exists to provide.

## Wireless: what legacy Nokia can and cannot do

- **Bluetooth** — real: legacy Series 40/60 phones expose OBEX/PBAP/OPP over
  Bluetooth even when their USB port is dead. The app can force a pair and pull
  contacts/SMS. This is the primary "over the air" path for these phones.
- **Wi-Fi** — legacy Nokia phones have no Wi-Fi phone-management service to force
  a link to; Wi-Fi pairing only makes sense for the Android/iOS handlers (which
  have their own agents: ADB wireless pairing, iOS RSD). For legacy Nokia the
  Wi-Fi channel stays a network scan, not a phone link.

## Where the code stands

| Path | Status |
|---|---|
| Serial (OS ttyUSB) | Works; needs an OS serial driver. |
| **Raw USB (app-owned, `--usb`)** | Implemented — claims the device via `nusb` (pure Rust), auto-discovers endpoints; needs real hardware to validate on-device. |
| Bluetooth scan / PBAP contacts | Implemented via BlueZ/obexd, with app-driven *pairing* (force pair + connect). Needs hardware to validate. |
| Android/iOS | Dispatch + capabilities only; real transfer via `adb_client`/`idevice` is future work. |

See `docs/ROADMAP.md` for the sequence and `docs/DEVICE_DETECTION.md` for how a
device is recognised before a transport is chosen.
