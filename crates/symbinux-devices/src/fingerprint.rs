//! USB fingerprinting: a cascade that classifies a connected device into a
//! platform strategy (Nokia legacy / Android / Apple iOS / unknown).
//!
//! The classifier works on an abstract [`UsbFingerprint`] (vendor/product ids,
//! device class, and the interface class/subclass/protocol triples plus the
//! interface string), so it is fully unit-testable with synthetic fingerprints
//! and independent of any USB backend. Constants and their confidence levels are
//! documented in `docs/DEVICE_DETECTION.md`.

// --- Vendor IDs -------------------------------------------------------------

/// Nokia Mobile Phones.
pub const NOKIA_VID: u16 = 0x0421;
/// Apple.
pub const APPLE_VID: u16 = 0x05ac;
/// Google — used by the Android Open Accessory (AOA) mode.
pub const AOA_VID: u16 = 0x18d1;

// --- Apple product-id ranges (usbmux modes) ---------------------------------

/// Normal iPhone/iPad usbmux PID range.
pub const APPLE_PID_NORMAL: (u16, u16) = (0x1290, 0x12af);
/// Apple-Silicon restore range.
pub const APPLE_PID_RESTORE: (u16, u16) = (0x1901, 0x1905);
/// T2 coprocessor.
pub const APPLE_PID_T2: u16 = 0x8600;

/// The Apple usbmux interface descriptor (vendor-specific).
pub const APPLE_USBMUX_IFACE: (u8, u8, u8) = (0xff, 0xfe, 0x02);

// --- Android accessory (AOA) product ids ------------------------------------

/// AOA accessory-mode product ids (accessory, accessory+adb, audio variants).
pub const AOA_PID_RANGE: (u16, u16) = (0x2d00, 0x2d05);

// --- Android interface fingerprints -----------------------------------------

/// ADB interface: vendor-specific / 0x42 / 0x01.
pub const ADB_IFACE: (u8, u8, u8) = (0xff, 0x42, 0x01);
/// Fastboot interface: vendor-specific / 0x42 / 0x03.
pub const FASTBOOT_IFACE: (u8, u8, u8) = (0xff, 0x42, 0x03);
/// Android MTP interface: vendor-specific / 0xff / 0x00, with iInterface "MTP".
pub const MTP_IFACE: (u8, u8, u8) = (0xff, 0xff, 0x00);
/// PTP (USB Still Image) interface: 0x06 / 0x01 / 0x01.
pub const PTP_IFACE: (u8, u8, u8) = (0x06, 0x01, 0x01);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceFingerprint {
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub interface_string: Option<String>,
}

impl InterfaceFingerprint {
    pub fn new(class: u8, subclass: u8, protocol: u8) -> Self {
        Self {
            class,
            subclass,
            protocol,
            interface_string: None,
        }
    }

    pub fn with_string(mut self, s: &str) -> Self {
        self.interface_string = Some(s.to_string());
        self
    }

