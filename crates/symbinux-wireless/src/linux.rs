//! Linux backends: BlueZ (`bluetoothctl`) scan, NetworkManager (`nmcli`)
//! scan and PBAP contact transfers through obexd (`busctl --user`).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::exec::{require_command, run_command};
use crate::{BluetoothDevice, WifiNetwork, WirelessError};

pub(crate) fn scan_bluetooth(cancel: &AtomicBool) -> Result<Vec<BluetoothDevice>, WirelessError> {
    require_command(
        "bluetoothctl",
        "Bluetooth scan requires bluetoothctl (BlueZ).",
    )?;

    let show = run_command("bluetoothctl", &["show"], Duration::from_secs(6), cancel)?;
    if show.contains("No default controller") {
        return Err(WirelessError::Unavailable(
            "No Bluetooth adapter available.".to_string(),
        ));
    }

    let _ = run_command(
        "bluetoothctl",
        &["--timeout", "8", "scan", "on"],
        Duration::from_secs(13),
        cancel,
    )?;
    let devices_out = run_command("bluetoothctl", &["devices"], Duration::from_secs(6), cancel)?;
    let paired_out = run_command(
        "bluetoothctl",
        &["paired-devices"],
        Duration::from_secs(6),
        cancel,
    )?;

    Ok(parse_bluetoothctl_devices(&devices_out, &paired_out))
}

fn parse_bluetoothctl_devices(devices_out: &str, paired_out: &str) -> Vec<BluetoothDevice> {
    let paired = paired_out
        .lines()
        .filter_map(|line| line.strip_prefix("Device "))
        .filter_map(|tail| tail.split_whitespace().next())
        .map(str::to_string)
        .collect::<Vec<_>>();

    let mut devices = Vec::new();
    for line in devices_out.lines() {
        let Some(tail) = line.strip_prefix("Device ") else {
            continue;
        };
        let mut parts = tail.splitn(2, char::is_whitespace);
        let Some(address) = parts.next() else {
            continue;
        };
        let name = parts.next().unwrap_or("").trim().to_string();
        devices.push(BluetoothDevice {
            address: address.to_string(),
            name,
            paired: paired.iter().any(|item| item == address),
        });
    }
    devices
}

pub(crate) fn scan_wifi(cancel: &AtomicBool) -> Result<Vec<WifiNetwork>, WirelessError> {
    require_command("nmcli", "Wi-Fi scan requires nmcli (NetworkManager).")?;
    let output = run_command(
        "nmcli",
        &[
            "-t",
            "-f",
            "SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "--rescan",
            "yes",
        ],
        Duration::from_secs(20),
        cancel,
    )?;

    let mut networks = Vec::new();
    let mut seen = Vec::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let fields = split_nmcli_fields(line);
        if fields.len() < 3 {
            continue;
        }
        let ssid = if fields[0].is_empty() {
            "(hidden)".to_string()
        } else {
            fields[0].clone()
        };
        if seen.iter().any(|item| item == &ssid) {
            continue;
        }
        seen.push(ssid.clone());
        networks.push(WifiNetwork {
            ssid,
            signal: fields[1].clone(),
            security: if fields[2].is_empty() {
                "open".to_string()
            } else {
                fields[2].clone()
            },
        });
    }
    Ok(networks)
}

fn split_nmcli_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            field.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == ':' {
            fields.push(field);
            field = String::new();
        } else {
            field.push(ch);
        }
    }

    fields.push(field);
    fields
}

pub(crate) fn pull_contacts_pbap(
    address: &str,
    cancel: &AtomicBool,
) -> Result<String, WirelessError> {
    require_command(
        "bluetoothctl",
        "Bluetooth contacts require bluetoothctl (BlueZ).",
    )?;
    require_command(
        "busctl",
        "Bluetooth contacts require busctl and bluez-obex.",
    )?;

    let _ = run_command(
        "bluetoothctl",
        &["pair", address],
        Duration::from_secs(30),
        cancel,
    );
    let _ = run_command(
        "bluetoothctl",
        &["connect", address],
        Duration::from_secs(15),
        cancel,
    );

    let session_out = run_command(
        "busctl",
        &[
            "--user",
            "call",
            "org.bluez.obex",
            "/org/bluez/obex",
            "org.bluez.obex.Client1",
            "CreateSession",
            "sa{sv}",
            address,
            "1",
            "Target",
            "s",
            "pbap",
        ],
        Duration::from_secs(20),
        cancel,
    )?;
    let session = first_dbus_path(&session_out).ok_or_else(|| {
        WirelessError::Failed("PBAP session was not created by obexd.".to_string())
    })?;

    let result = pull_contacts_from_session(&session, address, cancel);
    let _ = run_command(
        "busctl",
        &[
            "--user",
            "call",
            "org.bluez.obex",
            "/org/bluez/obex",
            "org.bluez.obex.Client1",
            "RemoveSession",
            "o",
            &session,
        ],
        Duration::from_secs(5),
        cancel,
    );
    result
}

