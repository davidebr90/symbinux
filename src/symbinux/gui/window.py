"""Main application window (GTK4 + libadwaita).

Layout goals:
- The logo is prominent (large on the empty state, a wordmark in the header) and
  swaps between its light/dark variants with the active colour scheme; the
  version is always shown.
- A minimum window size keeps every element comfortably spaced; the action bar
  wraps gracefully on narrow widths.
- A channel selector (USB / Bluetooth / Wi-Fi) with a contextual empty state per
  channel.
- Function buttons carry an icon and stay disabled until a usable device is
  selected.
- Long operations show honest feedback: a spinner while scanning, and a real
  progress bar (driven by actual completed steps) for staged operations.
"""

from __future__ import annotations

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("GdkPixbuf", "2.0")

from gi.repository import Adw, Gdk, GdkPixbuf, Gio, GLib, Gtk  # noqa: E402

from symbinux import __version__
from symbinux.gui import backend, theme
from symbinux.gui.i18n import N_, NATIVE_LANGUAGES, _
from symbinux.gui.widgets import ProgressPanel, run_async

CHANNELS = [
    ("usb", "USB", "drive-harddisk-usb-symbolic"),
    ("bluetooth", "Bluetooth", "bluetooth-symbolic"),
    ("wifi", "Wi-Fi", "network-wireless-symbolic"),
]

# key, source label, icon, source tooltip, required capability (None = always).
# The capability gates the button against the selected device's advertised
# capabilities, so the UI adapts per platform (Nokia / Android / iOS) without
# assuming feature parity.
FUNCTIONS = [
    ("identify", N_("Identify"), "dialog-information-symbolic", N_("Read model, IMEI and firmware"), "identify"),
    ("phonebook", N_("Phonebook"), "contact-new-symbolic", N_("Import / export contacts"), "phonebook"),
    ("sms", N_("SMS"), "mail-message-new-symbolic", N_("Read and send messages"), "sms"),
    ("netmon", N_("Netmonitor"), "network-cellular-signal-excellent-symbolic", N_("Network diagnostics"), "netmonitor"),
    ("advanced", N_("Advanced"), "utilities-terminal-symbolic", N_("Raw USB/Bluetooth device list"), None),
]


def _make_logo(height: int, halign: Gtk.Align) -> Gtk.Picture:
    # A Gtk.Picture bound to a texture pre-scaled to the target height, so its
    # natural size is exactly what we want (Picture otherwise expands to the
    # image's full intrinsic size). The texture is set later by _apply_logo,
    # which also rescales it on theme change.
    picture = Gtk.Picture()
    picture.set_content_fit(Gtk.ContentFit.CONTAIN)
    picture.set_can_shrink(True)
    picture.set_hexpand(False)
    picture.set_vexpand(False)
    picture.set_halign(halign)
    picture.set_valign(Gtk.Align.CENTER)
    picture._logo_height = height  # type: ignore[attr-defined]
    return picture


def _scaled_texture(path: str, height: int) -> Gdk.Texture | None:
    try:
        pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, -1, height, True)
        return Gdk.Texture.new_for_pixbuf(pixbuf)
    except Exception:
        return None


