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

## Breve termine

1. **Copertura comandi FBUS/2** — cablare il parsing end-to-end della risposta di
   `identify` (struct modello/IMEI/firmware), lettura rubrica → contatti
   strutturati, import/export vCard. Il framing è completo; i decoder delle
   risposte sono parziali.
2. **MBUS v1 su hardware** — validare il codec sintetico contro un telefono reale,
   aggiungere lo scarico dell'eco half-duplex nel loop di scambio, sostituire la
   fixture sintetica con una cattura reale.
3. **Finestra di ritrasmissione** — timeout ACK configurabile (200–500 ms) e
   retry per FBUS/2, secondo lo schema di sequence number di gnokii.

## Medio termine

4. **FBUS/2 su USB raw (DKU-2 nativo)** — selezionare la configurazione/interfaccia
   USB alternativa che espone i due endpoint bulk FBUS (la config di default
   emula un modem AT), poi riusare il framing esistente su `UsbTransport`.
5. **Telefoni BB5** — auto-discovery degli endpoint sull'interfaccia bulk PhoNet;
   tabella interface/altsetting per modello in `devices.json`.
6. **Comunicazione telefono via Bluetooth** — il canale Bluetooth già rileva i
   dispositivi; il prossimo passo è FBUS/MBUS (o OBEX) su RFCOMM via BlueZ per
   parlare davvero con un telefono.

## Esplicitamente fuori ambito

- Flashing firmware / operazioni di scrittura (rischio brick su hardware non
  supportato).
- Qualsiasi dipendenza da software Nokia proprietario o binari reverse-engineered.

## Come aiutare

Le catture reali sono il collo di bottiglia. Vedi `docs/PROTOCOL_NOTES.md` §7 per
le domande aperte e la metodologia di cattura.
