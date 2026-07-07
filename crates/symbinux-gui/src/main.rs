//! Symbinux desktop GUI (gtk4-rs).
//!
//! Phase 1 of the desktop port keeps the current Python GUI usable while this
//! binary reaches parity widget by widget. The Rust GUI links the core crates
//! directly; no command-line bridge is used for USB detection.

mod i18n;

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, ButtonsType, CssProvider, Dialog,
    FlowBox, HeaderBar, Image, Label, ListBox, ListBoxRow, MessageDialog, MessageType, Orientation,
    Picture, ProgressBar, ResponseType, Revealer, RevealerTransitionType, ScrolledWindow,
    SelectionMode, Spinner, Stack, StackTransitionType, ToggleButton,
};
use i18n::{Translator, LANGUAGES};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use symbinux_transport::Transport as _;

const APP_ID: &str = "it.davidebr90.Symbinux";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const SERIAL_VIDS: &[u16] = &[0x0421, 0x067b, 0x10c4, 0x0403, 0x1a86];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone)]
struct GuiSettings {
    theme: ThemeMode,
    language: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    Usb,
    Bluetooth,
    Wifi,
}

impl Channel {
    fn empty_title(self) -> &'static str {
        match self {
            Channel::Usb => "No phone or cable detected",
            Channel::Bluetooth => "Bluetooth scan pending in the Rust GUI",
            Channel::Wifi => "Wi-Fi scan pending in the Rust GUI",
        }
    }

    fn empty_description(self) -> &'static str {
        match self {
            Channel::Usb => {
                "Connect a Nokia phone with a DKU-2/CA-42 cable, then press Refresh."
            }
            Channel::Bluetooth => {
                "The Python GUI remains available for Bluetooth contacts until this channel is wired here."
            }
            Channel::Wifi => {
                "The Python GUI remains available for Wi-Fi scanning until this channel is wired here."
            }
        }
    }
}

struct ChannelSpec {
    channel: Channel,
    label: &'static str,
    icon: &'static str,
}

const CHANNELS: &[ChannelSpec] = &[
    ChannelSpec {
        channel: Channel::Usb,
        label: "USB",
        icon: "drive-harddisk-usb-symbolic",
    },
    ChannelSpec {
        channel: Channel::Bluetooth,
        label: "Bluetooth",
        icon: "bluetooth-symbolic",
    },
    ChannelSpec {
        channel: Channel::Wifi,
        label: "Wi-Fi",
        icon: "network-wireless-symbolic",
    },
];

struct FunctionSpec {
    action: FunctionAction,
    label: &'static str,
    icon: &'static str,
    tooltip: &'static str,
    capability: &'static str,
}

