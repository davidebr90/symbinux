//! Raw USB bulk transport (nusb) — the app-owned connection path.
//!
//! Instead of relying on the OS exposing a `/dev/ttyUSB*` serial port, this
//! claims the phone's USB device directly and drives FBUS over its bulk
//! endpoints — detaching any kernel driver first. That is how the app *forces*
//! the connection on machines whose OS has no idea how to talk to the phone
//! (the model Nokia PC Suite / NSS used). DKU-2 native-USB and BB5 phones carry
//! FBUS this way; a serial-bridge cable can still use the serial path instead.
//!
//! Built on the pure-Rust [`nusb`] crate (no libusb C dependency). On Linux the
//! kernel driver is detached via `detach_and_claim_interface` and re-attached
//! automatically when the endpoints (and thus the interface) are dropped. On
//! Windows the target must be bound to the WinUSB driver — that is a WinUSB
//! limitation shared with libusb, and unrelated to the serial path most Windows
//! users take.

use std::time::Duration;

use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, Direction, In, Out, TransferError};
use nusb::{Endpoint, MaybeFuture};

use crate::{Transport, TransportError};

pub struct UsbTransport {
    ep_in: Endpoint<Bulk, In>,
    ep_out: Endpoint<Bulk, Out>,
    timeout: Duration,
}

impl UsbTransport {
    /// Claim the first connected device with vendor id `vid` and auto-discover
    /// the FBUS bulk endpoints. This is the "force the connection" entry point:
    /// the app takes the device from the OS (kernel driver detached) and speaks
    /// the protocol itself. Final on-device behaviour needs real hardware; the
    /// endpoint-selection logic is unit-tested via [`pick_bulk_pair`].
    pub fn open_fbus_auto(vid: u16) -> Result<Self, TransportError> {
        for info in nusb::list_devices().wait()? {
            if info.vendor_id() != vid {
                continue;
            }
            let device = match info.open().wait() {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Search every configuration/interface for a bulk IN+OUT pair,
            // preferring a vendor-specific (class 0xff) interface.
            let mut best: Option<(u8, u8, u8, u8, bool)> = None; // config, iface, in, out, vendor
            for config in device.configurations() {
                let cfg_value = config.configuration_value();
                for iface in config.interface_alt_settings() {
                    let eps: Vec<_> = iface
                        .endpoints()
                        .map(|e| (e.address(), e.direction(), e.transfer_type()))
                        .collect();
                    if let Some((ep_in, ep_out)) = pick_bulk_pair(&eps) {
                        let vendor = iface.class() == 0xff;
                        if best.is_none() || (vendor && !best.as_ref().unwrap().4) {
                            best =
                                Some((cfg_value, iface.interface_number(), ep_in, ep_out, vendor));
                        }
                    }
                }
            }

            let Some((cfg_value, iface_num, ep_in_addr, ep_out_addr, _)) = best else {
                continue;
            };

            // Select the configuration that carries the FBUS interface if it
            // isn't already active (best-effort; unsupported on Windows).
            if device
                .active_configuration()
                .map(|c| c.configuration_value())
                != Ok(cfg_value)
            {
                let _ = device.set_configuration(cfg_value).wait();
            }

            // Detach any kernel driver (cdc-acm/phonet on Linux) and claim.
            let interface = device
                .detach_and_claim_interface(iface_num)
                .wait()
                .map_err(|e| TransportError::Open {
                    path: format!("usb {vid:04x} iface {iface_num}"),
                    source: e.into(),
                })?;

            let ep_in =
                interface
                    .endpoint::<Bulk, In>(ep_in_addr)
                    .map_err(|e| TransportError::Open {
                        path: format!("usb {vid:04x} ep_in {ep_in_addr:#04x}"),
                        source: e.into(),
                    })?;
            let ep_out =
                interface
                    .endpoint::<Bulk, Out>(ep_out_addr)
                    .map_err(|e| TransportError::Open {
                        path: format!("usb {vid:04x} ep_out {ep_out_addr:#04x}"),
                        source: e.into(),
                    })?;

            // The endpoints keep the interface and device alive; on drop the
            // interface is released and the kernel driver re-attached.
            return Ok(Self {
                ep_in,
                ep_out,
                timeout: Duration::from_millis(500),
            });
        }
        Err(TransportError::NotFound {
            what: format!("no claimable FBUS USB device with vendor {vid:04x}"),
        })
    }
}

/// From a list of `(address, direction, transfer_type)` endpoints, pick the
/// first bulk IN and bulk OUT pair. Returns `None` if either is missing.
fn pick_bulk_pair(eps: &[(u8, Direction, TransferType)]) -> Option<(u8, u8)> {
    let mut ep_in = None;
    let mut ep_out = None;
    for (addr, dir, tt) in eps {
        if *tt != TransferType::Bulk {
            continue;
        }
        match dir {
            Direction::In if ep_in.is_none() => ep_in = Some(*addr),
            Direction::Out if ep_out.is_none() => ep_out = Some(*addr),
            _ => {}
        }
    }
    Some((ep_in?, ep_out?))
}

impl Transport for UsbTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut written = 0;
        while written < bytes.len() {
            let c = self
                .ep_out
                .transfer_blocking(bytes[written..].to_vec().into(), self.timeout);
            c.status.map_err(TransportError::Transfer)?;
            if c.actual_len == 0 {
                return Err(TransportError::NotFound {
                    what: "bulk write made no progress".into(),
                });
            }
            written += c.actual_len;
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        // An IN transfer's requested length must be a nonzero multiple of the
        // endpoint's max packet size. Request as many whole packets as fit in
        // `buf` (callers use a buffer at least one packet wide).
        let mps = self.ep_in.max_packet_size().max(1);
        let requested = (buf.len() / mps).max(1) * mps;
        let c = self
            .ep_in
            .transfer_blocking(Buffer::new(requested), self.timeout);
        match c.status {
            Ok(()) => {
                let n = c.actual_len.min(buf.len());
                buf[..n].copy_from_slice(&c.buffer[..n]);
                Ok(n)
            }
            // A timeout cancels the transfer; treat it as "no data yet", matching
            // the serial backend's read-timeout semantics.
            Err(TransferError::Cancelled) => Ok(0),
            Err(e) => Err(TransportError::Transfer(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
