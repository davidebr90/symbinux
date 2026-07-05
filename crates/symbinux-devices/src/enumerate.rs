//! Build [`DetectedDevice`] fingerprints from the live USB bus via libusb (rusb).
//!
//! Reads the device descriptor (vid/pid/class), walks the active configuration's
//! interface descriptors for the class/subclass/protocol triples and interface
//! strings, and records the stable physical port path (bus + port chain). String
//! descriptors are best-effort: they need the device opened, which may fail
//! without permission — in that case ids and topology are still reported.

use std::time::Duration;

use crate::device::{DetectedDevice, PortKey};
use crate::fingerprint::{InterfaceFingerprint, UsbFingerprint};

const STRING_TIMEOUT: Duration = Duration::from_millis(200);

/// Enumerate every USB device and fingerprint it.
pub fn detect() -> Result<Vec<DetectedDevice>, rusb::Error> {
    let mut out = Vec::new();
    for device in rusb::devices()?.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let mut interfaces = Vec::new();
        // Open once (if allowed) to read interface/product strings.
        let handle = device.open().ok();
        let lang = handle
            .as_ref()
            .and_then(|h| h.read_languages(STRING_TIMEOUT).ok())
            .and_then(|langs| langs.first().copied());

        if let Ok(config) = device.active_config_descriptor() {
            for iface in config.interfaces() {
                for id in iface.descriptors() {
                    let string = match (&handle, lang, id.description_string_index()) {
                        (Some(h), Some(l), Some(idx)) => {
                            h.read_string_descriptor(l, idx, STRING_TIMEOUT).ok()
                        }
                        _ => None,
                    };
                    let mut fp = InterfaceFingerprint::new(
                        id.class_code(),
                        id.sub_class_code(),
                        id.protocol_code(),
                    );
                    fp.interface_string = string;
                    interfaces.push(fp);
                }
            }
        }

        let (manufacturer, product, serial) = match (&handle, lang) {
            (Some(h), Some(l)) => (
                h.read_manufacturer_string(l, &desc, STRING_TIMEOUT).ok(),
                h.read_product_string(l, &desc, STRING_TIMEOUT).ok(),
                h.read_serial_number_string(l, &desc, STRING_TIMEOUT).ok(),
            ),
            _ => (None, None, None),
        };

        let ports = device.port_numbers().unwrap_or_default();

        out.push(DetectedDevice {
            port: PortKey::new(device.bus_number(), ports),
            fingerprint: UsbFingerprint {
                vendor_id: desc.vendor_id(),
                product_id: desc.product_id(),
                device_class: desc.class_code(),
                interfaces,
            },
            manufacturer,
            product,
            serial,
        });
    }
    Ok(out)
}