const FUNCTIONS: &[FunctionSpec] = &[
    FunctionSpec {
        action: FunctionAction::Identify,
        label: "Identify",
        icon: "dialog-information-symbolic",
        tooltip: "Read model, IMEI and firmware",
        capability: "identify",
    },
    FunctionSpec {
        action: FunctionAction::Pending,
        label: "Phonebook",
        icon: "contact-new-symbolic",
        tooltip: "Import / export contacts",
        capability: "phonebook",
    },
    FunctionSpec {
        action: FunctionAction::Pending,
        label: "SMS",
        icon: "mail-message-new-symbolic",
        tooltip: "Read and send messages",
        capability: "sms",
    },
    FunctionSpec {
        action: FunctionAction::Pending,
        label: "Netmonitor",
        icon: "network-cellular-signal-excellent-symbolic",
        tooltip: "Network diagnostics",
        capability: "netmonitor",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionAction {
    Identify,
    Pending,
}

#[derive(Debug, Clone)]
struct UiDevice {
    title: String,
    subtitle: String,
    capabilities: Vec<String>,
}

#[derive(Debug)]
enum DetectMessage {
    Progress {
        done: usize,
        total: usize,
        stage: String,
    },
    Finished(Result<Vec<UiDevice>, String>),
}

#[derive(Debug)]
enum IdentifyMessage {
    Finished(Result<PhoneIdentity, String>),
}

#[derive(Debug)]
enum WirelessMessage {
    Bluetooth(Result<Vec<BluetoothDevice>, String>),
    Wifi(Result<Vec<WifiNetwork>, String>),
}

#[derive(Debug, Clone)]
struct PhoneIdentity {
    port: String,
    model: String,
    firmware: String,
    date: String,
}

#[derive(Debug, Clone)]
struct BluetoothDevice {
    address: String,
    name: String,
    paired: bool,
}

#[derive(Debug, Clone)]
struct WifiNetwork {
    ssid: String,
    signal: String,
    security: String,
}

type CancelCallback = Rc<RefCell<Option<Box<dyn Fn()>>>>;

#[derive(Clone)]
struct ProgressPanel {
    root: Revealer,
    spinner: Spinner,
    label: Label,
    bar: ProgressBar,
    cancel_button: Button,
    on_cancel: CancelCallback,
}

impl ProgressPanel {
    fn new(cancel_label: &str) -> Self {
        let root = Revealer::builder()
            .transition_type(RevealerTransitionType::SlideDown)
            .reveal_child(false)
            .build();

        let box_ = GtkBox::new(Orientation::Horizontal, 12);
        box_.add_css_class("symbinux-progress");
        box_.set_margin_top(6);
        box_.set_margin_bottom(6);
        box_.set_margin_start(12);
        box_.set_margin_end(12);

        let spinner = Spinner::new();
        spinner.set_size_request(18, 18);

        let label = Label::new(None);
        label.set_xalign(0.0);

        let bar = ProgressBar::new();
        bar.set_hexpand(true);
        bar.set_show_text(true);
        bar.set_valign(Align::Center);

        let cancel_button = Button::with_label(cancel_label);
        cancel_button.add_css_class("flat");
        cancel_button.set_valign(Align::Center);

        box_.append(&spinner);
        box_.append(&label);
        box_.append(&bar);
        box_.append(&cancel_button);
        root.set_child(Some(&box_));

        let on_cancel: CancelCallback = Rc::new(RefCell::new(None));
        let panel = Self {
            root,
            spinner,
            label,
            bar,
            cancel_button,
            on_cancel,
        };

        let panel_for_cancel = panel.clone();
        panel.cancel_button.connect_clicked(move |_| {
            let callback = panel_for_cancel.on_cancel.borrow_mut().take();
            panel_for_cancel.finish();
            if let Some(callback) = callback {
                callback();
            }
        });

        panel.finish();
        panel
    }

    fn widget(&self) -> &Revealer {
        &self.root
    }

    fn determinate<F>(&self, text: &str, on_cancel: F)
    where
        F: Fn() + 'static,
    {
        self.spinner.stop();
        self.spinner.set_visible(false);
        self.bar.set_visible(true);
        self.cancel_button.set_visible(true);
        self.bar.set_fraction(0.0);
        self.bar.set_text(Some("0%"));
        self.label.set_text(text);
        *self.on_cancel.borrow_mut() = Some(Box::new(on_cancel));
        self.root.set_reveal_child(true);
    }

    fn indeterminate<F>(&self, text: &str, on_cancel: F)
    where
        F: Fn() + 'static,
    {
        self.spinner.set_visible(true);
        self.spinner.start();
        self.bar.set_visible(false);
        self.cancel_button.set_visible(true);
        self.label.set_text(text);
        *self.on_cancel.borrow_mut() = Some(Box::new(on_cancel));
        self.root.set_reveal_child(true);
    }

    fn set_progress(&self, done: usize, total: usize, stage: &str) {
        let fraction = if total == 0 {
            1.0
        } else {
            (done as f64 / total as f64).clamp(0.0, 1.0)
        };
        self.bar.set_fraction(fraction);
        self.bar
            .set_text(Some(&format!("{}%", (fraction * 100.0).round())));
        self.label
            .set_text(&format!("Detecting devices... {stage}"));
    }

    fn finish(&self) {
        self.spinner.stop();
        self.spinner.set_visible(false);
        self.bar.set_visible(false);
        self.cancel_button.set_visible(false);
        self.root.set_reveal_child(false);
        *self.on_cancel.borrow_mut() = None;
    }
}

#[derive(Clone)]
struct FunctionButton {
    capability: &'static str,
    button: Button,
}

struct AppUi {
    window: ApplicationWindow,
    i18n: Rc<Translator>,
    progress: ProgressPanel,
    stack: Stack,
    empty_title: Label,
    empty_description: Label,
    device_list: ListBox,
    devices: RefCell<Vec<UiDevice>>,
    selected_capabilities: RefCell<Vec<String>>,
    function_buttons: Vec<FunctionButton>,
    current_cancel: RefCell<Option<Arc<AtomicBool>>>,
    channel: RefCell<Channel>,
}

impl AppUi {
    fn new(app: &Application, i18n: Rc<Translator>) -> Rc<Self> {
        install_css();

        let (header, refresh_button) = build_header(&i18n);
        let channel_bar = GtkBox::new(Orientation::Horizontal, 6);
        channel_bar.set_halign(Align::Center);
        channel_bar.set_margin_top(10);
        channel_bar.set_margin_bottom(10);

        let progress = ProgressPanel::new(&i18n.tr("Cancel"));

        let stack = Stack::builder()
            .vexpand(true)
            .transition_type(StackTransitionType::Crossfade)
            .build();

        let (empty, empty_title, empty_description) = build_empty_state();
        stack.add_named(&empty, Some("empty"));

        let device_list = ListBox::new();
        device_list.set_selection_mode(SelectionMode::Single);
        device_list.add_css_class("boxed-list");
        device_list.set_valign(Align::Start);

        let scroller = ScrolledWindow::builder()
            .vexpand(true)
            .child(&device_list)
            .build();
        scroller.set_margin_top(18);
        scroller.set_margin_bottom(18);
        scroller.set_margin_start(18);
        scroller.set_margin_end(18);
        stack.add_named(&scroller, Some("list"));

        let actions = FlowBox::new();
        actions.set_selection_mode(SelectionMode::None);
        actions.set_halign(Align::Center);
        actions.set_column_spacing(8);
        actions.set_row_spacing(8);
        actions.set_margin_top(12);
        actions.set_margin_bottom(12);
        actions.set_margin_start(12);
        actions.set_margin_end(12);

        let mut function_buttons = Vec::new();
        for function in FUNCTIONS {
            let button = function_button(function, &i18n);
            button.set_sensitive(false);
            actions.insert(&button, -1);
            function_buttons.push(FunctionButton {
                capability: function.capability,
                button,
            });
        }

        let content = GtkBox::new(Orientation::Vertical, 0);
        content.append(&channel_bar);
        content.append(progress.widget());
        content.append(&stack);
        content.append(&actions);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&header);
        root.append(&content);

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Symbinux")
            .default_width(860)
            .default_height(680)
            .child(&root)
            .build();
        window.set_size_request(720, 600);

        let ui = Rc::new(Self {
            window,
            i18n,
            progress,
            stack,
            empty_title,
            empty_description,
            device_list,
            devices: RefCell::new(Vec::new()),
            selected_capabilities: RefCell::new(Vec::new()),
            function_buttons,
            current_cancel: RefCell::new(None),
            channel: RefCell::new(Channel::Usb),
        });

        build_channel_buttons(&channel_bar, &ui);
        wire_header(&refresh_button, &ui);
        wire_device_selection(&ui);
        wire_function_buttons(&ui);

        ui.show_channel_empty(Channel::Usb);
        ui.refresh();
        ui
    }

    fn tr(&self, message: &str) -> String {
        self.i18n.tr(message)
    }

    fn present(&self) {
        self.window.present();
    }

    fn refresh(self: &Rc<Self>) {
        self.cancel_current();
        self.clear_device_list();
        self.selected_capabilities.borrow_mut().clear();
        self.update_function_sensitivity();

        match *self.channel.borrow() {
            Channel::Usb => self.refresh_usb(),
            Channel::Bluetooth => self.refresh_bluetooth(),
            Channel::Wifi => self.refresh_wifi(),
        }
    }

    fn refresh_usb(self: &Rc<Self>) {
        let cancel = Arc::new(AtomicBool::new(false));
        *self.current_cancel.borrow_mut() = Some(cancel.clone());

        let cancel_for_button = cancel.clone();
        self.progress
            .determinate(&self.tr("Detecting devices…"), move || {
                cancel_for_button.store(true, Ordering::SeqCst)
            });

        let (sender, receiver) = mpsc::channel();
        let progress_sender = sender.clone();
        let cancel_for_thread = cancel.clone();

        thread::spawn(move || {
            let detected = symbinux_devices::detect_staged(|done, total, stage| {
                if !cancel_for_thread.load(Ordering::SeqCst) {
                    let _ = progress_sender.send(DetectMessage::Progress {
                        done,
                        total,
                        stage: stage.to_string(),
                    });
                }
            })
            .map(|devices| {
                devices
                    .into_iter()
                    .filter(|device| device.kind() != symbinux_devices::DeviceKind::Unknown)
                    .map(ui_device)
                    .collect()
            })
            .map_err(|err| err.to_string());

            let _ = sender.send(DetectMessage::Finished(detected));
        });

        let ui = Rc::clone(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if cancel.load(Ordering::SeqCst) {
                return glib::ControlFlow::Break;
            }

            loop {
                match receiver.try_recv() {
                    Ok(DetectMessage::Progress { done, total, stage }) => {
                        ui.progress.set_progress(done, total, &stage);
                    }
                    Ok(DetectMessage::Finished(result)) => {
                        ui.progress.finish();
                        *ui.current_cancel.borrow_mut() = None;
                        match result {
                            Ok(devices) => ui.populate_devices(devices),
                            Err(err) => {
                                let title = ui.tr("Core not available");
                                ui.show_empty(&title, &err);
                            }
                        }
                        return glib::ControlFlow::Break;
                    }
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        ui.progress.finish();
                        *ui.current_cancel.borrow_mut() = None;
                        return glib::ControlFlow::Break;
                    }
                }
            }
        });
    }

    fn run_identify(self: &Rc<Self>) {
        self.cancel_current();

        let cancel = Arc::new(AtomicBool::new(false));
        *self.current_cancel.borrow_mut() = Some(cancel.clone());

        let cancel_for_button = cancel.clone();
        self.progress
            .indeterminate(&self.tr("Identifying…"), move || {
                cancel_for_button.store(true, Ordering::SeqCst)
            });

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(IdentifyMessage::Finished(identify_phone()));
        });

        let ui = Rc::clone(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if cancel.load(Ordering::SeqCst) {
                *ui.current_cancel.borrow_mut() = None;
                return glib::ControlFlow::Break;
            }

            match receiver.try_recv() {
                Ok(IdentifyMessage::Finished(result)) => {
                    ui.progress.finish();
                    *ui.current_cancel.borrow_mut() = None;
                    match result {
                        Ok(identity) => show_identity_card(&ui.window, &ui.i18n, &identity),
                        Err(err) => show_dialog(&ui.window, &ui.tr("Identify"), &err),
                    }
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    ui.progress.finish();
                    *ui.current_cancel.borrow_mut() = None;
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn refresh_bluetooth(self: &Rc<Self>) {
        self.refresh_wireless(
            &self.tr("Scanning Bluetooth…"),
            |cancel| WirelessMessage::Bluetooth(scan_bluetooth(cancel)),
            |ui, result| match result {
                WirelessMessage::Bluetooth(Ok(devices)) => ui.populate_bluetooth(devices),
                WirelessMessage::Bluetooth(Err(err)) => {
                    let title = ui.tr(Channel::Bluetooth.empty_title());
                    ui.show_empty(&title, &err);
                }
                WirelessMessage::Wifi(_) => {}
            },
        );
    }

    fn refresh_wifi(self: &Rc<Self>) {
        self.refresh_wireless(
            &self.tr("Scanning Wi-Fi…"),
            |cancel| WirelessMessage::Wifi(scan_wifi(cancel)),
            |ui, result| match result {
                WirelessMessage::Wifi(Ok(networks)) => ui.populate_wifi(networks),
                WirelessMessage::Wifi(Err(err)) => {
                    let title = ui.tr(Channel::Wifi.empty_title());
                    ui.show_empty(&title, &err);
                }
                WirelessMessage::Bluetooth(_) => {}
            },
        );
    }

    fn refresh_wireless<F, G>(self: &Rc<Self>, label: &str, work: F, done: G)
    where
        F: FnOnce(Arc<AtomicBool>) -> WirelessMessage + Send + 'static,
        G: Fn(&Rc<Self>, WirelessMessage) + 'static,
    {
        let cancel = Arc::new(AtomicBool::new(false));
        *self.current_cancel.borrow_mut() = Some(cancel.clone());

        let cancel_for_button = cancel.clone();
        self.progress.indeterminate(label, move || {
            cancel_for_button.store(true, Ordering::SeqCst)
        });

        let (sender, receiver) = mpsc::channel();
        let cancel_for_thread = cancel.clone();
        thread::spawn(move || {
            let _ = sender.send(work(cancel_for_thread));
        });

        let ui = Rc::clone(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if cancel.load(Ordering::SeqCst) {
                *ui.current_cancel.borrow_mut() = None;
                return glib::ControlFlow::Break;
            }

            match receiver.try_recv() {
                Ok(result) => {
                    ui.progress.finish();
                    *ui.current_cancel.borrow_mut() = None;
                    done(&ui, result);
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    ui.progress.finish();
                    *ui.current_cancel.borrow_mut() = None;
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn populate_devices(&self, devices: Vec<UiDevice>) {
        if devices.is_empty() {
            self.show_channel_empty(Channel::Usb);
            return;
        }

        *self.devices.borrow_mut() = devices;
        for device in self.devices.borrow().iter() {
            self.device_list.append(&device_row(device));
        }
        self.stack.set_visible_child_name("list");
    }

    fn populate_bluetooth(self: &Rc<Self>, devices: Vec<BluetoothDevice>) {
        if devices.is_empty() {
            self.show_empty(
                &self.tr("No Bluetooth devices found"),
                &self.tr("Make sure Bluetooth is on and nearby devices are discoverable."),
            );
            return;
        }

        self.devices.borrow_mut().clear();
        for device in devices {
            self.device_list.append(&bluetooth_row(self, &device));
        }
        self.stack.set_visible_child_name("list");
    }

    fn populate_wifi(&self, networks: Vec<WifiNetwork>) {
        if networks.is_empty() {
            self.show_empty(
                &self.tr("No Wi-Fi networks found"),
                &self.tr("No wireless networks are in range."),
            );
            return;
        }

        self.devices.borrow_mut().clear();
        for network in networks {
            self.device_list.append(&wifi_row(&network));
        }
        self.stack.set_visible_child_name("list");
    }

    fn clear_device_list(&self) {
        while let Some(child) = self.device_list.first_child() {
            self.device_list.remove(&child);
        }
        self.devices.borrow_mut().clear();
    }

    fn show_empty(&self, title: &str, description: &str) {
        self.empty_title.set_text(title);
        self.empty_description.set_text(description);
        self.stack.set_visible_child_name("empty");
    }

    fn show_channel_empty(&self, channel: Channel) {
        self.show_empty(
            &self.tr(channel.empty_title()),
            &self.tr(channel.empty_description()),
        );
    }

    fn update_function_sensitivity(&self) {
        let selected = self.selected_capabilities.borrow();
        for function in &self.function_buttons {
            let enabled = selected
                .iter()
                .any(|capability| capability == function.capability);
            function.button.set_sensitive(enabled);
        }
    }

    fn cancel_current(&self) {
        if let Some(cancel) = self.current_cancel.borrow_mut().take() {
            cancel.store(true, Ordering::SeqCst);
        }
        self.progress.finish();
    }

    fn show_pending(&self, label: &str) {
        let title = self.tr(label);
        let body = self.tr("This function is not wired up yet on this channel.");
        show_dialog(&self.window, &title, &body);
        send_notification(&self.window, &title, &body);
    }

    fn show_contacts_pending(&self) {
        let title = self.tr("Contacts");
        let body = self.tr(
            "Bluetooth PBAP contacts are pending in the Rust GUI. No contact transfer has been started.",
        );
        show_dialog(&self.window, &title, &body);
        send_notification(&self.window, &title, &body);
    }
}

fn build_header(i18n: &Translator) -> (HeaderBar, Button) {
    let header = HeaderBar::new();

    let wordmark = Label::new(Some("SYMBINUX"));
    wordmark.add_css_class("symbinux-wordmark");
    wordmark.set_margin_start(6);
    header.pack_start(&wordmark);

    let version = Label::new(Some(&format!("v{APP_VERSION}")));
    version.add_css_class("dim-label");
    version.add_css_class("caption");
    header.set_title_widget(Some(&version));

    let refresh = Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(i18n.tr("Rescan the selected channel"))
        .build();
    header.pack_end(&refresh);

    let menu = gio::Menu::new();
    let appearance = gio::Menu::new();
    appearance.append(Some(&i18n.tr("Automatic")), Some("app.theme::auto"));
    appearance.append(Some(&i18n.tr("Light")), Some("app.theme::light"));
    appearance.append(Some(&i18n.tr("Dark")), Some("app.theme::dark"));
    menu.append_submenu(Some(&i18n.tr("Appearance")), &appearance);

    let language = gio::Menu::new();
    for (code, native_label) in LANGUAGES {
        let label = if *code == "auto" {
            i18n.tr("Automatic")
        } else {
            native_label.to_string()
        };
        language.append(Some(&label), Some(&format!("app.language::{code}")));
    }
    menu.append_submenu(Some(&i18n.tr("Language")), &language);

    let menu_button = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .build();
    header.pack_end(&menu_button);

    (header, refresh)
}

fn wire_header(refresh: &Button, ui: &Rc<AppUi>) {
    let ui_for_refresh = Rc::clone(ui);
    refresh.connect_clicked(move |_| ui_for_refresh.refresh());
}

fn build_channel_buttons(container: &GtkBox, ui: &Rc<AppUi>) {
    let mut first: Option<ToggleButton> = None;

    for spec in CHANNELS {
        let button = ToggleButton::new();
        button.add_css_class("flat");
        button.set_tooltip_text(Some(&ui.tr(spec.label)));

        let inner = GtkBox::new(Orientation::Horizontal, 8);
        inner.set_margin_start(4);
        inner.set_margin_end(4);
        inner.append(&Image::from_icon_name(spec.icon));
        inner.append(&Label::new(Some(&ui.tr(spec.label))));
        button.set_child(Some(&inner));

        if let Some(first_button) = &first {
            button.set_group(Some(first_button));
        } else {
            first = Some(button.clone());
        }

        if spec.channel == Channel::Usb {
            button.set_active(true);
        }

        let ui_for_toggle = Rc::clone(ui);
        let channel = spec.channel;
        button.connect_toggled(move |button| {
            if button.is_active() {
                *ui_for_toggle.channel.borrow_mut() = channel;
                ui_for_toggle.refresh();
            }
        });

        container.append(&button);
    }
}

fn wire_device_selection(ui: &Rc<AppUi>) {
    let ui_for_selection = Rc::clone(ui);
    ui.device_list.connect_row_selected(move |_, row| {
        let mut selected = ui_for_selection.selected_capabilities.borrow_mut();
        selected.clear();

        if let Some(row) = row {
            let index = row.index();
            if index >= 0 {
                if let Some(device) = ui_for_selection.devices.borrow().get(index as usize) {
                    selected.extend(device.capabilities.iter().cloned());
                }
            }
        }
        drop(selected);
        ui_for_selection.update_function_sensitivity();
    });
}

fn wire_function_buttons(ui: &Rc<AppUi>) {
    for (spec, function_button) in FUNCTIONS.iter().zip(ui.function_buttons.iter()) {
        let ui_for_click = Rc::clone(ui);
        let label = spec.label;
        let action = spec.action;
        function_button
            .button
            .connect_clicked(move |_| match action {
                FunctionAction::Identify => ui_for_click.run_identify(),
                FunctionAction::Pending => ui_for_click.show_pending(label),
            });
    }
}

fn build_empty_state() -> (GtkBox, Label, Label) {
    let box_ = GtkBox::new(Orientation::Vertical, 20);
    box_.set_valign(Align::Center);
    box_.set_vexpand(true);
    box_.set_margin_top(24);
    box_.set_margin_bottom(24);
    box_.set_margin_start(24);
    box_.set_margin_end(24);

    if let Some(path) = logo_path(prefer_dark()) {
        let logo = Picture::for_filename(path);
        logo.set_can_shrink(true);
        logo.set_size_request(360, 120);
        logo.set_halign(Align::Center);
        box_.append(&logo);
    } else {
        let fallback = Label::new(Some("SYMBINUX"));
        fallback.add_css_class("symbinux-empty-logo");
        box_.append(&fallback);
    }

    let title = Label::new(None);
    title.add_css_class("title-1");
    title.set_wrap(true);
    title.set_justify(gtk4::Justification::Center);
    box_.append(&title);

    let description = Label::new(None);
    description.add_css_class("dim-label");
    description.set_wrap(true);
    description.set_justify(gtk4::Justification::Center);
    description.set_max_width_chars(58);
    box_.append(&description);

    (box_, title, description)
}

fn function_button(function: &FunctionSpec, i18n: &Translator) -> Button {
    let inner = GtkBox::new(Orientation::Horizontal, 8);
    inner.set_margin_top(6);
    inner.set_margin_bottom(6);
    inner.set_margin_start(8);
    inner.set_margin_end(8);
    inner.append(&Image::from_icon_name(function.icon));
    inner.append(&Label::new(Some(&i18n.tr(function.label))));

    let button = Button::new();
    button.set_tooltip_text(Some(&i18n.tr(function.tooltip)));
    button.set_child(Some(&inner));
    button
}

fn device_row(device: &UiDevice) -> ListBoxRow {
    let row = ListBoxRow::new();
    let outer = GtkBox::new(Orientation::Horizontal, 12);
    outer.set_margin_top(10);
    outer.set_margin_bottom(10);
    outer.set_margin_start(12);
    outer.set_margin_end(12);

    let icon = Image::from_icon_name("phone-symbolic");
    icon.set_valign(Align::Center);
    outer.append(&icon);

    let text = GtkBox::new(Orientation::Vertical, 4);
    text.set_hexpand(true);

    let title = Label::new(Some(&device.title));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let subtitle = Label::new(Some(&device.subtitle));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");

    text.append(&title);
    text.append(&subtitle);
    outer.append(&text);
    row.set_child(Some(&outer));
    row
}

fn bluetooth_row(ui: &Rc<AppUi>, device: &BluetoothDevice) -> ListBoxRow {
    let row = ListBoxRow::new();
    let outer = GtkBox::new(Orientation::Horizontal, 12);
    outer.set_margin_top(10);
    outer.set_margin_bottom(10);
    outer.set_margin_start(12);
    outer.set_margin_end(12);

    let icon = Image::from_icon_name("bluetooth-symbolic");
    icon.set_valign(Align::Center);
    outer.append(&icon);

    let text = GtkBox::new(Orientation::Vertical, 4);
    text.set_hexpand(true);

    let title_text = if device.name.is_empty() {
        device.address.as_str()
    } else {
        device.name.as_str()
    };
    let title = Label::new(Some(title_text));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let mut subtitle = device.address.clone();
    if device.paired {
        subtitle.push_str(&format!(" - {}", ui.tr("paired")));
    }
    let subtitle = Label::new(Some(&subtitle));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("dim-label");

    text.append(&title);
    text.append(&subtitle);
    outer.append(&text);

    let contacts = Button::with_label(&ui.tr("Contacts"));
    contacts.add_css_class("flat");
    contacts.set_valign(Align::Center);
    let ui_for_contacts = Rc::clone(ui);
    contacts.connect_clicked(move |_| ui_for_contacts.show_contacts_pending());
    outer.append(&contacts);

    row.set_child(Some(&outer));
    row
}

fn wifi_row(network: &WifiNetwork) -> ListBoxRow {
    let row = ListBoxRow::new();
    let outer = GtkBox::new(Orientation::Horizontal, 12);
    outer.set_margin_top(10);
    outer.set_margin_bottom(10);
    outer.set_margin_start(12);
    outer.set_margin_end(12);

    let icon = Image::from_icon_name("network-wireless-symbolic");
    icon.set_valign(Align::Center);
    outer.append(&icon);

    let text = GtkBox::new(Orientation::Vertical, 4);
    text.set_hexpand(true);

    let title = Label::new(Some(&network.ssid));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let subtitle = Label::new(Some(&format!("{}% - {}", network.signal, network.security)));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("dim-label");

    text.append(&title);
    text.append(&subtitle);
    outer.append(&text);

    row.set_child(Some(&outer));
    row
}

fn ui_device(device: symbinux_devices::DetectedDevice) -> UiDevice {
    let handler = symbinux_devices::dispatch(device);
    let identity = handler.identify();
    let capabilities = handler
        .capabilities()
        .iter()
        .map(|capability| capability.as_str().to_string())
        .collect::<Vec<_>>();
    let vid_pid = format!("{:04x}:{:04x}", identity.vendor_id, identity.product_id);
    let title = identity
        .model
        .clone()
        .filter(|model| model != "?")
        .unwrap_or_else(|| identity.platform.as_str().to_string());
    let caps = if capabilities.is_empty() {
        "no capabilities".to_string()
    } else {
        capabilities.join(", ")
    };
    UiDevice {
        title,
        subtitle: format!("{} - {vid_pid} - {caps}", identity.platform.as_str()),
        capabilities,
    }
}

fn scan_bluetooth(cancel: Arc<AtomicBool>) -> Result<Vec<BluetoothDevice>, String> {
    require_command(
        "bluetoothctl",
        "Bluetooth scan requires bluetoothctl (BlueZ).",
    )?;

    let show = run_command("bluetoothctl", &["show"], Duration::from_secs(6), &cancel)?;
    if show.contains("No default controller") {
        return Err("No Bluetooth adapter available.".to_string());
    }

    let _ = run_command(
        "bluetoothctl",
        &["--timeout", "8", "scan", "on"],
        Duration::from_secs(13),
        &cancel,
    )?;
    let devices_out = run_command(
        "bluetoothctl",
        &["devices"],
        Duration::from_secs(6),
        &cancel,
    )?;
    let paired_out = run_command(
        "bluetoothctl",
        &["paired-devices"],
        Duration::from_secs(6),
        &cancel,
    )?;

    let paired = paired_out
        .lines()
        .filter_map(|line| line.strip_prefix("Device "))
        .filter_map(|tail| tail.split_whitespace().next())
        .map(str::to_string)
        .collect::<Vec<_>>();

    let mut devices = Vec::new();
    for line in devices_out.lines() {
        let Some(tail) = line.strip_prefix("Device ") else {
            continue;
        };
        let mut parts = tail.splitn(2, char::is_whitespace);
        let Some(address) = parts.next() else {
            continue;
        };
        let name = parts.next().unwrap_or("").trim().to_string();
        devices.push(BluetoothDevice {
            address: address.to_string(),
            name,
            paired: paired.iter().any(|item| item == address),
        });
    }
    Ok(devices)
}

fn scan_wifi(cancel: Arc<AtomicBool>) -> Result<Vec<WifiNetwork>, String> {
    require_command("nmcli", "Wi-Fi scan requires nmcli (NetworkManager).")?;
    let output = run_command(
        "nmcli",
        &[
            "-t",
            "-f",
            "SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "--rescan",
            "yes",
        ],
        Duration::from_secs(20),
        &cancel,
    )?;

    let mut networks = Vec::new();
    let mut seen = Vec::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let fields = split_nmcli_fields(line);
        if fields.len() < 3 {
            continue;
        }
        let ssid = if fields[0].is_empty() {
            "(hidden)".to_string()
        } else {
            fields[0].clone()
        };
        if seen.iter().any(|item| item == &ssid) {
            continue;
        }
        seen.push(ssid.clone());
        networks.push(WifiNetwork {
            ssid,
            signal: fields[1].clone(),
            security: if fields[2].is_empty() {
                "open".to_string()
            } else {
                fields[2].clone()
            },
        });
    }
    Ok(networks)
}

fn require_command(program: &str, message: &str) -> Result<(), String> {
    if command_exists(program) {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn command_exists(program: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths)
        .any(|dir| dir.join(program).is_file() || dir.join(format!("{program}.exe")).is_file())
}

fn run_command(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Could not start {program}: {err}"))?;
    let deadline = Instant::now() + timeout;

    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            return Err("Operation cancelled.".to_string());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(format!("{program} timed out."));
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|err| format!("Could not read {program} output: {err}"))?;
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
                }

                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let detail = if stderr.trim().is_empty() {
                    stdout.trim()
                } else {
                    stderr.trim()
                };
                return Err(if detail.is_empty() {
                    format!("{program} failed.")
                } else {
                    format!("{program} failed: {detail}")
                });
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(err) => {
                let _ = child.kill();
                return Err(format!("Could not wait for {program}: {err}"));
            }
        }
    }
}

fn split_nmcli_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            field.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == ':' {
            fields.push(field);
            field = String::new();
        } else {
            field.push(ch);
        }
    }

    fields.push(field);
    fields
}

