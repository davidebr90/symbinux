# FBUS / MBUS protocol notes

Clean-room reconstruction of the Nokia serial protocols, assembled from the
open-source gnokii and gammu projects and cross-checked against documented real
captures. **No proprietary Nokia code, libraries or binaries were used.**

Each fact carries a confidence tag:

- **CONFIRMED** — present in gnokii/gammu source or corroborated by ≥2 independent
  sources, and (for FBUS/2) validated against a real capture with a checksum oracle.
- **LIKELY** — a single credible source.
- **SPERIMENTALE / EXPERIMENTAL** — folklore or inference; must be validated on
  hardware before being trusted.

Primary sources: gammu protocol docs (`docs.gammu.org/protocol`), gammu `fbus2.c`,
gammu/gsm-docs `vendors/nokia/nokia.txt`, the Embedtronics "F-Bus made simple"
writeup, the insidegadgets F-Bus SMS article (Nokia 3310 captures), and the
gnokii DKU-2/libusb documentation.

---

## 1. Transport summary

| Cable / phone | Linux enumeration | Access | Confidence |
|---|---|---|---|
| DKU-5 / CA-42 / clones (FTDI, CP210x, PL2303) | `/dev/ttyUSB*` via `ftdi_sio` / `cp210x` / `pl2303` | serial (termios) | CONFIRMED (that they are ttyUSB); per-unit driver LIKELY |
| DKU-2 native USB (Series 40/60) | raw USB device, VID `0x0421` | libusb bulk endpoints | CONFIRMED |
| BB5 phones | raw USB bulk (PhoNet) | libusb | LIKELY |

Serial line settings for FBUS on cable: **115200 baud, 8N1, no flow control**,
control lines **DTR high / RTS low**. CONFIRMED.

MBUS is single-wire half-duplex: **every transmitted byte is echoed back into the
local RX buffer and must be read and discarded** before the reply. CONFIRMED.

---

## 2. FBUS/2 frame (cable)

```
1E | DestDEV | SrcDEV | MsgType | LenHi | LenLo | <data…> | [pad] | Csum1 | Csum2
```

| Field | Value | Confidence |
|---|---|---|
| Frame id (cable) | `0x1E` | CONFIRMED (`FBUS2_FRAME_ID 0x1e`) |
| Frame id (IrDA) | `0x1C` vs `0x1F` — sources conflict | UNCERTAIN — do not use blind |
| DestDEV | phone `0x00` | CONFIRMED |
| SrcDEV | PC `0x0C` | CONFIRMED |
| Len | 16-bit `LenHi<<8 \| LenLo`; length of `data` | CONFIRMED |
| data | `block ++ [FramesToGo, SeqNo]` for command frames | CONFIRMED |
| FramesToGo | `0x01` = last/only frame; `>1` = fragments follow | CONFIRMED |
| SeqNo | `0x4Y` first block, `0x0Y` continuation, `Y`=0–7 rolling | CONFIRMED |
| pad | one `0x00` iff `Len` is odd (word alignment); **counted by checksums** | CONFIRMED |
| Csum1 | XOR of bytes at **even** indices (0,2,4,…) | CONFIRMED (oracle) |
| Csum2 | XOR of bytes at **odd** indices (1,3,5,…) | CONFIRMED (oracle) |

Both checksums run over the whole frame from the `0x1E` up to and including any
padding byte. The `symbinux-protocol` implementation is validated in
`tests/frames.rs` against the two oracles below.

### Checksum oracles (real Nokia 3310 captures)

```
ACK:               1E 0C 00 7F 00 02 D1 00 | CF 71
HW/SW request:     1E 00 0C D1 00 07 00 01 00 03 00 01 60 00 | 72 D5
```

Recompute: for the ACK, `1E^00^00^D1 = CF` (even indices) and `0C^7F^02^00 = 71`
(odd indices). Any implementation that reproduces `CF 71` and `72 D5` has the
indexing right.

### Acknowledge frame

