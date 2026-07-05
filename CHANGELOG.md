# Changelog

Tutte le modifiche rilevanti a questo progetto sono documentate qui.

Il formato segue [Keep a Changelog](https://keepachangelog.com/it/1.1.0/),
e il progetto aderisce a [Semantic Versioning](https://semver.org/lang/it/).

## [Unreleased]

## [0.3.0] - 2026-07-05

### Aggiunto
- **Selettore tema chiaro/scuro** (menu Aspetto): Automatico / Chiaro / Scuro.
  In "Automatico" segue la preferenza del desktop (light/dark) tramite libadwaita
  (portale freedesktop); se il DE non espone la preferenza, ripiega su **Scuro**.
  Il logo si adatta automaticamente alla variante corretta (blu su chiaro,
  arancione su scuro). La scelta è persistente.
- **Internazionalizzazione (gettext)**: interfaccia in **inglese** (sorgente) e
  **italiano** (`it`), selezionabile dal menu Lingua (Automatico segue il locale
  di sistema). Infrastruttura `po/` completa (`symbinux.pot`, `it.po`, `LINGUAS`,
  `compile.sh`, guida per traduttori) predisposta per aggiungere altre lingue.
- Impostazioni GUI persistite in `~/.config/symbinux/settings.json`.

## [0.2.0] - 2026-07-05

### Aggiunto
- **Core FBUS/MBUS in Rust** (workspace `crates/`): implementazione clean-room dei
  protocolli seriali Nokia, ricostruita da gnokii/gammu e validata contro catture
  reali documentate (nessun codice/binario proprietario Nokia).
  - `symbinux-protocol`: codec FBUS/2 e MBUS v1 con checksum doppio/singolo,
    reader incrementale, builder di comandi con classificazione di sicurezza.
    Validato dai fixture-oracolo `CF 71` e `72 D5`.
  - `symbinux-transport`: trasporto seriale (termios 115200 8N1) e USB raw
    (libusb), enumerazione dispositivi in stile lsusb, scambio richiesta/risposta.
  - `symbinux-cli` (`symbinux-fbus`): comandi `devices`, `identify`,
    `getphonebook`, `netmon`, `raw` (protetto), in stile gnokii.
- **Modalità avanzata** di enumerazione USB (VID:PID, nomi estesi,
  classificazione) per il debug del riconoscimento dispositivi.
- **Regole udev** (`udev/69-nokia-legacy.rules`) per accesso non privilegiato e
  tabella `data/devices.json` dei VID/PID noti.
- **Documentazione**: `docs/PROTOCOL_NOTES.md` (con livelli di confidenza),
  `docs/FUNCTIONS.md`, `docs/ROADMAP.md`, `docs/SETUP.md`.
- **Suite di test** con fixture binarie (catture reali FBUS/2 + fixture MBUS
  sintetica etichettata come tale).

### Modificato
- **GUI ridisegnata**: logo e versione sempre visibili (header + About),
  selettore canale USB/Bluetooth/Wi-Fi, pulsanti-funzione con icone disabilitati
  finché non è presente una connessione compatibile, empty state contestuali (non
  più il messaggio generico), notifiche desktop native integrate con GNOME/KDE.
  La GUI ora invoca il core Rust (`symbinux-fbus`).

## [0.1.1] - 2026-07-05

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
