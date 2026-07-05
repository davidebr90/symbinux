"""Test del modulo symbinux.core.devices.

Su piattaforme non-Linux (o senza pyudev) list_usb_devices deve fallire in
modo esplicito e comprensibile, non con un errore di import a sorpresa.
"""

import sys

import pytest

from symbinux.core.devices import list_usb_devices


@pytest.mark.skipif(sys.platform == "linux", reason="su Linux il comportamento dipende da pyudev")
def test_list_usb_devices_raises_clearly_off_linux():
    with pytest.raises(RuntimeError, match="pyudev"):
        list_usb_devices()
