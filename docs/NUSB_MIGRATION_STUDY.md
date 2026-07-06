# nusb migration study (rusb/libusb → nusb)

> Status: **research / decision document — no code changed yet.**
> Purpose: decide, with eyes open, whether and how to replace the `rusb`
> (libusb C binding) USB layer with the pure-Rust [`nusb`](https://docs.rs/nusb)
> crate, covering the API mapping, the async/sync bridge, per-OS behaviour and
> the edge cases that could bite on real hardware.

## 1. Why consider this at all

Today the raw-USB path depends on **libusb** through the `rusb` binding:

- `crates/symbinux-transport/src/enumerate.rs` — lsusb-style listing.
- `crates/symbinux-transport/src/usb.rs` — `UsbTransport::open_fbus_auto`
  (app-owned claim + kernel-driver detach + bulk endpoint discovery + bulk I/O).
- `crates/symbinux-transport/src/lib.rs` — `TransportError::Usb(rusb::Error)`.
- `crates/symbinux-devices/src/enumerate.rs` — USB fingerprint listing.

libusb is a **C library**: it must be present at build and (for dynamic linking)
at run time. That means either bundling `libusb-1.0.{dll,dylib,so}` with the
Windows/macOS builds or asking users to install it, and it complicates the
cross-compiled release artifacts. `nusb` is **pure Rust** with no C dependency,
so the goal is a genuinely self-contained binary per platform.

This is a *quality-of-distribution* change, not a feature. It should only land if
it is a clean net win (build + tests + clippy all green) and does not regress any
platform we currently support.

## 2. nusb at a glance

- Version studied: **0.2.4** (released 2026-06-21). Pure Rust, MIT/Apache-2.0.
- Backends: **Linux** (usbfs/sysfs), **Windows** (WinUSB via SetupAPI), **macOS**
  (IOKit / IOUSBHost). No `Context` object — you open devices directly.
- **Async-first, but blocking-capable.** Most fallible operations return a
  `MaybeFuture`, which you can either `.await` (async) *or* `.wait()` (blocks the
  current thread). Endpoint bulk I/O additionally exposes `EndpointRead` /
  `EndpointWrite` wrappers that implement `std::io::Read` / `std::io::Write`.

### 2.1 Consequence for us: **no executor dependency needed**

Earlier I assumed nusb was async-only and that we'd have to add a `block_on`
executor (`pollster`/`futures-lite`) to bridge it into our synchronous
`Transport` trait. **That assumption was wrong for 0.2.x.** The native `.wait()`
and the `Read`/`Write` endpoint wrappers cover our synchronous needs directly, so
the bridge is a non-issue. This lowers the risk and the dependency footprint of
the migration.

## 3. API mapping (rusb → nusb 0.2.x)

| Concern | rusb (today) | nusb 0.2.x |
|---|---|---|
| Enumerate | `rusb::devices()` → iterate `Device` | `nusb::list_devices()?` → iterate `DeviceInfo` |
| VID/PID/serial | `device_descriptor()` then fields | `info.vendor_id()`, `info.product_id()`, `info.serial_number()`, `info.manufacturer_string()`, `info.product_string()` (already cached — **no open needed**) |
| Open | `device.open()?` | `info.open().wait()?` → `Device` |
| Active config | `handle.active_configuration()?` | `device.active_configuration()?` (returns a parsed `Configuration`) |
| Claim interface | `handle.claim_interface(n)?` | `device.claim_interface(n).wait()?` → `Interface` |
| Detach kernel driver (Linux) | `handle.set_auto_detach_kernel_driver(true)` + `detach_kernel_driver(n)` | `device.detach_and_claim_interface(n).wait()?` (single call; re-attaches on drop) |
| Descriptors | manual `config_descriptor`/`interface`/`endpoint_descriptor` walk | `config.interfaces()` → `InterfaceDescriptor` → `.endpoints()` → `EndpointDescriptor` |
| Endpoint handle | (implicit, transfer by address) | `interface.endpoint::<Bulk, In>(addr)?` / `::<Bulk, Out>(addr)?` → `Endpoint` |
| Bulk read (blocking) | `handle.read_bulk(addr, buf, timeout)?` | `EndpointRead` (impl `std::io::Read`), or `endpoint.submit(Buffer)` + `endpoint.next_complete().wait()` → `Completion` |
| Bulk write (blocking) | `handle.write_bulk(addr, buf, timeout)?` | `EndpointWrite` (impl `std::io::Write`), or submit/`next_complete` |
| Control transfer | `handle.read_control(...)` | `interface.control_in(ControlIn, timeout).wait()?` / `control_out(...)` |
| Error type | `rusb::Error` | `nusb::Error` (open/claim) and `nusb::transfer::TransferError` (I/O) — **two error types**, see §6 |

### 3.1 Endpoint discovery (`pick_bulk_pair`) still works

Our `pick_bulk_pair` logic (find one bulk-IN + one bulk-OUT endpoint) maps
directly onto `config.interfaces() → descriptor.endpoints()`, filtering on
`endpoint.transfer_type() == Bulk` and `endpoint.direction()`. The address we
extract is what we pass to `interface.endpoint::<Bulk, Dir>(addr)`.

### 3.2 The one real design change: the transfer model

rusb's `read_bulk`/`write_bulk` are one-shot blocking calls with a timeout.
nusb's fundamental model is a **submit/complete queue** (`Buffer` in, `Completion`
out via `next_complete`). The `EndpointRead`/`EndpointWrite` `std::io` wrappers
hide that behind a familiar blocking interface, which is what we want for the
FBUS request/response loop. **Open question to settle in implementation:**
whether the `Read`/`Write` wrappers expose a per-call **timeout** the way
`read_bulk(timeout)` does. Our exchange layer already has its own
`ExchangeConfig` timeout/retry logic at the frame level, so even if the
low-level read blocks without a native timeout we can drive cancellation from
above — but this must be verified, not assumed (it is the single most likely
source of a behavioural regression).

