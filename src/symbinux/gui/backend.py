"""Bridge from the Python GUI to the Rust core (`symbinux-fbus` binary).

The GUI does no protocol work itself: it shells out to the compiled Rust CLI for
device enumeration and phone operations, and parses the results. This keeps a
single source of truth for the FBUS/MBUS logic. If the binary is not found, the
functions raise `BackendUnavailable` so the UI can degrade gracefully.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

BIN_NAME = "symbinux-fbus"


class BackendUnavailable(RuntimeError):
    """Raised when the Rust core binary cannot be located or run."""


@dataclass(frozen=True)
class Device:
    bus_addr: str
    vid_pid: str
    name: str
    role: str

    @property
    def is_phone(self) -> bool:
        return self.role.startswith("Nokia")


@dataclass(frozen=True)
class DetectedPhone:
    vid_pid: str
    platform: str
    model: str
    serial: str
    detail: str
    capabilities: tuple[str, ...]

    def has_capability(self, cap: str) -> bool:
        return cap in self.capabilities


@dataclass(frozen=True)
class BluetoothDevice:
    address: str
    name: str
    paired: bool


@dataclass(frozen=True)
class WifiNetwork:
    ssid: str
    signal: str
    security: str


@dataclass(frozen=True)
class SerialPort:
    path: str
    vid: str | None
    pid: str | None
    product: str | None


# USB vendor ids that expose a phone's serial port: Nokia plus common cable
# bridges (Prolific, Silicon Labs, FTDI, CH340).
_SERIAL_VIDS = {"0421", "067b", "10c4", "0403", "1a86"}


def _find_binary() -> str:
    # 1) explicit override, 2) PATH, 3) local cargo build output (dev)
    env = os.environ.get("SYMBINUX_FBUS_BIN")
    if env and Path(env).exists():
        return env
    found = shutil.which(BIN_NAME)
    if found:
        return found
    here = Path(__file__).resolve()
    for root in here.parents:
        for candidate in (
            root / "target" / "debug" / BIN_NAME,
            root / "target" / "release" / BIN_NAME,
        ):
            if candidate.exists():
                return str(candidate)
    raise BackendUnavailable(
        f"'{BIN_NAME}' not found. Build the core with `cargo build` or install it, "
        f"or set SYMBINUX_FBUS_BIN to its path."
    )


def _run(args: list[str], timeout: float = 10.0) -> str:
    binary = _find_binary()
    try:
        result = subprocess.run(
            [binary, *args],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise BackendUnavailable(str(exc)) from exc
    if result.returncode != 0:
        raise BackendUnavailable(result.stderr.strip() or "command failed")
    return result.stdout


def core_version() -> str | None:
    try:
        out = _run(["--version"])
    except BackendUnavailable:
        return None
    # "symbinux-fbus 0.2.0"
    parts = out.strip().split()
    return parts[-1] if parts else None


def list_usb_devices(include_all: bool = False) -> list[Device]:
    """Enumerate USB devices via the Rust CLI (advanced diagnostics view).

    Uses the CLI's stable JSON output, so there is no brittle column parsing.
    """
    args = ["devices", "--json"]
    if include_all:
        args.append("--all")
    out = _run(args)
    try:
        data = json.loads(out)
    except (ValueError, TypeError):
        return []
    devices: list[Device] = []
    for entry in data:
        devices.append(
            Device(
                bus_addr=f"{entry.get('bus', 0):03d}:{entry.get('address', 0):03d}",
                vid_pid=f"{entry.get('vid', '')}:{entry.get('pid', '')}",
                name=entry.get("name", ""),
                role=entry.get("role", "other"),
            )
        )
    return devices


def identify(port: str) -> dict:
    """Run identify against a serial port and return the decoded fields.

    Returns a dict with `model`/`firmware`/`date`, or `{"error": ...}`.
    """
    out = _run(["identify", "--port", port, "--json"], timeout=8.0)
    try:
        data = json.loads(out)
    except (ValueError, TypeError):
        return {"error": out.strip() or "no output"}
    return data if isinstance(data, dict) else {"error": "unexpected output"}


def serial_ports() -> list[SerialPort]:
    """List the serial ports the OS exposes (via the Rust CLI)."""
    try:
        data = json.loads(_run(["ports", "--json"]))
    except (ValueError, TypeError, BackendUnavailable):
        return []
    return [
        SerialPort(
            path=p.get("path", ""),
            vid=p.get("vid"),
            pid=p.get("pid"),
            product=p.get("product"),
        )
        for p in data
    ]


def resolve_port() -> str | None:
    """Pick the serial port most likely to be the phone: one matching a known
    phone/cable vendor id, or the sole USB serial port if unambiguous."""
    ports = serial_ports()
    for p in ports:
        if p.vid and p.vid.lower() in _SERIAL_VIDS:
            return p.path
    usb = [p for p in ports if p.vid]
    return usb[0].path if len(usb) == 1 else None


def detect_devices(progress_cb=None, timeout: float = 15.0) -> list[DetectedPhone]:
    """Run `detect --progress`, driving `progress_cb(fraction, stage)` from the
    real `PROGRESS done total stage` lines, and return the detected phones.

    The fractions come straight from the cascade's completed steps — this is a
    genuine progress signal, not a timed animation.
    """
    binary = _find_binary()
    try:
        proc = subprocess.Popen(
            [binary, "detect", "--progress"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as exc:
        raise BackendUnavailable(str(exc)) from exc

    phones: list[DetectedPhone] = []
    assert proc.stdout is not None
    for line in proc.stdout:
        line = line.rstrip("\n")
        if line.startswith("PROGRESS "):
            parts = line.split(None, 3)
            if len(parts) >= 3 and progress_cb is not None:
                try:
                    done, total = int(parts[1]), int(parts[2])
                    stage = parts[3] if len(parts) > 3 else ""
                    progress_cb(done / total if total else 1.0, stage)
                except ValueError:
                    pass
        elif line.startswith("DEVICE\t"):
            cols = line.split("\t")
            if len(cols) >= 7:
                phones.append(
                    DetectedPhone(
                        vid_pid=cols[1],
                        platform=cols[2],
                        model=cols[3],
                        serial=cols[4],
                        detail=cols[5],
                        capabilities=tuple(c for c in cols[6].split(",") if c),
                    )
                )
    try:
        proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        proc.kill()
    return phones


def scan_bluetooth(duration: int = 8) -> list[BluetoothDevice]:
    """Discover Bluetooth devices via BlueZ (`bluetoothctl`).

    Runs a real timed inquiry, then lists known devices, marking paired ones.
    Raises `BackendUnavailable` if BlueZ or an adapter is missing — never a fake
    result.
    """
    if not shutil.which("bluetoothctl"):
        raise BackendUnavailable("bluetoothctl not found — install BlueZ (bluez).")

    def _bctl(args: list[str], timeout: float):
        return subprocess.run(
            ["bluetoothctl", *args], capture_output=True, text=True, timeout=timeout
        )

    try:
        show = _bctl(["show"], timeout=6)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise BackendUnavailable(str(exc)) from exc
    if "No default controller" in (show.stdout + show.stderr) or show.returncode != 0:
        raise BackendUnavailable("No Bluetooth adapter available.")

    try:
        # A timed active inquiry (blocks for `duration` seconds).
        _bctl(["--timeout", str(duration), "scan", "on"], timeout=duration + 5)
        devices_out = _bctl(["devices"], timeout=6).stdout
        paired_out = _bctl(["paired-devices"], timeout=6).stdout
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise BackendUnavailable(str(exc)) from exc

    paired = {line.split()[1] for line in paired_out.splitlines() if line.startswith("Device ")}
    devices: list[BluetoothDevice] = []
    for line in devices_out.splitlines():
        parts = line.split(None, 2)
        if len(parts) >= 3 and parts[0] == "Device":
            address = parts[1]
            devices.append(BluetoothDevice(address=address, name=parts[2], paired=address in paired))
    return devices


def ensure_paired(address: str) -> None:
    """App-driven Bluetooth pairing: force-pair and connect a device via BlueZ
    (system bus), rather than requiring it to be pre-paired in the OS.

    Best-effort and non-fatal — the caller (PBAP) reports the real outcome. This
    is the "force the link" step; it needs a real adapter to exercise and may
    require confirming a code on the phone. Not validated on-device.
    """
    try:
        import dbus  # python3-dbus
    except ImportError:
        return
    try:
        bus = dbus.SystemBus()
        mgr = dbus.Interface(
            bus.get_object("org.bluez", "/"), "org.freedesktop.DBus.ObjectManager"
        )
        objects = mgr.GetManagedObjects()
        target = address.upper()
        device_path = None
        for path, ifaces in objects.items():
            dev = ifaces.get("org.bluez.Device1")
            if dev and str(dev.get("Address", "")).upper() == target:
                if bool(dev.get("Paired", False)):
                    return  # already paired
                device_path = path
                break
        if device_path is None:
            adapter = next((p for p, i in objects.items() if "org.bluez.Adapter1" in i), None)
            if adapter is None:
                return
            device_path = f"{adapter}/dev_" + target.replace(":", "_")
        device = dbus.Interface(bus.get_object("org.bluez", device_path), "org.bluez.Device1")
        for op in (device.Pair, device.Connect):
            try:
                op()
            except Exception:  # noqa: BLE001 - already paired / needs on-device confirm
                pass
    except Exception:  # noqa: BLE001 - non-fatal; PBAP surfaces real errors
        return


def pull_contacts_pbap(address: str, timeout: float = 30.0) -> str:
    """Pull a phone's contacts over Bluetooth PBAP via BlueZ obexd (D-Bus).

    Force-pairs the device first (app-driven), then pulls the phonebook as vCard
    text. Raises `BackendUnavailable` with actionable text when the stack isn't
    present — never a fake result.

    NOTE: this drives the standard `org.bluez` + `org.bluez.obex` APIs; it needs
    real Bluetooth hardware + a phone to exercise and has not been validated
    on-device.
    """
    try:
        import dbus  # python3-dbus
    except ImportError as exc:
        raise BackendUnavailable(
            "python3-dbus is required for Bluetooth contacts (install python3-dbus)."
        ) from exc

    import tempfile
    import time

    # Force the pairing/connection before talking PBAP (best-effort).
    ensure_paired(address)

    try:
        bus = dbus.SessionBus()
        obex = bus.get_object("org.bluez.obex", "/org/bluez/obex")
        client = dbus.Interface(obex, "org.bluez.obex.Client1")
        session_path = client.CreateSession(address, {"Target": dbus.String("pbap")})
        pbap = dbus.Interface(
            bus.get_object("org.bluez.obex", session_path),
            "org.bluez.obex.PhonebookAccess1",
        )
        pbap.Select("int", "pb")

        with tempfile.NamedTemporaryFile(suffix=".vcf", delete=False) as tmp:
            target = tmp.name
        transfer_path, _props = pbap.PullAll(target, dbus.Dictionary({}, signature="sv"))

        # Poll the transfer until it completes or errors.
        transfer_props = dbus.Interface(
            bus.get_object("org.bluez.obex", transfer_path),
            "org.freedesktop.DBus.Properties",
        )
        deadline = time.time() + timeout
        while time.time() < deadline:
            status = str(transfer_props.Get("org.bluez.obex.Transfer1", "Status"))
            if status == "complete":
                break
            if status == "error":
                raise BackendUnavailable("Bluetooth contact transfer failed.")
            time.sleep(0.2)

        try:
            with open(target, encoding="utf-8", errors="replace") as fh:
                return fh.read()
        finally:
            try:
                os.unlink(target)
            except OSError:
                pass
    except BackendUnavailable:
        raise
    except Exception as exc:  # dbus.DBusException and friends
        raise BackendUnavailable(
            f"Bluetooth contacts unavailable ({exc}). Pair the phone and make sure "
            f"obexd is running."
        ) from exc


def scan_wifi(timeout: float = 20.0) -> list[WifiNetwork]:
    """Scan for Wi-Fi networks via NetworkManager (`nmcli`).

    Raises `BackendUnavailable` if nmcli is missing or the scan fails.
    """
    if not shutil.which("nmcli"):
        raise BackendUnavailable("nmcli not found — install NetworkManager.")
    try:
        result = subprocess.run(
            ["nmcli", "-t", "-f", "SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "yes"],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise BackendUnavailable(str(exc)) from exc
    if result.returncode != 0:
        raise BackendUnavailable(result.stderr.strip() or "Wi-Fi scan failed.")

    networks: list[WifiNetwork] = []
    seen: set[str] = set()
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        # nmcli -t escapes ':' inside fields as '\:'; split on unescaped colons.
        fields = [f.replace("\\:", ":").replace("\\\\", "\\") for f in re.split(r"(?<!\\):", line)]
        if len(fields) < 3:
            continue
        ssid = fields[0] or "(hidden)"
        if ssid in seen:
            continue
        seen.add(ssid)
        networks.append(WifiNetwork(ssid=ssid, signal=fields[1], security=fields[2] or "open"))
    return networks