fn show_dialog(parent: &ApplicationWindow, title: &str, body: &str) {
    let dialog = MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .message_type(MessageType::Info)
        .buttons(ButtonsType::Ok)
        .text(title)
        .secondary_text(body)
        .build();
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.present();
}

fn send_notification(parent: &ApplicationWindow, title: &str, body: &str) {
    let Some(app) = parent.application() else {
        return;
    };

    let notification = gio::Notification::new(title);
    notification.set_body(Some(body));
    notification.set_priority(gio::NotificationPriority::Normal);
    app.send_notification(None, &notification);
}

fn show_identity_card(parent: &ApplicationWindow, i18n: &Translator, identity: &PhoneIdentity) {
    let dialog = Dialog::builder()
        .transient_for(parent)
        .modal(true)
        .title(i18n.tr("Phone identity"))
        .build();
    dialog.add_button(&i18n.tr("Close"), ResponseType::Close);

    let card = GtkBox::new(Orientation::Vertical, 8);
    card.add_css_class("symbinux-card");
    card.set_margin_top(16);
    card.set_margin_bottom(16);
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.append(&property_row(&i18n.tr("Model"), &identity.model));
    card.append(&property_row(&i18n.tr("Firmware"), &identity.firmware));
    card.append(&property_row(&i18n.tr("Date"), &identity.date));
    card.append(&property_row("Port", &identity.port));

    dialog.content_area().append(&card);
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.present();
}

