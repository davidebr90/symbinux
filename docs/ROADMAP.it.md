# Roadmap

*[Read in English](ROADMAP.md)*

Stato dello stack Symbinux e percorso previsto verso un supporto più ampio.

## Fatto (fino alla v0.4.0)

- **Core di protocollo (`symbinux-protocol`)** — codec FBUS/2 e MBUS v1 con
  checksum doppio/singolo, reader incrementale, builder di comandi con
  classificazione di sicurezza. Validato contro oracoli di checksum da catture
  reali.
- **Trasporto (`symbinux-transport`)** — backend seriale (termios, 115200 8N1),
  scheletro backend USB raw (libusb), enumerazione dispositivi stile lsusb,
  scambio richiesta/risposta FBUS/2.
- **Rilevamento dispositivi (`symbinux-devices`)** — fingerprinting a cascata
  (Nokia/Android/Apple iOS/sconosciuto), strategy `DeviceHandler` con capability
  per piattaforma, tracciamento per porta attraverso gli switch di modalità
  AOA/iOS.
- **CLI (`symbinux-fbus`)** — `devices`, `detect`, `identify`, `getphonebook`,
  `netmon`, `raw` (protetto). Flag in stile gnokii.
- **GUI** — GTK4/libadwaita: selettore di canale con rilevamento USB reale più
  scansioni reali Bluetooth (BlueZ) e Wi-Fi (NetworkManager), pulsanti-funzione
  consapevoli delle capability, progresso a percentuale reale, selettore tema,
  localizzazione in 7 lingue.
- **Packaging** — manifest Flatpak, regole udev per categoria, `devices.json`.
- **Decodifica tipizzata (iniziata)** — `symbinux-protocol::decode` trasforma la
  risposta versione HW/SW in una struct (validata contro la cattura reale del
  3310); output `--json` stabile su `devices`/`detect`; logging strutturato
  (`RUST_LOG`).

- **Link USB app-owned (iniziato)** — `symbinux-fbus identify --usb` fa claim
  diretto del dispositivo Nokia via libusb (driver kernel staccato, endpoint bulk
  FBUS auto-scoperti), così un telefono è raggiungibile senza alcun driver
  seriale del SO. Vedi `docs/CONNECTION_MODEL.md` — l'app possiede la connessione
  e forza il link, invece di dipendere da driver/demoni del SO.

Il backlog qui sotto è prioritizzato da una review multi-progetto; vedi
`docs/COMPARISON.md` per i progetti simili e `docs/CROSS_PLATFORM.md` per la
portabilità.

## Breve termine (P0/P1)

1. **Cablare le funzioni della GUI end-to-end.** I pulsanti
   Identifica/Rubrica/SMS/Netmonitor non chiamano ancora il core — serve un
   **risolutore di porta seriale** che mappi un dispositivo USB rilevato
   (`PortKey`/VID:PID) su un percorso `/dev/ttyUSB*`, poi la GUI può eseguire
   `identify` e mostrare il risultato decodificato. Questo è il blocco attuale
   per ogni operazione reale sul telefono dalla GUI.
2. **Decodifica tipizzata → formati PIM.** Estendere `decode` alle voci di
   rubrica (→ vCard `.vcf`) e al PDU SMS (7-bit/UCS2), poi `--json` su ogni
   comando. Sblocca contatti e SMS.
3. **SMS lista/lettura/invio end-to-end** e **rubrica lettura/scrittura** cablati
   in CLI + GUI, con conferma esplicita per le scritture `Experimental`.
4. **Finestra di ritrasmissione** — timeout ACK configurabile (200–500 ms) +
   retry secondo lo schema gnokii. (Il riassemblaggio delle risposte multi-frame
   è fatto: `exchange_fbus2` legge fino all'ultimo frammento e `reassemble_fbus2`
   li unisce.)
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

11. **CLI su Windows/macOS** — il core è già portabile; distribuire
    `symbinux-fbus` multipiattaforma, eventualmente migrando l'USB a `nusb` per
    eliminare la dipendenza libusb. Vedi `docs/CROSS_PLATFORM.md`.
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
