"""Finestra principale dell'applicazione (GTK4 + libadwaita)."""

from __future__ import annotations

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gtk  # noqa: E402

from symbinux.core.devices import list_usb_devices


class SymbinuxWindow(Adw.ApplicationWindow):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Symbinux")
        self.set_default_size(480, 360)

        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)

        header = Adw.HeaderBar()
        toolbar_view.add_top_bar(header)

        refresh_button = Gtk.Button(icon_name="view-refresh-symbolic")
        refresh_button.set_tooltip_text("Aggiorna elenco dispositivi")
        refresh_button.connect("clicked", self._on_refresh_clicked)
        header.pack_start(refresh_button)

        self._device_list = Gtk.ListBox()
        self._device_list.set_selection_mode(Gtk.SelectionMode.NONE)
        self._device_list.add_css_class("boxed-list")

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_child(self._device_list)
        scrolled.set_margin_top(12)
        scrolled.set_margin_bottom(12)
        scrolled.set_margin_start(12)
        scrolled.set_margin_end(12)
        toolbar_view.set_content(scrolled)

        self._refresh_devices()

    def _on_refresh_clicked(self, _button: Gtk.Button) -> None:
        self._refresh_devices()

    def _refresh_devices(self) -> None:
        while row := self._device_list.get_row_at_index(0):
            self._device_list.remove(row)

        try:
            devices = list_usb_devices()
        except RuntimeError as exc:
            self._device_list.append(Gtk.Label(label=str(exc), wrap=True))
            return

        if not devices:
            self._device_list.append(Gtk.Label(label="Nessun dispositivo USB rilevato."))
            return

        for device in devices:
            title = device.product_name or device.device_node or "Dispositivo sconosciuto"
            subtitle = device.vendor_name or device.vendor_id or ""
            row = Adw.ActionRow(title=title, subtitle=subtitle)
            self._device_list.append(row)
