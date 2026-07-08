# Vanilla connectivity: talking to phones and watches with no software installed

What Symbinux can do with a **stock** smartphone / smartwatch / smart device —
nothing installed on it, only what the device's OS exposes natively — over
Bluetooth and Wi-Fi. Developer-facing reference (English only).

"Vanilla" here means: the only requirements allowed are the ones built into
the device's firmware/OS (pairing consent prompts included). Anything that
needs a companion app on the phone is out of scope for this document.

## 1. Bluetooth classic (BR/EDR) — the rich, standardised surface

Bluetooth classic profiles are the closest thing to a universal, no-app API
for phones. This is exactly the "car kit" surface: car head units read
contacts and SMS from any phone brand with zero software installed on the
phone. The trust model is a **pairing** + per-profile **consent prompt** on
the phone.

| Profile | What it gives us | Android | iPhone | Legacy Nokia/Symbian |
|---|---|---|---|---|
| **SDP** (service discovery) | list of RFCOMM services a device offers — free reconnaissance after (often before) pairing | ✅ | ✅ | ✅ |
| **OPP** (Object Push) | push/receive files, vCards, vCal | ✅ send+receive | ❌ (iOS has no OPP; AirDrop is proprietary/AWDL) | ✅ |
| **PBAP** (Phone Book Access) | pull the whole phonebook + call history as vCards | ✅ (consent prompt) | ✅ (consent prompt; the "sync contacts" car-kit path) | partially (later Symbian) |
| **MAP** (Message Access) | read/notify (and on Android send) SMS | ✅ | ⚠️ effectively car-kit oriented; iOS gates it hard | ❌ (Nokia legacy uses FBUS/AT instead) |
| **HFP/HSP** | calls, audio gateway | ✅ | ✅ | ✅ |
| **A2DP/AVRCP** | audio streaming / media control + metadata | ✅ | ✅ | later models |
| **SPP / raw RFCOMM** | arbitrary serial channels — this is where **FBUS-over-Bluetooth** and OBEX-based **SyncML** live on legacy Nokia | vendor-specific | ❌ | ✅ (the mission target) |

Key implications for us:

- **PBAP is the universal contacts path** — one client implementation covers
  Android, iPhone and many feature phones. Our Linux path (BlueZ + obexd)
  already speaks it; Phase 4 of `CROSS_PLATFORM_GUI_PLAN.md` ports the OBEX
  client to Windows/macOS over each OS's RFCOMM API.
- **MAP is the SMS path on Android** without any app; expect it to be
  unavailable or crippled against iPhones outside car-kit contexts.
- **OPP receive** lets a stock Android push us a contact/file with two taps
  ("Share → Bluetooth") — cheap ingest channel, no pairing UI beyond confirm.
- Everything above requires classic BR/EDR — **BLE-only stacks (btleplug) can
  not reach any of it.**

## 2. Bluetooth Low Energy — reconnaissance for free, data behind pairing

What a stock phone/watch exposes over BLE:

- **Advertisements (no connection, no consent):** local name (often empty on
  phones), **manufacturer-specific data** (AD type 0xFF, first two bytes =
  Bluetooth SIG **company identifier**), advertised **service UUIDs**, TX
  power, RSSI. This is the layer our scan already sees and is the main
  input for the device classification in §4.
- **GATT after connecting (usually no pairing for these):** GAP **Device
  Name** (0x2A00) and **Appearance** (0x2A01), **Device Information Service**
  (0x180A — manufacturer/model, phones often blank it), **Battery** (0x180F),
  **Current Time** (0x1805).
- **ANCS** (Apple Notification Center Service): iPhones expose incoming
  notifications to a *paired* BLE peripheral. Powerful, but the phone must
  initiate/accept an encrypted bond — still "no app", but not silent.
- **Privacy note:** modern phones advertise with **resolvable private
  addresses** — the MAC rotates every few minutes, so a BLE address is not a
  stable device identity unless bonded. Don't key long-term state on it.

Smartwatches: the Apple Watch pairs exclusively with iPhones and Wear OS
watches pair through a companion app, so their vanilla surface is
essentially the advertisement + the public GATT services above (battery,
device info when not blanked). Fitness bands vary; many expose standard
Heart Rate (0x180D) only while in pairing mode.

## 3. Wi-Fi — phones are silent; TVs and media devices are chatty

A stock phone on the LAN **listens to nothing** by default: no inbound API,
no discovery beacon that promises a dialogue. Wi-Fi "vanilla" dialogue is
really about **service discovery protocols**, and the devices that answer
are mostly TVs, casting targets and set-top boxes:

- **mDNS / DNS-SD (Bonjour):** multicast to `224.0.0.251:5353`, browse
  service types: `_googlecast._tcp` (Chromecast / Android TV),
  `_airplay._tcp` + `_raop._tcp` (Apple TV / AirPlay speakers),
  `_spotify-connect._tcp`, `_printer._tcp`, `_ipp._tcp`,
  `_companion-link._tcp` (Apple devices). The **TXT records** carry
  identification: `md=` (model, Googlecast), `am=` (Apple model),
  `fn=`/`n=` (friendly name).
