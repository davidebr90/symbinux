"""Reusable GUI widgets: a progress panel and a background-work helper.

The progress panel has two honest modes:
- *indeterminate*: an Adw.Spinner shown while an operation of unknown duration
  runs (e.g. a subprocess enumeration). It never pretends to know a percentage.
- *determinate*: a real Gtk.ProgressBar driven by actual completed steps of a
  staged operation. Callers must feed it genuine fractions — nothing here
  animates a fake percentage.
"""

from __future__ import annotations

import threading
from typing import Callable

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, GLib, Gtk  # noqa: E402


def run_async(
    work: Callable[[], object],
    on_done: Callable[[object], None],
    on_error: Callable[[Exception], None] | None = None,
) -> None:
    """Run ``work()`` on a worker thread and deliver the result back on the GTK
    main loop via ``on_done`` (or ``on_error``)."""

    def target() -> None:
        try:
            result = work()
        except Exception as exc:  # noqa: BLE001 - surfaced to on_error
            GLib.idle_add(_dispatch, on_error, exc)
            return
        GLib.idle_add(_dispatch, on_done, result)

    threading.Thread(target=target, daemon=True).start()


def _dispatch(callback, arg) -> bool:
    if callback is not None:
        callback(arg)
    return False  # run once


class ProgressPanel(Gtk.Revealer):
    """A slim, revealable status bar showing a spinner or a real progress bar."""

    def __init__(self) -> None:
        super().__init__()
        self.set_transition_type(Gtk.RevealerTransitionType.SLIDE_DOWN)
        self.set_reveal_child(False)

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.add_css_class("toolbar")
        for side in ("start", "end"):
            getattr(box, f"set_margin_{side}")(12)
        for side in ("top", "bottom"):
            getattr(box, f"set_margin_{side}")(6)

        self._spinner = Adw.Spinner()
        self._spinner.set_size_request(18, 18)

        self._label = Gtk.Label()
        self._label.set_xalign(0.0)

        self._bar = Gtk.ProgressBar()
        self._bar.set_show_text(True)
        self._bar.set_hexpand(True)
        self._bar.set_valign(Gtk.Align.CENTER)

        box.append(self._spinner)
        box.append(self._label)
        box.append(self._bar)
        self.set_child(box)

        # Nothing shows until an operation explicitly requests a mode.
        self._spinner.set_visible(False)
        self._bar.set_visible(False)

    def indeterminate(self, text: str) -> None:
        self._spinner.set_visible(True)
        self._bar.set_visible(False)
        self._label.set_text(text)
        self.set_reveal_child(True)

    def determinate(self, text: str) -> None:
        self._spinner.set_visible(False)
        self._bar.set_visible(True)
        self._bar.set_fraction(0.0)
        self._bar.set_text("0%")
        self._label.set_text(text)
        self.set_reveal_child(True)

    def set_progress(self, fraction: float, label: str | None = None) -> None:
        fraction = max(0.0, min(1.0, fraction))
        self._bar.set_fraction(fraction)
        self._bar.set_text(f"{round(fraction * 100)}%")
        if label is not None:
            self._label.set_text(label)

    def finish(self) -> None:
        self.set_reveal_child(False)
