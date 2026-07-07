//! `symbinux-fbus`: a gnokii-style command-line tool for legacy Nokia phones.
//!
//! Talks FBUS/2 over a serial cable (DKU-2/CA-42) or raw USB. Includes an
//! "advanced" device-enumeration mode for debugging what is physically
//! connected, and a raw-frame mode for protocol reverse-engineering.

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use log::debug;
use serde_json::json;

use symbinux_devices::{detect_staged, dispatch, DeviceKind};
use symbinux_protocol::message::{self, MemoryType, Safety};
use symbinux_protocol::{decode_sms_deliver, hw_sw_version, reassemble_fbus2, Fbus2Frame};
use symbinux_transport::{
    available_serial_ports, exchange_fbus2_with, list_usb_devices, ExchangeConfig, Role,
    SerialTransport, Transport, UsbTransport,
};

mod config;
use config::Config;

/// Nokia Mobile Phones USB vendor id (used by the app-owned USB path).
const NOKIA_VID: u16 = 0x0421;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "symbinux-fbus",
    version = VERSION,
    about = "Talk to legacy Nokia phones (Series 40/60, BB5) over FBUS/MBUS",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List USB devices the host can see (advanced diagnostics, lsusb-style).
    Devices {
        /// Show every USB device, not just phones and known cable bridges.
        #[arg(long)]
        all: bool,
        /// Emit a JSON array instead of a text table (stable machine format).
        #[arg(long)]
        json: bool,
    },
    /// Auto-detect connected phones and show their platform and capabilities.
    Detect {
        /// Emit machine-readable progress lines (`PROGRESS done total stage`)
        /// so a caller can drive a real progress bar.
        #[arg(long)]
        progress: bool,
        /// Emit a JSON array of detected phones instead of text lines.
        #[arg(long)]
        json: bool,
    },
    /// List the serial ports the OS exposes (with USB ids where known).
    Ports {
        /// Emit a JSON array instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Query the phone's hardware and software version.
    Identify {
        /// Serial port (Linux: /dev/ttyUSB0 or /dev/nokia_fbus; Windows: COM3).
        #[arg(long)]
        port: Option<String>,
        /// Claim the Nokia USB device directly via libusb instead of a serial
        /// port — the app-owned path that works without an OS serial driver.
        #[arg(long)]
        usb: bool,
        /// Emit the decoded identity (model/firmware/date) as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print shell completions to stdout (bash, zsh, fish, powershell, elvish).
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Print the man page (roff) to stdout.
    Man,
    /// Decode a captured FBUS/2 frame from hex (offline, no device needed).
    DecodeFrame {
        /// The frame bytes as hex, e.g. "1E 0C 00 7F 00 02 D1 00 CF 71".
        hex: String,
    },
    /// Decode a captured SMS-DELIVER PDU from hex (offline, no device needed).
    DecodeSms {
        /// The PDU bytes as hex.
        hex: String,
    },
    /// Read a phonebook entry.
    Getphonebook {
        /// Serial port; falls back to `default_port` from config.toml.
        #[arg(long)]
        port: Option<String>,
        /// Memory: me (phone), sim, combined, own, dialled, missed.
        #[arg(long, default_value = "me")]
        mem: String,
        /// 1-based entry location.
        #[arg(long)]
        location: u8,
    },
    /// Show or control the netmonitor.
    Netmon {
        /// Serial port; falls back to `default_port` from config.toml.
        #[arg(long)]
        port: Option<String>,
        /// Screen number, or 255 for "next".
        #[arg(long, default_value_t = 255)]
        screen: u8,
    },
    /// Send a raw FBUS/2 frame (reverse-engineering mode). Prints the reply.
    Raw {
        /// Serial port; falls back to `default_port` from config.toml.
        #[arg(long)]
        port: Option<String>,
        /// Message type byte, e.g. 0xD1.
        #[arg(long, value_parser = parse_u8_hex)]
        msg_type: u8,
        /// Block payload as hex, e.g. "00 03 00".
        #[arg(long, default_value = "")]
        block: String,
        /// I understand a wrong frame could in theory harm the phone.
        #[arg(long)]
        i_understand_risk: bool,
    },
}

fn parse_u8_hex(s: &str) -> Result<u8, String> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(s, 16).map_err(|e| e.to_string())
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>> {
    s.split_whitespace()
        .map(|tok| {
            let tok = tok.trim_start_matches("0x").trim_start_matches("0X");
            u8::from_str_radix(tok, 16).with_context(|| format!("invalid hex byte '{tok}'"))
        })
        .collect()
}

/// Parse hex bytes, tolerating spaces and separators (e.g. "1E0C" or "1E 0C").
fn parse_hex_flexible(s: &str) -> Result<Vec<u8>> {
    let compact: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if !compact.len().is_multiple_of(2) {
        bail!("hex string has an odd number of digits");
    }
    (0..compact.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&compact[i..i + 2], 16)
                .with_context(|| format!("invalid hex byte at offset {i}"))
        })
        .collect()
}

