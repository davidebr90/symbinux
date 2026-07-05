# Symbinux

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo/symbinux_logo_transparent_dark.png">
  <img alt="Symbinux logo" src="assets/logo/symbinux_logo_transparent_light.png" width="320">
</picture>

*[Leggi questo documento in italiano](README.it.md)*

Modern USB and Bluetooth device management for GNU/Linux, built with a
separated **Core + GUI** architecture and Flatpak packaging.

**Symbinux is a declared fork of [Nokinux](https://launchpad.net/nokinux)**
(2008-2010), a Bash/Python project born in the Italian Ubuntu community to
configure Nokia phones from Linux, of which Davide Pica (davidebr90) was one
of the original authors alongside other contributors. Nokinux's original
purpose has long been overtaken by the evolution of mobile devices; Symbinux
carries its name and spirit forward, rewritten from scratch to today's Linux
application standards (separated Core + GUI architecture, Flatpak
packaging) and generalized to managing any modern USB/Bluetooth device.

## Architecture

```
symbinux/
├── src/symbinux/
│   ├── core/       # symbinux.core - pure library, no GUI dependency
│   │   ├── devices.py    # USB detection (pyudev)
│   │   └── bluetooth.py  # Bluetooth integration (BlueZ over D-Bus)
│   └── gui/        # symbinux.gui - GTK4 + libadwaita frontend
│       ├── main.py
│       └── window.py
├── packaging/
│   └── flatpak/    # Flatpak manifest for distribution
├── tests/
└── pyproject.toml
```

The Core is an independent, headless-testable Python library. The GUI is a
separate layer that consumes the Core: no business logic in the GUI, no GTK
dependency in the Core.

## Requirements

- Python >= 3.11
- Linux with `udev` and (optionally) `bluez` for real functionality
- For the GUI: GTK4 and libadwaita (`gir1.2-gtk-4.0`, `gir1.2-adw-1` on
  Debian/Ubuntu)

## Development

```bash
python -m venv .venv
source .venv/bin/activate
pip install -e ".[gui,dev]"

# Core only (headless, no GUI dependencies)
pip install -e .

# Tests
pytest
```

## Note on the development environment

The Core and GUI depend on Linux-specific libraries (`pyudev`, GTK4/libadwaita
over D-Bus/GObject) and **do not run on Windows**. Development and testing
must happen on a real Linux machine or in **WSL2** with a desktop environment
(or X11/Wayland forwarding).

## Packaging

The Flatpak manifest lives in `packaging/flatpak/`. Local build:

```bash
flatpak-builder build-dir packaging/flatpak/it.davidebr90.Symbinux.yml --force-clean
```

## License

**GNU AGPLv3** (GNU Affero General Public License v3, or later). Chosen over
plain GPL to also cover network/SaaS use of the code, not just distribution.
See [LICENSE](LICENSE).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).