fn pull_contacts_from_session(
    session: &str,
    address: &str,
    cancel: &AtomicBool,
) -> Result<String, WirelessError> {
    let _ = run_command(
        "busctl",
        &[
            "--user",
            "call",
            "org.bluez.obex",
            session,
            "org.bluez.obex.PhonebookAccess1",
            "Select",
            "ss",
            "int",
            "pb",
        ],
        Duration::from_secs(10),
        cancel,
    )?;

    let target = pbap_target_path(address);
    let target_str = target.to_str().ok_or_else(|| {
        WirelessError::Failed("Could not create a UTF-8 target path for contacts.".to_string())
    })?;
    let transfer_out = run_command(
        "busctl",
        &[
            "--user",
            "call",
            "org.bluez.obex",
            session,
            "org.bluez.obex.PhonebookAccess1",
            "PullAll",
            "sa{sv}",
            target_str,
            "0",
        ],
        Duration::from_secs(10),
        cancel,
    )?;
    let transfer = first_dbus_path(&transfer_out).ok_or_else(|| {
        WirelessError::Failed("PBAP transfer was not created by obexd.".to_string())
    })?;

    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(WirelessError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(WirelessError::Failed(
                "Bluetooth contact transfer timed out.".to_string(),
            ));
        }

        let status = run_command(
            "busctl",
            &[
                "--user",
                "get-property",
                "org.bluez.obex",
                &transfer,
                "org.bluez.obex.Transfer1",
                "Status",
            ],
            Duration::from_secs(3),
            cancel,
        )?;
        if status.contains("\"complete\"") {
            let text = fs::read_to_string(&target).map_err(|err| {
                WirelessError::Failed(format!("Could not read PBAP contacts: {err}"))
            })?;
            let _ = fs::remove_file(&target);
            return Ok(text);
        }
        if status.contains("\"error\"") {
            let _ = fs::remove_file(&target);
            return Err(WirelessError::Failed(
                "Bluetooth contact transfer failed.".to_string(),
            ));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn pbap_target_path(address: &str) -> PathBuf {
    let clean_address = address
        .chars()
        .map(|ch| if ch.is_ascii_hexdigit() { ch } else { '_' })
        .collect::<String>();
    std::env::temp_dir().join(format!(
        "symbinux-pbap-{clean_address}-{}.vcf",
        std::process::id()
    ))
}

fn first_dbus_path(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|part| {
        let value = part.trim_matches('"');
        value.starts_with('/').then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bluetoothctl_devices_and_pairing() {
        let devices =
            "Device 00:11:22:33:44:55 Nokia 3310\nDevice AA:BB:CC:DD:EE:FF Speaker\nnoise\n";
        let paired = "Device AA:BB:CC:DD:EE:FF Speaker\n";
        let parsed = parse_bluetoothctl_devices(devices, paired);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].address, "00:11:22:33:44:55");
        assert_eq!(parsed[0].name, "Nokia 3310");
        assert!(!parsed[0].paired);
        assert!(parsed[1].paired);
    }

    #[test]
    fn splits_nmcli_escaped_fields() {
        let fields = split_nmcli_fields(r"my\:net:82:WPA2");
        assert_eq!(fields, vec!["my:net", "82", "WPA2"]);
    }

    #[test]
    fn finds_first_dbus_path() {
        let output = "o \"/org/bluez/obex/client/session0\"\n";
        assert_eq!(
            first_dbus_path(output).as_deref(),
            Some("/org/bluez/obex/client/session0")
        );
        assert_eq!(first_dbus_path("u 42"), None);
    }

    #[test]
    fn sanitises_pbap_target_path() {
        let path = pbap_target_path("AA:BB!/..");
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        assert!(name.starts_with("symbinux-pbap-AA_BB____-"));
        assert!(name.ends_with(".vcf"));
    }
}
