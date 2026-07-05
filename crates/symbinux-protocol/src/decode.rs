//! Typed decoders that turn raw response frames into structured data.
//!
//! The framing layer gives you bytes; this turns known replies into Rust
//! structs. Start small and grow: the HW/SW version reply is fully covered and
//! validated against the real Nokia 3310 capture fixture. Phonebook and SMS
//! decoders are the natural next additions (see `docs/ROADMAP.md`).

use crate::fbus2::Fbus2Frame;
use crate::message::MSG_HW_SW_RESP;

// --- GSM 03.38 / 03.40 primitives (reused by SMS decoding) ------------------

/// Map a GSM default-alphabet septet to a char. This covers the ASCII-compatible
/// subset that legacy contact names and messages use in practice; the handful of
/// accented/special GSM code points that differ from ASCII fall back to '?'.
/// The 7-bit *unpacking* below is exact; only the alphabet mapping is simplified.
fn gsm_default_char(septet: u8) -> char {
    match septet {
        0x00 => '@',
        0x0A => '\n',
        0x0D => '\r',
        0x20..=0x3F | 0x41..=0x5A | 0x61..=0x7A => septet as char,
        _ => '?',
    }
}

/// Unpack `septets` GSM 7-bit packed characters (LSB-first, GSM 03.38 packing)
/// from `packed` into a String. The bit unpacking is exact and unit-tested
/// against the canonical "hello" = `E8 32 9B FD 06` vector.
pub fn gsm7_unpack(packed: &[u8], septets: usize) -> String {
    let mut out = String::with_capacity(septets);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for &byte in packed {
        buffer |= (byte as u32) << bits;
        bits += 8;
        while bits >= 7 && out.len() < septets {
            out.push(gsm_default_char((buffer & 0x7F) as u8));
            buffer >>= 7;
            bits -= 7;
        }
    }
    out
}

/// Decode a semi-octet (BCD) phone number of `digit_count` digits, as used in
/// SMS address fields and BCD number encodings. High nibble of each octet is the
/// second digit; `0xF` padding is dropped.
pub fn decode_semi_octets(octets: &[u8], digit_count: usize) -> String {
    let mut s = String::with_capacity(digit_count);
    for &o in octets {
        let low = o & 0x0F;
        if low != 0x0F {
            s.push((b'0' + low) as char);
        }
        if s.len() >= digit_count {
            break;
        }
        let high = o >> 4;
        if high != 0x0F {
            s.push((b'0' + high) as char);
        }
        if s.len() >= digit_count {
            break;
        }
    }
    s
}

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
    Some(HwSwVersion {
        firmware,
        date,
        model,
    })
}

// --- Phonebook entry + vCard ------------------------------------------------

/// A single phonebook entry, platform-neutral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhonebookEntry {
    pub name: String,
    pub number: String,
    /// Source memory (e.g. "ME", "SIM").
    pub memory: String,
    /// 1-based location.
    pub location: u8,
}

impl PhonebookEntry {
    /// Render as a vCard 3.0 record (the interchange format gnokii/gammu use).
    pub fn to_vcard(&self) -> String {
        format!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:{}\r\nTEL:{}\r\nEND:VCARD\r\n",
            self.name, self.number
        )
    }
}

// --- SMS (3GPP TS 23.040 SMS-DELIVER) ---------------------------------------

/// A decoded incoming text message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sms {
    pub sender: String,
    pub text: String,
}

/// Decode an SMS-DELIVER PDU (the format a phone stores received messages in).
/// Handles the SMSC prefix, sender address (semi-octet + type-of-address), and
/// 7-bit GSM or 8-bit user data. Returns `None` on a malformed/short PDU.
pub fn decode_sms_deliver(pdu: &[u8]) -> Option<Sms> {
    let mut i = 0usize;
    // SMSC: length byte + that many octets.
    let smsc_len = *pdu.get(i)? as usize;
    i += 1 + smsc_len;
    // PDU type (first-octet flags).
    let _first = *pdu.get(i)?;
    i += 1;
    // Sender address: digit count, type-of-address, then ceil(n/2) octets.
    let digits = *pdu.get(i)? as usize;
    i += 1;
    let toa = *pdu.get(i)?;
    i += 1;
    let addr_octets = digits.div_ceil(2);
    let addr = pdu.get(i..i + addr_octets)?;
    i += addr_octets;
    let mut sender = decode_semi_octets(addr, digits);
    if (toa & 0x70) == 0x10 {
        sender.insert(0, '+'); // international type-of-number
    }
    // PID, DCS.
    let _pid = *pdu.get(i)?;
    i += 1;
    let dcs = *pdu.get(i)?;
    i += 1;
    // Service-centre timestamp: 7 octets.
    i += 7;
    // User-data length (septets for 7-bit) + user data.
    let udl = *pdu.get(i)? as usize;
    i += 1;
    let ud = pdu.get(i..)?;
    let text = if dcs & 0x0C == 0x00 {
        gsm7_unpack(ud, udl)
    } else {
        String::from_utf8_lossy(ud).into_owned()
    };
    Some(Sms { sender, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsm7_unpacks_hello() {
        assert_eq!(gsm7_unpack(&[0xE8, 0x32, 0x9B, 0xFD, 0x06], 5), "hello");
    }

    #[test]
    fn semi_octets_decode() {
        assert_eq!(decode_semi_octets(&[0x21, 0x43], 4), "1234");
        assert_eq!(decode_semi_octets(&[0x21, 0xF3], 3), "123");
    }

    #[test]
    fn vcard_export() {
        let e = PhonebookEntry {
            name: "Bob".into(),
            number: "+39123".into(),
            memory: "ME".into(),
            location: 1,
        };
        let v = e.to_vcard();
        assert!(v.contains("FN:Bob"));
        assert!(v.contains("TEL:+39123"));
        assert!(v.starts_with("BEGIN:VCARD"));
    }

    #[test]
    fn decodes_synthetic_sms_deliver() {
        // Controlled SMS-DELIVER: no SMSC, sender +1234 (intl), 7-bit "hello".
        let pdu = [
            0x00, 0x04, 0x04, 0x91, 0x21, 0x43, 0x00, 0x00, 0x22, 0x70, 0x60, 0x21, 0x22, 0x74,
            0x00, 0x05, 0xE8, 0x32, 0x9B, 0xFD, 0x06,
        ];
        let sms = decode_sms_deliver(&pdu).expect("decoded");
        assert_eq!(sms.sender, "+1234");
        assert_eq!(sms.text, "hello");
    }

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