fn cmd_decode_frame(hex: &str) -> Result<()> {
    let bytes = parse_hex_flexible(hex)?;
    let (frame, used) =
        Fbus2Frame::decode(&bytes).map_err(|e| anyhow::anyhow!("not a valid FBUS/2 frame: {e}"))?;
    println!(
        "dest={:#04x} src={:#04x} msg_type={:#04x}  ({used} bytes)",
        frame.dest, frame.src, frame.msg_type
    );
    if frame.is_ack() {
        // ACK payload is [acked_msg_type, acked_seq], not a normal block.
        let acked = frame.data.first().copied().unwrap_or(0);
        println!("(acknowledgement of msg {acked:#04x})");
    } else {
        match frame.block_parts() {
            Some((block, frames_to_go, seq)) => println!(
                "block={}  frames_to_go={frames_to_go} seq={seq:#04x}",
                hexdump(block)
            ),
            None => println!("data={}", hexdump(&frame.data)),
        }
        if let Some(v) = hw_sw_version(&frame) {
            println!(
                "decoded: model={} firmware={} date={}",
                v.model, v.firmware, v.date
            );
        }
    }
    Ok(())
}

fn cmd_decode_sms(hex: &str) -> Result<()> {
    let bytes = parse_hex_flexible(hex)?;
    match decode_sms_deliver(&bytes) {
        Some(sms) => {
            println!("from: {}", sms.sender);
            println!("text: {}", sms.text);
            Ok(())
        }
        None => bail!("could not decode the input as an SMS-DELIVER PDU"),
    }
}

fn parse_mem(s: &str) -> Result<MemoryType> {
    Ok(match s.to_lowercase().as_str() {
        "me" | "phone" => MemoryType::Phone,
        "sim" => MemoryType::Sim,
        "combined" | "all" => MemoryType::Combined,
        "own" => MemoryType::Own,
        "dialled" | "dialed" => MemoryType::Dialled,
        "missed" => MemoryType::Missed,
        other => bail!("unknown memory type '{other}' (use me/sim/combined/own/dialled/missed)"),
    })
}

fn hexdump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Send the FBUS init preamble over an already-open transport, run the command,
/// print the reply. Generic so it works over serial OR the app-owned raw USB.
fn run_command<T: Transport>(
    mut link: T,
    cmd: &message::Command,
    xcfg: &ExchangeConfig,
) -> Result<()> {
    if cmd.safety == Safety::Dangerous {
        bail!(
            "refusing to send '{}': classified Dangerous (firmware/flash). Not supported.",
            cmd.name
        );
    }

    // Wake the phone's UART and lock framing.
    link.write_all(&message::fbus_init_preamble(128))
        .context("sending FBUS init preamble")?;

    let request = cmd.frame.encode();
    debug!("request {} = {}", cmd.name, hexdump(&request));
    println!("→ {} : {}", cmd.name, hexdump(&request));
    let frames = exchange_fbus2_with(&mut link, &cmd.frame, xcfg)
        .context("no valid reply from the phone")?;

    let mut got_data = false;
    for f in &frames {
        if f.is_ack() {
            println!("← ACK");
        } else {
            got_data = true;
            println!(
                "← reply msg_type={:#04x} : {}",
                f.msg_type,
                hexdump(&f.data)
            );
            // Typed decode of known replies.
            if let Some(v) = hw_sw_version(f) {
                println!(
                    "  model={} firmware={} date={}",
                    v.model, v.firmware, v.date
                );
            } else if let Ok(text) = std::str::from_utf8(&f.data) {
                let printable: String = text
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n')
                    .collect();
                if printable.trim().len() > 2 {
                    println!("  as text: {}", printable.trim());
                }
            }
        }
    }
    if !got_data {
        // Distinguish "only ACK(s), no reply" from a real answer.
        eprintln!("warning: acknowledged but no data reply before timeout");
    }
    Ok(())
}

