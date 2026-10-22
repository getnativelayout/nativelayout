"""Parser of CSS declarations and values (port of Css.kt)."""
from __future__ import annotations

import math
import re
import threading
from dataclasses import dataclass
from typing import Callable

from .tree import AUTO, Dim, Length, Percent


@dataclass(frozen=True)
class CssDeclaration:
    name: str
    value: str
    line: int


def _round(x: float) -> int:
    """Like Kotlin roundToInt: halves round up."""
    return int(math.floor(x + 0.5))


def _to_float(s: str) -> float | None:
    """Like toFloatOrNull: a finite decimal number, without '_'."""
    if "_" in s or not s.strip() == s:
        return None
    try:
        v = float(s)
    except ValueError:
        return None
    return v if math.isfinite(v) else None


def _clamp(v, lo, hi):
    return max(lo, min(hi, v))


def split_top_level(text: str, sep: str) -> list[str]:
    parts: list[str] = []
    cur: list[str] = []
    depth = 0
    quote: str | None = None
    for c in text:
        if quote is not None:
            cur.append(c)
            if c == quote:
                quote = None
        elif c == '"' or c == "'":
            quote = c
            cur.append(c)
        elif c == "(":
            depth += 1
            cur.append(c)
        elif c == ")":
            if depth > 0:
                depth -= 1
            cur.append(c)
        elif c == sep and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(c)
    parts.append("".join(cur))
    return parts


_IMPORTANT = re.compile(r"\s*!important\s*$", re.IGNORECASE)
_cache: dict[str, tuple[list[tuple[str, str]], list[str]]] = {}
_lock = threading.Lock()


def _parse(text: str):
    items: list[tuple[str, str]] = []
    warnings: list[str] = []
    for part in split_top_level(text, ";"):
        p = part.strip()
        if not p:
            continue
        colon = p.find(":")
        if colon <= 0:
            warnings.append(f"CSS: declaration without ':' was ignored ('{p}')")
            continue
        name = p[:colon].strip().lower()
        value = _IMPORTANT.sub("", p[colon + 1:].strip())
        if not value:
            warnings.append(f"CSS: property '{name}' has no value and was ignored")
            continue
        items.append((name, value))
    return items, warnings


def declarations(text: str, line: int, warn: Callable[[str, int], None]) -> list[CssDeclaration]:
    with _lock:
        parsed = _cache.get(text)
    if parsed is None:
        parsed = _parse(text)
        with _lock:
            if len(_cache) > 512:
                _cache.clear()
            _cache[text] = parsed
    items, warnings = parsed
    for w in warnings:
        warn(w, line)
    return [CssDeclaration(n, v, line) for n, v in items]


_OPAQUE = 0xFF000000

_NAMES = {
    "black": 0x000000, "white": 0xFFFFFF, "red": 0xFF0000, "green": 0x008000,
    "blue": 0x0000FF, "yellow": 0xFFFF00, "orange": 0xFFA500, "purple": 0x800080,
    "pink": 0xFFC0CB, "gray": 0x808080, "grey": 0x808080, "silver": 0xC0C0C0,
    "maroon": 0x800000, "navy": 0x000080, "teal": 0x008080, "lime": 0x00FF00,
    "aqua": 0x00FFFF, "cyan": 0x00FFFF, "magenta": 0xFF00FF, "fuchsia": 0xFF00FF,
    "olive": 0x808000, "brown": 0xA52A2A, "gold": 0xFFD700, "indigo": 0x4B0082,
    "violet": 0xEE82EE, "coral": 0xFF7F50, "crimson": 0xDC143C, "tomato": 0xFF6347,
    "salmon": 0xFA8072, "khaki": 0xF0E68C, "beige": 0xF5F5DC, "ivory": 0xFFFFF0,
    "lavender": 0xE6E6FA, "turquoise": 0x40E0D0, "darkgray": 0xA9A9A9,
    "darkgrey": 0xA9A9A9, "lightgray": 0xD3D3D3, "lightgrey": 0xD3D3D3,
    "darkblue": 0x00008B, "darkgreen": 0x006400, "darkred": 0x8B0000,
    "lightblue": 0xADD8E6, "lightgreen": 0x90EE90, "whitesmoke": 0xF5F5F5,
}

_REGEX_RGB = re.compile(r"^rgba?\(([^)]*)\)$")
_REGEX_LENGTH = re.compile(r"^(-?\d*\.?\d+)(px|rem)$", re.ASCII)
_HEXCHARS = set("0123456789abcdef")


def _pack(r: int, g: int, b: int, a: int) -> int:
    return (a << 24) | (r << 16) | (g << 8) | b


def color(text: str) -> int | None:
    t = text.strip().lower()
    if not t:
        return None
    if t == "transparent":
        return 0
    if t in _NAMES:
        return _NAMES[t] | _OPAQUE
    if t.startswith("#"):
        h = t[1:]
        if not h or any(ch not in _HEXCHARS for ch in h):
            return None
        dbl = lambda p: int(h[p], 16) * 17
        pair = lambda p: int(h[p:p + 2], 16)
        n = len(h)
        if n == 3:
            return _pack(dbl(0), dbl(1), dbl(2), 255)
        if n == 4:
            return _pack(dbl(0), dbl(1), dbl(2), dbl(3))
        if n == 6:
            return _pack(pair(0), pair(2), pair(4), 255)
        if n == 8:
            return _pack(pair(0), pair(2), pair(4), pair(6))
        return None

    m = _REGEX_RGB.match(t)
    if m is None:
        return None
    parts = [p.strip() for p in re.split(r"[, /]", m.group(1)) if p.strip()]
    if not 3 <= len(parts) <= 4:
        return None

    def channel(s: str) -> int | None:
        if s.endswith("%"):
            v = _to_float(s[:-1])
            return None if v is None else _round(v * 2.55)
        v = _to_float(s)
        return None if v is None else _round(v)

    r, g, b = channel(parts[0]), channel(parts[1]), channel(parts[2])
    if r is None or g is None or b is None:
        return None
    a = 255
    if len(parts) == 4:
        s = parts[3]
        v = _to_float(s[:-1]) if s.endswith("%") else _to_float(s)
        if v is None:
            return None
        if s.endswith("%"):
            v = v / 100.0
        a = _round(_clamp(v, 0.0, 1.0) * 255.0)
    return _pack(_clamp(r, 0, 255), _clamp(g, 0, 255), _clamp(b, 0, 255), a)


def length(text: str) -> Length | None:
    t = text.strip().lower()
    if t == "0":
        return Length.ZERO
    m = _REGEX_LENGTH.match(t)
    if m is None:
        return None
    n = _to_float(m.group(1))
    if n is None:
        return None
    return Length.rem(n) if m.group(2) == "rem" else Length.px(n)


def fraction(text: str) -> float | None:
    """A number from 0 to 1, or a percentage (opacity)."""
    t = text.strip().lower()
    if t.endswith("%"):
        v = _to_float(t[:-1])
        v = None if v is None else v / 100.0
    else:
        v = _to_float(t)
    return None if v is None else _clamp(v, 0.0, 1.0)


def size(text: str):
    t = text.strip().lower()
    if t == "auto":
        return AUTO
    if t.endswith("%"):
        v = _to_float(t[:-1])
        return None if v is None else Percent(v)
    ln = length(t)
    return None if ln is None else Dim(ln)
