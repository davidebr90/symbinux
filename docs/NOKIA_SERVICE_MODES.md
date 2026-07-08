# Nokia service modes & channels (NSS study)

What we learned from studying **Nemesis Service Suite (NSS)** and the wider
DCT4/BB5 service-tool ecosystem, and what of it is worth building into Symbinux
for **reading data, settings and contacts** off abandoned Nokia phones. This is
developer-facing reference material (English only).

**Scope & ethics.** These devices are long abandoned by the manufacturer; the
goal is **data recovery and interoperability from a device the owner controls**
— contacts, messages, calendar, settings, identifiers. Everything here is
reconstructed clean-room from open documentation (gnokii/gammu, the Linux
kernel PhoNet driver, public service-tool notes), never from Nokia proprietary
software or reverse-engineered binaries. **Writes that can brick or that touch
locks/identity (flashing, PM writes, SIM-lock, product-code, IMEI) are out of
scope** — read-only recovery only. This continues the safety line already in
`ROADMAP.md` and `docs/PROTOCOL_NOTES.md`.

Confidence tags match `PROTOCOL_NOTES.md`: **CONFIRMED** (gnokii/gammu source
or ≥2 independent sources), **LIKELY** (one credible source), **UNCERTAIN**
(inference — verify on hardware before relying on it).

## 1. What NSS actually is (and what we take from it)

NSS is a Windows service suite for DCT4/BB5 Nokia phones. Its feature set
(from public release notes and tutorials): **phone info**, **product-code
change**, **Permanent Memory (PM) read/write**, **SIM-lock** operations,
**warranty/lifetimer**, **security-area rebuild**, and **flashing**. It talks
to the phone over **FBUS** (DKU / S-FBUS cable) and over **USB** in the phone's
"PC Suite"/service USB profile. CONFIRMED (release notes, tutorials).

What is useful to us is **not** the flashing/unlock machinery (dangerous,
out of scope) but the **map of channels and services** it exercises:

- the phone speaks the **same message-based protocol** over cable-FBUS and
  over USB — we already own the FBUS/2 codec, so most of the value is reaching
  the *services*, not new framing;
- there are **two operating modes** with different service surfaces
  (normal vs local/test, §3);
- **Permanent Memory** is a structured, per-field store we can **read** for
  recovery (identifiers, settings, calibration) even when the phone's normal
  UI is dead (§4).

NSS itself is not something we integrate or ship; it is a **specification by
example** of what the hardware exposes.

## 2. Transport channels to the phone

| Channel | Medium | Symbinux status | Confidence |
|---|---|---|---|
| Cable **FBUS/2** (DKU-5/CA-42/S-FBUS) | serial over USB-serial bridge → `/dev/ttyUSB*` / `COMn` | **have it** (`symbinux-transport` serial) | CONFIRMED |
| **DKU-2 native USB** (Series 40/60), VID `0x0421` | raw USB bulk, app-owned claim | **have it** (`nusb`, `identify --usb`) | CONFIRMED |
| **PhoNet over USB** (the "PC Suite" USB profile) | USB CDC-PhoNet interface carrying the message protocol | **the main under-used channel** (see §5) | LIKELY |
| MBUS v1 | slow 2-wire serial | have codec, unvalidated on HW | LIKELY |
| Bluetooth SPP/RFCOMM (FBUS-over-BT) | classic RFCOMM | planned (Phase 4, `CROSS_PLATFORM_GUI_PLAN.md`) | LIKELY |

**PhoNet** is the key insight from the NSS/PC-Suite side. Phonet is Nokia's
packet protocol for IPC/RPC; the "PC Suite" USB profile exposes a **CDC-PhoNet
interface** (the Linux kernel ships `cdc-phonet` for exactly this) carrying the
same request/response services as cable-FBUS. Reaching it the **app-owned** way
— claim the USB interface with `nusb` and speak the framing ourselves, no OS
serial driver — is a direct fit for our connection model
(`docs/CONNECTION_MODEL.md`) and stays cross-platform and libusb-free.
CONFIRMED that the interface exists (kernel driver + LWN article); LIKELY that
our FBUS service messages map onto it with minor framing differences — **verify
on hardware**.

## 3. Operating modes = different service surfaces

A Nokia phone answers a **different set of services depending on the mode it
booted in**. CONFIRMED that the modes exist; the exact per-mode service list is
LIKELY/UNCERTAIN and must be probed per model.

- **Normal mode** — the phone runs its normal firmware and exposes the
  **PC-Suite service catalog**: phonebook, SMS, calendar/to-do, filesystem,
  identification, profile/settings. This is the **primary data-recovery
  surface** and where contacts/messages/calendar live.
- **Local mode** — a service state (historically entered by a **resistor in
  the service cable** pulling a pin, or by a command) that exposes
  **service-level** functions: PM access, self-tests, RF, and — on many models
  — the **network monitor**. This is where identifiers/settings/calibration in
  PM are reachable even if the normal UI won't boot. CONFIRMED (multiple
  service-tool sources).
- **Test mode** — deeper diagnostic state for RF/hardware self-tests; not
  needed for data recovery.

Practical consequence for Symbinux: a robust recovery flow should **detect the
mode** and, for a phone that only reaches local mode, fall back to **PM reads**
(§4) for whatever identity/settings it can still surface, while the rich PIM
data (§4 table) needs normal mode.

## 4. The service catalog worth targeting (read-only)

Message families are the FBUS `MsgType` values gnokii/gammu use; we already
implement a subset (see `PROTOCOL_NOTES.md` §5). Ordered by recovery value:

