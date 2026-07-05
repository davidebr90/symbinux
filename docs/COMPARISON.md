# Prior art & feature backlog

A survey of comparable open-source projects and the concrete ideas worth
adopting, from a multi-project review. Sources are linked inline. This is
developer-facing reference material (English only).

## Where Symbinux sits

A cross-project search found **no existing Rust implementation of the Nokia
FBUS/MBUS protocols** — gnokii (C) remains the only real PC-side reference, and
it is dormant. So the Symbinux protocol core appears to be a genuine first, and
the "modern pure-Rust reimplementation of a legacy phone protocol" strategy has
a direct sibling precedent in [`idevice`](https://github.com/jkcoxson/idevice)
(a pure-Rust reimplementation of libimobiledevice for iOS). The per-platform
handler dispatch we use (a `DeviceHandler` trait per platform) is exactly the
pattern recommended over runtime plugin loading in Rust.

## Projects reviewed

| Project | Domain | License | Takeaway |
|---|---|---|---|
| [gnokii](https://github.com/pkot/gnokii) | Nokia FBUS/MBUS/AT | GPL | The feature yardstick: phonebook (raw/vCard/LDIF), SMS folders, calendar/todo (vCal), logos, ringtones (RTTTL), phone filesystem, call management, netmonitor, profiles, WAP settings. |
| [gammu / Wammu](https://github.com/gammu/gammu) | Nokia + multi-vendor | GPLv2 | Unified `.backup` format for all data; **gammu-smsd** daemon with SQL backends; MMS; contact/todo categories; ringtone format conversion. Protocol docs at [docs.gammu.org](https://docs.gammu.org/protocol/nokia.html). |
| [gnome-phone-manager](https://wiki.gnome.org/Attic(2f)PhoneManager.html) | GNOME SMS app | GPL | Prior art for our exact niche: built on gnokii, SMS + Evolution address-book integration. Validates the backend-library/GUI split. |
| [libimobiledevice](https://github.com/libimobiledevice/libimobiledevice) / [idevice (Rust)](https://github.com/jkcoxson/idevice) / [go-ios](https://github.com/danielpaulus/go-ios) | Apple iOS | LGPL / MIT / MIT | Pairing/trust handshake is foundational; backup/restore, syslog, diagnostics, app install. **go-ios's explicit design rule: "all output as JSON."** |
| [libmtp](https://github.com/libmtp/libmtp) / [android-file-transfer-linux](https://github.com/whoozle/android-file-transfer-linux) / [adb_client (Rust)](https://github.com/cocool97/adb_client) / [scrcpy](https://github.com/Genymobile/scrcpy) | Android | LGPL / LGPL / MIT / Apache-2 | MTP file transfer; embed `adb_client` (pure Rust) instead of shelling to `adb`; scrcpy for screen mirroring; wireless (mDNS) ADB pairing. |
| [KDE Connect](https://invent.kde.org/network/kdeconnect-kde) | Phone↔desktop | GPLv2/3 | Formal **plugin architecture** + a **D-Bus service** other apps consume; network discovery + TLS trust-on-first-use pairing (no cable). |
| [gvfs](https://gitlab.gnome.org/GNOME/gvfs) (mtp/gphoto2 backends) | GNOME VFS | LGPL-2 | Expose the phone as a **GVfs mount** so it appears natively in Files; reuse libmtp/libgphoto2 for Android/PTP rather than reimplementing. |
| [obexd (BlueZ)](https://github.com/bluez/bluez/tree/master/obexd) + [OpenOBEX/obexftp](https://github.com/zuckschwerdt/obexftp) | Bluetooth OBEX | GPLv2 / LGPL | **PBAP** (contacts as vCard) and **MAP** (SMS) over Bluetooth via BlueZ D-Bus — the cable-free way to reach old Nokias whose USB is dead but Bluetooth works. |
| [nusb](https://github.com/kevinmehall/nusb), [usb-ids](https://crates.io/crates/usb-ids), [zbus](https://github.com/dbus2/zbus) | Rust infra | MIT/Apache | `nusb` = pure-Rust USB (drop the libusb C dependency); `usb-ids` = canonical device names; `zbus` = expose a D-Bus service. |

## Prioritized ideas to adopt

**HIGH value**
- **Typed decoding → vCard/iCalendar/SMS.** gnokii/gammu expose phonebook, SMS, calendar as standard `.vcf`/`.vcs`/`.ics`. We now decode HW/SW version; phonebook→vCard and SMS PDU decode are the next unlocks. (gnokii/gammu)
- **`--json` output everywhere.** go-ios's "all output as JSON" rule; we added it to `devices`/`detect`. Extend to every command so the GUI and scripts consume structured data, not scraped text.
- **Bluetooth PBAP/MAP transport.** Reach cable-dead Nokias over Bluetooth for contacts/SMS via BlueZ `obexd` D-Bus. Complements the existing Bluetooth *scan*. (obexd)
- **Backup/restore bundle.** A single command dumping phonebook + SMS (+ calendar) — the natural payoff of typed decoding. (gammu `.backup`)

**MEDIUM value**
- **Call log, calendar/todo, ringtones, logos.** Straightforward once the phonebook-read framing is generalised (gnokii wires these on the same `0x03`/security families).
- **Embed `adb_client` (Rust)** for the Android handler instead of shelling out — native errors, no external `adb`, free wireless pairing. Link `idevice` (Rust) for iOS instead of the C stack.
- **Expose a D-Bus service** (zbus) so the GUI and other desktop apps talk to a persistent daemon with live hotplug events, KDE-Connect style.
- **`usb-ids` for device names** in the classifier, instead of relying on descriptor strings only.

**LOW value / research-first**
- GVfs backend to mount the phone in Files (Android/PTP path; reuse libmtp).
- scrcpy-style screen mirroring (Android) — large, out of core scope.
- Network/Wi-Fi discovery + TLS pairing (KDE Connect model) — only relevant if we add a phone-side agent, which legacy Nokias can't run.

See `docs/ROADMAP.md` for how these map onto the planned work, and
`docs/CROSS_PLATFORM.md` for the portability picture.