fn property_row(name: &str, value: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.set_hexpand(true);

    let name_label = Label::new(Some(name));
    name_label.add_css_class("dim-label");
    name_label.set_xalign(0.0);
    name_label.set_width_chars(10);

    let value_label = Label::new(Some(value));
    value_label.set_xalign(0.0);
    value_label.set_selectable(true);
    value_label.set_hexpand(true);

    row.append(&name_label);
    row.append(&value_label);
    row
}

fn identify_phone() -> Result<PhoneIdentity, String> {
    let port = symbinux_transport::resolve_serial_port(SERIAL_VIDS).ok_or_else(|| {
        "No serial port found. Connect a DKU-2/CA-42 cable and try again.".to_string()
    })?;

    let mut link = symbinux_transport::SerialTransport::open_fbus(&port)
        .map_err(|err| format!("Opening serial port {port} failed: {err}"))?;
    link.write_all(&symbinux_protocol::message::fbus_init_preamble(128))
        .map_err(|err| format!("Sending FBUS init preamble failed: {err}"))?;

    let command = symbinux_protocol::message::identify_hw_sw(0x40);
    let frames = symbinux_transport::exchange_fbus2_with(
        &mut link,
        &command.frame,
        &symbinux_transport::ExchangeConfig::default(),
    )
    .map_err(|err| format!("No valid reply from the phone: {err}"))?;

    let (msg_type, data) = symbinux_protocol::reassemble_fbus2(&frames)
        .ok_or_else(|| "No data reply from the phone.".to_string())?;
    let reply = symbinux_protocol::Fbus2Frame {
        dest: 0,
        src: 0,
        msg_type,
        data,
    };
    let version = symbinux_protocol::hw_sw_version(&reply)
        .ok_or_else(|| "Reply is not a decodable HW/SW version.".to_string())?;

    Ok(PhoneIdentity {
        port,
        model: version.model,
        firmware: version.firmware,
        date: version.date,
    })
}

