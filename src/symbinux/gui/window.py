"""Main application window (GTK4 + libadwaita).

Layout goals:
- The logo and version are always visible (header bar + About dialog); the logo
  swaps between its light/dark variants with the active colour scheme.
- A channel selector (USB / Bluetooth / Wi-Fi) lets the user pick how to look for
  phones; each channel has its own contextual empty state.
- Function buttons carry an icon and stay disabled ("greyed out") until a usable
  device is selected, so capabilities are visible even with nothing connected.
- Appearance (Automatic/Light/Dark) and Language are chosen from the menu.
- Actions surface results through native desktop notifications (see main.py).
"""

from __future__ import annotations

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gio, Gtk  # noqa: E402

from symbinux import __version__
from symbinux.gui import backend, theme
from symbinux.gui.i18n import N_, NATIVE_LANGUAGES, _

# Channels the user can scan on. Labels are product names, left untranslated.
CHANNELS = [
    ("usb", "USB", "drive-harddisk-usb-symbolic"),
    ("bluetooth", "Bluetooth", "bluetooth-symbolic"),
    ("wifi", "Wi-Fi", "network-wireless-symbolic"),
]

# Every phone function: key, source label, icon, source tooltip. Labels/tooltips
# are marked with N_() for extraction and translated at build time with _().
FUNCTIONS = [
    ("identify", N_("Identify"), "dialog-information-symbolic", N_("Read model, IMEI and firmware")),
    ("phonebook", N_("Phonebook"), "contact-new-symbolic", N_("Import / export contacts")),
    ("sms", N_("SMS"), "mail-message-new-symbolic", N_("Read and send messages")),
    ("netmon", N_("Netmonitor"), "network-cellular-signal-excellent-symbolic", N_("Network diagnostics")),
    ("advanced", N_("Advanced"), "utilities-terminal-symbolic", N_("Raw USB/Bluetooth device list")),
]


