//! High-level, named Nokia commands built on top of the FBUS/2 framing.
//!
//! Every command carries a [`Safety`] classification. The transport/CLI layer
//! MUST refuse to send a [`Safety::Dangerous`] command without explicit user
//! confirmation. Payload bytes and confidence levels are documented in
//! `docs/PROTOCOL_NOTES.md`; anything not marked CONFIRMED there should be
//! treated as experimental on real hardware.

use crate::fbus2::Fbus2Frame;

// --- Known message-type bytes (6110 FBUS family) -------------------------------

/// Hardware & software version request. Response arrives as [`MSG_HW_SW_RESP`].
pub const MSG_HW_SW: u8 = 0xD1;
/// Hardware & software version response.
pub const MSG_HW_SW_RESP: u8 = 0xD2;
/// Combined phone info (model / IMEI / HW / SW).
pub const MSG_PHONE_INFO: u8 = 0x64;
/// Security / test channel (IMEI, netmonitor).
pub const MSG_SECURITY: u8 = 0x40;
/// Phonebook read/write.
pub const MSG_PHONEBOOK: u8 = 0x03;
/// SMS send/receive (message content).
pub const MSG_SMS: u8 = 0x02;
/// SMS folder/status operations.
pub const MSG_SMS_FOLDER: u8 = 0x14;

// --- Phonebook memory types ----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemoryType {
    /// Combined ME + SIM.
    Combined = 0x01,
    /// Phone memory (ME).
    Phone = 0x02,
    /// SIM card.
    Sim = 0x03,
    /// Own numbers.
    Own = 0x05,
    /// Dialled calls.
    Dialled = 0x07,
    /// Missed calls.
    Missed = 0x08,
}

/// How much we trust a command against real, out-of-support hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    /// Read-only, corroborated by gnokii/gammu and/or real captures.
    Confirmed,
    /// Modifies data on the phone, or payload boundaries not fully verified.
    Experimental,
    /// Can brick the device (firmware write / flashing). Never sent without
    /// explicit user confirmation and client-side validation.
    Dangerous,
}

/// A named command ready to be framed and sent, with its safety tag.
#[derive(Debug, Clone)]
pub struct Command {
    pub name: &'static str,
    pub safety: Safety,
    pub frame: Fbus2Frame,
}

fn single(name: &'static str, safety: Safety, msg_type: u8, block: &[u8], seq: u8) -> Command {
    Command {
        name,
        safety,
        // frames_to_go = 0x01 (single, last frame)
        frame: Fbus2Frame::command(msg_type, block, 0x01, seq),
    }
}

/// Request hardware & software version (firmware, build date, model code).
pub fn identify_hw_sw(seq: u8) -> Command {
    single("identify:hw_sw", Safety::Confirmed, MSG_HW_SW, &[0x00, 0x03, 0x00], seq)
}

/// Request combined phone info (model / IMEI / HW / SW in one reply).
pub fn identify_phone_info(seq: u8) -> Command {
    single("identify:phone_info", Safety::Confirmed, MSG_PHONE_INFO, &[0x00, 0x10], seq)
}

/// Request the IMEI via the security/test channel.
pub fn get_imei(seq: u8) -> Command {
    single("identify:imei", Safety::Confirmed, MSG_SECURITY, &[0x66], seq)
}

/// Read a phonebook entry at `location` (1-based) from `mem`.
pub fn read_phonebook(mem: MemoryType, location: u8, seq: u8) -> Command {
    single(
        "phonebook:read",
        Safety::Confirmed,
        MSG_PHONEBOOK,
        &[0x00, 0x01, mem as u8, location, 0x00],
        seq,
    )
}

/// Netmonitor: show screen / control. `field` = `0x00` next screen, `0xF0`
/// reset, `0xF1` off, otherwise a specific screen number.
pub fn netmonitor(field: u8, seq: u8) -> Command {
    single("netmonitor", Safety::Confirmed, MSG_SECURITY, &[0x7E, field], seq)
}

/// Write a phonebook entry (modifies the phone — experimental).
pub fn write_phonebook(mem: MemoryType, location: u8, name: &str, number: &str, seq: u8) -> Command {
    let name_b = name.as_bytes();
    let num_b = number.as_bytes();
    let mut block = vec![0x00, 0x04, mem as u8, location, name_b.len() as u8];
    block.extend_from_slice(name_b);
    block.push(num_b.len() as u8);
    block.extend_from_slice(num_b);
    block.push(0x00); // caller group, "no group"
    single("phonebook:write", Safety::Experimental, MSG_PHONEBOOK, &block, seq)
}

/// The `0x55` wake/sync preamble that must precede the first FBUS/2 frame.
///
/// `count` bytes of `0x55` maximise UART edge density for framing lock. Common
/// tutorial value is 128; anything from ~55 upward works. This does not append
/// gammu's optional `0xC1` terminator (target-dependent, LIKELY not CONFIRMED).
pub fn fbus_init_preamble(count: usize) -> Vec<u8> {
    vec![0x55; count]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hw_sw_command_is_confirmed_and_frames_correctly() {
        let cmd = identify_hw_sw(0x00);
        assert_eq!(cmd.safety, Safety::Confirmed);
        assert_eq!(cmd.frame.msg_type, MSG_HW_SW);
        let (block, ftg, _seq) = cmd.frame.block_parts().unwrap();
        assert_eq!(block, &[0x00, 0x03, 0x00]);
        assert_eq!(ftg, 0x01);
    }

    #[test]
    fn phonebook_read_payload() {
        let cmd = read_phonebook(MemoryType::Phone, 5, 0x40);
        let (block, _, _) = cmd.frame.block_parts().unwrap();
        assert_eq!(block, &[0x00, 0x01, 0x02, 0x05, 0x00]);
    }

    #[test]
    fn preamble_is_all_u() {
        let p = fbus_init_preamble(128);
        assert_eq!(p.len(), 128);
        assert!(p.iter().all(|b| *b == 0x55));
    }
}
