"""Bridge from the Python GUI to the Rust core (`symbinux-fbus` binary).

The GUI does no protocol work itself: it shells out to the compiled Rust CLI for
device enumeration and phone operations, and parses the results. This keeps a
single source of truth for the FBUS/MBUS logic. If the binary is not found, the
functions raise `BackendUnavailable` so the UI can degrade gracefully.
"""

from __future__ import annotations

import os
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
