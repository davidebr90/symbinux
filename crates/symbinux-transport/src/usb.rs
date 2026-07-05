//! Raw USB bulk transport (libusb) — the app-owned connection path.
//!
//! Instead of relying on the OS exposing a `/dev/ttyUSB*` serial port, this
//! claims the phone's USB device directly and drives FBUS over its bulk
//! endpoints — detaching any kernel driver first. That is how the app *forces*
//! the connection on machines whose OS has no idea how to talk to the phone
//! (the model Nokia PC Suite / NSS used). DKU-2 native-USB and BB5 phones carry
//! FBUS this way; a serial-bridge cable can still use the serial path instead.

use std::time::Duration;

use crate::{Transport, TransportError};

pub struct UsbTransport {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    interface: u8,
    ep_in: u8,
    ep_out: u8,
    timeout: Duration,
}

impl UsbTransport {
    /// Open the first device matching `vid`/`pid`, claim `interface`, and use the
    /// given bulk endpoint addresses.
    pub fn open(
        vid: u16,
        pid: u16,
        interface: u8,
        ep_in: u8,
        ep_out: u8,
    ) -> Result<Self, TransportError> {
        let handle =
            rusb::open_device_with_vid_pid(vid, pid).ok_or_else(|| TransportError::NotFound {
                what: format!("USB device {vid:04x}:{pid:04x}"),
            })?;

        // Detach any kernel driver (e.g. cdc-acm/phonet) so we can claim it.
        #[cfg(target_os = "linux")]
        {
            let _ = handle.set_auto_detach_kernel_driver(true);
        }
        handle
            .claim_interface(interface)
            .map_err(|e| TransportError::Open {
                path: format!("usb {vid:04x}:{pid:04x} iface {interface}"),
                source: e.into(),
            })?;

        Ok(Self {
            handle,
            interface,
            ep_in,
            ep_out,
            timeout: Duration::from_millis(500),
        })
    }

    /// Claim the first connected device with vendor id `vid` and auto-discover
    /// the FBUS bulk endpoints. This is the "force the connection" entry point:
    /// the app takes the device from the OS (kernel driver detached) and speaks
    /// the protocol itself. Final on-device behaviour needs real hardware; the
    /// endpoint-selection logic is unit-tested via [`pick_bulk_pair`].
    pub fn open_fbus_auto(vid: u16) -> Result<Self, TransportError> {
        for device in rusb::devices()?.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if desc.vendor_id() != vid {
                continue;
            }
            let pid = desc.product_id();
            let handle = match device.open() {
                Ok(h) => h,
                Err(_) => continue,
            };
            #[cfg(target_os = "linux")]
            {
                let _ = handle.set_auto_detach_kernel_driver(true);
            }

            // Search every configuration/interface for a bulk IN+OUT pair,
            // preferring a vendor-specific (class 0xff) interface.
            for ci in 0..desc.num_configurations() {
                let config = match device.config_descriptor(ci) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let mut best: Option<(u8, u8, u8, bool)> = None; // iface, in, out, vendor
                for iface in config.interfaces() {
                    for id in iface.descriptors() {
                        let eps: Vec<_> = id
                            .endpoint_descriptors()
                            .map(|e| (e.address(), e.direction(), e.transfer_type()))
                            .collect();
                        if let Some((ep_in, ep_out)) = pick_bulk_pair(&eps) {
                            let vendor = id.class_code() == 0xff;
                            if best.is_none() || (vendor && !best.as_ref().unwrap().3) {
                                best = Some((id.interface_number(), ep_in, ep_out, vendor));
                            }
                        }
                    }
                }
                if let Some((iface_num, ep_in, ep_out, _)) = best {
                    let _ = handle.set_active_configuration(config.number());
                    handle
                        .claim_interface(iface_num)
                        .map_err(|e| TransportError::Open {
                            path: format!("usb {vid:04x}:{pid:04x} iface {iface_num}"),
                            source: e.into(),
                        })?;
                    return Ok(Self {
                        handle,
                        interface: iface_num,
                        ep_in,
                        ep_out,
                        timeout: Duration::from_millis(500),
                    });
                }
            }
        }
        Err(TransportError::NotFound {
            what: format!("no claimable FBUS USB device with vendor {vid:04x}"),
        })
    }
}

/// From a list of `(address, direction, transfer_type)` endpoints, pick the
/// first bulk IN and bulk OUT pair. Returns `None` if either is missing.
fn pick_bulk_pair(eps: &[(u8, rusb::Direction, rusb::TransferType)]) -> Option<(u8, u8)> {
    let mut ep_in = None;
    let mut ep_out = None;
    for (addr, dir, tt) in eps {
        if *tt != rusb::TransferType::Bulk {
            continue;
        }
        match dir {
            rusb::Direction::In if ep_in.is_none() => ep_in = Some(*addr),
            rusb::Direction::Out if ep_out.is_none() => ep_out = Some(*addr),
            _ => {}
        }
    }
    Some((ep_in?, ep_out?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusb::{Direction, TransferType};

    #[test]
    fn picks_bulk_in_out_pair() {
        let eps = [
            (0x81, Direction::In, TransferType::Interrupt), // ignored
            (0x82, Direction::In, TransferType::Bulk),
            (0x02, Direction::Out, TransferType::Bulk),
        ];
        assert_eq!(pick_bulk_pair(&eps), Some((0x82, 0x02)));
    }

    #[test]
    fn none_without_both_directions() {
        let eps = [(0x82, Direction::In, TransferType::Bulk)];
        assert_eq!(pick_bulk_pair(&eps), None);
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.interface);
    }
}

impl Transport for UsbTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut written = 0;
        while written < bytes.len() {
            let n = self
                .handle
                .write_bulk(self.ep_out, &bytes[written..], self.timeout)
                .map_err(TransportError::Usb)?;
            if n == 0 {
                return Err(TransportError::NotFound {
                    what: "bulk write made no progress".into(),
                });
            }
            written += n;
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        match self.handle.read_bulk(self.ep_in, buf, self.timeout) {
            Ok(n) => Ok(n),
            Err(rusb::Error::Timeout) => Ok(0),
            Err(e) => Err(TransportError::Usb(e)),
        }
    }
}
