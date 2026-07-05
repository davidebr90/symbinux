"""CLI minimale del core, utile per test headless senza GUI."""

from __future__ import annotations

import sys

from symbinux.core.devices import list_usb_devices


def main() -> int:
    try:
        devices = list_usb_devices()
    except RuntimeError as exc:
        print(f"Errore: {exc}", file=sys.stderr)
        return 1

    if not devices:
        print("Nessun dispositivo USB rilevato.")
        return 0

    for device in devices:
        label = device.product_name or device.device_node or "dispositivo sconosciuto"
        vendor = device.vendor_name or device.vendor_id or "?"
        print(f"- {label} ({vendor}) [{device.device_node}]")
    return 0


if __name__ == "__main__":
    sys.exit(main())
