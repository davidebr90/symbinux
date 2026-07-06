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

/// Largest frame length the reader will wait to complete. A false `0x1E` in
/// line noise can declare a huge length; beyond this we treat the marker as
/// garbage and resync rather than buffering forever. Real FBUS/2 frames are far
/// smaller than this.
const MAX_PLAUSIBLE_FRAME: usize = 1200;

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

    /// Number of bytes currently buffered (not yet consumed as a frame).
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
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
                Err(Fbus2Error::LengthOverflow { declared, .. })
                    if declared > MAX_PLAUSIBLE_FRAME =>
                {
                    // A false marker in noise claimed an implausible length;
                    // skip this byte and resync instead of buffering forever.
                    self.buf.drain(..1);
                }
                Err(Fbus2Error::TooShort(..)) | Err(Fbus2Error::LengthOverflow { .. }) => {
                    return None; // genuinely waiting for more bytes
                }
                Err(_) => {
                    // Bad frame at this marker; skip it and resync.
                    self.buf.drain(..1);
                }
            }
        }
    }
}

/// Reassemble a (possibly fragmented) FBUS/2 response into `(msg_type, data)`.
///
/// A phone can split a long reply across several frames, each carrying part of
/// the payload plus a `FramesToGo` counter that reaches `1` on the last frame.
/// This skips ACKs and concatenates the blocks of consecutive fragments of the
/// same message type, stopping at the final fragment. Returns `None` if there is
/// no data frame.
pub fn reassemble_fbus2(frames: &[Fbus2Frame]) -> Option<(u8, Vec<u8>)> {
    let mut data = Vec::new();
    let mut msg_type: Option<u8> = None;
    for frame in frames {
        if frame.is_ack() {
            continue;
        }
        let (block, frames_to_go, _seq) = match frame.block_parts() {
            Some(parts) => parts,
            None => (frame.data.as_slice(), 1u8, 0u8),
        };
        match msg_type {
            None => msg_type = Some(frame.msg_type),
            Some(mt) if mt == frame.msg_type => {}
            Some(_) => break, // a different message started; stop here
        }
        data.extend_from_slice(block);
        if frames_to_go <= 1 {
            break; // final fragment
        }
    }
    msg_type.map(|mt| (mt, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassembles_two_fragments() {
        // Two fragments of msg 0x03: FramesToGo 2 then 1, blocks [AA BB] + [CC].
        let f1 = Fbus2Frame::command(0x03, &[0xAA, 0xBB], 2, 0x40);
        let f2 = Fbus2Frame::command(0x03, &[0xCC], 1, 0x41);
        let (mt, data) = reassemble_fbus2(&[f1, f2]).unwrap();
        assert_eq!(mt, 0x03);
        assert_eq!(data, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn reassemble_skips_acks_and_single_frame() {
        let (ack, _) =
            Fbus2Frame::decode(&[0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71])
                .unwrap();
        let data = Fbus2Frame::command(0xD2, &[0x01, 0x02], 1, 0x40);
        let (mt, out) = reassemble_fbus2(&[ack, data]).unwrap();
        assert_eq!(mt, 0xD2);
        assert_eq!(out, vec![0x01, 0x02]);
    }

    #[test]
    fn reader_does_not_wedge_on_false_marker_noise() {
        let mut r = Fbus2Reader::new();
        // A false 0x1E followed by bytes declaring a huge length, then a real
        // ACK frame. The reader must resync and still find the real frame,
        // without its buffer growing without bound.
        r.feed(&[0x1E, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00]);
        r.feed(&[0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71]);
        let f = r.next_frame().expect("resynced to real frame");
        assert!(f.is_ack());
    }

    #[test]
    fn reader_survives_random_noise_without_panic() {
        let mut r = Fbus2Reader::new();
        // Deterministic pseudo-random byte stream (LCG) — must never panic and
        // the buffer must stay bounded.
        let mut state: u32 = 0x1234_5678;
        for _ in 0..20_000 {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            r.feed(&[(state >> 16) as u8]);
            let _ = r.next_frame();
            assert!(r.buffered_len() <= MAX_PLAUSIBLE_FRAME + 64);
        }
    }

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
