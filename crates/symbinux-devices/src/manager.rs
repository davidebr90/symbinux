//! Tracking devices across re-enumeration by physical port.
//!
//! An Android AOA accessory switch and an iOS trust-dialog re-probe both make
//! the same physical device disconnect and come back with a DIFFERENT vid/pid
//! (and device address). Correlating by vid/pid or address would treat the
//! re-enumerated device as brand new. [`DeviceManager`] keys on [`PortKey`] (the
//! stable bus + hub-port path) so a mode switch is reported as a transition on
//! the same device rather than a remove + add.

use std::collections::HashMap;

use crate::device::{DetectedDevice, PortKey};
use crate::fingerprint::DeviceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// First time this physical port is seen.
    Arrived(DeviceKind),
    /// Same port, same classification.
    Unchanged(DeviceKind),
    /// Same physical port, different classification — a mode switch
    /// (AOA accessory, iOS trust/mode change). Re-probe with the new handler.
    Switched { from: DeviceKind, to: DeviceKind },
    /// The port no longer has a device.
    Departed(DeviceKind),
}

#[derive(Debug, Default)]
pub struct DeviceManager {
    seen: HashMap<PortKey, DeviceKind>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one observation and report how it relates to the last state of
    /// that physical port.
    pub fn observe(&mut self, device: &DetectedDevice) -> Transition {
        let kind = device.kind();
        match self.seen.insert(device.port.clone(), kind) {
            None => Transition::Arrived(kind),
            Some(prev) if prev == kind => Transition::Unchanged(kind),
            Some(prev) => Transition::Switched { from: prev, to: kind },
        }
    }

    /// Reconcile the full current device set with the previous one, returning a
    /// transition per affected port (including `Departed` for unplugged ports).
    pub fn sync(&mut self, current: &[DetectedDevice]) -> Vec<(PortKey, Transition)> {
        let mut out = Vec::new();
        let mut present = HashMap::new();
        for device in current {
            present.insert(device.port.clone(), device.kind());
        }

        for device in current {
            out.push((device.port.clone(), self.observe(device)));
        }

        let departed: Vec<PortKey> = self
            .seen
            .keys()
            .filter(|port| !present.contains_key(*port))
            .cloned()
            .collect();
        for port in departed {
            if let Some(kind) = self.seen.remove(&port) {
                out.push((port, Transition::Departed(kind)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::{AndroidMode, InterfaceFingerprint, UsbFingerprint};

    fn dev(port: PortKey, vid: u16, pid: u16, ifaces: Vec<InterfaceFingerprint>) -> DetectedDevice {
        DetectedDevice {
            port,
            fingerprint: UsbFingerprint { vendor_id: vid, product_id: pid, device_class: 0, interfaces: ifaces },
            manufacturer: None,
            product: None,
            serial: None,
        }
    }

    #[test]
    fn aoa_switch_is_a_transition_on_the_same_port() {
        let port = PortKey::new(1, vec![2]);
        let mut mgr = DeviceManager::new();

        // Android in ADB mode.
        let adb = dev(port.clone(), 0x18d1, 0x4ee7, vec![InterfaceFingerprint::new(0xff, 0x42, 0x01)]);
        assert_eq!(mgr.observe(&adb), Transition::Arrived(DeviceKind::Android(AndroidMode::Adb)));

        // Same physical port re-enumerates as an AOA accessory (new PID).
        let acc = dev(port.clone(), 0x18d1, 0x2d01, vec![]);
        assert_eq!(
            mgr.observe(&acc),
            Transition::Switched {
                from: DeviceKind::Android(AndroidMode::Adb),
                to: DeviceKind::Android(AndroidMode::Accessory),
            }
        );
    }

    #[test]
    fn sync_reports_departure() {
        let mut mgr = DeviceManager::new();
        let port = PortKey::new(2, vec![1]);
        let nokia = dev(port.clone(), 0x0421, 0x0400, vec![]);
        mgr.sync(&[nokia]);
        let transitions = mgr.sync(&[]);
        assert_eq!(transitions, vec![(port, Transition::Departed(DeviceKind::NokiaLegacy))]);
    }
}