fn logo_path(dark: bool) -> Option<PathBuf> {
    let name = if dark {
        "symbinux_logo_transparent_dark.png"
    } else {
        "symbinux_logo_transparent_light.png"
    };

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)?;
    let path = root.join("assets").join("logo").join(name);
    path.exists().then_some(path)
}

fn prefer_dark() -> bool {
    gtk4::Settings::default()
        .map(|settings| settings.property::<bool>("gtk-application-prefer-dark-theme"))
        .unwrap_or(false)
}

fn load_settings() -> GuiSettings {
    let value = settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());

    let theme = value
        .as_ref()
        .and_then(|value| value.get("theme"))
        .and_then(serde_json::Value::as_str)
        .and_then(ThemeMode::parse)
        .unwrap_or(ThemeMode::Auto);

    let language = value
        .as_ref()
        .and_then(|value| value.get("language"))
        .and_then(serde_json::Value::as_str)
        .filter(|code| language_setting_exists(code))
        .unwrap_or("auto")
        .to_string();

    GuiSettings { theme, language }
}

fn save_theme(mode: ThemeMode) {
    save_setting("theme", mode.as_str());
}

fn save_language(code: &str) {
    save_setting("language", code);
}

fn save_setting(key: &str, value: &str) {
    let Some(path) = settings_path() else {
        return;
    };

    let mut object = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    object.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(&object) {
        let _ = fs::write(path, text);
    }
}