fn open_serial(port: &str) -> Result<SerialTransport> {
    SerialTransport::open_fbus(port).with_context(|| format!("opening serial port {port}"))
}

/// Run `identify` over an open transport, in human or JSON form.
fn run_identify<T: Transport>(link: T, as_json: bool, xcfg: &ExchangeConfig) -> Result<()> {
    if as_json {
        identify_json(link, xcfg)
    } else {
        run_command(link, &message::identify_hw_sw(0x40), xcfg)
    }
}

/// Send the identify command and print the decoded identity as JSON.
fn identify_json<T: Transport>(mut link: T, xcfg: &ExchangeConfig) -> Result<()> {
    let cmd = message::identify_hw_sw(0x40);
    link.write_all(&message::fbus_init_preamble(128))
        .context("sending FBUS init preamble")?;
    let frames = exchange_fbus2_with(&mut link, &cmd.frame, xcfg)
        .context("no valid reply from the phone")?;

    let obj = match reassemble_fbus2(&frames) {
        Some((msg_type, data)) => {
            let reply = Fbus2Frame {
                dest: 0,
                src: 0,
                msg_type,
                data,
            };
            match hw_sw_version(&reply) {
                Some(v) => json!({"model": v.model, "firmware": v.firmware, "date": v.date}),
                None => json!({"error": "reply is not a decodable HW/SW version"}),
            }
        }
        None => json!({"error": "no data reply"}),
    };
    println!("{}", serde_json::to_string_pretty(&obj)?);
    Ok(())
}

fn role_str(role: &Role) -> String {
    match role {
        Role::NokiaPhone => "Nokia phone".to_string(),
        Role::CableBridge(name) => format!("cable bridge ({name})"),
        Role::Other => "other".to_string(),
    }
}

/// Format a bus id for the BUS:ADDR column: zero-pad it lsusb-style when it is a
/// plain number (Linux), otherwise show the platform bus id string as-is.
fn bus_label(bus: &str) -> String {
    match bus.parse::<u32>() {
        Ok(n) => format!("{n:03}"),
        Err(_) => bus.to_string(),
    }
}

