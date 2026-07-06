# Riferimento funzioni

*[Read in English](FUNCTIONS.md)*

Cosa può fare Symbinux oggi, tra GUI e CLI `symbinux-fbus`. Per ogni funzione
sono indicati l'icona usata nella GUI, cosa serve per abilitarla e la classe di
sicurezza (vedi `docs/PROTOCOL_NOTES.md` §6).

## Canali di connessione (GUI)

Il selettore di canale sceglie come Symbinux cerca un telefono. Ogni canale
esegue una scansione reale con spinner e uno stato vuoto/errore onesto — mai un
loader finto.

| Canale | Icona | Comportamento |
|---|---|---|
| USB | `drive-harddisk-usb-symbolic` | Rilevamento dispositivi multi-piattaforma + I/O seriale FBUS/2 (Nokia). |
| Bluetooth | `bluetooth-symbolic` | Ricerca dispositivi reale via BlueZ (`bluetoothctl`); serve un adattatore. |
| Wi-Fi | `network-wireless-symbolic` | Scansione reti reale via NetworkManager (`nmcli`). |

## Funzioni telefono (GUI)

I pulsanti-funzione sono **disabilitati (grigi) finché non si seleziona un
dispositivo compatibile**, così l'insieme delle capacità è sempre visibile anche
senza nulla collegato. Un pulsante si abilita solo se la sua capability è
presente.

| Funzione | Icona | Abilitata quando | Sicurezza | Cosa fa |
|---|---|---|---|---|
| Identifica | `dialog-information-symbolic` | è selezionato un telefono Nokia | Confermato | Legge modello, IMEI, versione hardware e firmware. |
| Rubrica | `contact-new-symbolic` | è selezionato un telefono Nokia | Confermato (lettura) / Sperimentale (scrittura) | Importa/esporta contatti da memoria ME/SIM. |
| SMS | `mail-message-new-symbolic` | è selezionato un telefono Nokia | Sperimentale | Legge e invia messaggi di testo. |
| Netmonitor | `network-cellular-signal-excellent-symbolic` | è selezionato un telefono Nokia | Confermato | Schermate di diagnostica/ingegneria di rete. |
| Avanzate | `utilities-terminal-symbolic` | sempre | Confermato | Inventario grezzo di tutto ciò che è collegato (vedi sotto). |

Il logo e la versione sono sempre mostrati nell'header e nella finestra
Informazioni. I risultati delle azioni arrivano come **notifiche desktop native**
(specifica freedesktop), quindi compaiono in GNOME, KDE e altri desktop senza
dipendenze aggiuntive.

## Modalità avanzata (diagnostica)

La funzione Avanzate elenca **ogni dispositivo USB visibile all'host** (stile
lsusb) con id vendor:product, nomi estesi produttore/prodotto, bus/indirizzo e
una classificazione (telefono Nokia / cavo bridge noto / altro). Serve a fare
debug delle segnalazioni "perché il telefono non viene rilevato" — si vedono gli
id grezzi anche per cavi non riconosciuti. Non esegue I/O col telefono.

## Comandi CLI (`symbinux-fbus`)

| Comando | Scopo | Sicurezza |
|---|---|---|
| `devices [--all] [--json]` | Enumerazione avanzata. Senza `--all` mostra solo telefoni e cavi bridge noti. `--json` produce un formato macchina stabile. | Confermato |
| `detect [--progress] [--json]` | Rileva automaticamente piattaforma e capability del telefono collegato. `--json` per lo scripting. | Confermato |
| `ports [--json]` | Elenca le porte seriali esposte dal SO (con id USB). | Confermato |
| `identify [--port <p>] [--usb] [--json]` | Query versione HW/SW. `--usb` fa claim diretto del Nokia via libusb (nessun driver seriale); `--json` stampa modello/firmware/data decodificati. | Confermato |
| `getphonebook --port <p> --mem <me\|sim\|…> --location <n>` | Legge una voce di rubrica. | Confermato |
| `netmon --port <p> [--screen <n>]` | Schermata/controllo netmonitor. | Confermato |
| `raw --port <p> --msg-type <hex> --block "<hex …>" --i-understand-risk` | Invia un frame FBUS/2 arbitrario (reverse engineering). | Sperimentale |
| `completions <bash\|zsh\|fish\|…>` | Stampa uno script di completamento shell su stdout. | Confermato |
| `man` | Stampa la man page roff su stdout. | Confermato |
| `decode-frame <hex>` | Decodifica offline un frame FBUS/2 catturato (senza dispositivo). | Confermato |
| `decode-sms <hex>` | Decodifica offline un PDU SMS-DELIVER catturato (senza dispositivo). | Confermato |

Nella GUI, il pulsante **Identifica** mostra l'identità decodificata come scheda
(modello / firmware / data) invece che testo grezzo, risolvendo automaticamente
la porta seriale del telefono. Vedi `docs/CONNECTION_MODEL.md` per i percorsi
app-owned USB/Bluetooth.

Ogni comando telefono invia prima il preambolo di init `0x55`, poi la richiesta
in frame, e stampa i byte richiesti, l'ACK e la risposta decodificata (con resa
ASCII quando il payload è testo).

### Garanzie di sicurezza

- Le scritture firmware/flash **non sono implementate** e sono rifiutate come
  `Dangerous`.
- La modalità `raw` richiede il flag esplicito `--i-understand-risk`.
- Di default girano solo i comandi `Confermato`; quelli `Sperimentale` modificano
  il telefono e richiedono un opt-in deliberato.

## API libreria (Rust)

- `symbinux-protocol` — `Fbus2Frame`, `MbusFrame`, `Fbus2Reader` e il modulo
  `message` (builder di comandi + `Safety`). Solo framing, nessun I/O,
  interamente testato.
- `symbinux-transport` — trait `Transport` con `SerialTransport` e
  `UsbTransport`, `list_usb_devices()` e `exchange_fbus2()`.
