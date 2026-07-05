//! The common device-handler interface (strategy pattern).
//!
//! Every platform strategy implements [`DeviceHandler`], exposing the SAME shape
//! to the application layer while advertising a per-platform subset of
//! [`Capability`] via [`DeviceHandler::capabilities`], so the UI can adapt
//! without assuming feature parity across Nokia, Android and iOS.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    NokiaLegacy,
    Android,
    AppleIos,
    Unknown,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::NokiaLegacy => "Nokia (legacy)",
            Platform::Android => "Android",
            Platform::AppleIos => "Apple iOS",
            Platform::Unknown => "Unknown",
        }
    }
}

/// A capability a handler may or may not offer. Deliberately a superset across
/// platforms; each handler returns only what it actually supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Identify,
    Phonebook,
    Sms,
    Calendar,
    Netmonitor,
    FileTransfer,
    AppInstall,
    Backup,
    Screenshot,
    /// Dump of raw interfaces/endpoints for unrecognised devices.
    RawSniff,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::Identify => "identify",
            Capability::Phonebook => "phonebook",
            Capability::Sms => "sms",
            Capability::Calendar => "calendar",
            Capability::Netmonitor => "netmonitor",
            Capability::FileTransfer => "file-transfer",
            Capability::AppInstall => "app-install",
            Capability::Backup => "backup",
            Capability::Screenshot => "screenshot",
            Capability::RawSniff => "raw-sniff",
        }
    }
}

/// A platform-neutral identity summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub platform: Platform,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
    /// Free-form detail (e.g. Android mode, Apple usbmux mode).
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("operation not supported by the {0} handler")]
    NotSupported(&'static str),
    #[error("requires an external service: {0}")]
    RequiresDaemon(String),
    #[error("backend error: {0}")]
    Backend(String),
}

/// The strategy interface implemented by each platform handler.
pub trait DeviceHandler {
    fn platform(&self) -> Platform;
    /// Identity derived from what has been observed so far (no blocking I/O).
    fn identify(&self) -> DeviceIdentity;
    /// The subset of capabilities this device/mode actually offers.
    fn capabilities(&self) -> Vec<Capability>;
    /// Send a platform-specific payload and return the response.
    fn transfer(&mut self, payload: &[u8]) -> Result<Vec<u8>, HandlerError>;
    /// Release any resources / connections held by this handler.
    fn disconnect(&mut self);
}
