//! Symbinux desktop GUI (gtk4-rs).
//!
//! Phase 1 of the cross-platform desktop port (see `docs/CROSS_PLATFORM_GUI_PLAN.md`):
//! a native GTK4 GUI, no libadwaita, that **links the Rust core directly** instead
//! of shelling out to the `symbinux-fbus` CLI. This first slice replicates the
//! core function — detecting connected devices — with the rest of the current
//! GUI's widgets (channel selector, identity card, wireless, i18n, theme) ported
//! in on top. The existing Python GUI stays usable until this reaches parity.

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, Orientation,
    ScrolledWindow,
};

const APP_ID: &str = "it.davidebr90.Symbinux";

/// One-line label for a detected device.
fn device_line(dev: &symbinux_devices::DetectedDevice) -> String {
    let vid = dev.fingerprint.vendor_id;
    let pid = dev.fingerprint.product_id;
    let name = dev
        .product
        .clone()
        .or_else(|| dev.manufacturer.clone())
        .unwrap_or_else(|| "USB device".to_string());
    format!("{vid:04x}:{pid:04x}   {name}   [{:?}]", dev.kind())
}

/// Clear the list and repopulate it straight from the core (no subprocess).
fn refresh_devices(list: &ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let row = |text: &str| {
        let label = Label::new(Some(text));
        label.set_xalign(0.0);
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(10);
        label.set_margin_end(10);
        list.append(&label);
    };

    match symbinux_devices::detect_staged(|_done, _total, _stage| {}) {
        Ok(devices) if !devices.is_empty() => {
            for dev in &devices {
                row(&device_line(dev));
            }
        }
        Ok(_) => row("No devices detected — plug a phone in and press Refresh."),
        Err(err) => row(&format!("Detection error: {err}")),
    }
}

fn build_ui(app: &Application) {
    let header = Label::new(None);
    header.set_markup("<b>SYMBINUX</b>");
    header.set_margin_top(10);

    let refresh = Button::with_label("Refresh");

    let list = ListBox::new();
    let scroller = ScrolledWindow::builder()
        .min_content_height(260)
        .vexpand(true)
        .child(&list)
        .build();

    refresh_devices(&list);

    let list_for_click = list.clone();
    refresh.connect_clicked(move |_| refresh_devices(&list_for_click));

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.set_margin_top(10);
    root.set_margin_bottom(10);
    root.set_margin_start(10);
    root.set_margin_end(10);
    root.append(&header);
    root.append(&refresh);
    root.append(&scroller);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Symbinux")
        .default_width(560)
        .default_height(440)
        .child(&root)
        .build();
    window.present();
}

fn main() -> gtk4::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}
