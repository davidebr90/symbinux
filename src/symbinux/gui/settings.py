"""Small persisted GUI preferences (theme, language).

Stored as JSON under the XDG config dir so the choice survives restarts without
requiring an installed GSettings schema.
"""

from __future__ import annotations

import json
from pathlib import Path

from gi.repository import GLib

_CONFIG = Path(GLib.get_user_config_dir()) / "symbinux" / "settings.json"

# theme: "auto" | "light" | "dark" ; language: "auto" | "en" | "it" | ...
_DEFAULTS = {"theme": "auto", "language": "auto"}


def load() -> dict:
    try:
        data = json.loads(_CONFIG.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        data = {}
    return {**_DEFAULTS, **{k: v for k, v in data.items() if k in _DEFAULTS}}


def save(settings: dict) -> None:
    try:
        _CONFIG.parent.mkdir(parents=True, exist_ok=True)
        _CONFIG.write_text(json.dumps(settings, indent=2), encoding="utf-8")
    except OSError:
        pass