| Data | MsgType (family) | What we get | Symbinux status | Confidence |
|---|---|---|---|---|
| **Phonebook / contacts** | `0x03` | names + numbers per memory/location → vCard | **have read** (`getphonebook`, decoder tested) | CONFIRMED |
| **SMS / messages** | `0x02`/`0x14` | inbox/sent/folders → decode PDU (3GPP) | SMS-DELIVER decoder done; **response parsing pending HW** | LIKELY |
| **Calendar / to-do** | `0x13` | events/todos → iCal `.ics` | not yet | LIKELY |
| **Filesystem** | `0x6D` (n6510-class) | browse & pull files (media, `.vcf`/`.vmg`, settings) on Series 40/60 | not yet | LIKELY |
| **Identification** | `0x1B`/`0xD1` | model/HW/SW (already decoded via `0x1B`/HW-SW reply) | **have** (`identify`) | CONFIRMED |
| **IMEI** | `0x40` (`66`) | serial number | **have** | CONFIRMED |
| **Network monitor** | `0x40` (`7E <field>`) | cell/RF field values (cable/local mode) | **have** (`netmon`) | CONFIRMED |
| **Permanent Memory read** | PM per-field | identity/settings/calibration blocks (§ below) | not yet — **read-only target** | LIKELY |

Not everything lives in the same place: **contacts/SMS/calendar are in the
phone's memory/filesystem** (normal-mode services), while **identifiers,
locks, calibration and counters live in Permanent Memory** (local-mode /
PM access). A complete recovery tool needs both.

### Permanent Memory (PM) — read for recovery

PM is a **numbered set of fields/blocks**, each a service store. Publicly
documented fields (LIKELY; numbering varies by platform — **verify per model**):

- **field 1** — RF tuning / calibration (protected).
- **IMEI**, **product code**, **SIM-lock** data (SIM-lock commonly in the
  PM 120 area), **Bluetooth/WLAN MAC**, **lifetimer** (total call-time
  counter, the `lifetimer.pm` NSS writes/reads).

For Symbinux this is a **read-only** target: dumping PM fields recovers the
phone's identity and settings snapshot (product code, MACs, lock status,
lifetimer) as diagnostic/forensic data. **We never write PM** — PM writes are
how NSS changes product codes / removes SIM-locks and are exactly the
brick/legality risk we exclude.

## 5. What to build in Symbinux (backlog, read-only)

Mapped to our crates, in rough priority. None re-introduces libusb; all are
reads.

1. **PhoNet-over-USB service channel** (`symbinux-transport`): claim the
   CDC-PhoNet interface with `nusb` and carry our FBUS service messages over
   it, so the full catalog works on the app-owned USB path without a serial
   driver — the biggest reach-extension, and cross-platform. *Spike first:*
   confirm the framing delta vs cable-FBUS on real hardware.
2. **Mode detection** (`symbinux-devices`/CLI): report normal vs local/test so
   the GUI can steer recovery (rich PIM in normal mode; PM/identity in local
   mode).
3. **Calendar/to-do read → iCal** (`symbinux-protocol::decode` + CLI/GUI):
   extend the typed decoder (`0x13`) the way phonebook/SMS already are.
4. **Filesystem browse & pull** (`0x6D` family): list and download files
   (media, `.vcf`/`.vmg`, settings) on Series 40/60 — a second, richer
   recovery path than per-record reads.
5. **PM field read + dump** (CLI `pm-read <field>` / `pm-dump`): read-only
   identity/settings/calibration recovery, with each field tagged by
   confidence; **no write path exposed**.
6. **Backup bundle** (already on the roadmap): one command dumping
   phonebook + SMS + calendar (+ PM identity) to `.vcf`/`.vmg`/`.ics`/`.json`.

Each item starts with a hardware spike (the confidence tags above say why) and
lands only behind the existing green gate. Writes to the phone stay gated as
`Experimental` and PM/lock/flash writes remain **out of scope**.

## References

- [Nemesis Service Suite release history (feature set: PM, product code, security area)](https://www.scribd.com/document/74633034/Nemesis-Service-Suite)
- [NSS PM read/write & product-code tutorial](https://mobfunda.wordpress.com/2010/09/07/nemesis-service-suite-beta-1-0-38-15-its-tutorials-change-ur-product-code/)
- [Gammu — Nokia protocols (FBUS/MBUS framing, Service Software 0xD0)](https://docs.gammu.org/protocol/nokia.html)
- [Gammu — Nokia 6510 driver (calendar/to-do, SMS folders, filesystem)](https://docs.gammu.org/protocol/n6510.html)
- [Linux kernel PhoNet documentation](https://www.kernel.org/doc/Documentation/networking/phonet.txt)
- [USB host CDC-Phonet network interface (PC Suite USB profile)](https://lwn.net/Articles/342908/)
- [What is a Nokia protocol? FBUS/MBUS/AT overview](https://forum.gsmhosting.com/vbb/f83/what-exactly-nokia-protocol-fbus-mbus-command-430526/)
- [NuukiaWorld — FBUS/MBUS cable basics](https://panuworld.net/nuukiaworld/hardware/cables/basics.htm)
- [Nokia network monitor / field test mode (cable-activated)](https://en.wikipedia.org/wiki/Nokia_network_monitor)
- [BB5 local/test mode over USB (service-tool discussion)](https://forum.gsmhosting.com/vbb/f311/how-put-local-mode-nokia-bb5-usb-1416246/)
- [Nokia Permanent Memory fields (forum reference)](http://forum.gsmhosting.com/vbb/f83/nokia-permanent-memory-1080640/)
