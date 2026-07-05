"""Entry point of the GUI application."""

from __future__ import annotations

import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gio, Gtk  # noqa: E402

from symbinux import __version__
from symbinux.gui.window import SymbinuxWindow

APP_ID = "it.davidebr90.Symbinux"


class SymbinuxApplication(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.DEFAULT_FLAGS)

        about_action = Gio.SimpleAction.new("about", None)
        about_action.connect("activate", self._on_about)
        self.add_action(about_action)

    def do_activate(self) -> None:
        window = self.props.active_window
        if window is None:
            window = SymbinuxWindow(application=self)
        window.present()

    def notify(self, title: str, body: str) -> None:
        """Send a native desktop notification.

        `Gio.Application.send_notification` routes through the freedesktop
        notification spec, so it integrates with GNOME, KDE and other desktops
        via the application id / desktop file, with no extra dependency.
        """
        notification = Gio.Notification.new(title)
        notification.set_body(body)
        notification.set_priority(Gio.NotificationPriority.NORMAL)
        self.send_notification(None, notification)

    def _on_about(self, _action, _param) -> None:
        about = Adw.AboutWindow(
            transient_for=self.props.active_window,
            application_name="Symbinux",
            application_icon=APP_ID,
            version=__version__,
            developer_name="Davide Pica",
            license_type=Gtk.License.AGPL_3_0,
            website="https://github.com/davidebr90/symbinux",
            comments="Modern USB/Bluetooth device management for GNU/Linux, with "
            "clean-room FBUS/MBUS support for legacy Nokia phones.",
        )
        about.present()


def run() -> int:
    app = SymbinuxApplication()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(run())