fn language_setting_exists(code: &str) -> bool {
    LANGUAGES.iter().any(|(candidate, _)| *candidate == code)
}

fn settings_path() -> Option<PathBuf> {
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(config);
        if !path.as_os_str().is_empty() {
            return Some(path.join("symbinux").join("settings.json"));
        }
    }

    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return Some(
                PathBuf::from(appdata)
                    .join("symbinux")
                    .join("settings.json"),
            );
        }
    }

    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("symbinux")
            .join("settings.json")
    })
}

fn apply_theme(mode: ThemeMode) {
    if let Some(settings) = gtk4::Settings::default() {
        settings.set_property("gtk-application-prefer-dark-theme", mode == ThemeMode::Dark);
    }
}

fn install_theme_action(app: &Application, initial: ThemeMode) {
    if app.lookup_action("theme").is_some() {
        return;
    }

    let action = gio::SimpleAction::new_stateful(
        "theme",
        Some(&String::static_variant_type()),
        &initial.as_str().to_variant(),
    );
    action.connect_activate(|action, parameter| {
        let Some(mode) = parameter
            .and_then(|value| value.str())
            .and_then(ThemeMode::parse)
        else {
            return;
        };
        action.set_state(&mode.as_str().to_variant());
        apply_theme(mode);
        save_theme(mode);
    });
    app.add_action(&action);
}

