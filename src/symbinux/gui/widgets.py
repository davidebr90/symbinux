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


class CancelToken:
    """A one-shot cancellation flag shared with a background operation."""

    def __init__(self) -> None:
        self.cancelled = False

    def cancel(self) -> None:
        self.cancelled = True


def run_async(
    work: Callable[[], object],
    on_done: Callable[[object], None],
    on_error: Callable[[Exception], None] | None = None,
    token: CancelToken | None = None,
) -> None:
    """Run ``work()`` on a worker thread and deliver the result back on the GTK
    main loop. If ``token`` is cancelled by the time the result arrives, the
    callbacks are skipped (the user dismissed the wait)."""

    def target() -> None:
        try:
            result = work()
        except Exception as exc:  # noqa: BLE001 - surfaced to on_error
            GLib.idle_add(_dispatch, on_error, exc, token)
            return
        GLib.idle_add(_dispatch, on_done, result, token)

    threading.Thread(target=target, daemon=True).start()


def _dispatch(callback, arg, token) -> bool:
    if callback is not None and (token is None or not token.cancelled):
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

        self._cancel_button = Gtk.Button()
        self._cancel_button.add_css_class("flat")
        self._cancel_button.set_valign(Gtk.Align.CENTER)
        self._cancel_button.connect("clicked", self._on_cancel_clicked)
        self._on_cancel: Callable[[], None] | None = None

        box.append(self._spinner)
        box.append(self._label)
        box.append(self._bar)
        box.append(self._cancel_button)
        self.set_child(box)

        # Nothing shows until an operation explicitly requests a mode.
        self._spinner.set_visible(False)
        self._bar.set_visible(False)
        self._cancel_button.set_visible(False)

    def _configure_cancel(self, label: str | None, on_cancel: Callable[[], None] | None) -> None:
        self._on_cancel = on_cancel
        if on_cancel is not None and label is not None:
            self._cancel_button.set_label(label)
            self._cancel_button.set_visible(True)
        else:
            self._cancel_button.set_visible(False)

    def _on_cancel_clicked(self, _button) -> None:
        callback = self._on_cancel
        self.finish()
        if callback is not None:
            callback()

    def indeterminate(
        self,
        text: str,
        on_cancel: Callable[[], None] | None = None,
        cancel_label: str | None = None,
    ) -> None:
        self._spinner.set_visible(True)
        self._bar.set_visible(False)
        self._label.set_text(text)
        self._configure_cancel(cancel_label, on_cancel)
        self.set_reveal_child(True)

    def determinate(
        self,
        text: str,
        on_cancel: Callable[[], None] | None = None,
        cancel_label: str | None = None,
    ) -> None:
        self._spinner.set_visible(False)
        self._bar.set_visible(True)
        self._bar.set_fraction(0.0)
        self._bar.set_text("0%")
        self._label.set_text(text)
        self._configure_cancel(cancel_label, on_cancel)
        self.set_reveal_child(True)

    def set_progress(self, fraction: float, label: str | None = None) -> None:
        fraction = max(0.0, min(1.0, fraction))
        self._bar.set_fraction(fraction)
        self._bar.set_text(f"{round(fraction * 100)}%")
        if label is not None:
            self._label.set_text(label)

    def finish(self) -> None:
        self.set_reveal_child(False)