## 4. Per-OS analysis and edge cases

This is the part that matters most, because the USB bulk path is **not testable
in CI or WSL** — it only exercises on real hardware, per OS.

### 4.1 Linux — primary raw-USB target ✅ (lowest risk)

- Access goes through **usbfs**; our existing udev rules
  (`69-nokia-legacy.rules`, `51-android.rules`) already grant unprivileged
  access by VID, so nothing changes for the user.
- **Kernel driver detach**: legacy Nokia cables often bind `cdc-acm` or a
  pl2303/ftdi/cp210x serial driver. rusb needs an explicit detach; nusb folds it
  into `detach_and_claim_interface(n)` and, importantly, **re-attaches the kernel
  driver when the `Interface`/`Device` is dropped**. This matches our
  "app owns the connection, then gives it back" model in `CONNECTION_MODEL.md`.
- Edge case: if the interface is claimed by another process (ModemManager is the
  classic culprit on desktop Linux — it grabs `cdc-acm` modems), the claim fails.
  We should surface a hint ("ModemManager may be holding the port; try
  `mmcli`/udev `ID_MM_DEVICE_IGNORE`"). This is already partly covered by our
  `_augment_error` hints on the GUI side; extend the Rust error mapping to say
  *which* interface was busy.

### 4.2 Windows — the big caveat ⚠️ (design constraint, not a regression)

- **nusb on Windows requires the target device/interface to be bound to the
  WinUSB driver.** If a device uses a vendor driver or the inbox serial/CDC
  driver, nusb's raw path cannot open it — you'd need a WCID descriptor (device
  firmware side, impossible for 20-year-old Nokias) or a manual driver swap via
  **Zadig/libwdi**. Swapping the driver breaks the phone's normal COM-port
  behaviour, so this is user-hostile for our use case.
- **This is *not* a nusb regression:** libusb on Windows has the *exact same*
  requirement (WinUSB/libusbK/libusb-win32 backend). So `rusb` today is equally
  unusable against an inbox-serial Nokia on Windows.
- **Why it doesn't hurt us:** our Windows story for legacy phones is the
  **serial `Transport`** (the `serialport` crate → the COM port the vendor/CDC
  driver exposes), *not* the raw-USB `UsbTransport`. The raw-USB path is a
  Linux-first capability. So on Windows the migration changes nothing that users
  actually rely on — both before and after, raw USB needs Zadig, and nobody is
  asked to do that because they use the COM port instead.
- Known nusb Windows wrinkle to watch: discussion
  [#43](https://github.com/kevinmehall/nusb/discussions/43) reports a case where
  `control_in` worked but `bulk_in` blocking reads returned `Cancelled` under
  WinUSB for a UVC device, while rusb worked. If we ever *do* want raw USB on
  Windows, this must be smoke-tested on a real WinUSB device before trusting it.
- **Action for the study:** document in `CROSS_PLATFORM.md` that raw USB on
  Windows is "advanced / Zadig-only" for both the old and new backend, and keep
  serial as the supported Windows path.

### 4.3 macOS — medium risk 🟡

- nusb uses **IOKit / IOUSBHost**; no codeless kext or special entitlement is
  needed for user-space USB from a normal app bundle (unlike some historical
  libusb setups). Detach of the default kernel driver is handled by the backend.
- Nokia legacy on macOS also realistically goes through a **usb-serial driver**
  (if one exists for that cable's chipset) → serial `Transport`, same as Windows.
- Edge case: macOS may require the app to be granted USB access on first use;
  and unsigned/ad-hoc-signed CLI binaries can behave differently from a signed
  `.app`. Note this but it is out of scope until we ship a macOS bundle.

## 5. Testability matrix (what actually gets verified)

| Layer | Linux | Windows | macOS |
|---|---|---|---|
| Enumeration (`list_devices`) | CI-testable (no device: empty list, no panic) | same | same |
| Descriptor parse / `pick_bulk_pair` | **unit-testable** with synthetic descriptors (already have such tests) | same | same |
| Error-type mapping | unit-testable | unit-testable | unit-testable |
| Open + claim + **bulk I/O** | **real Nokia + cable only** | Zadig+WinUSB device only | real device only |

Take-away: ~70% of the migration (enumerate, descriptor walk, endpoint
selection, error mapping) is unit-testable and can be landed with confidence. The
remaining bulk-I/O path needs **one real on-device smoke test on Linux** before
we can claim parity with the current rusb path. The plan below reflects that.

## 6. Edge-case catalogue (things to explicitly handle, not discover later)

1. **Two error types.** nusb splits `Error` (open/claim/config) from
   `transfer::TransferError` (I/O: `Cancelled`, `Stall`, `Disconnected`,
   `Fault`, `Unknown`). Our `TransportError::Usb(rusb::Error)` must become either
   two variants or one variant wrapping an enum. Map `Stall` → clear-halt hint,
   `Disconnected` → "phone unplugged", `Cancelled` → timeout/retry path.
2. **Timeout on bulk reads** (§3.2) — verify whether `EndpointRead`/`Endpoint`
   expose one; if not, drive cancellation from the exchange layer.
3. **Kernel-driver re-attach on drop** — confirm nusb re-attaches (it documents
   doing so) so we don't leave the phone's modem interface detached after exit.
   Add an integration note; possibly hold the `Device` for the session lifetime.
4. **String descriptors** — `manufacturer_string()`/`product_string()` are cached
   on `DeviceInfo` **without opening the device** on Linux/Windows; confirm macOS
   populates them too, else fall back to reading them post-open.
5. **Multi-configuration / multi-interface devices** — Nokia DKU/CA cables can
   expose several interfaces; keep our "pick the interface that has the bulk
   pair" heuristic rather than assuming interface 0.
6. **Zero-length packets & short reads** — the FBUS reader already tolerates
   partial frames; make sure a nusb short read maps to "got N bytes" not an error.
7. **Hotplug** — nusb has `watch_devices()`; out of scope for the migration but
   noted as a future GUI improvement (live device list) that nusb makes cheap.
8. **`Buffer` ownership** — nusb's submit/complete model takes ownership of a
   `Buffer` and hands it back in the `Completion`; if we use the raw queue rather
   than the `Read`/`Write` wrappers, pool and reuse buffers to avoid churn.
9. **ModemManager / driver contention on Linux** (§4.1) — map "resource busy" to
   an actionable hint.
10. **Windows WinUSB absence** (§4.2) — the raw path should fail with an
    *explicit, honest* "device not bound to WinUSB; use the serial connection"
    message, never a cryptic backend error.

## 7. Refined migration plan (supersedes the earlier estimate)

Re-scoped from the earlier "M": it is a **medium-to-large** change, but the
async-executor risk is gone (§2.1) and most of it is unit-testable (§5).

| # | Step | Testable? | Notes |
|---|---|---|---|
| 0 | Spike: confirm exact 0.2.4 signatures for `endpoint()`, blocking read/write, and **bulk timeout** on a scratch binary | build-only | Kills the last unknown (§3.2) before touching real code |
| 1 | `Cargo.toml`: drop `rusb`, add `nusb = "0.2"` (no executor dep) | build | Mechanical |
| 2 | `transport/lib.rs`: redesign `TransportError` for the two nusb error types (§6.1) | unit | Do first — everything depends on it |
| 3 | `transport/enumerate.rs`: `list_devices` + cached strings | unit | Low risk |
| 4 | `devices/enumerate.rs`: fingerprint listing | unit | Low risk |
| 5 | `transport/usb.rs`: `open_fbus_auto` (open → `detach_and_claim_interface` → `pick_bulk_pair` over `config.interfaces()` → `endpoint::<Bulk,_>`) + bulk read/write via `Read`/`Write` wrappers | **descriptor/selection unit-testable; bulk I/O NOT** | The core; the only part needing hardware |
| 6 | `fmt --all --check` + `test --workspace` + `clippy --workspace -D warnings` | gate | **Commit only if fully green, else `git checkout` and report** |
| 7 | **Real Linux on-device smoke test** with a Nokia + cable: identify + one bulk round-trip | manual | Sign-off for I/O parity; until then mark the path "migrated, pending hardware validation" in CHANGELOG |
| 8 | Update `CROSS_PLATFORM.md` (Windows raw-USB = Zadig-only, unchanged) and `CHANGELOG` | — | Docs |

Suggested execution: a single focused session, Steps 0–6 in one unit, Step 7
whenever the hardware is on hand. Keep agent fan-out minimal (this is
sequential, edit-heavy work that doesn't parallelise well) to respect the
session/day budget.

## 8. Risk assessment & recommendation

- **Upside:** self-contained binaries, no libusb C dependency, `detach_and_claim`
  simplifies the Linux claim path, and `watch_devices()` opens a future hotplug
  feature — all with a *smaller* dependency tree.
- **Downside / risk:** the bulk-I/O path is only provable on real hardware (Linux
  first); the bulk-timeout semantics (§3.2) are the one genuine unknown; Windows
  raw USB stays Zadig-only (but that is already true with rusb, so **no
  regression**).
- **Recommendation:** **Go**, as a dedicated session following §7, with the Step 0
  spike de-risking the timeout question before any real edits, and a hard
  green-only commit gate. Do **not** treat "tests pass in CI" as "works on the
  phone" — the CHANGELOG entry must be honest that on-device I/O validation
  (Step 7) is pending until a Nokia is physically tested, consistent with the
  project's no-fake-status principle.

## 9. Open questions to answer in the spike (Step 0)

1. Does `EndpointRead`/`Endpoint` expose a per-read timeout, or must cancellation
   come from the exchange layer?
2. Exact shape of `Completion` (bytes transferred, error, buffer hand-back) if we
   bypass the `Read`/`Write` wrappers.
3. On macOS, are `manufacturer_string`/`product_string` populated pre-open?
4. Does `detach_and_claim_interface` re-attach reliably on `Drop`, or do we need
   to hold the handle for the whole session?

---

*This document is analysis only. No production code, dependency, or git history
has been modified. Execution is deferred to a dedicated session per §7.*
