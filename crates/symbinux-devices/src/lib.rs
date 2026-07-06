//! Automatic USB device detection and per-platform dispatch.
//!
//! A cascade fingerprints each connected device (Nokia legacy / Android / Apple
//! iOS / unknown), and [`handlers::dispatch`] returns a [`handler::DeviceHandler`]
//! strategy exposing a common interface with a per-platform capability set. A
//! [`manager::DeviceManager`] tracks devices by physical port so mode switches
//! (Android AOA, iOS trust) are followed rather than lost.
//!
//! The fingerprinting and dispatch logic is pure and unit-tested; only
//! [`enumerate`] touches the USB bus.

pub mod device;
pub mod enumerate;
pub mod fingerprint;
pub mod handler;
pub mod handlers;
pub mod manager;

pub use device::{DetectedDevice, PortKey};
pub use fingerprint::{classify, AndroidMode, DeviceKind, UsbFingerprint};
pub use handler::{Capability, DeviceHandler, DeviceIdentity, HandlerError, Platform};
pub use handlers::dispatch;
pub use manager::{DeviceManager, Transition};

/// The stages of the detection cascade, in order, for progress reporting.
pub const DETECT_STAGES: &[&str] = &[
    "enumerate", // read the USB bus
    "classify",  // fingerprint each device
];

/// Detect and classify all connected devices, reporting real progress as each
/// stage completes. `progress(done, total, label)` is called with genuine
/// counts — never a synthetic animation.
pub fn detect_staged<F>(mut progress: F) -> Result<Vec<DetectedDevice>, nusb::Error>
where
    F: FnMut(usize, usize, &str),
{
    // total = enumerate + one classify step per device (known only after enumerate).
    progress(0, 1, "enumerate");
    let devices = enumerate::detect()?;
    let total = 1 + devices.len();
    progress(1, total, "enumerate");

    for (i, device) in devices.iter().enumerate() {
        let _ = device.kind(); // classification is the real work of this step
        progress(2 + i, total, "classify");
    }
    Ok(devices)
}
