//! Transport layer: moves raw bytes between the host and the phone over a serial
//! port or raw USB, and drives a simple FBUS/2 request/response exchange on top.
//!
//! Framing lives in `symbinux-protocol`; this crate is the I/O half. The two are
//! kept separate so the protocol stays deterministically testable without any
//! hardware.

use std::time::{Duration, Instant};

use symbinux_protocol::{Fbus2Frame, Fbus2Reader};

pub mod enumerate;
pub mod ports;
pub mod serial;
pub mod usb;

pub use enumerate::{list_usb_devices, Role, UsbDeviceInfo};
pub use ports::{available_serial_ports, resolve_serial_port, SerialPortInfo};
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
                // A data reply may be fragmented across several frames; keep
                // reading until the last one (FramesToGo <= 1). ACKs don't count.
                let done = !frame.is_ack()
                    && frame
                        .block_parts()
                        .map(|(_, ftg, _)| ftg <= 1)
                        .unwrap_or(true);
                frames.push(frame);
                if done {
                    return Ok(frames);
                }
            }
        } else {
            // A read timeout (no bytes): back off briefly so we don't spin the
            // CPU polling the port hundreds of times a second while waiting.
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    if frames.is_empty() {
        Err(TransportError::Timeout)
    } else {
        // We saw an ACK but no data frame before the deadline.
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbinux_protocol::reassemble_fbus2;

    /// A scripted transport that replays canned bytes, for testing the exchange.
    struct MockTransport {
        data: Vec<u8>,
        pos: usize,
    }

    impl Transport for MockTransport {
        fn write_all(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
            Ok(())
        }
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
            if self.pos >= self.data.len() {
                return Ok(0);
            }
            let n = (self.data.len() - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn exchange_collects_fragmented_reply() {
        // Reply = ACK, then two fragments of msg 0x03 (FramesToGo 2 then 1).
        let ack = Fbus2Frame::decode(&[0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71])
            .unwrap()
            .0;
        let f1 = Fbus2Frame::command(0x03, &[0xAA, 0xBB], 2, 0x40);
        let f2 = Fbus2Frame::command(0x03, &[0xCC], 1, 0x41);
        let mut data = ack.encode();
        data.extend(f1.encode());
        data.extend(f2.encode());

        let mut mock = MockTransport { data, pos: 0 };
        let frames = exchange_fbus2(&mut mock, &f1, Duration::from_millis(500)).unwrap();
        // The exchange kept reading past the first fragment to the last one.
        let (msg_type, payload) = reassemble_fbus2(&frames).unwrap();
        assert_eq!(msg_type, 0x03);
        assert_eq!(payload, vec![0xAA, 0xBB, 0xCC]);
    }
}
