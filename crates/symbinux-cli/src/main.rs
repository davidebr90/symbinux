//! `symbinux-fbus`: a gnokii-style command-line tool for legacy Nokia phones.
//!
//! Talks FBUS/2 over a serial cable (DKU-2/CA-42) or raw USB. Includes an
//! "advanced" device-enumeration mode for debugging what is physically
//! connected, and a raw-frame mode for protocol reverse-engineering.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use log::debug;
use serde_json::json;

use symbinux_devices::{detect_staged, dispatch, DeviceKind};
use symbinux_protocol::message::{self, MemoryType, Safety};
use symbinux_protocol::{hw_sw_version, Fbus2Frame};
use symbinux_transport::{exchange_fbus2, list_usb_devices, Role, SerialTransport, Transport};

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
    /// Query the phone's hardware and software version.
    Identify {
        /// Serial port, e.g. /dev/ttyUSB0 or /dev/nokia_fbus.
        #[arg(long)]
        port: String,
    },
    /// Read a phonebook entry.
    Getphonebook {
        #[arg(long)]
        port: String,
        /// Memory: me (phone), sim, combined, own, dialled, missed.
        #[arg(long, default_value = "me")]
        mem: String,
        /// 1-based entry location.
        #[arg(long)]
        location: u8,
    },
    /// Show or control the netmonitor.
    Netmon {
        #[arg(long)]
        port: String,
        /// Screen number, or 255 for "next".
        #[arg(long, default_value_t = 255)]
        screen: u8,
    },
    /// Send a raw FBUS/2 frame (reverse-engineering mode). Prints the reply.
    Raw {
        #[arg(long)]
        port: String,
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

/// Open the port, send the FBUS init preamble, run the command, print reply.
fn run_command(port: &str, cmd: &message::Command) -> Result<()> {
    if cmd.safety == Safety::Dangerous {
        bail!(
            "refusing to send '{}': classified Dangerous (firmware/flash). Not supported.",
            cmd.name
        );
    }
    let mut link = SerialTransport::open_fbus(port)
        .with_context(|| format!("opening serial port {port}"))?;

    // Wake the phone's UART and lock framing.
    link.write_all(&message::fbus_init_preamble(128))
        .context("sending FBUS init preamble")?;

    let request = cmd.frame.encode();
    debug!("request {} = {}", cmd.name, hexdump(&request));
    println!("→ {} : {}", cmd.name, hexdump(&request));
    let frames = exchange_fbus2(&mut link, &cmd.frame, Duration::from_millis(1500))
        .context("no valid reply from the phone")?;

    let mut got_data = false;
    for f in &frames {
        if f.is_ack() {
            println!("← ACK");
        } else {
            got_data = true;
            println!("← reply msg_type={:#04x} : {}", f.msg_type, hexdump(&f.data));
            // Typed decode of known replies.
            if let Some(v) = hw_sw_version(f) {
                println!("  model={} firmware={} date={}", v.model, v.firmware, v.date);
            } else if let Ok(text) = std::str::from_utf8(&f.data) {
                let printable: String =
                    text.chars().filter(|c| !c.is_control() || *c == '\n').collect();
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

fn role_str(role: &Role) -> String {
    match role {
        Role::NokiaPhone => "Nokia phone".to_string(),
        Role::CableBridge(name) => format!("cable bridge ({name})"),
        Role::Other => "other".to_string(),
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
            format!("{:03}:{:03}", d.bus, d.address),
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
    let recognised: Vec<_> = devices.into_iter().filter(|d| d.kind() != DeviceKind::Unknown).collect();

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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

fn main() -> Result<()> {
    // Logging goes to stderr (RUST_LOG=debug for frame traces); stdout stays
    // clean for the machine-readable output the GUI parses.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Devices { all, json } => cmd_devices(all, json),
        Commands::Detect { progress, json } => cmd_detect(progress, json),
        Commands::Identify { port } => run_command(&port, &message::identify_hw_sw(0x40)),
        Commands::Getphonebook { port, mem, location } => {
            let mem = parse_mem(&mem)?;
            run_command(&port, &message::read_phonebook(mem, location, 0x40))
        }
        Commands::Netmon { port, screen } => {
            let field = if screen == 255 { 0x00 } else { screen };
            run_command(&port, &message::netmonitor(field, 0x40))
        }
        Commands::Raw { port, msg_type, block, i_understand_risk } => {
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
            run_command(&port, &cmd)
        }
    }
}
