//! A detected device: its fingerprint, classification, best-effort string
//! descriptors, and — crucially — its stable physical USB port path.

use crate::fingerprint::{classify, DeviceKind, UsbFingerprint};

/// Stable identifier for a physical USB port: bus number plus the chain of hub
/// port numbers from the root (e.g. bus 1, ports `[1, 3]` == sysfs `1-1.3`).
///
/// Unlike the USB device address (`devnum`) or the VID/PID, this survives a
/// re-enumeration in place — which is exactly what an Android AOA accessory
/// switch or an iOS trust-dialog re-probe triggers. Track devices by this.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortKey {
    pub bus: u8,
    pub ports: Vec<u8>,
}

impl PortKey {
    pub fn new(bus: u8, ports: Vec<u8>) -> Self {
        Self { bus, ports }
    }

    /// sysfs-style path, e.g. `1-1.3` (or `1-0` for a root device).
    pub fn path(&self) -> String {
        if self.ports.is_empty() {
            format!("{}-0", self.bus)
        } else {
            let chain = self
                .ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(".");
            format!("{}-{}", self.bus, chain)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DetectedDevice {
    pub port: PortKey,
    pub fingerprint: UsbFingerprint,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
}

impl DetectedDevice {
    pub fn kind(&self) -> DeviceKind {
        classify(&self.fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_path_formatting() {
        assert_eq!(PortKey::new(1, vec![1, 3]).path(), "1-1.3");
        assert_eq!(PortKey::new(2, vec![]).path(), "2-0");
    }
}
