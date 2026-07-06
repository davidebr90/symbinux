# Device detection & dispatch

Symbinux auto-detects a connected phone and dispatches to the right
communication strategy (Nokia legacy / Android / Apple iOS), exposing a common
interface with a **per-platform capability set** so the UI adapts without
assuming feature parity.

Implemented in the `symbinux-devices` crate. Constants are reconstructed from
the gnokii/gammu, AOSP/AOA, and libimobiledevice/usbmuxd projects; each carries a
confidence tag (CONFIRMED = official docs/source or ≥2 sources; LIKELY = single
source; UNCERTAIN).

## 1. Detection cascade

Each USB device is reduced to a fingerprint (vendor/product ids, device class,
and the class/subclass/protocol + string of every interface), then classified:

1. **Apple** — vendor id `0x05ac`. CONFIRMED. The usbmux interface is
   `0xff/0xfe/0x02`; usbmux PID ranges `0x1290–0x12af`, `0x1901–0x1905`, `0x8600`.
2. **Android accessory (AOA)** — Google vendor `0x18d1`, product `0x2d00–0x2d05`.
   CONFIRMED.
3. **Android by interface** — CONFIRMED:
   - ADB `0xff/0x42/0x01`
   - Fastboot `0xff/0x42/0x03`
   - MTP `0xff/0xff/0x00` **and** iInterface string `"MTP"`
   - PTP `0x06/0x01/0x01`
4. **Nokia legacy** — vendor id `0x0421`. CONFIRMED.
5. **Unknown** — anything else, offered for `raw-sniff` inspection (interface
   dump) to support community-driven extension.

Probes must be short (1–2 s) so an unrecognised device never blocks
enumeration. The fingerprint/classify logic is I/O-free and unit-tested; only
enumeration touches the bus (via `nusb`, pure Rust).

## 2. Capability matrix

Every handler advertises only what it actually supports:

| Platform / mode | Capabilities |
|---|---|
| Nokia legacy | identify, phonebook, sms, netmonitor |
| Android — ADB | identify, file-transfer, app-install, backup, screenshot |
| Android — MTP | file-transfer (only) |
| Android — PTP | file-transfer (photos) |
| Android — fastboot | identify |
| Android — accessory (AOA) | raw-sniff |
| Apple iOS | identify, file-transfer, app-install, backup |
| Unknown | raw-sniff |

The application layer reads these and enables/greys features accordingly (the
GUI's function buttons follow the selected device's capabilities).

## 3. Tracking across re-enumeration

An **AOA accessory switch** (after `ACCESSORY_START`, request 53) and an **iOS
trust-dialog** both make the same physical device disconnect and reappear with a
**different vid/pid and USB address**. CONFIRMED. Correlating by vid/pid or
address would treat it as a new device, so the `DeviceManager` keys on the
**stable physical port path** — bus number + hub-port chain (`nusb`
`busnum()` + `port_chain()`, e.g. `1-1.3`). A mode switch on the same port
is reported as a `Switched` transition, prompting a re-probe with the new
handler. After an iOS trust grant, re-probe lockdown.

## 4. Platform integration notes

- **Nokia** — the real transfer runs over the serial FBUS/MBUS transport
  (`symbinux-transport`), driven by the CLI's `identify`/`getphonebook`/etc.
- **Android** — a real transfer wraps the ADB protocol. The pure-Rust
  `adb_client` crate (with its `usb` feature) speaks ADB directly to the device;
  AOA mode uses vendor control requests `51/52/53` on endpoint 0. Not bundled by
  default to avoid the extra dependency.
- **Apple iOS** — **requires the `usbmuxd` daemon** (Unix socket
  `/var/run/usbmuxd`) plus pairing/trust and a TLS lockdown session; lockdownd
  listens on device TCP `62078`. This is why we **link libimobiledevice / the
  pure-Rust `idevice` crate** rather than reimplement the mux + trust + TLS
  stack. usbmuxd ships its own udev rule and systemd activation — install the
  system package (see `udev/README.md`). CONFIRMED.

## 5. Safety & fallback

- Unknown devices are never assumed; they expose only `raw-sniff` (a dump of
  interfaces/endpoints) to help extend recognition, consistent with the
  community approach used for the Nokia protocol notes.
- No firmware/flash operations are performed by any handler.

## 6. Sources

gnokii/gammu (Nokia), source.android.com AOA 1.0/2.0 and AOSP `adb.h` /
`fastboot` (Android), USB-IF class codes (PTP), libimobiledevice/usbmuxd
`usb.h` / `39-usbmuxd.rules.in` (Apple), Linux kernel sysfs-bus-usb ABI and
`docs.rs/nusb` (enumeration / physical port path).
