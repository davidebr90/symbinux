//! Raw USB bulk transport (libusb) for phones that do NOT expose a ttyUSB port.
//!
//! DKU-2 native-USB and BB5 phones carry FBUS over USB bulk endpoints rather than
//! a serial bridge. Endpoint discovery and the exact interface/altsetting differ
//! per model, so this is intentionally a thin, roadmap-stage wrapper: it opens
//! the device and claims an interface, and does bulk read/write once endpoints
//! are supplied. Full BB5 endpoint auto-detection is tracked in docs/ROADMAP.md.

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
        let handle = rusb::open_device_with_vid_pid(vid, pid).ok_or_else(|| TransportError::NotFound {
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
