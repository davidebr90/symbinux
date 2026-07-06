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
/// Manufacturer/product/serial strings are read from the OS's cached descriptor
/// data without opening the device (so no permission is needed). On Windows the
/// manufacturer string is not cached and comes back as `None`; ids and topology
/// are always reported.
pub fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, nusb::Error> {
    use nusb::MaybeFuture;

    let mut out = Vec::new();
    for info in nusb::list_devices().wait()? {
        let vid = info.vendor_id();

        #[cfg(target_os = "linux")]
        let bus = info.busnum();
        #[cfg(not(target_os = "linux"))]
        let bus = 0u8;

        out.push(UsbDeviceInfo {
            bus,
            address: info.device_address(),
            vendor_id: vid,
            product_id: info.product_id(),
            manufacturer: info.manufacturer_string().map(String::from),
            product: info.product_string().map(String::from),
            serial: info.serial_number().map(String::from),
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
