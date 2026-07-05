//! Concrete platform handlers and the dispatcher that picks one for a device.
//!
//! - `NokiaLegacyHandler` — FBUS/MBUS phones. `transfer` is delegated to the
//!   serial FBUS path (see `symbinux-transport`); identify/capabilities are real.
//! - `AndroidHandler` — capabilities depend on the USB mode (ADB / MTP-only /
//!   PTP / fastboot / accessory). Real transfer would wrap the ADB protocol
//!   (e.g. the `adb_client` crate) or an AOA switch; not bundled here.
//! - `AppleHandler` — iOS via usbmux/lockdown. The real path links
//!   libimobiledevice / usbmuxd (a running daemon) rather than reimplementing
//!   pairing + TLS; see `docs/DEVICE_DETECTION.md`.

use crate::device::DetectedDevice;
use crate::fingerprint::{AndroidMode, DeviceKind};
use crate::handler::{Capability, DeviceHandler, DeviceIdentity, HandlerError, Platform};

fn base_identity(device: &DetectedDevice, platform: Platform, detail: String) -> DeviceIdentity {
    DeviceIdentity {
        platform,
        vendor: device.manufacturer.clone(),
        model: device.product.clone(),
        vendor_id: device.fingerprint.vendor_id,
        product_id: device.fingerprint.product_id,
        serial: device.serial.clone(),
        detail,
    }
}

// --- Nokia legacy -----------------------------------------------------------

pub struct NokiaLegacyHandler {
    device: DetectedDevice,
}

impl NokiaLegacyHandler {
    pub fn new(device: DetectedDevice) -> Self {
        Self { device }
    }
}

impl DeviceHandler for NokiaLegacyHandler {
    fn platform(&self) -> Platform {
        Platform::NokiaLegacy
    }

    fn identify(&self) -> DeviceIdentity {
        base_identity(&self.device, Platform::NokiaLegacy, "FBUS/MBUS".into())
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::Identify,
            Capability::Phonebook,
            Capability::Sms,
            Capability::Netmonitor,
        ]
    }

    fn transfer(&mut self, _payload: &[u8]) -> Result<Vec<u8>, HandlerError> {
        // The real FBUS exchange runs over the serial transport
        // (symbinux-transport::exchange_fbus2); it needs an open /dev/ttyUSB*,
        // which the CLI's `identify`/`getphonebook` commands drive directly.
        Err(HandlerError::NotSupported("Nokia (use the FBUS serial commands)"))
    }

    fn disconnect(&mut self) {}
}

// --- Android ----------------------------------------------------------------

pub struct AndroidHandler {
    device: DetectedDevice,
    mode: AndroidMode,
}

impl AndroidHandler {
    pub fn new(device: DetectedDevice, mode: AndroidMode) -> Self {
        Self { device, mode }
    }

    fn mode_str(&self) -> &'static str {
        match self.mode {
            AndroidMode::Adb => "ADB",
            AndroidMode::Fastboot => "fastboot",
            AndroidMode::Mtp => "MTP",
            AndroidMode::Ptp => "PTP",
            AndroidMode::Accessory => "accessory (AOA)",
        }
    }
}

impl DeviceHandler for AndroidHandler {
    fn platform(&self) -> Platform {
        Platform::Android
    }

    fn identify(&self) -> DeviceIdentity {
        base_identity(&self.device, Platform::Android, format!("mode: {}", self.mode_str()))
    }

    fn capabilities(&self) -> Vec<Capability> {
        match self.mode {
            AndroidMode::Adb => vec![
                Capability::Identify,
                Capability::FileTransfer,
                Capability::AppInstall,
                Capability::Backup,
                Capability::Screenshot,
            ],
            // MTP/PTP only expose file/photo transfer — no extended commands.
            AndroidMode::Mtp | AndroidMode::Ptp => vec![Capability::FileTransfer],
            AndroidMode::Fastboot => vec![Capability::Identify],
            AndroidMode::Accessory => vec![Capability::RawSniff],
        }
    }

    fn transfer(&mut self, _payload: &[u8]) -> Result<Vec<u8>, HandlerError> {
        Err(HandlerError::RequiresDaemon(
            "Android transfer needs the ADB protocol (e.g. the adb_client crate) or an AOA switch"
                .into(),
        ))
    }

    fn disconnect(&mut self) {}
}

// --- Apple iOS --------------------------------------------------------------

pub struct AppleHandler {
    device: DetectedDevice,
}

impl AppleHandler {
    pub fn new(device: DetectedDevice) -> Self {
        Self { device }
    }
}

impl DeviceHandler for AppleHandler {
    fn platform(&self) -> Platform {
        Platform::AppleIos
    }

