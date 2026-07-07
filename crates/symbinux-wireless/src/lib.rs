//! Cross-platform wireless backends for Symbinux.
//!
//! This crate owns the platform details that used to live in the GUI:
//! Bluetooth and Wi-Fi scanning, PBAP contact transfers and desktop
//! notifications. Callers get one portable, synchronous API with honest
//! [`WirelessError::Unavailable`] errors where a platform has no backend yet.
//!
//! Per-OS coverage today:
//!
//! | Capability | Linux | Windows | macOS |
//! |---|---|---|---|
//! | Bluetooth scan | BlueZ (`bluetoothctl`), classic + LE | BLE only (`btleplug`) | BLE only (`btleplug`) |
//! | Wi-Fi scan | NetworkManager (`nmcli`) | unavailable | unavailable |
//! | PBAP contacts | BlueZ + obexd over D-Bus | unavailable | unavailable |
//! | Notifications | freedesktop | toast | notification centre |
//!
//! Legacy Nokia phones speak Bluetooth *classic*, which `btleplug` (BLE-only)
//! cannot discover; classic discovery and OBEX/PBAP on Windows/macOS arrive
//! with the per-OS RFCOMM work planned in
//! `docs/CROSS_PLATFORM_GUI_PLAN.md` (Phase 4).

use std::sync::atomic::AtomicBool;

#[cfg(any(windows, target_os = "macos"))]
mod ble;
#[cfg(target_os = "linux")]
mod exec;
#[cfg(target_os = "linux")]
mod linux;

/// A device discovered by a Bluetooth scan.
#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub paired: bool,
}

/// A network discovered by a Wi-Fi scan.
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: String,
    pub security: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WirelessError {
    /// The capability has no backend on this platform, or a required host
    /// tool/adapter is missing. The message says exactly what is missing.
    #[error("{0}")]
    Unavailable(String),
    /// The caller cancelled the operation.
    #[error("Operation cancelled.")]
    Cancelled,
    /// The backend ran but the operation failed.
    #[error("{0}")]
    Failed(String),
}

/// Scan for nearby Bluetooth devices.
///
/// Linux discovers classic and LE devices through BlueZ; Windows and macOS
/// discover **BLE devices only** for now (see the module docs).
#[cfg(target_os = "linux")]
pub fn scan_bluetooth(cancel: &AtomicBool) -> Result<Vec<BluetoothDevice>, WirelessError> {
    linux::scan_bluetooth(cancel)
}

#[cfg(any(windows, target_os = "macos"))]
pub fn scan_bluetooth(cancel: &AtomicBool) -> Result<Vec<BluetoothDevice>, WirelessError> {
    ble::scan_bluetooth(cancel)
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub fn scan_bluetooth(_cancel: &AtomicBool) -> Result<Vec<BluetoothDevice>, WirelessError> {
    Err(WirelessError::Unavailable(
        "Bluetooth scan is not supported on this platform yet.".to_string(),
    ))
}

/// Scan for nearby Wi-Fi networks. Linux only (NetworkManager) for now.
#[cfg(target_os = "linux")]
pub fn scan_wifi(cancel: &AtomicBool) -> Result<Vec<WifiNetwork>, WirelessError> {
    linux::scan_wifi(cancel)
}

#[cfg(not(target_os = "linux"))]
pub fn scan_wifi(_cancel: &AtomicBool) -> Result<Vec<WifiNetwork>, WirelessError> {
    Err(WirelessError::Unavailable(
        "Wi-Fi scan is not available on this platform yet.".to_string(),
    ))
}

/// Force pair/connect and pull the phonebook over Bluetooth PBAP, returning
/// the raw vCard text. Linux only (BlueZ + obexd) for now.
#[cfg(target_os = "linux")]
pub fn pull_contacts_pbap(address: &str, cancel: &AtomicBool) -> Result<String, WirelessError> {
    linux::pull_contacts_pbap(address, cancel)
}

#[cfg(not(target_os = "linux"))]
pub fn pull_contacts_pbap(_address: &str, _cancel: &AtomicBool) -> Result<String, WirelessError> {
    Err(WirelessError::Unavailable(
        "Bluetooth contacts require the Linux BlueZ/obexd stack for now.".to_string(),
    ))
}

/// Show a best-effort desktop notification (freedesktop / Windows toast /
/// macOS notification centre). Failures are ignored: a notification is a
/// courtesy, never load-bearing.
pub fn notify(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .appname("Symbinux")
        .summary(summary)
        .body(body)
        .show();
}
