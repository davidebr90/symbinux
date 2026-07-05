//! Serial-port discovery and resolution.
//!
//! Maps a connected phone/cable to the serial port it exposes (`/dev/ttyUSB*` on
//! Linux, `COMn` on Windows, `/dev/cu.*` on macOS) so the caller doesn't have to
//! know the device path. Uses the cross-platform `serialport` enumeration, which
//! reports the USB vendor/product id behind each port.

use serialport::SerialPortType;

/// A serial port and, when it is USB-backed, the ids of the bridge/device.
#[derive(Debug, Clone)]
pub struct SerialPortInfo {
    pub path: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub product: Option<String>,
}

/// List all serial ports the OS exposes.
pub fn available_serial_ports() -> Vec<SerialPortInfo> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let (vendor_id, product_id, product) = match p.port_type {
                SerialPortType::UsbPort(info) => (Some(info.vid), Some(info.pid), info.product),
                _ => (None, None, None),
            };
            SerialPortInfo {
                path: p.port_name,
                vendor_id,
                product_id,
                product,
            }
        })
        .collect()
}

/// Resolve the serial port to use for a phone reachable through a device whose
/// USB vendor id is one of `vids` (the phone's own VID and/or known cable-bridge
/// VIDs). Prefers a USB serial port matching one of the ids; if none match but
/// there is exactly one USB serial port, returns that. `None` if ambiguous.
pub fn resolve_serial_port(vids: &[u16]) -> Option<String> {
    let ports = available_serial_ports();
    for p in &ports {
        if let Some(v) = p.vendor_id {
            if vids.contains(&v) {
                return Some(p.path.clone());
            }
        }
    }
    let usb: Vec<&SerialPortInfo> = ports.iter().filter(|p| p.vendor_id.is_some()).collect();
    if usb.len() == 1 {
        return Some(usb[0].path.clone());
    }
    None
}
