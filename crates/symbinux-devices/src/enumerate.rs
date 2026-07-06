//! Build [`DetectedDevice`] fingerprints from the live USB bus via nusb.
//!
//! Reads each device's cached descriptor data (vid/pid/class), the active
//! configuration's per-interface class/subclass/protocol triples and interface
//! strings, and the stable physical port path (bus + port chain). None of this
//! opens the device: nusb exposes the OS-cached strings and interface summaries
//! from enumeration, so ids, topology and strings are reported without needing
//! permission to claim the device.

use nusb::MaybeFuture;

use crate::device::{DetectedDevice, PortKey};
use crate::fingerprint::{InterfaceFingerprint, UsbFingerprint};

/// Enumerate every USB device and fingerprint it.
pub fn detect() -> Result<Vec<DetectedDevice>, nusb::Error> {
    let mut out = Vec::new();
    for info in nusb::list_devices().wait()? {
        let interfaces = info
            .interfaces()
            .map(|intf| {
                let mut fp =
                    InterfaceFingerprint::new(intf.class(), intf.subclass(), intf.protocol());
                fp.interface_string = intf.interface_string().map(String::from);
                fp
            })
            .collect();

        #[cfg(target_os = "linux")]
        let bus = info.busnum();
        #[cfg(not(target_os = "linux"))]
        let bus = 0u8;

        out.push(DetectedDevice {
            port: PortKey::new(bus, info.port_chain().to_vec()),
            fingerprint: UsbFingerprint {
                vendor_id: info.vendor_id(),
                product_id: info.product_id(),
                device_class: info.class(),
                interfaces,
            },
            manufacturer: info.manufacturer_string().map(String::from),
            product: info.product_string().map(String::from),
            serial: info.serial_number().map(String::from),
        });
    }
    Ok(out)
}
