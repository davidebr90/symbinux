//! List nearby Bluetooth devices using the platform backend.
//!
//! On Linux this discovers classic + LE devices through BlueZ; on Windows and
//! macOS it is a BLE-only scan (see the crate docs).

use std::sync::atomic::AtomicBool;

fn main() {
    let cancel = AtomicBool::new(false);
    match symbinux_wireless::scan_bluetooth(&cancel) {
        Ok(devices) => {
            for device in &devices {
                let name = if device.name.is_empty() {
                    "(no name)"
                } else {
                    &device.name
                };
                let vendor = device.vendor.label().unwrap_or("?");
                let kind = device.kind.label().unwrap_or("?");
                println!("{}  {name}  [{vendor} / {kind}]", device.address);
            }
            println!("{} device(s) found", devices.len());
        }
        Err(err) => {
            eprintln!("scan failed: {err}");
            std::process::exit(1);
        }
    }
}
