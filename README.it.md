# Symbinux

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo/symbinux_logo_transparent_dark.png">
  <img alt="Logo Symbinux" src="assets/logo/symbinux_logo_transparent_light.png" width="320">
</picture>

*[Read this document in English](README.md)*

Comunica con i telefoni Nokia legacy da un desktop moderno Linux, Windows o
macOS. Symbinux è un'implementazione clean-room dei protocolli seriali Nokia
**FBUS/MBUS** su USB (oggi via cavo seriale; USB raw/BB5 in roadmap), distribuita
come core + CLI in Rust con una **GUI GTK4 (`gtk4-rs`) cross-platform**. Una GUI
libadwaita in Python resta usabile finché la GUI Rust non la sostituisce del
tutto.

**Symbinux è un fork dichiarato di [Nokinux](https://launchpad.net/nokinux)**
(2008-2010), progetto Bash/Python nato nella comunità Ubuntu italiana per
configurare cellulari Nokia da Linux, di cui Davide Pica (davidebr90) è stato tra
gli autori originali insieme ad altri contributori. Symbinux ne riprende il nome
e lo spirito, riscritto da zero secondo gli standard attuali.

Il protocollo è ricostruito dai progetti open source **gnokii** e **gammu** e
validato contro catture reali documentate. Non usa **alcun codice, libreria o
binario proprietario Nokia**.

## Cosa fa

- **Identificazione** del telefono (modello, IMEI, versione hardware e firmware).
- **Rubrica** in lettura (e scrittura sperimentale) su memoria ME/SIM.
- **Netmonitor** per diagnostica di rete.
- **SMS** lettura/invio (sperimentale).
- **Rilevamento wireless** — scansione Bluetooth e Wi-Fi, più scaricamento
  contatti PBAP via Bluetooth su Linux, dietro un'unica API portabile
  `symbinux-wireless` (BLE su Windows/macOS via `btleplug`, stati "non
  disponibile" onesti altrove).
- **Classificazione dispositivi** — i dispositivi Bluetooth rilevati sono
  etichettati per produttore e forma (orologio Apple, telefono Android, TV,
  cuffie…) da segnali di identificazione vanilla, mostrati come badge combinati.
- **Recupero / export dati** — rubrica, messaggi e calendario recuperati si
  normalizzano in record portabili **vCard / vMessage / iCalendar**
  indipendentemente dal trasporto.
- **Inventario dispositivi avanzato** — vista in stile lsusb di tutto ciò che è
  collegato (VID:PID, nomi estesi, classificazione) per il debug del
  riconoscimento.
- **Modalità frame raw** per il reverse engineering del protocollo.

Vedi [docs/FUNCTIONS.md](docs/FUNCTIONS.md) per il riferimento CLI completo e le
classi di sicurezza, e [docs/NOKIA_SERVICE_MODES.md](docs/NOKIA_SERVICE_MODES.md)
/ [docs/VANILLA_CONNECTIVITY.md](docs/VANILLA_CONNECTIVITY.md) per come si
raggiunge il telefono senza alcun software installato su di esso.

## Architettura

```
symbinux/
├── crates/                     # workspace Rust (il core)
│   ├── symbinux-protocol/      # framing FBUS/MBUS + decoder/export tipizzati — puro, senza I/O, testato
│   ├── symbinux-transport/     # seriale (termios) + USB raw (nusb, Rust puro), enumerazione
│   ├── symbinux-devices/       # fingerprinting USB + dispatch per piattaforma
│   ├── symbinux-wireless/      # Bluetooth/Wi-Fi/PBAP/notifiche portabili
│   ├── symbinux-cli/           # `symbinux-fbus`, CLI in stile gnokii
│   └── symbinux-gui/           # GUI desktop gtk4-rs — linka il core direttamente
├── src/symbinux/               # GUI legacy GTK4 + libadwaita (Python), invoca la CLI
├── udev/                       # regole per accesso non privilegiato
├── data/devices.json           # tabella VID/PID nota (mantenuta dalla community)
├── docs/                       # PROTOCOL_NOTES / FUNCTIONS / ROADMAP / SETUP / …
└── packaging/                  # flatpak/ (Linux) + windows/ (installer)
```

I livelli sono nettamente separati: framing (no I/O) → trasporto (I/O) → CLI /
GUI. La GUI Rust (`symbinux-gui`) **linka i crate del core direttamente** —
nessun bridge a sottoprocesso; la GUI Python legacy non contiene logica di
protocollo e invoca `symbinux-fbus`.

## Avvio rapido

```bash
# Core + CLI
cargo build --release

# Cosa è collegato ora (senza telefono):
target/release/symbinux-fbus devices --all

# Identifica un telefono via cavo DKU-2/CA-42 (o --usb per il claim diretto):
target/release/symbinux-fbus identify --port /dev/nokia_fbus

# Completamenti shell (bash/zsh/fish/…):
target/release/symbinux-fbus completions bash > ~/.local/share/bash-completion/completions/symbinux-fbus

# GUI (Rust · GTK4, Linux/Windows/macOS)
cargo build --release -p symbinux-gui
target/release/symbinux-gui

# GUI Python legacy (Linux, finché la GUI Rust non la sostituisce del tutto)
pip install -e ".[gui]"
symbinux
```

Su Windows la GUI si distribuisce come installer per-utente avviabile con
doppio clic (runtime GTK incluso) — vedi
[packaging/windows/README.md](packaging/windows/README.md).

L'accesso non privilegiato (niente `sudo` nell'uso normale) richiede
un'installazione udev una tantum — vedi [docs/SETUP.md](docs/SETUP.md). Come
l'app possiede la connessione (claim diretto dell'USB, pairing Bluetooth forzato)
è descritto in [docs/CONNECTION_MODEL.md](docs/CONNECTION_MODEL.md).

## Requisiti

- Rust ≥ 1.89. Su Linux, `libudev` + `pkg-config` (per l'enumerazione delle porte
  seriali). Niente libusb — l'accesso USB raw è in Rust puro via
  [`nusb`](https://docs.rs/nusb).
- Per la GUI Rust: le librerie di sviluppo GTK4 (`libgtk-4-dev` su
  Debian/Ubuntu, MSYS2 `mingw-w64-x86_64-gtk4` su Windows, `brew install gtk4`
  su macOS).
- Per la GUI Python legacy: Python ≥ 3.11, GTK4 e libadwaita
  (`gir1.2-gtk-4.0`, `gir1.2-adw-1` su Debian/Ubuntu).
- Una macchina Linux reale, o WSL2 con passthrough USB per i test hardware. Il
  codec di protocollo è testabile senza hardware (`cargo test`).

## Test

```bash
cargo test        # codec di protocollo contro fixture di catture reali + trasporto
pytest            # GUI/backend Python
```

## Sicurezza

Le scritture firmware/flash **non sono implementate** e vengono rifiutate. Di
default girano solo comandi di lettura; tutto ciò che modifica il telefono è
opt-in, e la modalità frame raw è protetta da un flag esplicito. Dettagli in
[docs/PROTOCOL_NOTES.md](docs/PROTOCOL_NOTES.md).

## Licenza

**GNU AGPLv3** (o successiva). Vedi [LICENSE](LICENSE). È compatibile con la
GPL/LGPL della documentazione gnokii/gammu da cui derivano le note di protocollo.

## Changelog

Vedi [CHANGELOG.md](CHANGELOG.md).
