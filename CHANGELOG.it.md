# Changelog

*[Read this changelog in English](CHANGELOG.md)*

Tutte le modifiche rilevanti a questo progetto sono documentate qui.

Il formato segue [Keep a Changelog](https://keepachangelog.com/it/1.1.0/),
e il progetto aderisce a [Semantic Versioning](https://semver.org/lang/it/).

## [Unreleased]

### Aggiunto
- **Scansioni Bluetooth e Wi-Fi reali**: il canale Bluetooth rileva i dispositivi
  via BlueZ (`bluetoothctl`) e il canale Wi-Fi elenca le reti via NetworkManager
  (`nmcli`), ciascuno con spinner reale e stati vuoto/errore onesti (niente loader
  finto). Sostituisce il precedente segnaposto "non disponibile".
- **Decodifica tipizzata delle risposte** (`symbinux-protocol::decode`): la
  risposta versione HW/SW è ora parsata in una struct (`model`/`firmware`/`date`),
  validata contro la cattura reale del Nokia 3310. `identify` stampa i campi
  decodificati.
- **Output JSON stabile** (`--json`) su `devices` e `detect`, così GUI e script
  usano dati strutturati invece di testo da scansionare; la GUI ora lo usa per
  l'enumerazione dispositivi (elimina una classe di bug di parsing a colonne).
- **Logging strutturato** via i crate `log`/`env_logger` (prima dichiarati ma
  inutilizzati): `RUST_LOG=debug` dà i trace dei frame su stderr, tenendo pulito
  lo stdout per il parsing automatico.
- Nuovi documenti di riferimento da una review multi-progetto: `docs/COMPARISON.md`
  (progetti simili + backlog di funzioni prioritizzato) e `docs/CROSS_PLATFORM.md`
  (matrice di compatibilità Linux/Windows/macOS + strategia). La roadmap è
  aggiornata col backlog.
- **Risolutore porta seriale** (`symbinux-transport::ports`) + comando CLI
  `ports`: mappa un telefono/cavo rilevato sulla sua porta seriale in modo
  multipiattaforma. Il pulsante **Identifica della GUI ora esegue un identify
  reale** end-to-end e mostra modello/firmware decodificati, risolvendo la porta
  automaticamente (o un onesto "nessuna porta").
- **Contatti Bluetooth (PBAP)**: i telefoni accoppiati espongono un'azione
  "Contatti" che scarica la rubrica in vCard via BlueZ obexd. Implementato
  secondo l'API standard `org.bluez.obex`; richiede hardware Bluetooth reale +
  obexd per la validazione.
- **Decodifica SMS/vCard** (`symbinux-protocol::decode`): unpacking GSM 7-bit,
  decodifica numeri BCD, parser PDU SMS-DELIVER ed export vCard 3.0 — i mattoni
  per le funzioni contatti/SMS (testati).
- **Workflow CI/release** (`.github/workflows/`): fmt/clippy/test al push, e
  binari `symbinux-fbus` cross-compilati per Linux/Windows/macOS su tag.

### Corretto
- **Busy-loop della CPU in `exchange_fbus2`**: aggiunto un breve back-off tra le
  read vuote, così l'attesa di una risposta non satura più un core.
- **Troncamento silenzioso in `write_phonebook`**: ora restituisce un errore
  invece di troncare nome/numero più lunghi del campo lunghezza a un byte.

### Modificato
- L'inglese è ora lo standard per la documentazione; la descrizione del
  repository su GitHub è in inglese e il changelog è in inglese come primario con
  variante italiana (`CHANGELOG.it.md`), come per il README. Le varianti italiane
  sono fornite per i documenti rivolti all'utente (README, CHANGELOG, FUNCTIONS,
  SETUP, ROADMAP).

## [0.4.0] - 2026-07-05

### Aggiunto
- **Layer di rilevamento e dispatch multi-piattaforma** (`symbinux-devices`):
  fingerprinting USB a cascata che riconosce Nokia legacy / Android
  (ADB/fastboot/MTP/PTP/AOA) / Apple iOS / sconosciuto, con costanti confermate
  da gnokii, AOSP/AOA e libimobiledevice. Interfaccia comune `DeviceHandler`
  (strategy pattern) con `NokiaLegacyHandler`, `AndroidHandler`, `AppleHandler`
  e capability differenziate per piattaforma. `DeviceManager` traccia i
  dispositivi per **porta fisica** (bus + catena porte) così gli switch di
  modalità (AOA Android, trust iOS) sono seguiti e non persi. 15 test con
  fingerprint sintetici per ogni categoria.
- Comando CLI **`detect`** (con `--progress` per progresso reale) che mostra
  piattaforma e capability dei telefoni collegati.
- Regole udev separate per categoria (`51-android.rules`), guida `udev/README.md`
  e `docs/DEVICE_DETECTION.md` (cascata, matrice capability, note d'integrazione:
  usbmuxd per iOS, adb_client/idevice per Android).
- La GUI ora usa il rilevamento multi-piattaforma: la lista mostra piattaforma e
  capability di ogni telefono, e i pulsanti-funzione si abilitano in base alle
  capability effettive del dispositivo selezionato. La **barra di progresso a
  percentuale** è guidata dai passi reali del comando `detect`.

### Modificato
- **Revisione UX/UI della GUI**: dimensione minima della finestra imposta
  (720×600, default 860×680) così il contenuto non è mai compresso; logo più
  grande e ben proporzionato (wordmark compatto in header + logo grande
  nell'empty state, entrambi si adattano al tema); pulsanti-funzione in un
  `Adw.WrapBox` con spaziatura corretta che va a capo su finestre strette;
  versione mostrata senza duplicare il nome.
- **Feedback di attesa onesto**: scansione USB spostata fuori dal thread della
  UI (non blocca più l'interfaccia) con **spinner** durante l'attesa; aggiunto
  un pannello di progresso con **barra a percentuale reale** (guidata dagli step
  effettivi di un'operazione, mai animazioni finte).
- In alto a sinistra ora c'è il nome "SYMBINUX" in maiuscolo grassetto (al posto
  del logo piccolo); il logo grande resta nell'empty state.
- All'avvio la lingua "Automatico" sceglie la lingua di sistema se ne è
  disponibile una traduzione, altrimenti ripiega esplicitamente sull'inglese.

## [0.3.0] - 2026-07-05

### Aggiunto
- **Selettore tema chiaro/scuro** (menu Aspetto): Automatico / Chiaro / Scuro.
  In "Automatico" segue la preferenza del desktop (light/dark) tramite libadwaita
  (portale freedesktop); se il DE non espone la preferenza, ripiega su **Scuro**.
  Il logo si adatta automaticamente alla variante corretta (blu su chiaro,
  arancione su scuro). La scelta è persistente.
- **Internazionalizzazione (gettext)**: interfaccia tradotta in **7 lingue** —
  inglese (sorgente), italiano, tedesco, spagnolo, francese, olandese,
  portoghese — selezionabili dal menu Lingua (Automatico segue il locale di
  sistema). Infrastruttura `po/` completa (`symbinux.pot`, un `.po` per lingua,
  `LINGUAS`, `compile.sh`, guida per traduttori) predisposta per aggiungerne
  altre.
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
