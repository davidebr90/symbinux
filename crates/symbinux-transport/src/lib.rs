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
    Usb(#[from] nusb::Error),
    #[error("usb transfer error: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),
    #[error("timed out waiting for a reply from the phone")]
    Timeout,
}

/// A byte-stream link to the phone. Implemented by serial and USB backends.
pub trait Transport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Read up to `buf.len()` bytes. Returns `Ok(0)` on read timeout (not an error).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError>;
}

/// Tunables for an FBUS/2 exchange: how long to wait for the phone to respond to
/// each transmission, how many times to retransmit if it stays silent, and the
/// hard overall cap.
#[derive(Debug, Clone)]
pub struct ExchangeConfig {
    /// Per-attempt wait for any response before retransmitting the command.
    pub ack_timeout: Duration,
    /// Number of retransmissions if the phone doesn't respond (gnokii resends
    /// the command frame when no ACK arrives in time).
    pub retries: u32,
    /// Hard cap on the whole exchange.
    pub overall_timeout: Duration,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            ack_timeout: Duration::from_millis(400),
            retries: 3,
            overall_timeout: Duration::from_millis(2000),
        }
    }
}

/// Pull any complete frames the reader has into `frames`. Returns `true` once the
/// last fragment of a (non-ACK) data reply has been collected.
fn drain_ready(reader: &mut Fbus2Reader, frames: &mut Vec<Fbus2Frame>) -> bool {
    while let Some(frame) = reader.next_frame() {
        let done = !frame.is_ack()
            && frame
                .block_parts()
                .map(|(_, ftg, _)| ftg <= 1)
                .unwrap_or(true);
        frames.push(frame);
        if done {
            return true;
        }
    }
    false
}

/// Send one FBUS/2 command and collect the reply, using the default config
/// (retransmit on silence) bounded by `overall_timeout`.
pub fn exchange_fbus2<T: Transport>(
    link: &mut T,
    command: &Fbus2Frame,
    overall_timeout: Duration,
) -> Result<Vec<Fbus2Frame>, TransportError> {
    exchange_fbus2_with(
        link,
        command,
        &ExchangeConfig {
            overall_timeout,
            ..Default::default()
        },
    )
}

/// Send one FBUS/2 command, retransmitting it if the phone stays silent, and
/// collect the (possibly fragmented) reply. Returns every frame received.
pub fn exchange_fbus2_with<T: Transport>(
    link: &mut T,
    command: &Fbus2Frame,
    config: &ExchangeConfig,
) -> Result<Vec<Fbus2Frame>, TransportError> {
    let overall_deadline = Instant::now() + config.overall_timeout;
    let mut reader = Fbus2Reader::new();
    let mut scratch = [0u8; 512];
    let mut frames = Vec::new();
    let mut got_response = false;

    // Phase 1: (re)transmit until the phone answers with anything, or give up.
    'attempts: for _ in 0..=config.retries {
        link.write_all(&command.encode())?;
        let attempt_deadline = Instant::now() + config.ack_timeout;
        while Instant::now() < attempt_deadline {
            if Instant::now() >= overall_deadline {
                break 'attempts;
            }
            let n = link.read(&mut scratch)?;
            if n > 0 {
                reader.feed(&scratch[..n]);
                got_response = true;
                if drain_ready(&mut reader, &mut frames) {
                    return Ok(frames);
                }
                break; // got something; stop retransmitting, move to phase 2
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if got_response {
            break;
        }
        // Attempt window elapsed with no response → retransmit.
    }

    if !got_response {
        return Err(TransportError::Timeout);
    }

    // Phase 2: collect the rest of the (possibly fragmented) reply.
    // If the deadline expires here and no data frame has been collected,
    // we still return Ok with whatever we have (ACKs only is valid for
    // commands that have no data reply — the caller interprets the result).
    while Instant::now() < overall_deadline {
        let n = link.read(&mut scratch)?;
        if n > 0 {
            reader.feed(&scratch[..n]);
            if drain_ready(&mut reader, &mut frames) {
                return Ok(frames);
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    Ok(frames)
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

    /// A transport that stays silent until the command has been (re)transmitted
    /// at least twice, then replies. Counts writes so the test can assert a retry.
    struct RetryMock {
        response: Vec<u8>,
        pos: usize,
        writes: usize,
    }

    impl Transport for RetryMock {
        fn write_all(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
            self.writes += 1;
            Ok(())
        }
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
            if self.writes < 2 || self.pos >= self.response.len() {
                return Ok(0); // silent until retransmitted once
            }
            let n = (self.response.len() - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.response[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn retransmits_when_phone_is_silent() {
        let reply = Fbus2Frame::command(0xD2, &[0x01, 0x02], 1, 0x40).encode();
        let mut mock = RetryMock {
            response: reply,
            pos: 0,
            writes: 0,
        };
        let cfg = ExchangeConfig {
            ack_timeout: Duration::from_millis(20),
            retries: 5,
            overall_timeout: Duration::from_millis(1000),
        };
        let cmd = Fbus2Frame::command(0xD1, &[0x00, 0x03, 0x00], 1, 0x40);
        let frames = exchange_fbus2_with(&mut mock, &cmd, &cfg).unwrap();
        assert!(mock.writes >= 2, "command should have been retransmitted");
        assert!(frames.iter().any(|f| !f.is_ack()), "got the data reply");
    }

    #[test]
    fn times_out_when_phone_never_responds() {
        struct Silent;
        impl Transport for Silent {
            fn write_all(&mut self, _b: &[u8]) -> Result<(), TransportError> {
                Ok(())
            }
            fn read(&mut self, _b: &mut [u8]) -> Result<usize, TransportError> {
                Ok(0)
            }
        }
        let cfg = ExchangeConfig {
            ack_timeout: Duration::from_millis(10),
            retries: 2,
            overall_timeout: Duration::from_millis(80),
        };
        let cmd = Fbus2Frame::command(0xD1, &[], 1, 0x40);
        assert!(matches!(
            exchange_fbus2_with(&mut Silent, &cmd, &cfg),
            Err(TransportError::Timeout)
        ));
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
