# Symbinux

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo/symbinux_logo_transparent_dark.png">
  <img alt="Logo Symbinux" src="assets/logo/symbinux_logo_transparent_light.png" width="320">
</picture>

*[Read this document in English](README.md)*

Comunica con i telefoni Nokia legacy da un desktop GNU/Linux moderno. Symbinux è
un'implementazione clean-room dei protocolli seriali Nokia **FBUS/MBUS** su USB
(oggi via cavo seriale; USB raw/BB5 in roadmap), distribuita come core + CLI in
Rust con una GUI GTK4/libadwaita.

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
- **Inventario dispositivi avanzato** — vista in stile lsusb di tutto ciò che è
  collegato (VID:PID, nomi estesi, classificazione) per il debug del
  riconoscimento.
- **Modalità frame raw** per il reverse engineering del protocollo.

Vedi [docs/FUNCTIONS.md](docs/FUNCTIONS.md) per il riferimento completo e le
classi di sicurezza.

## Architettura

```
symbinux/
├── crates/                     # workspace Rust (il core)
│   ├── symbinux-protocol/      # framing FBUS/MBUS — puro, senza I/O, testato
│   ├── symbinux-transport/     # seriale (termios) + USB raw (libusb), enumerazione
│   └── symbinux-cli/           # `symbinux-fbus`, CLI in stile gnokii
├── src/symbinux/               # GUI GTK4 + libadwaita (Python), chiama la CLI
├── udev/                       # regole per accesso non privilegiato
├── data/devices.json           # tabella VID/PID nota (mantenuta dalla community)
├── docs/                       # PROTOCOL_NOTES / FUNCTIONS / ROADMAP / SETUP
└── packaging/flatpak/          # manifest Flatpak
```

I livelli sono nettamente separati: framing (no I/O) → trasporto (I/O) → CLI →
GUI. La GUI non contiene logica di protocollo; invoca `symbinux-fbus`.

## Avvio rapido

```bash
# Core + CLI
cargo build --release

# Cosa è collegato ora (senza telefono):
target/release/symbinux-fbus devices --all

# Identifica un telefono via cavo DKU-2/CA-42:
target/release/symbinux-fbus identify --port /dev/nokia_fbus

# GUI
pip install -e ".[gui]"
symbinux
```

L'accesso non privilegiato (niente `sudo` nell'uso normale) richiede
un'installazione udev una tantum — vedi [docs/SETUP.md](docs/SETUP.md).

## Requisiti

- Rust ≥ 1.74, `libusb-1.0`, `pkg-config`.
- Per la GUI: Python ≥ 3.11, GTK4 e libadwaita (`gir1.2-gtk-4.0`,
  `gir1.2-adw-1` su Debian/Ubuntu).
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