fn cmd_devices(all: bool, as_json: bool) -> Result<()> {
    let devices = list_usb_devices().context("enumerating USB devices")?;

    if as_json {
        let arr: Vec<_> = devices
            .iter()
            .filter(|d| all || d.is_relevant())
            .map(|d| {
                json!({
                    "bus": d.bus,
                    "address": d.address,
                    "vid": format!("{:04x}", d.vendor_id),
                    "pid": format!("{:04x}", d.product_id),
                    "name": d.display_name(),
                    "role": role_str(&d.role),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }

    let mut shown = 0;
    println!("{:<10} {:<10} {:<28} ROLE", "BUS:ADDR", "VID:PID", "NAME");
    for d in &devices {
        if !all && !d.is_relevant() {
            continue;
        }
        println!(
            "{:<10} {:04X}:{:04X}  {:<28} {}",
            format!("{}:{:03}", bus_label(&d.bus), d.address),
            d.vendor_id,
            d.product_id,
            truncate(&d.display_name(), 28),
            role_str(&d.role),
        );
        shown += 1;
    }
    if shown == 0 {
        println!("(no relevant devices — try --all to see every USB device)");
    }
    Ok(())
}

fn cmd_detect(progress: bool, as_json: bool) -> Result<()> {
    let devices = detect_staged(|done, total, stage| {
        // Progress lines drive a real progress bar; suppressed in JSON mode so
        // stdout stays a single valid JSON document.
        if progress && !as_json {
            println!("PROGRESS {done} {total} {stage}");
        }
    })
    .context("USB detection")?;

    // Report only phones/handsets; skip hubs and unrelated peripherals.
    let recognised: Vec<_> = devices
        .into_iter()
        .filter(|d| d.kind() != DeviceKind::Unknown)
        .collect();

    if as_json {
        let arr: Vec<_> = recognised
            .into_iter()
            .map(|device| {
                let handler = dispatch(device);
                let id = handler.identify();
                json!({
                    "vid": format!("{:04x}", id.vendor_id),
                    "pid": format!("{:04x}", id.product_id),
                    "platform": id.platform.as_str(),
                    "model": id.model,
                    "serial": id.serial,
                    "detail": id.detail,
                    "capabilities": handler.capabilities().iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }

    let mut shown = 0;
    for device in recognised {
        let handler = dispatch(device);
        let id = handler.identify();
        let caps = handler
            .capabilities()
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(",");
        // Tab-separated for unambiguous parsing (platform/detail may contain spaces):
        // DEVICE <vid:pid> <platform> <model> <serial> <detail> <caps>
        println!(
            "DEVICE\t{:04x}:{:04x}\t{}\t{}\t{}\t{}\t{}",
            id.vendor_id,
            id.product_id,
            id.platform.as_str(),
            id.model.as_deref().unwrap_or("?"),
            id.serial.as_deref().unwrap_or("?"),
            id.detail,
            caps,
        );
        shown += 1;
    }
    if shown == 0 {
        println!("No recognised phone detected. Use `devices --all` for a raw inventory.");
    }
    Ok(())
}

fn cmd_ports(as_json: bool) -> Result<()> {
    let ports = available_serial_ports();
    if as_json {
        let arr: Vec<_> = ports
            .iter()
            .map(|p| {
                json!({
                    "path": p.path,
                    "vid": p.vendor_id.map(|v| format!("{v:04x}")),
                    "pid": p.product_id.map(|v| format!("{v:04x}")),
                    "product": p.product,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }
    if ports.is_empty() {
        println!("No serial ports found.");
        return Ok(());
    }
    for p in &ports {
        let ids = match (p.vendor_id, p.product_id) {
            (Some(v), Some(d)) => format!("{v:04x}:{d:04x}"),
            _ => "-".to_string(),
        };
        println!(
            "{:<16} {:<10} {}",
            p.path,
            ids,
            p.product.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

fn resolve_port_arg(port: Option<String>, cfg: &Config) -> Result<String> {
    port.or_else(|| cfg.default_port.clone())
        .context("no --port given and no default_port set in config.toml")
}

fn main() -> Result<()> {
    let cfg = Config::load();

    // Logging goes to stderr (RUST_LOG=debug for frame traces); stdout stays
    // clean for the machine-readable output the GUI parses. The default filter
    // comes from config.toml unless RUST_LOG overrides it.
    let default_level = cfg.log_level.clone().unwrap_or_else(|| "warn".to_string());
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
        .init();

    let xcfg = cfg.exchange();
    let cli = Cli::parse();
    match cli.command {
        Commands::Devices { all, json } => cmd_devices(all, json),
        Commands::Detect { progress, json } => cmd_detect(progress, json),
        Commands::Ports { json } => cmd_ports(json),
        Commands::Identify { port, usb, json } => {
            if usb {
                let link = UsbTransport::open_fbus_auto(NOKIA_VID)
                    .context("claiming the Nokia USB device")?;
                run_identify(link, json, &xcfg)
            } else {
                let p = resolve_port_arg(port, &cfg)
                    .context("provide --port <path>, --usb, or a default_port in config.toml")?;
                run_identify(open_serial(&p)?, json, &xcfg)
            }
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "symbinux-fbus", &mut std::io::stdout());
            Ok(())
        }
        Commands::Man => {
            clap_mangen::Man::new(Cli::command())
                .render(&mut std::io::stdout())
                .context("rendering man page")?;
            Ok(())
        }
        Commands::DecodeFrame { hex } => cmd_decode_frame(&hex),
        Commands::DecodeSms { hex } => cmd_decode_sms(&hex),
        Commands::Getphonebook {
            port,
            mem,
            location,
        } => {
            let mem = parse_mem(&mem)?;
            let p = resolve_port_arg(port, &cfg)?;
            run_command(
                open_serial(&p)?,
                &message::read_phonebook(mem, location, 0x40),
                &xcfg,
            )
        }
        Commands::Netmon { port, screen } => {
            let field = if screen == 255 { 0x00 } else { screen };
            let p = resolve_port_arg(port, &cfg)?;
            run_command(open_serial(&p)?, &message::netmonitor(field, 0x40), &xcfg)
        }
        Commands::Raw {
            port,
            msg_type,
            block,
            i_understand_risk,
        } => {
            if !i_understand_risk {
                bail!("raw mode can send arbitrary frames; re-run with --i-understand-risk");
            }
            let block = parse_hex_bytes(&block)?;
            let frame = Fbus2Frame::command(msg_type, &block, 0x01, 0x40);
            let cmd = message::Command {
                name: "raw",
                safety: Safety::Experimental,
                frame,
            };
            let p = resolve_port_arg(port, &cfg)?;
            run_command(open_serial(&p)?, &cmd, &xcfg)
        }
    }
}
