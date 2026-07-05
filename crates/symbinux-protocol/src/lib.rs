//! Clean-room encoder/decoder for the Nokia FBUS/MBUS serial protocols used by
//! legacy phones (Series 40, Series 60/Symbian, BB5) via USB cables.
//!
//! This crate is pure framing and command construction: it performs NO I/O and
//! has no knowledge of serial ports or USB. Feed its byte buffers to a transport
//! (see the `symbinux-transport` crate). Keeping it I/O-free makes every frame
//! deterministically testable against the fixtures in `tests/`.
//!
//! The protocol details are reconstructed from the gnokii/gammu community
//! projects and cross-checked against documented real captures. Byte-level
//! sources and per-command confidence levels live in `docs/PROTOCOL_NOTES.md`.
//! Nothing here derives from proprietary Nokia libraries or binaries.

pub mod checksum;
pub mod decode;
pub mod fbus2;
pub mod mbus;
pub mod message;

pub use decode::{
    decode_sms_deliver, gsm7_unpack, hw_sw_version, HwSwVersion, PhonebookEntry, Sms,
};
pub use fbus2::{Fbus2Error, Fbus2Frame};
pub use mbus::{MbusError, MbusFrame};
pub use message::{Command, MemoryType, Safety};

/// A minimal incremental reader that pulls complete FBUS/2 frames out of a byte
/// stream, tolerating partial reads from the transport. Bytes before a valid
/// `0x1E` frame-id (e.g. MBUS-style local echo or line noise) are skipped.
#[derive(Debug, Default)]
pub struct Fbus2Reader {
    buf: Vec<u8>,
}

impl Fbus2Reader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append freshly-read bytes to the internal buffer.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Try to pull the next complete, checksum-valid frame. Returns `None` when
    /// more bytes are needed. Resynchronises past leading garbage.
    pub fn next_frame(&mut self) -> Option<Fbus2Frame> {
        loop {
            // Drop anything before the next frame-id marker.
            match self.buf.iter().position(|b| *b == fbus2::FRAME_ID_CABLE) {
                None => {
                    self.buf.clear();
                    return None;
                }
                Some(0) => {}
                Some(pos) => {
                    self.buf.drain(..pos);
                }
            }
            match Fbus2Frame::decode(&self.buf) {
                Ok((frame, used)) => {
                    self.buf.drain(..used);
                    return Some(frame);
                }
                Err(Fbus2Error::TooShort(..)) | Err(Fbus2Error::LengthOverflow { .. }) => {
                    return None; // wait for more bytes
                }
                Err(_) => {
                    // Bad frame at this marker; skip it and resync.
                    self.buf.drain(..1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_extracts_frame_after_echo_noise() {
        let mut r = Fbus2Reader::new();
        // Leading noise (as if half-duplex echo), then a valid ACK frame.
        r.feed(&[0xAA, 0x55, 0x00]);
        r.feed(&[0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71]);
        let f = r.next_frame().expect("frame");
        assert!(f.is_ack());
        assert!(r.next_frame().is_none());
    }

    #[test]
    fn reader_waits_for_partial_frame() {
        let mut r = Fbus2Reader::new();
        r.feed(&[0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02]);
        assert!(r.next_frame().is_none());
        r.feed(&[0xD1, 0x00, 0xCF, 0x71]);
        assert!(r.next_frame().unwrap().is_ack());
    }
}