MsgType `0x7F`. Payload is `[acked_MsgType, acked_SeqNo]`. CONFIRMED.

---

## 3. MBUS v1 (M2BUS) frame

```
DestDEV | SrcDEV | FrameLength | MsgType | <block…> | SeqNo | Csum
```

| Field | Value | Confidence |
|---|---|---|
| (no leading frame-id byte in v1) | — | CONFIRMED |
| DestDEV | phone `0x00` | CONFIRMED |
| SrcDEV | PC normal `0xE4`, wakeup `0xF8` | CONFIRMED |
| FrameLength | length of `block` | LIKELY |
| Csum | XOR of every preceding byte | CONFIRMED |

> There is **no public real capture with a checksum oracle for MBUS v1**, so the
> MBUS fixture in the test suite is **SYNTHETIC** (checksum computed from the
> XOR-all rule). The codec is validated; the on-wire behaviour against a phone
> is not.

M2BUS **v2** (distinct from v1) adds a `0x1F` frame-id and a two-byte LO-first
length; it is out of scope for the current implementation. See ROADMAP.

---

## 4. FBUS init / sync

Send the byte `0x55` (`'U'`) repeatedly before the first frame to let the phone's
UART lock onto the framing (`0x55` = `01010101`, maximum edge density).

- **128×** `0x55` — CONFIRMED (insidegadgets, Embedtronics).
- gammu uses **32×** `0x55` then a `0xC1` terminator — LIKELY (single source; the
  `0xC1` tail is not appended by this implementation).

Anything from ~55 bytes upward works; sending more is harmless.

---

## 5. Message types (6110 FBUS family)

| Op | MsgType | Request `block` | Safety | Confidence |
|---|---|---|---|---|
| HW/SW version | `0xD1` (resp `0xD2`) | `00 03 00` | Confirmed | CONFIRMED |
| Phone info (model/IMEI/HW/SW) | `0x64` | `00 10` | Confirmed | CONFIRMED |
| Get IMEI | `0x40` | `66` | Confirmed | CONFIRMED |
| Read phonebook | `0x03` | `00 01 <mem> <loc> 00` | Confirmed | CONFIRMED |
| Write phonebook | `0x03` | `00 04 <mem> <loc> <nl> <name> <ml> <num> <grp>` | Experimental | CONFIRMED |
| Send SMS | `0x02` | SMS submit block (SMSC + PDU) | Experimental | CONFIRMED (type); payload LIKELY |
| Read SMS | `0x02` | `00 07 02 <loc> 01 64` | Experimental | CONFIRMED |
| SMS folder ops | `0x14` | mark/delete variants | Experimental | CONFIRMED |
| Netmonitor | `0x40` | `7E <field>` (`00`=next, `F0`=reset, `F1`=off) | Confirmed | CONFIRMED |

Memory types: `01` combined, `02` phone (ME), `03` SIM, `05` own, `07` dialled,
`08` missed. CONFIRMED.

> `0x1B` as an identify command (mentioned in some notes) belongs to a *different*
> gnokii phone driver, **not** the 6110 FBUS family. Treat as UNCERTAIN for these
> models; this implementation uses `0xD1`/`0x64`.

---

## 6. Safety

- Write/flash/firmware commands are **not implemented** and are classified
  `Dangerous`; the CLI refuses to send them. Sending a malformed frame to
  out-of-support hardware can in theory brick it.
- Only the `Confirmed` read commands are exercised by default. `Experimental`
  commands (phonebook write, SMS) modify the phone and require explicit opt-in.
- Raw-frame mode (`symbinux-fbus raw`) requires `--i-understand-risk`.

---

## 7. Open questions to validate on hardware

- IrDA frame-id byte (`0x1C` vs `0x1F`).
- MBUS v1 real checksum/behaviour (no oracle yet).
- Exact block/trailer split when padding is present on longer frames.
- BB5 bulk endpoint discovery (interface / altsetting per model).

Contributions: capture real traffic with `usbmon` + Wireshark during a known
operation, recompute checksums, and record findings here with a confidence tag.