    fn identify(&self) -> DeviceIdentity {
        // The USB iSerialNumber is the device UDID.
        base_identity(&self.device, Platform::AppleIos, "usbmux/lockdown".into())
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::Identify,
            Capability::FileTransfer,
            Capability::AppInstall,
            Capability::Backup,
        ]
    }

    fn transfer(&mut self, _payload: &[u8]) -> Result<Vec<u8>, HandlerError> {
        Err(HandlerError::RequiresDaemon(
            "iOS requires a running usbmuxd daemon and lockdown pairing via libimobiledevice/idevice"
                .into(),
        ))
    }

    fn disconnect(&mut self) {}
}

// --- Unknown (raw sniff) ----------------------------------------------------

pub struct UnknownHandler {
    device: DetectedDevice,
}

impl UnknownHandler {
    pub fn new(device: DetectedDevice) -> Self {
        Self { device }
    }

    /// A human-readable dump of the raw interfaces for future recognition work.
    pub fn raw_sniff(&self) -> String {
        let fp = &self.device.fingerprint;
        let mut lines = vec![format!(
            "{:04x}:{:04x} deviceClass={:#04x}",
            fp.vendor_id, fp.product_id, fp.device_class
        )];
        for (i, iface) in fp.interfaces.iter().enumerate() {
            lines.push(format!(
                "  iface {i}: class={:#04x} subclass={:#04x} protocol={:#04x} str={:?}",
                iface.class, iface.subclass, iface.protocol, iface.interface_string
            ));
        }
        lines.join("\n")
    }
}

impl DeviceHandler for UnknownHandler {
    fn platform(&self) -> Platform {
        Platform::Unknown
    }

    fn identify(&self) -> DeviceIdentity {
        base_identity(&self.device, Platform::Unknown, "unrecognised".into())
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::RawSniff]
    }

    fn transfer(&mut self, _payload: &[u8]) -> Result<Vec<u8>, HandlerError> {
        Err(HandlerError::NotSupported("Unknown"))
    }

    fn disconnect(&mut self) {}
}

/// Pick the handler strategy for a detected device.
pub fn dispatch(device: DetectedDevice) -> Box<dyn DeviceHandler> {
    match device.kind() {
        DeviceKind::NokiaLegacy => Box::new(NokiaLegacyHandler::new(device)),
        DeviceKind::Android(mode) => Box::new(AndroidHandler::new(device, mode)),
        DeviceKind::AppleIos => Box::new(AppleHandler::new(device)),
        DeviceKind::Unknown => Box::new(UnknownHandler::new(device)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::PortKey;
    use crate::fingerprint::{InterfaceFingerprint, UsbFingerprint};

    fn detected(vid: u16, pid: u16, ifaces: Vec<InterfaceFingerprint>) -> DetectedDevice {
        DetectedDevice {
            port: PortKey::new(1, vec![2]),
            fingerprint: UsbFingerprint {
                vendor_id: vid,
                product_id: pid,
                device_class: 0,
                interfaces: ifaces,
            },
            manufacturer: Some("Acme".into()),
            product: Some("Phone".into()),
            serial: Some("SER123".into()),
        }
    }

    #[test]
    fn dispatch_nokia_capabilities() {
        let h = dispatch(detected(0x0421, 0x0400, vec![]));
        assert_eq!(h.platform(), Platform::NokiaLegacy);
        assert!(h.capabilities().contains(&Capability::Phonebook));
        assert!(!h.capabilities().contains(&Capability::AppInstall));
    }

    #[test]
    fn dispatch_android_mtp_is_file_transfer_only() {
        let h = dispatch(detected(0x04e8, 0x6860, vec![
            InterfaceFingerprint::new(0xff, 0xff, 0x00).with_string("MTP"),
        ]));
        assert_eq!(h.platform(), Platform::Android);
        assert_eq!(h.capabilities(), vec![Capability::FileTransfer]);
    }

    #[test]
    fn dispatch_apple_capabilities() {
        let h = dispatch(detected(0x05ac, 0x12a8, vec![
            InterfaceFingerprint::new(0xff, 0xfe, 0x02),
        ]));
        assert_eq!(h.platform(), Platform::AppleIos);
        assert!(h.capabilities().contains(&Capability::Backup));
        assert!(!h.capabilities().contains(&Capability::Netmonitor));
    }

    #[test]
    fn dispatch_unknown_raw_sniff() {
        let device = detected(0x1d6b, 0x0002, vec![InterfaceFingerprint::new(0x09, 0, 0)]);
        let h = dispatch(device);
        assert_eq!(h.platform(), Platform::Unknown);
        assert_eq!(h.capabilities(), vec![Capability::RawSniff]);
    }
}
