# Roadmap

*[Read in English](ROADMAP.md)*

Stato dello stack Symbinux e percorso previsto verso un supporto più ampio.

## Fatto (fino alla v0.4.0)

- **Core di protocollo (`symbinux-protocol`)** — codec FBUS/2 e MBUS v1 con
  checksum doppio/singolo, reader incrementale, builder di comandi con
  classificazione di sicurezza. Validato contro oracoli di checksum da catture
  reali.
- **Trasporto (`symbinux-transport`)** — backend seriale (termios, 115200 8N1),
  backend USB raw via `nusb` (Rust puro, niente libusb), enumerazione dispositivi
  stile lsusb, scambio richiesta/risposta FBUS/2.
- **Rilevamento dispositivi (`symbinux-devices`)** — fingerprinting a cascata
  (Nokia/Android/Apple iOS/sconosciuto), strategy `DeviceHandler` con capability
  per piattaforma, tracciamento per porta attraverso gli switch di modalità
  AOA/iOS.
- **CLI (`symbinux-fbus`)** — `devices`, `detect`, `identify`, `getphonebook`,
  `netmon`, `raw` (protetto). Flag in stile gnokii.
- **GUI (Rust, `symbinux-gui`)** — gtk4-rs senza libadwaita, che linka il core
  direttamente (nessun subprocess): selettore di canale con rilevamento USB
  reale, scansioni Bluetooth/Wi-Fi reali, pulsanti-funzione consapevoli delle
  capability, card Identifica diretta, progresso a percentuale reale con
  Annulla, selettore tema (Automatico segue il desktop via portal XDG),
  localizzazione in 11 lingue dai file `.po`. Gira su Linux e Windows; compila
  e passa i test su macOS in CI. La GUI Python GTK4/libadwaita resta usabile
  finché la parità non è validata su hardware (PBAP) e la Fase 5 di
  `docs/CROSS_PLATFORM_GUI_PLAN.md` non la ritira.
- **Core wireless (`symbinux-wireless`)** — scansioni Bluetooth/Wi-Fi,
  scaricamento contatti PBAP e notifiche desktop dietro un'unica API portabile:
  BlueZ / NetworkManager / obexd su Linux, scansione solo-BLE via `btleplug` su
  Windows/macOS (verificata dal vivo su Windows), notifiche `notify-rust`
  ovunque.
- **Packaging** — manifest Flatpak, regole udev per categoria, `devices.json`,
  più una dist portable Windows + installer Inno Setup per-utente
  (`packaging/windows/`, runtime GTK incluso, verificato end-to-end).
- **Decodifica tipizzata (iniziata)** — `symbinux-protocol::decode` trasforma la
  risposta versione HW/SW in una struct (validata contro la cattura reale del
  3310); output `--json` stabile su `devices`/`detect`; logging strutturato
  (`RUST_LOG`).

- **Link USB app-owned (iniziato)** — `symbinux-fbus identify --usb` fa claim
  diretto del dispositivo Nokia via `nusb` (driver kernel staccato, endpoint bulk
  FBUS auto-scoperti), così un telefono è raggiungibile senza alcun driver
  seriale del SO. Vedi `docs/CONNECTION_MODEL.md` — l'app possiede la connessione
  e forza il link, invece di dipendere da driver/demoni del SO.

Il backlog qui sotto è prioritizzato da una review multi-progetto; vedi
`docs/COMPARISON.md` per i progetti simili e `docs/CROSS_PLATFORM.md` per la
portabilità.

## Breve termine (P0/P1)

1. **Cablare le funzioni GUI rimanenti end-to-end.** Il risolutore di porta
   seriale esiste e Identifica chiama già il core direttamente; i pulsanti
   Rubrica, SMS e Netmonitor mostrano ancora uno stato onesto "non collegato" —
   sono bloccati dai decoder delle risposte (punto 4), non dalla plumbing.
2. **Decodifica tipizzata → formati PIM.** Estendere `decode` alle voci di
   rubrica (→ vCard `.vcf`) e al PDU SMS (7-bit/UCS2), poi `--json` su ogni
   comando. Sblocca contatti e SMS.
3. **SMS lista/lettura/invio end-to-end** e **rubrica lettura/scrittura** cablati
   in CLI + GUI, con conferma esplicita per le scritture `Experimental`.
4. **Decoder delle risposte su hardware** — il trasporto è ora robusto
   (riassemblaggio multi-frame, finestra di ritrasmissione, risync del reader
   tutti fatti); il lavoro a breve rimasto è decodificare le *risposte* reali di
   rubrica/SMS in struct tipizzate, che richiede una cattura reale o un telefono
   per validare il layout dei byte.
5. **MBUS v1 su hardware** — chiamare `drain_echo` in un loop di scambio MBUS,
   validare contro un telefono reale, sostituire la fixture sintetica.
6. **Robustezza** — differenziare gli errori GUI (binario mancante vs permessi vs
   timeout vs nessun device) con testo utile; test fuzz/property per
   `Fbus2Reader`; cancellazione per scansioni lunghe.

## Medio termine

7. **Backup/ripristino** — un comando che esporta rubrica + SMS (+ calendario)
   in `.vcf`/`.ics`/`.json`; aggiungere `Capability::Backup` all'handler Nokia.
8. **Registro chiamate, calendario/todo, suonerie, loghi** — riusare il framing
   `0x03`/security generalizzato (gnokii/gammu li cablano sulle stesse famiglie).
9. **Comunicazione telefono via Bluetooth** — PBAP (contatti) e MAP (SMS) via il
   demone BlueZ `obexd` su D-Bus, per raggiungere i Nokia con USB morta (vedi
   `docs/COMPARISON.md`).
10. **FBUS/2 su USB raw (DKU-2 nativo)** e **BB5** — auto-discovery degli endpoint
    bulk FBUS / interfaccia PhoNet; tabella per modello in `devices.json`.

## Infrastruttura e multipiattaforma

11. **CLI su Windows/macOS** — il core è già portabile e il layer USB usa già
    `nusb` (Rust puro, niente libusb), quindi i binari sono self-contained;
    distribuire `symbinux-fbus` multipiattaforma. Vedi `docs/CROSS_PLATFORM.md`.
12. **Servizio D-Bus** (zbus) che espone stato dispositivi + eventi hotplug alla
    GUI e ad altre app, stile KDE Connect — sostituisce a lungo termine lo
    scraping da subprocess.
13. **Transfer Android/iOS** — incorporare `adb_client` (Rust) e `idevice` (Rust)
    invece di lanciare processi esterni, quando lo scope si amplierà.

## Esplicitamente fuori ambito

- Flashing firmware / operazioni di scrittura (rischio brick su hardware non
  supportato).
- Qualsiasi dipendenza da software Nokia proprietario o binari reverse-engineered.

## Come aiutare

Le catture reali sono il collo di bottiglia. Vedi `docs/PROTOCOL_NOTES.md` §7 per
le domande aperte e la metodologia di cattura.
