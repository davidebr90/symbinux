//! MBUS v1 (M2BUS) frame codec.
//!
//! Frame layout (see `docs/PROTOCOL_NOTES.md`):
//!
//! ```text
//! DestDEV | SrcDEV | FrameLength | MsgType | <block ...> | SeqNo | Csum
//! ```
//!
//! `FrameLength` is the length of `block`. `Csum` is the XOR of every preceding
//! byte in the frame. MBUS v1 has NO leading frame-id byte (unlike FBUS/2 and
//! M2BUS v2). It is a single-wire half-duplex bus: transmitted bytes are echoed
//! back locally and must be drained before the reply — that concern lives in the
//! transport layer, not here.
//!
//! NOTE: unlike FBUS/2, there is no publicly documented real capture with a
//! checksum oracle for MBUS v1, so the fixtures for this codec are SYNTHETIC
//! (checksum computed from the XOR-all rule, not confirmed against hardware).

use crate::checksum;

/// Phone device address.
pub const DEV_PHONE: u8 = 0x00;
/// PC address in normal mode.
pub const DEV_PC: u8 = 0xE4;
/// PC address in wake-up mode.
pub const DEV_PC_WAKEUP: u8 = 0xF8;

#[derive(Debug, thiserror::Error)]
pub enum MbusError {
    #[error("frame too short: got {0} bytes, need at least {1}")]
    TooShort(usize, usize),
    #[error("declared block length {declared} does not fit in {available} available bytes")]
    LengthOverflow { declared: usize, available: usize },
    #[error("checksum mismatch: computed {computed:#04x}, frame carried {carried:#04x}")]
    BadChecksum { computed: u8, carried: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbusFrame {
    pub dest: u8,
    pub src: u8,
    pub msg_type: u8,
    pub block: Vec<u8>,
    pub seq: u8,
}

impl MbusFrame {
    /// Build a PC→phone command frame in normal mode.
    pub fn command(msg_type: u8, block: &[u8], seq: u8) -> Self {
        Self {
            dest: DEV_PHONE,
            src: DEV_PC,
            msg_type,
            block: block.to_vec(),
            seq,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.block.len() + 2);
        buf.push(self.dest);
        buf.push(self.src);
        buf.push(self.block.len() as u8);
        buf.push(self.msg_type);
        buf.extend_from_slice(&self.block);
        buf.push(self.seq);
        let c = checksum::mbus(&buf);
        buf.push(c);
        buf
    }

    /// Parse one frame from the front of `bytes`, returning the frame and the
    /// number of bytes consumed. Verifies the checksum.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), MbusError> {
        if bytes.len() < 6 {
            return Err(MbusError::TooShort(bytes.len(), 6));
        }
        let dest = bytes[0];
        let src = bytes[1];
        let block_len = bytes[2] as usize;
        let msg_type = bytes[3];
        let total = 4 + block_len + 1 + 1; // header + block + seq + csum
        if bytes.len() < total {
            return Err(MbusError::LengthOverflow {
                declared: total,
                available: bytes.len(),
            });
        }
        let computed = checksum::mbus(&bytes[..total - 1]);
        let carried = bytes[total - 1];
        if computed != carried {
            return Err(MbusError::BadChecksum { computed, carried });
        }
        let block = bytes[4..4 + block_len].to_vec();
        let seq = bytes[4 + block_len];
        Ok((
            Self {
                dest,
                src,
                msg_type,
                block,
                seq,
            },
            total,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_synthetic() {
        let f = MbusFrame::command(0xD1, &[0x00, 0x03, 0x00], 0x01);
        let wire = f.encode();
        // dest,src,len,msgtype, block(3), seq, csum
        assert_eq!(&wire[..4], &[0x00, 0xE4, 0x03, 0xD1]);
        let (back, used) = MbusFrame::decode(&wire).unwrap();
        assert_eq!(used, wire.len());
        assert_eq!(back, f);
    }

    #[test]
    fn detects_corruption() {
        let f = MbusFrame::command(0x03, &[0x00, 0x01, 0x02, 0x00, 0x00], 0x05);
        let mut wire = f.encode();
        let last = wire.len() - 1;
        wire[last] ^= 0xFF;
        assert!(matches!(
            MbusFrame::decode(&wire),
            Err(MbusError::BadChecksum { .. })
        ));
    }
}
