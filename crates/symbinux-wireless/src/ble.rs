//! BLE scanning through `btleplug` (Windows WinRT / macOS CoreBluetooth).
//!
//! This backend discovers **BLE devices only**: legacy Nokia phones use
//! Bluetooth classic and will not appear here. Classic discovery on
//! Windows/macOS is planned with the per-OS RFCOMM/OBEX work (Phase 4).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;

use crate::{BluetoothDevice, WirelessError};

const SCAN_WINDOW: Duration = Duration::from_secs(8);

pub(crate) fn scan_bluetooth(cancel: &AtomicBool) -> Result<Vec<BluetoothDevice>, WirelessError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            WirelessError::Failed(format!("Could not start the Bluetooth runtime: {err}"))
        })?;
    runtime.block_on(scan_inner(cancel))
}

async fn scan_inner(cancel: &AtomicBool) -> Result<Vec<BluetoothDevice>, WirelessError> {
    let manager = Manager::new()
        .await
        .map_err(|err| WirelessError::Unavailable(format!("Bluetooth is not available: {err}")))?;
    let adapters = manager.adapters().await.map_err(|err| {
        WirelessError::Unavailable(format!("Could not list Bluetooth adapters: {err}"))
    })?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or_else(|| WirelessError::Unavailable("No Bluetooth adapter available.".to_string()))?;

    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|err| WirelessError::Failed(format!("Bluetooth scan failed: {err}")))?;

    let steps = SCAN_WINDOW.as_millis() as u64 / 200;
    for _ in 0..steps {
        if cancel.load(Ordering::SeqCst) {
            let _ = adapter.stop_scan().await;
            return Err(WirelessError::Cancelled);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let _ = adapter.stop_scan().await;

    let peripherals = adapter
        .peripherals()
        .await
        .map_err(|err| WirelessError::Failed(format!("Could not list scan results: {err}")))?;

    let mut devices = Vec::new();
    for peripheral in peripherals {
        let address = peripheral.address().to_string();
        // CoreBluetooth hides the MAC address; fall back to the stable
        // peripheral identifier so entries stay distinguishable on macOS.
        let address = if address == "00:00:00:00:00:00" {
            peripheral.id().to_string()
        } else {
            address
        };
        let name = peripheral
            .properties()
            .await
            .ok()
            .flatten()
            .and_then(|properties| properties.local_name)
            .unwrap_or_default();
        devices.push(BluetoothDevice {
            address,
            name,
            paired: false,
        });
    }
    Ok(devices)
}
