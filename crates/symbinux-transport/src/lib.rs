//! Transport layer: moves raw bytes between the host and the phone over a serial
//! port or raw USB, and drives a simple FBUS/2 request/response exchange on top.
//!
//! Framing lives in `symbinux-protocol`; this crate is the I/O half. The two are
//! kept separate so the protocol stays deterministically testable without any
//! hardware.

use std::time::{Duration, Instant};

use symbinux_protocol::{Fbus2Frame, Fbus2Reader};

pub mod enumerate;
pub mod serial;
pub mod usb;

pub use enumerate::{list_usb_devices, Role, UsbDeviceInfo};
pub use serial::SerialTransport;
pub use usb::UsbTransport;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("could not open {path}: {source}")]
    Open {
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("device not found: {what}")]
    NotFound { what: String },
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("usb error: {0}")]
    Usb(#[from] rusb::Error),
    #[error("timed out waiting for a reply from the phone")]
    Timeout,
}

/// A byte-stream link to the phone. Implemented by serial and USB backends.
pub trait Transport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Read up to `buf.len()` bytes. Returns `Ok(0)` on read timeout (not an error).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError>;
}

/// Send one FBUS/2 command frame and collect reply frames until a non-ACK data
/// frame arrives or `overall_timeout` elapses. Returns all frames received (the
/// ACK, if any, plus the data frame), letting the caller interpret them.
pub fn exchange_fbus2<T: Transport>(
    link: &mut T,
    command: &Fbus2Frame,
    overall_timeout: Duration,
) -> Result<Vec<Fbus2Frame>, TransportError> {
    link.write_all(&command.encode())?;

    let deadline = Instant::now() + overall_timeout;
    let mut reader = Fbus2Reader::new();
    let mut scratch = [0u8; 512];
    let mut frames = Vec::new();

    while Instant::now() < deadline {
        let n = link.read(&mut scratch)?;
        if n > 0 {
            reader.feed(&scratch[..n]);
            while let Some(frame) = reader.next_frame() {
                let is_ack = frame.is_ack();
                frames.push(frame);
                if !is_ack {
                    // Got a data reply; we are done.
                    return Ok(frames);
                }
            }
        }
    }

    if frames.is_empty() {
        Err(TransportError::Timeout)
    } else {
        // We saw an ACK but no data frame before the deadline.
        Ok(frames)
    }
}
