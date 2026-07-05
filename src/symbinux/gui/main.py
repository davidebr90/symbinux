"""Entry point of the GUI application."""

from __future__ import annotations

import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gio, GLib, Gtk  # noqa: E402

from symbinux import __version__
from symbinux.gui import settings, theme
from symbinux.gui.i18n import _, set_language
from symbinux.gui.window import SymbinuxWindow

APP_ID = "it.davidebr90.Symbinux"


class SymbinuxApplication(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.DEFAULT_FLAGS)

        self._settings = settings.load()
        set_language(self._settings["language"])
        # Theme is applied in do_startup, once a default display exists.

        self.add_action(self._make_action("about", self._on_about))
        self.add_action(self._make_stateful("theme", self._settings["theme"], self._on_theme))
        self.add_action(self._make_stateful("language", self._settings["language"], self._on_language))

    def do_startup(self) -> None:
        Adw.Application.do_startup(self)
        theme.apply_theme(self._settings["theme"])

    def do_activate(self) -> None:
        window = self.props.active_window
        if window is None:
            window = SymbinuxWindow(application=self)
        window.present()

    # --- actions ----------------------------------------------------------

    @staticmethod
    def _make_action(name: str, handler) -> Gio.SimpleAction:
        action = Gio.SimpleAction.new(name, None)
        action.connect("activate", handler)
        return action

    @staticmethod
    def _make_stateful(name: str, initial: str, handler) -> Gio.SimpleAction:
        action = Gio.SimpleAction.new_stateful(
            name, GLib.VariantType.new("s"), GLib.Variant("s", initial)
        )
        action.connect("activate", handler)
        return action

    def _on_theme(self, action, param) -> None:
        mode = param.get_string()
        action.set_state(param)
        self._settings["theme"] = mode
        settings.save(self._settings)
        theme.apply_theme(mode)

    def _on_language(self, action, param) -> None:
        code = param.get_string()
        if code == self._settings["language"]:
            return
        action.set_state(param)
        self._settings["language"] = code
        settings.save(self._settings)
        set_language(code)
        # Rebuild the window so every translated string is refreshed.
        old = self.props.active_window
        SymbinuxWindow(application=self).present()
        if old is not None:
            old.destroy()

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
            comments=_("Talk to legacy Nokia phones over FBUS/MBUS on modern Linux."),
        )
        about.present()


def run() -> int:
    app = SymbinuxApplication()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(run())
