//! Symbinux desktop GUI (gtk4-rs).
//!
//! Phase 1 of the desktop port keeps the current Python GUI usable while this
//! binary reaches parity widget by widget. The Rust GUI links the core crates
//! directly; no command-line bridge is used for USB detection.

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, ButtonsType, CssProvider, Dialog,
    FlowBox, HeaderBar, Image, Label, ListBox, ListBoxRow, MessageDialog, MessageType, Orientation,
    Picture, ProgressBar, ResponseType, Revealer, RevealerTransitionType, ScrolledWindow,
    SelectionMode, Spinner, Stack, StackTransitionType, ToggleButton,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::Duration;
use symbinux_transport::Transport as _;

const APP_ID: &str = "it.davidebr90.Symbinux";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const SERIAL_VIDS: &[u16] = &[0x0421, 0x067b, 0x10c4, 0x0403, 0x1a86];

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
        tooltip: "Import or export contacts",
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

#[derive(Debug, Clone)]
struct PhoneIdentity {
    port: String,
    model: String,
    firmware: String,
    date: String,
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
    fn new() -> Self {
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

        let cancel_button = Button::with_label("Cancel");
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
    fn new(app: &Application) -> Rc<Self> {
        install_css();

        let (header, refresh_button) = build_header();
        let channel_bar = GtkBox::new(Orientation::Horizontal, 6);
        channel_bar.set_halign(Align::Center);
        channel_bar.set_margin_top(10);
        channel_bar.set_margin_bottom(10);

        let progress = ProgressPanel::new();

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
            let button = function_button(function);
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

        ui.show_empty(Channel::Usb.empty_title(), Channel::Usb.empty_description());
        ui.refresh();
        ui
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
            channel => self.show_empty(channel.empty_title(), channel.empty_description()),
        }
    }

    fn refresh_usb(self: &Rc<Self>) {
        let cancel = Arc::new(AtomicBool::new(false));
        *self.current_cancel.borrow_mut() = Some(cancel.clone());

        let cancel_for_button = cancel.clone();
        self.progress.determinate("Detecting devices...", move || {
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
                            Err(err) => ui.show_empty("Detection error", &err),
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
            .indeterminate("Identifying phone...", move || {
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
                        Ok(identity) => show_identity_card(&ui.window, &identity),
                        Err(err) => show_dialog(&ui.window, "Identify", &err),
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

    fn populate_devices(&self, devices: Vec<UiDevice>) {
        if devices.is_empty() {
            self.show_empty(Channel::Usb.empty_title(), Channel::Usb.empty_description());
            return;
        }

        *self.devices.borrow_mut() = devices;
        for device in self.devices.borrow().iter() {
            self.device_list.append(&device_row(device));
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
        show_dialog(
            &self.window,
            label,
            "This function is pending in the Rust GUI. The Python GUI remains available until parity is complete.",
        );
    }
}

fn build_header() -> (HeaderBar, Button) {
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
        .tooltip_text("Rescan the selected channel")
        .build();
    header.pack_end(&refresh);

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
        button.set_tooltip_text(Some(spec.label));

        let inner = GtkBox::new(Orientation::Horizontal, 8);
        inner.set_margin_start(4);
        inner.set_margin_end(4);
        inner.append(&Image::from_icon_name(spec.icon));
        inner.append(&Label::new(Some(spec.label)));
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

fn function_button(function: &FunctionSpec) -> Button {
    let inner = GtkBox::new(Orientation::Horizontal, 8);
    inner.set_margin_top(6);
    inner.set_margin_bottom(6);
    inner.set_margin_start(8);
    inner.set_margin_end(8);
    inner.append(&Image::from_icon_name(function.icon));
    inner.append(&Label::new(Some(function.label)));

    let button = Button::new();
    button.set_tooltip_text(Some(function.tooltip));
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

fn show_identity_card(parent: &ApplicationWindow, identity: &PhoneIdentity) {
    let dialog = Dialog::builder()
        .transient_for(parent)
        .modal(true)
        .title("Phone identity")
        .build();
    dialog.add_button("Close", ResponseType::Close);

    let card = GtkBox::new(Orientation::Vertical, 8);
    card.add_css_class("symbinux-card");
    card.set_margin_top(16);
    card.set_margin_bottom(16);
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.append(&property_row("Model", &identity.model));
    card.append(&property_row("Firmware", &identity.firmware));
    card.append(&property_row("Date", &identity.date));
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
    AppUi::new(app).present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}
