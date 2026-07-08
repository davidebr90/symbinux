//! BLE scanning through `btleplug` (Windows WinRT / macOS CoreBluetooth).
//!
//! This backend discovers **BLE devices only**: legacy Nokia phones use
//! Bluetooth classic and will not appear here. Classic discovery on
//! Windows/macOS is planned with the per-OS RFCOMM/OBEX work (Phase 4).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;

use crate::classify::{
    first_known_kind, first_known_vendor, kind_from_cod, kind_from_name, kind_from_service_ids,
    vendor_from_company_id, vendor_from_name,
};
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
        let properties = peripheral.properties().await.ok().flatten();
        let name = properties
            .as_ref()
            .and_then(|properties| properties.local_name.clone())
            .unwrap_or_default();

        // Classification signals from the advertisement, strongest first:
        // Class of Device (when the platform reports it), manufacturer
        // company identifier, advertised 16-bit services, then the name.
        let company_vendor = properties
            .as_ref()
            .and_then(|properties| {
                properties
                    .manufacturer_data
                    .keys()
                    .copied()
                    .map(vendor_from_company_id)
                    .find(|vendor| *vendor != crate::Vendor::Unknown)
            })
            .unwrap_or_default();
        let cod_kind = properties
            .as_ref()
            .and_then(|properties| properties.class)
            .map(kind_from_cod)
            .unwrap_or_default();
        let service_kind = properties
            .as_ref()
            .map(|properties| {
                let short_ids = properties
                    .services
                    .iter()
                    .filter_map(short_service_id)
                    .collect::<Vec<_>>();
                kind_from_service_ids(&short_ids)
            })
            .unwrap_or_default();

        devices.push(BluetoothDevice {
            address,
            paired: false,
            vendor: first_known_vendor(&[company_vendor, vendor_from_name(&name)]),
            kind: first_known_kind(&[cod_kind, service_kind, kind_from_name(&name)]),
            name,
        });
    }
    Ok(devices)
}

/// Extract the 16-bit service id from a UUID built on the Bluetooth base
/// UUID (`0000xxxx-0000-1000-8000-00805f9b34fb`).
fn short_service_id(uuid: &uuid::Uuid) -> Option<u16> {
    const BASE_TAIL: u128 = 0x0000_1000_8000_00805f9b34fb;
    let value = uuid.as_u128();
    let head = (value >> 96) as u32;
    let tail = value & 0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF;
    (tail == BASE_TAIL && head <= u16::MAX as u32).then_some(head as u16)
}
