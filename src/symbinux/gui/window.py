"""Main application window (GTK4 + libadwaita).

Layout goals:
- The logo and version are always visible (header bar + About dialog).
- A channel selector (USB / Bluetooth / Wi-Fi) lets the user pick how to look for
  phones; each channel has its own contextual empty state instead of a single
  generic "no device" message.
- Function buttons (Identify, Phonebook, SMS, Netmonitor, Advanced) each carry an
  icon and stay disabled ("greyed out") until a usable device is selected, so the
  available capabilities are always visible even with nothing connected.
- Actions surface results through native desktop notifications (see main.py).
"""

from __future__ import annotations

from pathlib import Path

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gio, Gtk  # noqa: E402

from symbinux import __version__
from symbinux.gui import backend

# Channels the user can scan on. Only USB drives real phone I/O today; the others
# are shown as selectable with honest "not yet wired up" states (see ROADMAP).
CHANNELS = [
    ("usb", "USB", "drive-harddisk-usb-symbolic"),
    ("bluetooth", "Bluetooth", "bluetooth-symbolic"),
    ("wifi", "Wi-Fi", "network-wireless-symbolic"),
]

# Every phone function, its icon and the capability tag it needs. Buttons are
# built from this table and enabled only when a compatible device is selected.
FUNCTIONS = [
    ("identify", "Identify", "dialog-information-symbolic", "Read model, IMEI and firmware"),
    ("phonebook", "Phonebook", "contact-new-symbolic", "Import / export contacts"),
    ("sms", "SMS", "mail-message-new-symbolic", "Read and send messages"),
    ("netmon", "Netmonitor", "network-cellular-signal-excellent-symbolic", "Network diagnostics"),
    ("advanced", "Advanced", "utilities-terminal-symbolic", "Raw USB/Bluetooth device list"),
]


def _logo_path() -> Path | None:
    here = Path(__file__).resolve()
    for root in here.parents:
        candidate = root / "assets" / "logo" / "symbinux_logo_transparent_light.png"
        if candidate.exists():
            return candidate
    return None


class SymbinuxWindow(Adw.ApplicationWindow):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Symbinux")
        self.set_default_size(560, 460)

        self._channel = "usb"
        self._selected_device: backend.Device | None = None
        self._function_buttons: dict[str, Gtk.Button] = {}
        self._setting_up = True

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
        self._device_list = self._build_device_list()
        self._device_stack.add_named(self._status_page, "empty")
        self._device_stack.add_named(self._device_list_scroller, "list")

        self._content_box.append(self._build_action_bar())

        # Select the default channel now that every widget exists, then let
        # signals through.
        self._usb_toggle.set_active(True)
        self._setting_up = False
        self.refresh()

    # --- construction helpers ---------------------------------------------

    def _build_header(self) -> Adw.HeaderBar:
        header = Adw.HeaderBar()

        logo = _logo_path()
        if logo is not None:
            image = Gtk.Image.new_from_file(str(logo))
            image.set_pixel_size(24)
            header.pack_start(image)

        title = Adw.WindowTitle(title="Symbinux", subtitle=f"v{__version__}")
        header.set_title_widget(title)

        refresh = Gtk.Button(icon_name="view-refresh-symbolic")
        refresh.set_tooltip_text("Rescan the selected channel")
        refresh.connect("clicked", lambda _b: self.refresh())
        header.pack_end(refresh)

        menu = Gio.Menu()
        menu.append("About Symbinux", "app.about")
        menu_button = Gtk.MenuButton(icon_name="open-menu-symbolic", menu_model=menu)
        header.pack_end(menu_button)

        return header

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
        logo = _logo_path()
        if logo is not None:
            page.set_paintable(Gtk.Image.new_from_file(str(logo)).get_paintable())
        else:
            page.set_icon_name("phone-symbolic")
        return page

    def _build_device_list(self) -> Gtk.ListBox:
        self._device_list_box = Gtk.ListBox()
        self._device_list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self._device_list_box.add_css_class("boxed-list")
        self._device_list_box.connect("row-selected", self._on_device_selected)

        self._device_list_scroller = Gtk.ScrolledWindow()
        self._device_list_scroller.set_child(self._device_list_box)
        for m in ("top", "bottom", "start", "end"):
            getattr(self._device_list_scroller, f"set_margin_{m}")(12)
        return self._device_list_box

    def _build_action_bar(self) -> Gtk.Box:
        bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        bar.set_halign(Gtk.Align.CENTER)
        for m in ("top", "bottom"):
            getattr(bar, f"set_margin_{m}")(10)

        for key, label, icon, tooltip in FUNCTIONS:
            btn = Gtk.Button()
            content = Adw.ButtonContent(label=label, icon_name=icon)
            btn.set_child(content)
            btn.set_tooltip_text(tooltip)
            btn.set_sensitive(False)  # greyed until a usable device is selected
            btn.connect("clicked", self._on_function_clicked, key)
            self._function_buttons[key] = btn
            bar.append(btn)
        return bar

    # --- behaviour --------------------------------------------------------

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
            if app is not None and hasattr(app, "notify"):
                app.notify(
                    "Identify",
                    "Connect over a serial cable (/dev/ttyUSB*) to read phone identity.",
                )
            return
        if app is not None and hasattr(app, "notify"):
            app.notify(label_for(key), "This function is not wired up yet on this channel.")

    def _update_function_sensitivity(self) -> None:
        has_phone = self._selected_device is not None and self._selected_device.is_phone
        for key, button in self._function_buttons.items():
            if key == "advanced":
                button.set_sensitive(True)  # diagnostics always available
            else:
                button.set_sensitive(has_phone)

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
                title=f"{label_for_channel(self._channel)} scanning not available yet",
                description="This channel is on the roadmap. USB is fully supported today.",
            )
            return

        try:
            devices = [d for d in backend.list_usb_devices() if d.is_phone or "cable" in d.role]
        except backend.BackendUnavailable as exc:
            self._show_empty(
                title="Core not available",
                description=str(exc),
            )
            return

        if not devices:
            self._show_empty(
                title="No phone or cable detected",
                description="Connect a Nokia phone via a DKU-2/CA-42 cable, then press "
                "the refresh button. Use Advanced to inspect every USB device.",
            )
            return

        for device in devices:
            subtitle = f"{device.vid_pid} · {device.role}"
            row = Adw.ActionRow(title=device.name, subtitle=subtitle)
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
            self._present_text_dialog("Advanced diagnostics", str(exc))
            return
        if not devices:
            body = "No USB devices visible to the host."
        else:
            lines = [f"{d.bus_addr}  {d.vid_pid}  {d.name}  [{d.role}]" for d in devices]
            body = "\n".join(lines)
        self._present_text_dialog("Connected USB devices", body)

    def _present_text_dialog(self, heading: str, body: str) -> None:
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading=heading,
            body=body,
        )
        dialog.add_response("ok", "Close")
        dialog.present()


def label_for(key: str) -> str:
    for k, label, _icon, _tt in FUNCTIONS:
        if k == key:
            return label
    return key


def label_for_channel(key: str) -> str:
    for k, label, _icon in CHANNELS:
        if k == key:
            return label
    return key
