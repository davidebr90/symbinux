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
import locale
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
    ("ja", "日本語"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("zh_CN", "中文（简体）"),
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


def _shipped_codes() -> set[str]:
    """Language codes we actually ship a translation for (excludes auto/en)."""
    return {code for code, _ in NATIVE_LANGUAGES if code not in ("auto", "en")}


def system_language() -> str | None:
    """Best guess at the desktop's preferred language as a 2-letter code."""
    for var in ("LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"):
        value = os.environ.get(var)
        if value:
            code = value.split(":")[0].split(".")[0].split("_")[0].strip().lower()
            if code and code not in ("c", "posix"):
                return code
    try:
        loc = locale.getdefaultlocale()[0]
    except Exception:
        loc = None
    return loc.split("_")[0].lower() if loc else None


def set_language(code: str) -> None:
    """Activate a language.

    ``"auto"`` picks the desktop's language if we ship a translation for it, and
    otherwise falls back to English. ``"en"`` uses the source strings; any other
    code loads its compiled translation.
    """
    global _current
    if code == "auto":
        sys_lang = system_language()
        code = sys_lang if sys_lang in _shipped_codes() else "en"
    if code == "en":
        _current = gettext.NullTranslations()
        return
    _current = gettext.translation(
        DOMAIN,
        localedir=_localedir(),
        languages=[code],
        fallback=True,
    )


def _(message: str) -> str:
    return _current.gettext(message)


def N_(message: str) -> str:
    """Mark a string for extraction; returns it unchanged."""
    return message
