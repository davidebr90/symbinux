# D-Bus service plan (`zbus`) — item B8

> Status: **plan / design document — not started.** Execution is a dedicated
> session per §8. Crate versions and exact API names marked *(verify in spike)*
> could not be web-verified while writing this (account session limit); the
> Step 0 spike confirms them against the real crate before any code is written,
> the same way the nusb migration was de-risked.

## 1. Goal

Expose the set of connected devices, their capabilities, and hotplug events over
a **session D-Bus service** built with the [`zbus`](https://docs.rs/zbus) crate,
so the GTK4/libadwaita GUI (and any other app) reads live device state from a
stable IPC surface **instead of shelling out to `symbinux-fbus` and scraping its
output**. This is the KDE-Connect / UPower model: one small daemon owns device
discovery; clients subscribe.

Why it matters:
- The GUI currently spawns `symbinux-fbus devices/detect --json` on a timer and
  parses stdout (`src/symbinux/gui/backend.py`). That is pull-based, latent, and
  re-runs enumeration per poll.
- With `nusb::watch_devices()` (already a dependency after the nusb migration)
  we have a real **hotplug event stream** — a service can push `DeviceAdded` /
  `DeviceRemoved` signals the instant hardware changes, no polling.

## 2. `zbus` at a glance *(verify in spike)*

- Pure-Rust D-Bus; **async-first** (runs on `tokio` or `async-io` via features),
  with a `zbus::blocking` façade for synchronous callers.
- Serve an interface by annotating an `impl` block with `#[zbus::interface(name
  = "…")]`; methods become D-Bus methods, `#[zbus(property)]` fields become
  properties (the object server emits `PropertiesChanged` automatically),
  `#[zbus(signal)]` async fns emit signals.
- Own a well-known name and register objects via the connection builder, roughly:
  `connection::Builder::session()?.name("it.davidebr90.Symbinux")?
  .serve_at("/it/davidebr90/Symbinux", iface)?.build().await?`.
- **Session bus** is the right bus for a per-user device companion (not system
  bus — no root, no polkit needed for a start).
- D-Bus **service activation**: a `*.service` file under
  `share/dbus-1/services/` with `Name=` + `Exec=` lets the bus start the daemon
  on first client call.

## 3. Proposed interface sketch

Bus name `it.davidebr90.Symbinux`, object `/it/davidebr90/Symbinux`, interface
`it.davidebr90.Symbinux.Devices`:

| Member | Kind | Maps to |
|---|---|---|
| `ListDevices() -> a(...)` | method | `symbinux_devices::detect_staged()` → `Vec<DetectedDevice>` serialised (port path, vid/pid, platform/kind, capabilities, strings) |
| `Identify(port: s) -> a{sv}` | method | resolve port → run the FBUS identify → decoded model/firmware/date (the same path the GUI Identify button uses today) |
| `DeviceAdded(dev: a{sv})` | signal | `nusb::watch_devices()` → `HotplugEvent::Connected`, re-fingerprinted |
| `DeviceRemoved(port: s)` | signal | `HotplugEvent::Disconnected` |
| `DeviceChanged(port: s, dev: a{sv})` | signal | `DeviceManager::Transition::Switched` (AOA/iOS mode switch on the same `PortKey`) |
| `Devices` | property (`a{...}`) | cached current set; `PropertiesChanged` on any transition |

The payload types mirror the existing model in
`crates/symbinux-devices/src/{device.rs,handler.rs,manager.rs}`:
`DetectedDevice`, `Capability`, `Platform`, and `DeviceManager` /
`Transition` — the service is a thin D-Bus skin over `DeviceManager`, not new
domain logic.

## 4. Architecture

- **New crate `symbinux-dbus`** (bin `symbinux-daemon`) depending on
  `symbinux-devices` (+ `symbinux-transport` for `Identify`). Keeps the CLI lean
  and the daemon optional.
- Async runtime: **`tokio`** (zbus + `nusb::watch_devices()` returns a
  `futures_core::Stream`, easily driven under tokio). Single-threaded flavour is
  enough.
- Core loop: hold a `DeviceManager`; seed it from `detect_staged()`; drive
  `watch_devices()` and translate each `HotplugEvent`/`Transition` into a signal
  + a properties update.
- The **CLI stays the source of truth for one-shot commands**; the daemon reuses
  the same library crates, so there is no duplicated protocol logic.

## 5. GUI integration (migration path)

- Python GUI consumes the service via **`Gio.DBusProxy`** (already available
  through PyGObject — no new dependency) or `dasbus`. It replaces the
  subprocess-poll in `backend.py` with: call `ListDevices` once, then subscribe
  to `DeviceAdded/Removed/Changed` and update the list reactively.
- **Coexistence**: keep the subprocess path as a fallback when the daemon is not
  running (older installs, or the daemon crashed). The GUI tries the bus first,
  falls back to `symbinux-fbus` — no hard cutover, honest degradation.

## 6. Per-OS reality

- **D-Bus is Linux-only.** This service is therefore Linux-only — which is fine:
  the GTK4/libadwaita GUI is already Linux-only (`pyproject.toml` gates
  `PyGObject` to `sys_platform == 'linux'`).
- **Windows/macOS keep the CLI/subprocess story** (no D-Bus, no GUI there today).
  The daemon is a Linux-desktop enhancement, not a portability item.

## 7. Edge cases & risks

1. **Bus-name contention** — only one process may own
   `it.davidebr90.Symbinux`. Handle "name already owned" (another instance) by
   exiting cleanly; rely on activation to keep a single instance.
2. **Activation vs manual start** — ship the `.service` file so the bus starts
   the daemon on demand; but also allow a `--systemd` user-unit for always-on.
3. **Lifecycle / idle exit** — optionally exit after N seconds with no clients
   (activation restarts it), or stay resident for hotplug signals. Decide in the
   spike; hotplug push argues for resident.
4. **Flatpak sandbox** — the manifest must add `--own-name=it.davidebr90.Symbinux`
   (the packaging already opens `org.bluez`/`obexd`/NetworkManager via
   `--talk-name`, so the pattern exists). The GUI client side needs matching
   `--talk-name`.
5. **Security** — read-only methods (`ListDevices`) are safe; anything that
   *acts* on hardware (`Identify`, future write ops) should be explicit and, for
   writes, gated. No system-bus/polkit needed as long as it stays session-scoped.
6. **udev permission still required** — the daemon reads USB via `nusb`/usbfs, so
   the existing udev rules still apply; D-Bus does not bypass device permissions.
7. **Testability without hardware** — the D-Bus surface is unit/integration
   testable against a mock `DeviceManager` (spawn the service on a private bus,
   assert signals). Hotplug against real hardware still needs a device.

## 8. Suggested phased plan

| # | Step | Testable? |
|---|---|---|
| 0 | **Spike:** confirm `zbus` 5.x API (`#[interface]`, signals, name ownership, builder), and that `nusb::watch_devices()` drives cleanly under tokio | build-only |
| 1 | New `symbinux-dbus` crate: define the interface types (serialise `DetectedDevice`/`Capability` to `a{sv}`) | unit |
| 2 | `ListDevices` + `Identify` methods over `symbinux-devices`/`-transport` | unit + integration on a private bus |
| 3 | Hotplug: drive `watch_devices()` → `DeviceAdded/Removed/Changed` signals + `Devices` property | integration (mock) |
| 4 | `.service` activation file + optional systemd user unit; Flatpak `--own-name` | packaging validation |
| 5 | GUI: consume via `Gio.DBusProxy`, subprocess fallback retained | pytest (mock proxy) |
| 6 | Docs (CONNECTION_MODEL/README) + CHANGELOG | — |

Keep agent fan-out minimal; this is sequential, edit-heavy work. Commit only
when the workspace is green (build/test/clippy/fmt).

## 9. Open questions for the spike

1. Exact `zbus` 5.x signal-emission ergonomics from a background task holding an
   `InterfaceRef` (how the hotplug loop reaches the object server to emit).
2. Resident vs activation-with-idle-exit — which fits hotplug push best.
3. Serialisation shape of `Capability`/`Platform` over D-Bus (`a{sv}` map vs a
   typed struct signature) for a stable client contract.
4. Whether the GUI should fully drop the subprocess path or keep it indefinitely
   as a fallback.

---

*Plan only. No production code, dependency, or git history has been modified.
Execution is a dedicated session per §8, starting with the Step 0 spike.*