class SymbinuxWindow(Adw.ApplicationWindow):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Symbinux")
        self.set_default_size(560, 460)

        self._channel = "usb"
        self._selected_device: backend.Device | None = None
        self._function_buttons: dict[str, Gtk.Button] = {}
        self._setting_up = True
        self._header_logo: Gtk.Image | None = None

        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)
        toolbar_view.add_top_bar(self._build_header())
        toolbar_view.add_top_bar(self._build_channel_bar())

        self._content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        toolbar_view.set_content(self._content_box)

        self._device_stack = Gtk.Stack()
        self._device_stack.set_vexpand(True)
        self._content_box.append(self._device_stack)
        self._status_page = self._build_status_page()
        self._build_device_list()
        self._device_stack.add_named(self._status_page, "empty")
        self._device_stack.add_named(self._device_list_scroller, "list")

        self._content_box.append(self._build_action_bar())

        # React to colour-scheme changes (system or manual) to swap the logo.
        self._style_manager = Adw.StyleManager.get_default()
        self._style_manager.connect("notify::dark", self._on_dark_changed)
        self._apply_logo(self._style_manager.get_dark())

        self._usb_toggle.set_active(True)
        self._setting_up = False
        self.refresh()

    # --- construction helpers ---------------------------------------------

    def _build_header(self) -> Adw.HeaderBar:
        header = Adw.HeaderBar()

        self._header_logo = Gtk.Image()
        self._header_logo.set_pixel_size(24)
        header.pack_start(self._header_logo)

        header.set_title_widget(Adw.WindowTitle(title="Symbinux", subtitle=f"v{__version__}"))

        refresh = Gtk.Button(icon_name="view-refresh-symbolic")
        refresh.set_tooltip_text(_("Rescan the selected channel"))
        refresh.connect("clicked", lambda _b: self.refresh())
        header.pack_end(refresh)

        header.pack_end(Gtk.MenuButton(icon_name="open-menu-symbolic", menu_model=self._build_menu()))
        return header

    def _build_menu(self) -> Gio.Menu:
        menu = Gio.Menu()

        appearance = Gio.Menu()
        appearance.append(_("Automatic"), "app.theme::auto")
        appearance.append(_("Light"), "app.theme::light")
        appearance.append(_("Dark"), "app.theme::dark")
        menu.append_submenu(_("Appearance"), appearance)

        language = Gio.Menu()
        for code, native_label in NATIVE_LANGUAGES:
            label = _("Automatic") if code == "auto" else native_label
            language.append(label, f"app.language::{code}")
        menu.append_submenu(_("Language"), language)

        about_section = Gio.Menu()
        about_section.append(_("About Symbinux"), "app.about")
        menu.append_section(None, about_section)
        return menu

    def _build_channel_bar(self) -> Gtk.Box:
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        bar.set_halign(Gtk.Align.CENTER)
        bar.set_margin_top(8)
        bar.set_margin_bottom(8)

        first_button: Gtk.ToggleButton | None = None
        for key, label, icon in CHANNELS:
            btn = Gtk.ToggleButton()
            inner = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
            inner.append(Gtk.Image.new_from_icon_name(icon))
            inner.append(Gtk.Label(label=label))
            btn.set_child(inner)
            if first_button is None:
                first_button = btn
            else:
                btn.set_group(first_button)
            btn.connect("toggled", self._on_channel_toggled, key)
            btn.add_css_class("flat")
            bar.append(btn)
            if key == "usb":
                self._usb_toggle = btn
        return bar

    def _build_status_page(self) -> Adw.StatusPage:
        page = Adw.StatusPage()
        page.set_vexpand(True)
        page.set_icon_name("phone-symbolic")
        return page

    def _build_device_list(self) -> None:
        self._device_list_box = Gtk.ListBox()
        self._device_list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self._device_list_box.add_css_class("boxed-list")
        self._device_list_box.connect("row-selected", self._on_device_selected)

        self._device_list_scroller = Gtk.ScrolledWindow()
        self._device_list_scroller.set_child(self._device_list_box)
        for m in ("top", "bottom", "start", "end"):
            getattr(self._device_list_scroller, f"set_margin_{m}")(12)

    def _build_action_bar(self) -> Gtk.Box:
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        bar.set_halign(Gtk.Align.CENTER)
        for m in ("top", "bottom"):
            getattr(bar, f"set_margin_{m}")(10)

        for key, label, icon, tooltip in FUNCTIONS:
            btn = Gtk.Button()
            btn.set_child(Adw.ButtonContent(label=_(label), icon_name=icon))
            btn.set_tooltip_text(_(tooltip))
            btn.set_sensitive(False)  # greyed until a usable device is selected
            btn.connect("clicked", self._on_function_clicked, key)
            self._function_buttons[key] = btn
            bar.append(btn)
        return bar

    # --- behaviour --------------------------------------------------------

    def _apply_logo(self, dark: bool) -> None:
        path = theme.logo_path(dark)
        if self._header_logo is not None and path is not None:
            self._header_logo.set_from_file(str(path))
        if path is not None:
            image = Gtk.Image.new_from_file(str(path))
            paintable = image.get_paintable()
            if paintable is not None:
                self._status_page.set_paintable(paintable)

    def _on_dark_changed(self, _manager, _param) -> None:
        self._apply_logo(self._style_manager.get_dark())

    def _on_channel_toggled(self, button: Gtk.ToggleButton, key: str) -> None:
        if self._setting_up:
            return
        if button.get_active():
            self._channel = key
            self._selected_device = None
            self.refresh()

    def _on_device_selected(self, _listbox, row) -> None:
        self._selected_device = getattr(row, "device", None) if row else None
        self._update_function_sensitivity()

    def _on_function_clicked(self, _button, key: str) -> None:
        app = self.get_application()
        if key == "advanced":
            self._show_advanced()
            return
        if key == "identify" and self._selected_device is not None:
            self._notify(app, _("Identify"),
                         _("Connect over a serial cable (/dev/ttyUSB*) to read phone identity."))
            return
        self._notify(app, label_for(key), _("This function is not wired up yet on this channel."))

    @staticmethod
    def _notify(app, title: str, body: str) -> None:
        if app is not None and hasattr(app, "notify"):
            app.notify(title, body)

    def _update_function_sensitivity(self) -> None:
        has_phone = self._selected_device is not None and self._selected_device.is_phone
        for key, button in self._function_buttons.items():
            button.set_sensitive(True if key == "advanced" else has_phone)

    def refresh(self) -> None:
        """Rescan the active channel and rebuild the device list / empty state."""
        self._selected_device = None
        self._update_function_sensitivity()

        child = self._device_list_box.get_first_child()
        while child is not None:
            self._device_list_box.remove(child)
            child = self._device_list_box.get_first_child()

        if self._channel != "usb":
            self._show_empty(
                title=_("%s scanning not available yet") % channel_label(self._channel),
                description=_("This channel is on the roadmap. USB is fully supported today."),
            )
            return

        try:
            devices = [d for d in backend.list_usb_devices() if d.is_phone or "cable" in d.role]
        except backend.BackendUnavailable as exc:
            self._show_empty(title=_("Core not available"), description=str(exc))
            return

        if not devices:
            self._show_empty(
                title=_("No phone or cable detected"),
                description=_(
                    "Connect a Nokia phone via a DKU-2/CA-42 cable, then press the "
                    "refresh button. Use Advanced to inspect every USB device."
                ),
            )
            return

        for device in devices:
            row = Adw.ActionRow(title=device.name, subtitle=f"{device.vid_pid} · {device.role}")
            row.add_prefix(Gtk.Image.new_from_icon_name("phone-symbolic"))
            row.device = device  # type: ignore[attr-defined]
            self._device_list_box.append(row)
        self._device_stack.set_visible_child_name("list")

    def _show_empty(self, title: str, description: str) -> None:
        self._status_page.set_title(title)
        self._status_page.set_description(description)
        self._device_stack.set_visible_child_name("empty")

    def _show_advanced(self) -> None:
        try:
            devices = backend.list_usb_devices(include_all=True)
        except backend.BackendUnavailable as exc:
            self._present_text_dialog(_("Advanced diagnostics"), str(exc))
            return
        if not devices:
            body = _("No USB devices visible to the host.")
        else:
            body = "\n".join(f"{d.bus_addr}  {d.vid_pid}  {d.name}  [{d.role}]" for d in devices)
        self._present_text_dialog(_("Connected USB devices"), body)

    def _present_text_dialog(self, heading: str, body: str) -> None:
        dialog = Adw.MessageDialog(transient_for=self, heading=heading, body=body)
        dialog.add_response("ok", _("Close"))
        dialog.present()


def label_for(key: str) -> str:
    for k, label, _icon, _tt in FUNCTIONS:
        if k == key:
            return _(label)
    return key


def channel_label(key: str) -> str:
    for k, label, _icon in CHANNELS:
        if k == key:
            return label
    return key
