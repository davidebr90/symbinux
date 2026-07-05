//! FBUS/2 frame codec (serial/cable transport).
//!
//! Frame layout (see `docs/PROTOCOL_NOTES.md`):
//!
//! ```text
//! 1E | DestDEV | SrcDEV | MsgType | LenHi | LenLo | <data ...> | [pad] | Csum1 | Csum2
//! ```
//!
//! `data` is exactly `Len = (LenHi<<8 | LenLo)` bytes long. For ordinary command
//! frames `data == block ++ [FramesToGo, SeqNo]`. A single word-alignment padding
//! byte `0x00` is appended iff `Len` is odd; it is covered by the checksums.

use crate::checksum;

/// FBUS/2 frame-id byte for the serial cable (DKU-2/CA-42 and clones).
pub const FRAME_ID_CABLE: u8 = 0x1E;
/// FBUS/2 frame-id byte for IrDA. UNCERTAIN across sources (0x1C vs 0x1F) — do
/// not rely on it without a real capture. Kept for completeness only.
pub const FRAME_ID_IRDA: u8 = 0x1C;

/// Device address of the phone.
pub const DEV_PHONE: u8 = 0x00;
/// Device address of the PC / terminal equipment.
pub const DEV_PC: u8 = 0x0C;

/// Acknowledge message type.
pub const MSG_ACK: u8 = 0x7F;

#[derive(Debug, thiserror::Error)]
pub enum Fbus2Error {
    #[error("frame too short: got {0} bytes, need at least {1}")]
    TooShort(usize, usize),
    #[error("bad frame id: expected 0x1E (cable), got {0:#04x}")]
    BadFrameId(u8),
    #[error("declared length {declared} does not fit in {available} available bytes")]
    LengthOverflow { declared: usize, available: usize },
    #[error("checksum mismatch: computed ({c1:#04x},{c2:#04x}), frame carried ({f1:#04x},{f2:#04x})")]
    BadChecksum { c1: u8, c2: u8, f1: u8, f2: u8 },
}

/// A decoded (or to-be-encoded) FBUS/2 frame at the transport level.
///
/// `data` is the length-delimited region as-is. Use [`Fbus2Frame::command`] to
/// build an ordinary command frame from a block plus framing counters, and
/// [`Fbus2Frame::block_parts`] to split a decoded frame back apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fbus2Frame {
    pub dest: u8,
    pub src: u8,
    pub msg_type: u8,
    pub data: Vec<u8>,
}

impl Fbus2Frame {
    /// Build a PC→phone command frame.
    ///
    /// `frames_to_go` is `0x01` for a single (last) frame. `seq` is the sequence
    /// byte (gammu convention: `0x40 | (n & 0x07)` on the first block).
    pub fn command(msg_type: u8, block: &[u8], frames_to_go: u8, seq: u8) -> Self {
        let mut data = Vec::with_capacity(block.len() + 2);
        data.extend_from_slice(block);
        data.push(frames_to_go);
        data.push(seq);
        Self {
            dest: DEV_PHONE,
            src: DEV_PC,
            msg_type,
            data,
        }
    }

    /// Split an ordinary command/response frame into `(block, frames_to_go, seq)`.
    ///
    /// Returns `None` for frames whose `data` is shorter than the two trailing
    /// framing bytes (e.g. ACK frames, which carry only `[acked_type, acked_seq]`).
    pub fn block_parts(&self) -> Option<(&[u8], u8, u8)> {
        let n = self.data.len();
        if n < 2 {
            return None;
        }
        Some((&self.data[..n - 2], self.data[n - 2], self.data[n - 1]))
    }

