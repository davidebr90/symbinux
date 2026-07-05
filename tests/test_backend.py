"""Tests for the GUI-to-core bridge that do not require the compiled binary."""

from symbinux.gui.backend import Device, core_version


def test_device_phone_classification():
    phone = Device(bus_addr="001:004", vid_pid="0421:0400", name="Nokia 3310", role="Nokia phone")
    cable = Device(bus_addr="001:005", vid_pid="067b:2303", name="USB-Serial", role="cable bridge")
    assert phone.is_phone
    assert not cable.is_phone


def test_core_version_never_raises():
    # Returns the version string if the binary is installed, otherwise None —
    # but never raises, so the GUI can degrade gracefully.
    result = core_version()
    assert result is None or isinstance(result, str)
