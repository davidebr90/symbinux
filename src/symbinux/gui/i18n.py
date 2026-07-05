"""Internationalisation for the GUI, built on GNU gettext.

Source strings are English. Translations live as ``po/<lang>.po`` and are
compiled to ``symbinux/locale/<lang>/LC_MESSAGES/symbinux.mo``. Adding a language
is just adding a ``.po`` file — see ``po/README.md``.

Use ``_()`` to translate at call time and ``N_()`` to mark a string for
extraction without translating it yet (e.g. in module-level tables); translate
the marked string later with ``_()``.
"""

from __future__ import annotations

import gettext
import os
from pathlib import Path

DOMAIN = "symbinux"

# Languages shipped natively. Translators/we can extend this list as more
# ``po/<code>.po`` files are added.
NATIVE_LANGUAGES: list[tuple[str, str]] = [
    ("auto", "Automatic"),
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("fr", "Français"),
    ("it", "Italiano"),
    ("nl", "Nederlands"),
    ("pt", "Português"),
]

_current: gettext.NullTranslations = gettext.NullTranslations()


def _localedir() -> str | None:
    env = os.environ.get("SYMBINUX_LOCALEDIR")
    if env and Path(env).is_dir():
        return env
    here = Path(__file__).resolve()
    for root in here.parents:
        for candidate in (root / "locale", root / "src" / "symbinux" / "locale"):
            if candidate.is_dir():
                return str(candidate)
    return None


def set_language(code: str) -> None:
    """Activate a language. ``"auto"`` follows the environment locale, ``"en"``
    uses the source strings, any other code loads its compiled translation."""
    global _current
    if code == "en":
        _current = gettext.NullTranslations()
        return
    languages = None if code == "auto" else [code]
    _current = gettext.translation(
        DOMAIN,
        localedir=_localedir(),
        languages=languages,
        fallback=True,
    )


def _(message: str) -> str:
    return _current.gettext(message)


def N_(message: str) -> str:
    """Mark a string for extraction; returns it unchanged."""
    return message
