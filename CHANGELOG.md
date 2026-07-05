# Changelog

Tutte le modifiche rilevanti a questo progetto sono documentate qui.

Il formato segue [Keep a Changelog](https://keepachangelog.com/it/1.1.0/),
e il progetto aderisce a [Semantic Versioning](https://semver.org/lang/it/).

## [Unreleased]

### Modificato
- **Licenza cambiata da GPLv3 ad AGPLv3**: copre anche l'uso del codice
  come servizio in rete (SaaS), non solo la distribuzione. Nessun rilascio
  pubblico era ancora avvenuto sotto GPLv3.
- README diviso in inglese (primario, `README.md`) e italiano
  (`README.it.md`), con link incrociato tra le due versioni.
- Il logo nel README ora usa le varianti a sfondo trasparente invece di
  quelle a sfondo pieno.

### Aggiunto
- Due varianti del logo a sfondo trasparente (`symbinux_logo_transparent_light.png`,
  `symbinux_logo_transparent_dark.png`), derivate dalle due varianti a
  sfondo pieno fornite in origine.

## [0.1.0] - 2026-07-05

### Aggiunto
- Scaffold iniziale del progetto: separazione Core/GUI/packaging.
- `symbinux.core`: modulo di rilevamento dispositivi USB (`pyudev`) e stub
  per integrazione Bluetooth via BlueZ/D-Bus.
- `symbinux.gui`: stub applicazione GTK4 + libadwaita.
- Manifest Flatpak iniziale per il packaging.
- Licenza GPLv3 (poi cambiata in AGPLv3, vedi sopra), in continuità con il
  progetto originale Nokinux (https://launchpad.net/nokinux), di cui questo
  progetto riprende il concept generalizzandolo a dispositivi USB/Bluetooth
  moderni.