    /// Serialize to wire bytes, computing length, padding and both checksums.
    pub fn encode(&self) -> Vec<u8> {
        let len = self.data.len();
        let mut buf = Vec::with_capacity(6 + len + 3);
        buf.push(FRAME_ID_CABLE);
        buf.push(self.dest);
        buf.push(self.src);
        buf.push(self.msg_type);
        buf.push((len >> 8) as u8);
        buf.push((len & 0xFF) as u8);
        buf.extend_from_slice(&self.data);
        if len % 2 == 1 {
            buf.push(0x00); // word-alignment padding, covered by the checksums
        }
        let (c1, c2) = checksum::fbus2(&buf);
        buf.push(c1);
        buf.push(c2);
        buf
    }

    /// Parse one frame from the front of `bytes`, returning the frame and the
    /// number of bytes consumed. Verifies both checksums.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), Fbus2Error> {
        if bytes.len() < 8 {
            return Err(Fbus2Error::TooShort(bytes.len(), 8));
        }
        if bytes[0] != FRAME_ID_CABLE {
            return Err(Fbus2Error::BadFrameId(bytes[0]));
        }
        let dest = bytes[1];
        let src = bytes[2];
        let msg_type = bytes[3];
        let len = ((bytes[4] as usize) << 8) | bytes[5] as usize;
        let pad = len % 2; // 1 padding byte if len is odd
        let total = 6 + len + pad + 2; // header + data + pad + 2 checksums
        if bytes.len() < total {
            return Err(Fbus2Error::LengthOverflow {
                declared: total,
                available: bytes.len(),
            });
        }
        let body = &bytes[..total - 2];
        let (c1, c2) = checksum::fbus2(body);
        let (f1, f2) = (bytes[total - 2], bytes[total - 1]);
        if (c1, c2) != (f1, f2) {
            return Err(Fbus2Error::BadChecksum { c1, c2, f1, f2 });
        }
        let data = bytes[6..6 + len].to_vec();
        Ok((
            Self {
                dest,
                src,
                msg_type,
                data,
            },
            total,
        ))
    }

    /// True if this frame is an acknowledgement.
    pub fn is_ack(&self) -> bool {
        self.msg_type == MSG_ACK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ack_oracle() {
        let wire = [0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71];
        let (frame, used) = Fbus2Frame::decode(&wire).unwrap();
        assert_eq!(used, wire.len());
        assert!(frame.is_ack());
        assert_eq!(frame.dest, DEV_PC);
        assert_eq!(frame.src, DEV_PHONE);
        assert_eq!(frame.data, vec![0xD1, 0x00]); // acked msg type + seq
        assert_eq!(frame.encode(), wire);
    }

    #[test]
    fn roundtrip_padded_request_oracle() {
        let wire = [
            0x1E, 0x00, 0x0C, 0xD1, 0x00, 0x07, 0x00, 0x01, 0x00, 0x03, 0x00, 0x01, 0x60, 0x00,
            0x72, 0xD5,
        ];
        let (frame, used) = Fbus2Frame::decode(&wire).unwrap();
        assert_eq!(used, wire.len());
        assert_eq!(frame.msg_type, 0xD1);
        let (block, ftg, seq) = frame.block_parts().unwrap();
        assert_eq!(block, &[0x00, 0x01, 0x00, 0x03, 0x00]);
        assert_eq!(ftg, 0x01);
        assert_eq!(seq, 0x60);
        assert_eq!(frame.encode(), wire);
    }

    #[test]
    fn command_builder_encodes_expected_bytes() {
        // Reproduce the documented HW/SW version request exactly.
        let f = Fbus2Frame::command(0xD1, &[0x00, 0x01, 0x00, 0x03, 0x00], 0x01, 0x60);
        assert_eq!(
            f.encode(),
            vec![
                0x1E, 0x00, 0x0C, 0xD1, 0x00, 0x07, 0x00, 0x01, 0x00, 0x03, 0x00, 0x01, 0x60, 0x00,
                0x72, 0xD5,
            ]
        );
    }

    #[test]
    fn detects_corrupted_checksum() {
        let mut wire = vec![0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71];
        wire[8] ^= 0xFF;
        assert!(matches!(
            Fbus2Frame::decode(&wire),
            Err(Fbus2Error::BadChecksum { .. })
        ));
    }
}
