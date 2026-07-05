//! Standard USB device enumeration for the "advanced" diagnostics mode.
//!
//! This lists every USB device the host can see (lsusb-style) with vendor/product
//! IDs and, where permission allows, the manufacturer/product strings. It also
//! classifies each device to help debugging: is it a Nokia phone, a known
//! serial-bridge cable chipset, or unrelated. None of this touches the FBUS/MBUS
//! protocol — it is purely to answer "what is physically connected".

/// Nokia Mobile Phones USB vendor id.
pub const NOKIA_VID: u16 = 0x0421;

/// Known serial-bridge chipsets used by CA-42/DKU-5 style cables and clones.
/// (vendor id, human label)
pub const KNOWN_CABLE_BRIDGES: &[(u16, &str)] = &[
    (0x067b, "Prolific PL2303"),
    (0x10c4, "Silicon Labs CP210x"),
    (0x0403, "FTDI"),
    (0x1a86, "QinHeng CH340/CH341"),
    (0x6547, "ArkMicro ARK3116"),
];

/// How a discovered USB device relates to Nokia connectivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A Nokia phone (vendor 0x0421) — a direct FBUS/USB target.
    NokiaPhone,
    /// A serial-bridge cable chipset that likely exposes a /dev/ttyUSB* port.
    CableBridge(&'static str),
    /// Anything else.
    Other,
}

#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    pub bus: u8,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub role: Role,
}

impl UsbDeviceInfo {
    /// A one-line, human-friendly label combining strings and ids.
    pub fn display_name(&self) -> String {
        match (&self.manufacturer, &self.product) {
            (Some(m), Some(p)) => format!("{m} {p}"),
            (None, Some(p)) => p.clone(),
            (Some(m), None) => m.clone(),
            (None, None) => match self.role {
                Role::NokiaPhone => "Nokia phone".to_string(),
                Role::CableBridge(name) => name.to_string(),
                Role::Other => "USB device".to_string(),
            },
        }
    }

    pub fn is_relevant(&self) -> bool {
        !matches!(self.role, Role::Other)
    }
}

fn classify(vid: u16) -> Role {
    if vid == NOKIA_VID {
        return Role::NokiaPhone;
    }
    for (bridge_vid, name) in KNOWN_CABLE_BRIDGES {
        if vid == *bridge_vid {
            return Role::CableBridge(name);
        }
    }
    Role::Other
}

/// Enumerate all USB devices visible to the host.
///
/// String descriptors (manufacturer/product/serial) require opening the device
/// and may be unavailable without permission; in that case they come back as
/// `None` while ids and topology are still reported.
pub fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, rusb::Error> {
    let mut out = Vec::new();
    for device in rusb::devices()?.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        let vid = desc.vendor_id();
        let pid = desc.product_id();

        let (manufacturer, product, serial) = match device.open() {
            Ok(handle) => {
                let timeout = std::time::Duration::from_millis(200);
                let langs = handle.read_languages(timeout).unwrap_or_default();
                let lang = langs.first().copied();
                let read = |f: &dyn Fn() -> rusb::Result<String>| match lang {
                    Some(_) => f().ok(),
                    None => None,
                };
                (
                    read(&|| handle.read_manufacturer_string(lang.unwrap(), &desc, timeout)),
                    read(&|| handle.read_product_string(lang.unwrap(), &desc, timeout)),
                    read(&|| handle.read_serial_number_string(lang.unwrap(), &desc, timeout)),
                )
            }
            Err(_) => (None, None, None),
        };

        out.push(UsbDeviceInfo {
            bus: device.bus_number(),
            address: device.address(),
            vendor_id: vid,
            product_id: pid,
            manufacturer,
            product,
            serial,
            role: classify(vid),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_nokia_and_bridges() {
        assert_eq!(classify(NOKIA_VID), Role::NokiaPhone);
        assert!(matches!(classify(0x067b), Role::CableBridge(_)));
        assert_eq!(classify(0x1234), Role::Other);
    }

    #[test]
    fn display_name_prefers_strings() {
        let d = UsbDeviceInfo {
            bus: 1,
            address: 4,
            vendor_id: NOKIA_VID,
            product_id: 0x0400,
            manufacturer: Some("Nokia".into()),
            product: Some("3310".into()),
            serial: None,
            role: Role::NokiaPhone,
        };
        assert_eq!(d.display_name(), "Nokia 3310");
    }
}
