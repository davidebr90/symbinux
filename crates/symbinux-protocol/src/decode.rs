//! Typed decoders that turn raw response frames into structured data.
//!
//! The framing layer gives you bytes; this turns known replies into Rust
//! structs. Start small and grow: the HW/SW version reply is fully covered and
//! validated against the real Nokia 3310 capture fixture. Phonebook and SMS
//! decoders are the natural next additions (see `docs/ROADMAP.md`).

use crate::fbus2::Fbus2Frame;
use crate::message::MSG_HW_SW_RESP;

/// Hardware & software identity parsed from a `0xD2` reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwSwVersion {
    /// Firmware version, e.g. "04.45".
    pub firmware: String,
    /// Firmware build date, e.g. "21-06-01".
    pub date: String,
    /// Model code, e.g. "NHM-5" (Nokia 3310).
    pub model: String,
}

/// Parse a HW/SW version reply. The payload carries ASCII of the form
/// `"V <firmware>\n<date>\n<model>\n(c) NMP."` somewhere inside the frame data.
/// Returns `None` if the frame is not a HW/SW reply or the marker is absent.
pub fn hw_sw_version(frame: &Fbus2Frame) -> Option<HwSwVersion> {
    if frame.msg_type != MSG_HW_SW_RESP {
        return None;
    }
    // Locate the "V " marker that opens the ASCII block.
    let start = frame.data.windows(2).position(|w| w == b"V ")?;
    let tail = &frame.data[start..];
    // The ASCII block ends at the first NUL (trailing framing bytes follow).
    let end = tail.iter().position(|b| *b == 0x00).unwrap_or(tail.len());
    let text = String::from_utf8_lossy(&tail[..end]);

    let mut lines = text.split('\n');
    let firmware = lines.next()?.trim_start_matches("V ").trim().to_string();
    let date = lines.next().unwrap_or("").trim().to_string();
    let model = lines.next().unwrap_or("").trim().to_string();
    if firmware.is_empty() && model.is_empty() {
        return None;
    }
    Some(HwSwVersion { firmware, date, model })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_real_3310_hw_sw_reply() {
        // The documented Nokia 3310 HW/SW response capture.
        let wire = [
            0x1E, 0x0C, 0x00, 0xD2, 0x00, 0x26, 0x01, 0x00, 0x00, 0x03, 0x56, 0x20, 0x30, 0x34,
            0x2E, 0x34, 0x35, 0x0A, 0x32, 0x31, 0x2D, 0x30, 0x36, 0x2D, 0x30, 0x31, 0x0A, 0x4E,
            0x48, 0x4D, 0x2D, 0x35, 0x0A, 0x28, 0x63, 0x29, 0x20, 0x4E, 0x4D, 0x50, 0x2E, 0x00,
            0x01, 0x43, 0x3F, 0xA6,
        ];
        let (frame, _) = Fbus2Frame::decode(&wire).unwrap();
        let v = hw_sw_version(&frame).expect("decoded");
        assert_eq!(v.firmware, "04.45");
        assert_eq!(v.date, "21-06-01");
        assert_eq!(v.model, "NHM-5");
    }

    #[test]
    fn returns_none_for_non_hwsw_frame() {
        let ack = [0x1E, 0x0C, 0x00, 0x7F, 0x00, 0x02, 0xD1, 0x00, 0xCF, 0x71];
        let (frame, _) = Fbus2Frame::decode(&ack).unwrap();
        assert!(hw_sw_version(&frame).is_none());
    }
}