- **SSDP / UPnP:** multicast `M-SEARCH` to `239.255.255.250:1900`; answers
  carry `SERVER` (OS/stack), `ST`/`USN` (device type urn, UUID) and a
  `LOCATION` URL whose XML gives `friendlyName`, `manufacturer`,
  `modelName`. Smart TVs and media renderers virtually always answer.
- **Phones appear only while a feature is active** (e.g. a phone casting or
  acting as a media server). AirDrop uses **AWDL** (Apple's own peer-to-peer
  Wi-Fi), unreachable from a normal LAN adapter. **Wi-Fi Direct / Miracast**
  are P2P modes that need explicit user action on the phone each time.
- **MAC OUI** (first 3 bytes of a device's MAC on the LAN) still identifies
  the vendor, subject to the same randomisation caveat as BLE.

Bottom line: on Wi-Fi the honest vanilla feature is **LAN service
discovery** (mDNS+SSDP) — it finds TVs, casting targets, printers and the
occasional phone-with-a-feature-on, each with a name and a model string.
[`mdns-sd`](https://crates.io/crates/mdns-sd) is a pure-Rust implementation
that fits our no-C-deps rule; SSDP is trivial UDP. A future
"LAN devices" upgrade of the Wi-Fi channel can build on that (see §5).

## 4. Identification signals (what drives the list icons)

Ordered by reliability; the classifier combines them:

| Signal | Where | Tells us | Notes |
|---|---|---|---|
| **Class of Device (CoD)** | BT classic inquiry; BlueZ `Class`/`Icon` | form factor: major class 5 bits (1 computer, 2 **phone**, 4 audio/video, 7 **wearable**), minor 6 bits (phone/3 = smartphone; wearable/1 = **watch**; A/V minors include headphones, portable audio) | the most trustworthy form-factor bit for classic devices |
| **GAP Appearance** (0x2A01 / AD type 0x19) | BLE advert or GATT | category = value >> 6: 1 **phone**, 2 computer, 3 **watch**, 5 display, 10 media player | set by watches/bands more often than by phones |
| **Company identifier** | BLE manufacturer data (first 2 LE bytes) | vendor: `0x004C` Apple, `0x0006` Microsoft, `0x00E0` Google, `0x0075` Samsung, `0x0087` Garmin, `0x012D` Sony, `0x027D` Huawei, `0x038F` Xiaomi, `0x0059` Nordic | vendor ≠ form factor: `0x004C` is iPhone *and* Watch *and* AirPods |
| **Advertised service UUIDs** | BLE advert | 0x180D heart rate → band/watch; 0x1812 HID → input device; Fast Pair `0xFE2C` → Android ecosystem accessory | |
| **Device name** | classic + BLE + mDNS | "iPhone di …", "Galaxy Watch…", "[TV] Samsung…" | user-editable, locale-dependent — use as a hint, last |
| **mDNS service type + TXT** | Wi-Fi | `_googlecast._tcp` + `md=` model → TV/cast target; `_airplay._tcp` + `am=` → Apple device | strongest Wi-Fi signal |
| **SSDP LOCATION XML** | Wi-Fi | `manufacturer`, `modelName`, `deviceType` urn | TVs/renderers |
| **MAC OUI** | any | vendor | defeated by address randomisation |

## 5. What this means for Symbinux

Now (this iteration):
- Enrich the Bluetooth list rows with **vendor + form factor** from the
  signals above — BlueZ `Icon`/`Class` + name heuristics on Linux;
  company-ID + service UUIDs + name heuristics from `btleplug` adverts on
  Windows/macOS — rendered as combined badges (vendor icon + form-factor
  icon) next to the friendly name.

Next (separate sessions, in plan order):
- **Phase 4 (classic BT per OS)** unlocks the real dialogue surface of §1
  (PBAP/OPP/MAP/FBUS-over-RFCOMM) on Windows/macOS; Linux already has it.
- **LAN discovery channel**: mDNS (`mdns-sd`, pure Rust) + SSDP sweep as the
  Wi-Fi channel's "devices" view — named, typed entries (TVs, cast targets,
  printers) instead of bare SSIDs. Complements, not replaces, the network
  list.
- **BLE GATT probe** (opt-in per device): connect and read Device Name /
  Appearance / Battery / Device Information for a richer detail card.

## References

- [List of Bluetooth profiles (overview)](https://en.wikipedia.org/wiki/List_of_Bluetooth_profiles)
- [Bluetooth SIG Assigned Numbers (company IDs, appearance values)](https://www.bluetooth.com/specifications/assigned-numbers/)
- [Nordic bluetooth-numbers-database (machine-readable IDs/UUIDs)](https://github.com/NordicSemiconductor/bluetooth-numbers-database)
- [Class of Device bit layout](https://www.ampedrftech.com/guides/cod_definition.pdf)
- [Android BluetoothClass.Device constants (CoD minors)](https://developer.android.com/reference/android/bluetooth/BluetoothClass.Device)
- [Manufacturer-specific data in BLE advertising](https://bleadvertiserapp.medium.com/manufacturer-specific-data-in-ble-advertising-how-it-works-7cdbade71581)
- [BBC device discovery & pairing study (mDNS/SSDP on real devices)](https://github.com/bbc/device-discovery-pairing/blob/master/document.md)
- [nOBEX — PBAP/MAP/HFP emulation used to test car kits](https://github.com/nccgroup/nOBEX)
