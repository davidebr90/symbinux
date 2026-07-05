# Setup & unprivileged access

Symbinux is meant to run **without `sudo`** in normal use. The only privileged
step is installing a udev rule once, so your user can open the phone's USB device
and its serial port.

## 1. Build the core

```bash
# Rust core + CLI
cargo build --release
# the binary lands at target/release/symbinux-fbus
```

Runtime dependencies (Debian/Ubuntu): `libusb-1.0-0`, and for a cable that
exposes a serial port the in-kernel `ftdi_sio` / `cp210x` / `pl2303` drivers
(present by default).

## 2. Install the udev rule

```bash
sudo cp udev/69-nokia-legacy.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

The rule grants access two ways (see the file for details):

- `TAG+="uaccess"` — systemd-logind hands access to the locally logged-in user.
  Preferred on modern distros (systemd ≥ 213). No group management needed.
- `GROUP="dialout"` — fallback. Add yourself once and re-login:

  ```bash
  sudo usermod -aG dialout "$USER"
  ```

It also creates a stable symlink `/dev/nokia_fbus` for the cable's serial port.

## 3. Verify

```bash
# What is physically connected (no phone needed):
symbinux-fbus devices --all

# With a phone connected via a serial cable:
symbinux-fbus identify --port /dev/nokia_fbus
```

If `identify` fails to open the port, check:

- the cable exposes a `/dev/ttyUSB*` (`dmesg | tail` after plugging in),
- your user has access (`ls -l /dev/ttyUSB0`; the udev rule should give group
  `dialout` / uaccess),
- the phone is on and not in PC Suite / mass-storage mode.

## 4. GUI

```bash
pip install -e ".[gui]"
symbinux            # launches the GTK4 GUI
```

The GUI calls the `symbinux-fbus` binary. If it is not on `PATH`, set
`SYMBINUX_FBUS_BIN=/path/to/symbinux-fbus`.

## Note on WSL2

WSL2 does not forward physical USB by default. To test against real hardware from
Windows, attach the device with [`usbipd-win`](https://github.com/dorssel/usbipd-win)
(`usbipd attach --wsl --busid <id>`). Framing and the codec are fully testable
without any hardware via `cargo test`.