    fn triple(&self) -> (u8, u8, u8) {
        (self.class, self.subclass, self.protocol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbFingerprint {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub interfaces: Vec<InterfaceFingerprint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidMode {
    /// ADB debugging interface present.
    Adb,
    /// Bootloader / fastboot.
    Fastboot,
    /// Media Transfer Protocol (file transfer only).
    Mtp,
    /// Picture Transfer Protocol (photos only).
    Ptp,
    /// Android Open Accessory mode (re-enumerated under Google's VID).
    Accessory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    NokiaLegacy,
    Android(AndroidMode),
    AppleIos,
    Unknown,
}

fn apple_pid_matches(pid: u16) -> bool {
    (APPLE_PID_NORMAL.0..=APPLE_PID_NORMAL.1).contains(&pid)
        || (APPLE_PID_RESTORE.0..=APPLE_PID_RESTORE.1).contains(&pid)
        || pid == APPLE_PID_T2
}

/// Classify a device via the cascade:
/// 1. Apple by vendor id (usbmux/lockdown path).
/// 2. Android accessory (AOA) by Google vendor id + accessory PID.
/// 3. Android by ADB/Fastboot/MTP/PTP interface fingerprint.
/// 4. Nokia legacy by vendor id (FBUS/MBUS fallback).
/// 5. Otherwise Unknown (candidate for raw-sniff inspection).
pub fn classify(fp: &UsbFingerprint) -> DeviceKind {
    if fp.vendor_id == APPLE_VID && (has_usbmux_interface(fp) || apple_pid_matches(fp.product_id)) {
        return DeviceKind::AppleIos;
    }

    if fp.vendor_id == AOA_VID && (AOA_PID_RANGE.0..=AOA_PID_RANGE.1).contains(&fp.product_id) {
        return DeviceKind::Android(AndroidMode::Accessory);
    }

    for iface in &fp.interfaces {
        match iface.triple() {
            ADB_IFACE => return DeviceKind::Android(AndroidMode::Adb),
            FASTBOOT_IFACE => return DeviceKind::Android(AndroidMode::Fastboot),
            _ => {}
        }
    }
    for iface in &fp.interfaces {
        if iface.triple() == MTP_IFACE
            && iface
                .interface_string
                .as_deref()
                .map(|s| s.trim().eq_ignore_ascii_case("MTP"))
                .unwrap_or(false)
        {
            return DeviceKind::Android(AndroidMode::Mtp);
        }
        if iface.triple() == PTP_IFACE {
            return DeviceKind::Android(AndroidMode::Ptp);
        }
    }

    if fp.vendor_id == NOKIA_VID {
        return DeviceKind::NokiaLegacy;
    }

    DeviceKind::Unknown
}

/// True if the fingerprint carries the Apple usbmux interface (a stronger Apple
/// signal than the vendor id alone; used by the handler before probing lockdown).
pub fn has_usbmux_interface(fp: &UsbFingerprint) -> bool {
    fp.interfaces
        .iter()
        .any(|i| i.triple() == APPLE_USBMUX_IFACE)
}

/// Whether a device presenting these Apple ids is in a normal (usbmux) mode
/// rather than restore/DFU.
pub fn apple_is_usbmux_mode(fp: &UsbFingerprint) -> bool {
    fp.vendor_id == APPLE_VID && apple_pid_matches(fp.product_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(vid: u16, pid: u16, ifaces: Vec<InterfaceFingerprint>) -> UsbFingerprint {
        UsbFingerprint {
            vendor_id: vid,
            product_id: pid,
            device_class: 0x00,
            interfaces: ifaces,
        }
    }

    #[test]
    fn apple_by_vendor() {
        let d = fp(
            APPLE_VID,
            0x12a8,
            vec![InterfaceFingerprint::new(0xff, 0xfe, 0x02)],
        );
        assert_eq!(classify(&d), DeviceKind::AppleIos);
        assert!(has_usbmux_interface(&d));
        assert!(apple_is_usbmux_mode(&d));
    }

    #[test]
    fn android_adb_by_interface() {
        let d = fp(
            0x18d1,
            0x4ee7,
            vec![InterfaceFingerprint::new(0xff, 0x42, 0x01)],
        );
        assert_eq!(classify(&d), DeviceKind::Android(AndroidMode::Adb));
    }

    #[test]
    fn android_fastboot_by_interface() {
        let d = fp(
            0x18d1,
            0x4ee0,
            vec![InterfaceFingerprint::new(0xff, 0x42, 0x03)],
        );
        assert_eq!(classify(&d), DeviceKind::Android(AndroidMode::Fastboot));
    }

    #[test]
    fn android_mtp_needs_string() {
        let with = fp(
            0x04e8,
            0x6860,
            vec![InterfaceFingerprint::new(0xff, 0xff, 0x00).with_string("MTP")],
        );
        assert_eq!(classify(&with), DeviceKind::Android(AndroidMode::Mtp));
        // Same triple without the "MTP" string is not classified as MTP.
        let without = fp(
            0x1234,
            0x5678,
            vec![InterfaceFingerprint::new(0xff, 0xff, 0x00)],
        );
        assert_eq!(classify(&without), DeviceKind::Unknown);
    }

    #[test]
    fn android_ptp() {
        let d = fp(
            0x04e8,
            0x6865,
            vec![InterfaceFingerprint::new(0x06, 0x01, 0x01)],
        );
        assert_eq!(classify(&d), DeviceKind::Android(AndroidMode::Ptp));
    }

    #[test]
    fn aoa_accessory() {
        let d = fp(AOA_VID, 0x2d01, vec![]);
        assert_eq!(classify(&d), DeviceKind::Android(AndroidMode::Accessory));
    }

    #[test]
    fn nokia_legacy() {
        let d = fp(NOKIA_VID, 0x0400, vec![]);
        assert_eq!(classify(&d), DeviceKind::NokiaLegacy);
    }

    #[test]
    fn unknown_device() {
        let d = fp(
            0x1d6b,
            0x0003,
            vec![InterfaceFingerprint::new(0x09, 0x00, 0x00)],
        );
        assert_eq!(classify(&d), DeviceKind::Unknown);
    }
}
