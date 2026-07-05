"""Entry point dell'applicazione GUI."""

from __future__ import annotations

import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gio  # noqa: E402

from symbinux.gui.window import SymbinuxWindow

APP_ID = "it.davidebr90.Symbinux"


class SymbinuxApplication(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.DEFAULT_FLAGS)

    def do_activate(self) -> None:
        window = self.props.active_window
        if window is None:
            window = SymbinuxWindow(application=self)
        window.present()


def run() -> int:
    app = SymbinuxApplication()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(run())