class SymbinuxWindow(Adw.ApplicationWindow):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Symbinux")
        self.set_default_size(860, 680)
        # Enforce a minimum size so content is never cramped.
        self.set_size_request(720, 600)

        self._channel = "usb"
        self._selected_device: backend.DetectedPhone | None = None
        self._function_buttons: dict[str, Gtk.Button] = {}
        self._function_capability: dict[str, str | None] = {}
        self._setting_up = True
        self._logos: list[Gtk.Picture] = []

        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)
        toolbar_view.add_top_bar(self._build_header())
        toolbar_view.add_top_bar(self._build_channel_bar())

        self._content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        toolbar_view.set_content(self._content_box)

        self._progress = ProgressPanel()
        self._content_box.append(self._progress)

        self._device_stack = Gtk.Stack()
        self._device_stack.set_vexpand(True)
        self._device_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self._content_box.append(self._device_stack)
        self._device_stack.add_named(self._build_empty_state(), "empty")
        self._build_device_list()
        self._device_stack.add_named(self._device_list_scroller, "list")

        self._content_box.append(self._build_action_bar())

        self._style_manager = Adw.StyleManager.get_default()
        self._style_manager.connect("notify::dark", self._on_dark_changed)
        self._apply_logo(self._style_manager.get_dark())

        self._usb_toggle.set_active(True)
        self._setting_up = False
        self.refresh()

    # --- construction helpers ---------------------------------------------

    def _build_header(self) -> Adw.HeaderBar:
        header = Adw.HeaderBar()

        # App name as bold uppercase text in the top-left; the large logo lives
        # on the empty state.
        name = Gtk.Label(label="SYMBINUX")
        name.add_css_class("heading")
        name.set_margin_start(6)
        header.pack_start(name)

        version = Gtk.Label(label=f"v{__version__}")
        version.add_css_class("dim-label")
        version.add_css_class("caption")
        header.set_title_widget(version)

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
        bar.set_margin_top(10)
        bar.set_margin_bottom(10)

        first_button: Gtk.ToggleButton | None = None
        for key, label, icon in CHANNELS:
            btn = Gtk.ToggleButton()
            inner = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            inner.set_margin_start(4)
            inner.set_margin_end(4)
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

    def _build_empty_state(self) -> Gtk.Box:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=20)
        box.set_valign(Gtk.Align.CENTER)
        box.set_vexpand(True)
        for side in ("start", "end", "top", "bottom"):
            getattr(box, f"set_margin_{side}")(24)

        logo = _make_logo(120, Gtk.Align.CENTER)
        self._logos.append(logo)
        box.append(logo)

        self._empty_title = Gtk.Label()
        self._empty_title.add_css_class("title-1")
        self._empty_title.set_wrap(True)
        self._empty_title.set_justify(Gtk.Justification.CENTER)
        box.append(self._empty_title)

        self._empty_desc = Gtk.Label()
        self._empty_desc.add_css_class("dim-label")
        self._empty_desc.set_wrap(True)
        self._empty_desc.set_justify(Gtk.Justification.CENTER)
        self._empty_desc.set_max_width_chars(52)
        box.append(self._empty_desc)

        return box

    def _build_device_list(self) -> None:
        self._device_list_box = Gtk.ListBox()
        self._device_list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self._device_list_box.add_css_class("boxed-list")
        self._device_list_box.set_valign(Gtk.Align.START)
        self._device_list_box.connect("row-selected", self._on_device_selected)

        self._device_list_scroller = Gtk.ScrolledWindow()
        self._device_list_scroller.set_child(self._device_list_box)
        for m in ("top", "bottom", "start", "end"):
            getattr(self._device_list_scroller, f"set_margin_{m}")(18)

    def _build_action_bar(self) -> Gtk.Widget:
        wrap = Adw.WrapBox()
        wrap.set_child_spacing(8)
        wrap.set_line_spacing(8)
        wrap.set_halign(Gtk.Align.CENTER)
        wrap.set_justify(Adw.JustifyMode.NONE)
        for side in ("start", "end", "top", "bottom"):
            getattr(wrap, f"set_margin_{side}")(12)

        for key, label, icon, tooltip, capability in FUNCTIONS:
            btn = Gtk.Button()
            btn.set_child(Adw.ButtonContent(label=_(label), icon_name=icon))
            btn.set_tooltip_text(_(tooltip))
            btn.set_sensitive(False)
            btn.connect("clicked", self._on_function_clicked, key)
            self._function_buttons[key] = btn
            self._function_capability[key] = capability
            wrap.append(btn)
        return wrap

    # --- behaviour --------------------------------------------------------

    def _apply_logo(self, dark: bool) -> None:
        path = theme.logo_path(dark)
        if path is None:
            return
        for picture in self._logos:
            texture = _scaled_texture(str(path), picture._logo_height)  # type: ignore[attr-defined]
            if texture is not None:
                picture.set_paintable(texture)

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
        phone = self._selected_device
        for key, button in self._function_buttons.items():
            if key == "advanced":
                button.set_sensitive(True)  # diagnostics always available
                continue
            capability = self._function_capability.get(key)
            button.set_sensitive(
                phone is not None and capability is not None and phone.has_capability(capability)
            )

    def refresh(self) -> None:
        """Rescan the active channel (off the UI thread) and rebuild the view."""
        self._selected_device = None
        self._update_function_sensitivity()
        self._clear_device_list()

        if self._channel != "usb":
            self._progress.finish()
            self._show_empty(
                title=_("%s scanning not available yet") % channel_label(self._channel),
                description=_("This channel is on the roadmap. USB is fully supported today."),
            )
            return

        # Real progress: the bar advances as the detection cascade completes
        # actual steps (reported by the core), not on a timer.
        self._progress.determinate(_("Detecting devices…"))

        def on_progress(fraction, _stage):
            GLib.idle_add(self._progress.set_progress, fraction, _("Detecting devices…"))

        def work():
            return backend.detect_devices(progress_cb=on_progress)

        def done(phones):
            self._progress.finish()
            self._populate(phones)

        def failed(exc):
            self._progress.finish()
            self._show_empty(title=_("Core not available"), description=str(exc))

        run_async(work, done, failed)

    def _populate(self, phones) -> None:
        if not phones:
            self._show_empty(
                title=_("No phone or cable detected"),
                description=_(
                    "Connect a Nokia phone via a DKU-2/CA-42 cable, then press the "
                    "refresh button. Use Advanced to inspect every USB device."
                ),
            )
            return
        for phone in phones:
            caps = ", ".join(phone.capabilities)
            subtitle = f"{phone.platform} · {phone.vid_pid}"
            if caps:
                subtitle += f" · {caps}"
            title = phone.model if phone.model and phone.model != "?" else phone.platform
            row = Adw.ActionRow(title=title, subtitle=subtitle)
            row.add_prefix(Gtk.Image.new_from_icon_name("phone-symbolic"))
            row.device = phone  # type: ignore[attr-defined]
            self._device_list_box.append(row)
        self._device_stack.set_visible_child_name("list")

    def _clear_device_list(self) -> None:
        child = self._device_list_box.get_first_child()
        while child is not None:
            self._device_list_box.remove(child)
            child = self._device_list_box.get_first_child()

    def _show_empty(self, title: str, description: str) -> None:
        self._empty_title.set_text(title)
        self._empty_desc.set_text(description)
        self._device_stack.set_visible_child_name("empty")

    def _show_advanced(self) -> None:
        self._progress.indeterminate(_("Scanning USB…"))

        def work():
            return backend.list_usb_devices(include_all=True)

        def done(devices):
            self._progress.finish()
            if not devices:
                body = _("No USB devices visible to the host.")
            else:
                body = "\n".join(f"{d.bus_addr}  {d.vid_pid}  {d.name}  [{d.role}]" for d in devices)
            self._present_text_dialog(_("Connected USB devices"), body)

        def failed(exc):
            self._progress.finish()
            self._present_text_dialog(_("Advanced diagnostics"), str(exc))

        run_async(work, done, failed)

    def _present_text_dialog(self, heading: str, body: str) -> None:
        dialog = Adw.MessageDialog(transient_for=self, heading=heading, body=body)
        dialog.add_response("ok", _("Close"))
        dialog.present()


def label_for(key: str) -> str:
    for k, label, _icon, _tt, _cap in FUNCTIONS:
        if k == key:
            return _(label)
    return key


def channel_label(key: str) -> str:
    for k, label, _icon in CHANNELS:
        if k == key:
            return label
    return key
