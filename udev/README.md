# udev rules

Access rules are split **per device category** rather than one monolithic file,
so each can be installed independently and reasoned about separately.

| File | Category | Ships here? |
|---|---|---|
| `69-nokia-legacy.rules` | Nokia legacy phones + DKU-2/CA-42 cables | ✅ yes |
| `51-android.rules` | Android (ADB, fastboot, MTP/PTP, AOA) | ✅ yes |
| Apple iOS | iPhone/iPad | ❌ provided by **usbmuxd** (see below) |

Install the ones you need:

```bash
sudo cp udev/69-nokia-legacy.rules udev/51-android.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

All rules prefer `TAG+="uaccess"` (systemd-logind grants the local user access)
with a group (`dialout` / `plugdev`) as a fallback for non-logind setups.

## Apple iOS is different: it needs a daemon, not just a rule

iOS devices are not reached by opening the USB device directly. They speak the
**usbmux** protocol brokered by the **`usbmuxd`** daemon over a Unix socket
(`/var/run/usbmuxd`), with pairing/trust and a TLS lockdown session on top.
Install the system package:

```bash
# Debian/Ubuntu
sudo apt install usbmuxd libimobiledevice6
```

`usbmuxd` ships its **own** udev rule (`39-usbmuxd.rules`) and a systemd service
that is socket/udev-activated on device plug — do **not** duplicate it here, as a
second rule that grabs the Apple device would conflict with the daemon. See
[docs/DEVICE_DETECTION.md](../docs/DEVICE_DETECTION.md) for details.
