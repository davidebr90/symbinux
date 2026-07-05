"""Colour-scheme handling and theme-aware logo selection.

Modes:
- ``auto``  — follow the desktop's light/dark preference (via libadwaita, which
  reads the freedesktop appearance portal). If the system exposes no preference,
  fall back to dark.
- ``light`` / ``dark`` — force that scheme regardless of the system.
"""

from __future__ import annotations

from pathlib import Path

import gi

gi.require_version("Adw", "1")

from gi.repository import Adw  # noqa: E402


def apply_theme(mode: str) -> None:
    manager = Adw.StyleManager.get_default()
    if mode == "light":
        manager.set_color_scheme(Adw.ColorScheme.FORCE_LIGHT)
    elif mode == "dark":
        manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)
    else:  # auto
        if manager.get_system_supports_color_schemes():
            manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
        else:
            # Cannot read the desktop preference: default to dark, as requested.
            manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)


def is_dark() -> bool:
    return Adw.StyleManager.get_default().get_dark()


def logo_path(dark: bool) -> Path | None:
    """Return the logo variant that reads well on the active background: the
    orange 'dark' asset on dark backgrounds, the blue 'light' asset otherwise."""
    name = (
        "symbinux_logo_transparent_dark.png"
        if dark
        else "symbinux_logo_transparent_light.png"
    )
    here = Path(__file__).resolve()
    for root in here.parents:
        candidate = root / "assets" / "logo" / name
        if candidate.exists():
            return candidate
    return None
