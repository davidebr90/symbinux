//! Checksum routines for the two Nokia framing families.
//!
//! FBUS/2 uses TWO checksums computed over the whole frame (excluding the two
//! trailing checksum bytes themselves): one over the bytes at even indices and
//! one over the bytes at odd indices. MBUS/M2BUS uses a SINGLE XOR over all
//! bytes. See `docs/PROTOCOL_NOTES.md` for sources and confidence levels.
//!
//! The FBUS/2 routine is validated in the test suite against real documented
//! Nokia 3310 capture frames (checksum oracles `CF 71` and `72 D5`).

/// FBUS/2 dual checksum over `frame` (the full frame *without* the two trailing
/// checksum bytes, i.e. from the `0x1E` frame-id up to and including any
/// word-alignment padding byte).
///
/// Returns `(chksum1, chksum2)` where `chksum1` XORs the even-index bytes and
/// `chksum2` XORs the odd-index bytes.
pub fn fbus2(frame: &[u8]) -> (u8, u8) {
    let mut c1 = 0u8;
    let mut c2 = 0u8;
    for (i, b) in frame.iter().enumerate() {
        if i % 2 == 0 {
            c1 ^= *b;
        } else {
            c2 ^= *b;
        }
    }
    (c1, c2)
}

/// MBUS/M2BUS single checksum: XOR of every byte in `frame` (the full frame
/// without the trailing checksum byte).
pub fn mbus(frame: &[u8]) -> u8 {
    frame.iter().fold(0u8, |acc, b| acc ^ *b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fbus2_matches_ack_oracle() {
        // Real documented ACK frame: 1E 0C 00 7F 00 02 D1 00 | CF 71
        let body = [0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00];
        assert_eq!(fbus2(&body), (0xCF, 0x71));
    }

    #[test]
    fn fbus2_matches_padded_oracle() {
        // Real documented request frame with padding: ... | 72 D5
        let body = [
            0x1E, 0x00, 0x0C, 0xD1, 0x00, 0x07, 0x00, 0x01, 0x00, 0x03, 0x00, 0x01, 0x60, 0x00,
        ];
        assert_eq!(fbus2(&body), (0x72, 0xD5));
    }

    #[test]
    fn mbus_xor_all() {
        assert_eq!(mbus(&[0x00, 0xE4, 0x02, 0xD1]), 0x00 ^ 0xE4 ^ 0x02 ^ 0xD1);
    }
}
