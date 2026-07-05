"""Rilevamento dispositivi USB tramite udev.

Il modulo è importabile su qualunque piattaforma (per tooling/lint/test),
ma `list_usb_devices` funziona solo su Linux con `pyudev` disponibile:
richiede l'accesso a udev, quindi fallisce esplicitamente altrove.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class UsbDevice:
    device_node: str | None
    vendor_id: str | None
    product_id: str | None
    vendor_name: str | None
    product_name: str | None
    serial: str | None


def list_usb_devices() -> list[UsbDevice]:
    """Elenca i dispositivi USB collegati (esclude gli hub e le interfacce)."""
    try:
        import pyudev
    except ImportError as exc:
        raise RuntimeError(
            "pyudev non disponibile: il rilevamento USB richiede Linux e "
            "il pacchetto 'pyudev' (pip install symbinux[gui] o pip install pyudev)."
        ) from exc

    context = pyudev.Context()
    devices = []
    for device in context.list_devices(subsystem="usb", DEVTYPE="usb_device"):
        devices.append(
            UsbDevice(
                device_node=device.device_node,
                vendor_id=device.get("ID_VENDOR_ID"),
                product_id=device.get("ID_MODEL_ID"),
                vendor_name=device.get("ID_VENDOR_FROM_DATABASE") or device.get("ID_VENDOR"),
                product_name=device.get("ID_MODEL_FROM_DATABASE") or device.get("ID_MODEL"),
                serial=device.get("ID_SERIAL_SHORT"),
            )
        )
    return devices
