//! Serial (termios) transport for DKU-2/CA-42 cables that expose /dev/ttyUSB*.
//!
//! gnokii talks FBUS at 115200 8N1, no flow control, with the cable's control
//! lines set DTR high / RTS low. MBUS runs half-duplex on a single wire, so the
//! transport exposes an explicit echo-drain helper.

use std::io::{Read, Write};
use std::time::Duration;

use crate::{Transport, TransportError};

pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl SerialTransport {
    /// Open a serial port for FBUS (115200 8N1, no flow control, DTR/RTS set).
    pub fn open_fbus(path: &str) -> Result<Self, TransportError> {
        Self::open(path, 115_200)
    }

    pub fn open(path: &str, baud: u32) -> Result<Self, TransportError> {
        let mut port = serialport::new(path, baud)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(500))
            .open()
            .map_err(|e| TransportError::Open {
                path: path.to_string(),
                source: e.into(),
            })?;

        // Cable power/control lines expected by FBUS: DTR asserted, RTS cleared.
        if let Err(e) = port.write_data_terminal_ready(true) {
            log::warn!("{path}: could not set DTR: {e}");
        }
        if let Err(e) = port.write_request_to_send(false) {
            log::warn!("{path}: could not clear RTS: {e}");
        }

        Ok(Self { port })
    }

    /// Read and discard bytes for up to `dur`, used to drain the half-duplex
    /// local echo on MBUS before the real reply arrives.
    pub fn drain_echo(&mut self, dur: Duration) {
        let deadline = std::time::Instant::now() + dur;
        let mut scratch = [0u8; 256];
        self.port.set_timeout(Duration::from_millis(20)).ok();
        while std::time::Instant::now() < deadline {
            match self.port.read(&mut scratch) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        self.port.set_timeout(Duration::from_millis(500)).ok();
    }
}

impl Transport for SerialTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.port.write_all(bytes).map_err(TransportError::Io)?;
        self.port.flush().map_err(TransportError::Io)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(TransportError::Io(e)),
        }
    }
}
