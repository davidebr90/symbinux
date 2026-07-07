# Changelog

*[Read this changelog in English](CHANGELOG.md)*

Tutte le modifiche rilevanti a questo progetto sono documentate qui.

Il formato segue [Keep a Changelog](https://keepachangelog.com/it/1.1.0/),
e il progetto aderisce a [Semantic Versioning](https://semver.org/lang/it/).

## [Unreleased]

### Aggiunto
- **Shell Phase 1 della GUI Rust GTK4**: `symbinux-gui` ora replica la cornice
  principale della GUI Python con header wordmark/versione, selettore
  USB/Bluetooth/Wi-Fi, logo nello stato vuoto adattato al tema, progresso reale
  della detection USB con Annulla e pulsanti Nokia abilitati in base alle
  capability.
- **Identifica nella GUI Rust**: l'azione Identifica ora risolve la porta seriale
  e chiama direttamente `symbinux-transport` / `symbinux-protocol`, poi mostra
  modello, firmware e data decodificati in una card GTK4.
- **Scansioni wireless nella GUI Rust**: i canali Bluetooth e Wi-Fi ora eseguono
  scansioni reali `bluetoothctl` / `nmcli` dalla GUI Rust con spinner annullabile
  ed errori onesti quando lo stack host manca. I contatti PBAP restano
  esplicitamente pendenti.
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
- **Riassemblaggio risposte multi-frame**: `exchange_fbus2` legge una risposta
  frammentata fino all'ultimo frame e `reassemble_fbus2` unisce i frammenti in
  un unico payload (testato), così le risposte lunghe non vengono troncate.
- **Finestra di ritrasmissione** (`ExchangeConfig` / `exchange_fbus2_with`): il
  comando viene rinviato se il telefono resta in silenzio oltre un timeout per
  tentativo (stile gnokii), fino a N retry, prima di fallire — testato con mock.
- **Scheda identità nella GUI**: il pulsante Identifica ora mostra modello /
  firmware / data decodificati come scheda (via `identify --json`) invece che
  testo grezzo.
- **Subcomando CLI `completions`** (bash/zsh/fish/…) e `identify --json`.
- **Packaging Flatpak**: launcher `.desktop`, `metainfo.xml` AppStream, una vera
  **icona quadrata** (`assets/logo/symbinux_icon.png`), completamenti shell e man
  page installati; il manifest apre i nomi D-Bus per Bluetooth (BlueZ/obexd) e
  Wi-Fi (NetworkManager). Il CI ora valida i metadati desktop e AppStream.
- **Comandi di decodifica offline**: `decode-frame <hex>` e `decode-sms <hex>`
  decodificano frame FBUS/2 e PDU SMS-DELIVER catturati senza un dispositivo —
  utile per il reverse engineering. Più un subcomando `man` (man page roff).
- **Altre quattro lingue UI**: polacco, russo, cinese semplificato e giapponese
  (**11 lingue** totali).
- **Errori GUI utili**: i fallimenti comuni (permesso negato, porta seriale
  mancante, timeout) ora riportano un suggerimento su come risolvere.
- **File di config CLI** (`~/.config/symbinux/config.toml`, `%APPDATA%` su
  Windows): `default_port`, `ack_timeout_ms`, `retries` e `log_level` opzionali;
  i comandi usano `default_port` quando `--port` è omesso.
- **Pulsante Annulla** nel pannello di progresso: chiude una scansione in corso
  (e killa il subprocess `detect`). "Annulla" tradotto in tutte le lingue.

### Corretto
- **Busy-loop della CPU in `exchange_fbus2`**: aggiunto un breve back-off tra le
  read vuote, così l'attesa di una risposta non satura più un core.
- **Troncamento silenzioso in `write_phonebook`**: ora restituisce un errore
  invece di troncare nome/numero più lunghi del campo lunghezza a un byte.
- **Wedge del `Fbus2Reader` su rumore di linea**: un falso marker `0x1E` che
  dichiara una lunghezza implausibile non blocca più il reader all'infinito —
  risincronizza e il buffer resta limitato (test fuzz pseudo-casuale).

### Modificato
- **Layer USB migrato da `rusb`/libusb a [`nusb`](https://docs.rs/nusb) in Rust
  puro**: il claim diretto del dispositivo USB, il distacco del driver kernel (ora
  `detach_and_claim_interface`), la scoperta degli endpoint e l'I/O bulk non
  dipendono più dalla libreria C libusb, quindi i binari Linux/Windows/macOS sono
  self-contained (niente `libusb-1.0.dll`/`.dylib` da distribuire; la CI non
  installa più `libusb-1.0-0-dev`). L'enumerazione legge ora le stringhe dei
  descrittori dalla cache del SO senza aprire ogni dispositivo. La logica di
  selezione endpoint e mappatura errori è coperta da unit test; la parità
  dell'I/O bulk su dispositivo va validata su hardware Nokia reale. Vedi
  `docs/NUSB_MIGRATION_STUDY.md` per l'analisi alla base della scelta.
- **La regola udev marca i dispositivi Nokia con `ID_MM_DEVICE_IGNORE`** così
  ModemManager non afferra più il telefono prima che l'app possa fare il claim
  sul path `--usb` (USB app-owned), sui desktop con ModemManager attivo.
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
