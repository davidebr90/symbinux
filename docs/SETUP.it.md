# Installazione e accesso non privilegiato

*[Read in English](SETUP.md)*

Symbinux è pensato per funzionare **senza `sudo`** nell'uso normale. L'unico
passo privilegiato è installare una volta una regola udev, così il tuo utente può
aprire il dispositivo USB del telefono e la sua porta seriale.

## 1. Compila il core

```bash
# Core + CLI in Rust
cargo build --release
# il binario finisce in target/release/symbinux-fbus
```

Dipendenze a runtime (Debian/Ubuntu): nessuna per l'USB raw — l'accesso è in Rust
puro via `nusb`, quindi non serve `libusb`. Per un cavo che espone una porta
seriale, i driver kernel `ftdi_sio` / `cp210x` / `pl2303` (presenti di default)
più `libudev` per l'enumerazione delle porte. Opzionali, per i canali wireless: `bluez` (scansione Bluetooth) e
`network-manager` (scansione Wi-Fi); il supporto iOS richiede inoltre il demone
`usbmuxd` (vedi `udev/README.md`).

## 2. Installa le regole udev

```bash
# Nokia (sempre) e Android (se usi il canale Android):
sudo cp udev/69-nokia-legacy.rules udev/51-android.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

La regola concede l'accesso in due modi (dettagli nel file):

- `TAG+="uaccess"` — systemd-logind dà l'accesso all'utente loggato localmente.
  Preferito sulle distro moderne (systemd ≥ 213). Nessuna gestione di gruppi.
- `GROUP="dialout"` — fallback. Aggiungiti una volta e rifai il login:

  ```bash
  sudo usermod -aG dialout "$USER"
  ```

Crea anche un symlink stabile `/dev/nokia_fbus` per la porta seriale del cavo.

## 3. Verifica

```bash
# Cosa è collegato fisicamente (senza telefono):
symbinux-fbus devices --all

# Con un telefono collegato via cavo seriale:
symbinux-fbus identify --port /dev/nokia_fbus
```

Se `identify` non riesce ad aprire la porta, controlla:

- che il cavo esponga una `/dev/ttyUSB*` (`dmesg | tail` dopo averlo collegato),
- che il tuo utente abbia accesso (`ls -l /dev/ttyUSB0`; la regola udev dovrebbe
  dare gruppo `dialout` / uaccess),
- che il telefono sia acceso e non in modalità PC Suite / memoria di massa.

## 4. GUI

```bash
pip install -e ".[gui]"
symbinux            # avvia la GUI GTK4
```

La GUI invoca il binario `symbinux-fbus`. Se non è nel `PATH`, imposta
`SYMBINUX_FBUS_BIN=/percorso/di/symbinux-fbus`.

## Nota su WSL2

WSL2 non inoltra l'USB fisico di default. Per testare su hardware reale da
Windows, collega il dispositivo con [`usbipd-win`](https://github.com/dorssel/usbipd-win)
(`usbipd attach --wsl --busid <id>`). Il framing e il codec sono comunque
testabili senza hardware con `cargo test`.
