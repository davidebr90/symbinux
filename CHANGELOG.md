# Changelog

Tutte le modifiche rilevanti a questo progetto sono documentate qui.

Il formato segue [Keep a Changelog](https://keepachangelog.com/it/1.1.0/),
e il progetto aderisce a [Semantic Versioning](https://semver.org/lang/it/).

## [Unreleased]

## [0.1.0] - 2026-07-05

### Aggiunto
- Scaffold iniziale del progetto: separazione Core/GUI/packaging.
- `symbinux.core`: modulo di rilevamento dispositivi USB (`pyudev`) e stub
  per integrazione Bluetooth via BlueZ/D-Bus.
- `symbinux.gui`: stub applicazione GTK4 + libadwaita.
- Manifest Flatpak iniziale per il packaging.
- Licenza GPLv3, in continuità con il progetto originale Nokinux
  (https://launchpad.net/nokinux), di cui questo progetto riprende il
  concept generalizzandolo a dispositivi USB/Bluetooth moderni.
