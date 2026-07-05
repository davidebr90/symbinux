"""Integrazione Bluetooth tramite BlueZ (D-Bus).

Stub dell'interfaccia prevista: l'implementazione reale (via `dbus-python` o
`pydbus`, interrogando `org.bluez` sul system bus) non è ancora scritta e
non può essere sviluppata/verificata su questa macchina (serve un adattatore
Bluetooth e BlueZ su Linux). Segnaposto per il prossimo passo del progetto.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class BluetoothDevice:
    address: str
    name: str | None
    paired: bool
    connected: bool


def list_bluetooth_devices() -> list[BluetoothDevice]:
    """Elenca i dispositivi Bluetooth noti a BlueZ (paired e in range)."""
    raise NotImplementedError(
        "Integrazione BlueZ non ancora implementata: da sviluppare su una "
        "macchina Linux con adattatore Bluetooth reale, interrogando "
        "org.bluez via D-Bus system bus."
    )
