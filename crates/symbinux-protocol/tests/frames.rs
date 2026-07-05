//! Integration tests against binary frame fixtures.
//!
//! FBUS/2 fixtures are documented real captures from a Nokia 3310 (via the
//! insidegadgets F-Bus writeup); their checksums are an external oracle. The
//! MBUS fixture is SYNTHETIC (no public real capture exists), with the checksum
//! computed from the XOR-all rule — it validates the codec, not the hardware.

use symbinux_protocol::message::MSG_HW_SW_RESP;
use symbinux_protocol::{Fbus2Frame, MbusFrame};

const ACK: &[u8] = include_bytes!("fixtures/fbus2_ack.bin");
const HWSW_REQ: &[u8] = include_bytes!("fixtures/fbus2_hwsw_request.bin");
const HWSW_RESP: &[u8] = include_bytes!("fixtures/fbus2_hwsw_response.bin");
const MBUS_REQ: &[u8] = include_bytes!("fixtures/mbus_hwsw_request_synthetic.bin");

#[test]
fn ack_fixture_decodes_and_reencodes() {
    let (frame, used) = Fbus2Frame::decode(ACK).unwrap();
    assert_eq!(used, ACK.len());
    assert!(frame.is_ack());
    assert_eq!(frame.encode(), ACK);
}

#[test]
fn hwsw_request_fixture_roundtrips() {
    let (frame, _) = Fbus2Frame::decode(HWSW_REQ).unwrap();
    assert_eq!(frame.msg_type, 0xD1);
    assert_eq!(frame.encode(), HWSW_REQ);
}

#[test]
fn hwsw_response_fixture_decodes_to_expected_text() {
    let (frame, used) = Fbus2Frame::decode(HWSW_RESP).unwrap();
    assert_eq!(used, HWSW_RESP.len());
    assert_eq!(frame.msg_type, MSG_HW_SW_RESP);
    // The payload contains the firmware/date/model ASCII of a Nokia 3310.
    let text = String::from_utf8_lossy(&frame.data);
    assert!(text.contains("04.45"), "firmware version present: {text:?}");
    assert!(text.contains("NHM-5"), "model code present: {text:?}");
    assert_eq!(frame.encode(), HWSW_RESP);
}

#[test]
fn mbus_synthetic_fixture_roundtrips() {
    let (frame, used) = MbusFrame::decode(MBUS_REQ).unwrap();
    assert_eq!(used, MBUS_REQ.len());
    assert_eq!(frame.msg_type, 0xD1);
    assert_eq!(frame.block, vec![0x00, 0x03, 0x00]);
    assert_eq!(frame.encode(), MBUS_REQ);
}