fn install_language_action(app: &Application, initial: &str) {
    if app.lookup_action("language").is_some() {
        return;
    }

    let action = gio::SimpleAction::new_stateful(
        "language",
        Some(&String::static_variant_type()),
        &initial.to_variant(),
    );
    let app_weak = app.downgrade();
    action.connect_activate(move |action, parameter| {
        let Some(code) = parameter
            .and_then(|value| value.str())
            .filter(|code| language_setting_exists(code))
            .map(|code| code.to_string())
        else {
            return;
        };

        action.set_state(&code.to_variant());
        save_language(&code);

        let Some(app) = app_weak.upgrade() else {
            return;
        };
        if let Some(window) = app.active_window() {
            window.close();
        }
        present_main_window(&app);
    });
    app.add_action(&action);
}

fn install_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        "
        .symbinux-wordmark {
            font-weight: 700;
            letter-spacing: 0.08em;
        }
        .symbinux-empty-logo {
            font-size: 32px;
            font-weight: 700;
            letter-spacing: 0.08em;
        }
        .symbinux-progress {
            background: alpha(currentColor, 0.06);
        }
        .symbinux-card {
            padding: 12px;
            border-radius: 8px;
            background: alpha(currentColor, 0.06);
        }
        ",
    );

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_ui(app: &Application) {
    let settings = load_settings();
    apply_theme(settings.theme);
    install_theme_action(app, settings.theme);
    install_language_action(app, &settings.language);
    present_main_window(app);
}

fn present_main_window(app: &Application) {
    let settings = load_settings();
    let translator = Rc::new(Translator::load(&settings.language));
    AppUi::new(app, translator).present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}
