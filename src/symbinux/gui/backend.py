"""Bridge from the Python GUI to the Rust core (`symbinux-fbus` binary).

The GUI does no protocol work itself: it shells out to the compiled Rust CLI for
device enumeration and phone operations, and parses the results. This keeps a
single source of truth for the FBUS/MBUS logic. If the binary is not found, the
functions raise `BackendUnavailable` so the UI can degrade gracefully.
"""

from __future__ import annotations

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
    """Enumerate USB devices via the Rust CLI (advanced diagnostics view)."""
    args = ["devices"]
    if include_all:
        args.append("--all")
    out = _run(args)
    devices: list[Device] = []
    for line in out.splitlines():
        line = line.rstrip()
        if not line or line.startswith("BUS:ADDR") or line.startswith("("):
            continue
        # "001:004   0421:0400  Nokia 3310                   Nokia phone"
        cols = line.split(None, 3)
        if len(cols) < 4:
            continue
        bus_addr, vid_pid, name_and_role = cols[0], cols[1], cols[2] + " " + cols[3]
        # name and role are separated by 2+ spaces in the CLI output
        name, _, role = name_and_role.partition("  ")
        devices.append(
            Device(
                bus_addr=bus_addr,
                vid_pid=vid_pid,
                name=name.strip(),
                role=role.strip() or "other",
            )
        )
    return devices


def identify(port: str) -> str:
    """Run the identify command against a serial port, returning raw output."""
    return _run(["identify", "--port", port], timeout=8.0)


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
