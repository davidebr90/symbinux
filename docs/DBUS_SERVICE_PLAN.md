# D-Bus service plan (`zbus`) — item B8

> Status: **plan / design document — not started.** Execution is a dedicated
> session per §8. The `zbus` (5.16.0) and `nusb` APIs below have been verified
> against the current docs; the Step 0 spike now just stands the pieces up
> end-to-end before the real build, the way the nusb migration was de-risked.

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

## 2. `zbus` at a glance (verified against 5.16.0)

- **Version:** `zbus` 5.16.0 (the 5.x line), pure-Rust D-Bus.
- **Async model:** runtime-agnostic; the **default backend is `async-io`** (zbus
  spins its own internal I/O thread). To ride our existing tokio runtime instead
  (the hotplug stream is async), depend as
  `zbus = { version = "5", default-features = false, features = ["tokio"] }` —
  this avoids a redundant second executor. A `zbus::blocking` façade also exists
  (feature `blocking-api`, default-on).
- **Interface macro:** `#[zbus::interface(name = "…")]` on an `impl` block (the
  old `#[dbus_interface]` is renamed — do not use it). Methods are async fns;
  `#[zbus(property)]` marks properties (object server emits `PropertiesChanged`
  automatically); `#[zbus(signal)]` marks bodiless async fns that emit signals.
- **Serving a name/object:**
  `zbus::connection::Builder::session()?.name("it.davidebr90.Symbinux")?`
  `.serve_at("/it/davidebr90/Symbinux", iface)?.build().await?` (note the path is
  `connection::Builder`, not the old `ConnectionBuilder`).
- **Emitting a signal from a background task** (our hotplug loop, i.e. *not*
  inside a method call): fetch an `InterfaceRef` via
  `connection.object_server().interface::<_, Devices>(path).await?` and call the
  generated signal method on it. This is the idiomatic 5.x way to push signals
  reactively from the tokio task that consumes `watch_devices()`.
- **Session bus** for a per-user companion (no root/polkit). D-Bus **service
  activation**: a `*.service` file (`Name=` + absolute `Exec=`, optionally
  `SystemdService=` to delegate to a systemd `--user` unit) lets the bus lazily
  start the daemon on first client call.

## 3. Proposed interface sketch

Bus name `it.davidebr90.Symbinux`, object `/it/davidebr90/Symbinux`, interface
`it.davidebr90.Symbinux.Devices`:

| Member | Kind | Maps to |
|---|---|---|
| `ListDevices() -> a(...)` | method | `symbinux_devices::detect_staged()` → `Vec<DetectedDevice>` serialised (port path, vid/pid, platform/kind, capabilities, strings) |
| `Identify(port: s) -> a{sv}` | method | resolve port → run the FBUS identify → decoded model/firmware/date (the same path the GUI Identify button uses today) |
| `DeviceAdded(dev: a{sv})` | signal | `nusb::watch_devices()` → `HotplugEvent::Connected`, re-fingerprinted |
| `DeviceRemoved(id: s)` | signal | `HotplugEvent::Disconnected(DeviceId)` — nusb yields only an opaque `DeviceId` on removal, so the daemon resolves it against a `DeviceId → DetectedDevice` cache |
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
- Async runtime: **`tokio`**, with `zbus` built `default-features = false,
  features = ["tokio"]` so zbus and the hotplug loop share one runtime (no second
  executor thread). `nusb::watch_devices()` is a *synchronous* call returning a
  `HotplugWatch` that implements `Stream<Item = HotplugEvent>`.
- Core loop: hold a `DeviceManager` + a `DeviceId → DetectedDevice` cache; seed
  from `detect_staged()`; a spawned task polls `watch_devices()` and, per event,
  emits the matching signal via an **`InterfaceRef`**
  (`connection.object_server().interface::<_, Devices>(path).await?`), because a
  signal fired outside a method call goes through the object server, not a bare
  `Connection`.
- `HotplugEvent::Disconnected` carries only a `DeviceId`; the cache turns it back
  into a `DetectedDevice` for the `DeviceRemoved`/`DeviceChanged` payload.
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
4. **Flatpak sandbox** — recommended: run the daemon **on the host** (a systemd
   `--user` unit) so it keeps raw USB (`nusb`/usbfs) access without sandbox device
   grants; then the Flatpak GUI only needs `--talk-name=it.davidebr90.Symbinux`.
   If daemon and GUI instead ship in one Flatpak, the manifest needs
   `--own-name=it.davidebr90.Symbinux` (the packaging already opens
   `org.bluez`/`obexd`/NetworkManager, so the pattern exists).
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
| 0 | **Spike (API confirmed in §2):** stand up a minimal `#[zbus::interface]` on the session bus and emit a signal from a tokio task (via `InterfaceRef`) driven by `nusb::watch_devices()`, end-to-end | build/run |
| 1 | New `symbinux-dbus` crate: define the interface types (serialise `DetectedDevice`/`Capability` to `a{sv}`) | unit |
| 2 | `ListDevices` + `Identify` methods over `symbinux-devices`/`-transport` | unit + integration on a private bus |
| 3 | Hotplug: drive `watch_devices()` → `DeviceAdded/Removed/Changed` signals + `Devices` property | integration (mock) |
| 4 | `.service` activation file + optional systemd user unit; Flatpak `--own-name` | packaging validation |
| 5 | GUI: consume via `Gio.DBusProxy`, subprocess fallback retained | pytest (mock proxy) |
| 6 | Docs (CONNECTION_MODEL/README) + CHANGELOG | — |

Keep agent fan-out minimal; this is sequential, edit-heavy work. Commit only
when the workspace is green (build/test/clippy/fmt).

## 9. Open questions for the spike

1. Resident daemon vs activation-with-idle-exit — a resident process is needed to
   hold the `watch_devices()` stream for push signals; activation only starts it
   on first call. Likely resident (systemd `--user`).
2. Serialisation shape of `Capability`/`Platform` over D-Bus (`a{sv}` map vs a
   typed struct signature) for a stable client contract.
3. Daemon on the host vs inside the Flatpak — host is simpler for USB access
   (§7.4).
4. Whether the GUI keeps the subprocess path indefinitely as a fallback.

---

*Plan only. No production code, dependency, or git history has been modified.
Execution is a dedicated session per §8, starting with the Step 0 spike.*
