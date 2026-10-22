"""Identity of the repetitions of a `for` (port of Scopes.kt, section 5.7)."""
from __future__ import annotations

SEP = "~"
_KEY_FIELDS = ("key", "id")
_HEX = "0123456789ABCDEF"


def key_of_item(item) -> str | None:
    """Automatic key of an item: its field `key`, or else its field `id`. Only text and whole numbers."""
    if not isinstance(item, dict):
        return None
    for field in _KEY_FIELDS:
        v = item.get(field)
        if isinstance(v, str):
            t = v.strip()
            if t:
                return t
        elif isinstance(v, int) and not isinstance(v, bool):
            return str(v)
    return None


def encode(s: str) -> str:
    """Letters, digits, `_`, `-` and `.` stay; the rest becomes %XX (`/`, `:` and `~` never appear)."""
    out = []
    for c in s.encode("utf-8"):
        plain = (97 <= c <= 122) or (65 <= c <= 90) or (48 <= c <= 57) or c in (95, 45, 46)
        out.append(chr(c) if plain else "%" + _HEX[c >> 4] + _HEX[c & 15])
    return "".join(out)


def token(key: str | None, index: int) -> str:
    """Token of a repetition: its key, or `%p` + the position when there is no key."""
    return encode(key) if key is not None else f"%p{index}"


def variable_of(segment: str) -> str:
    return segment.split(":", 1)[0]


def token_of(segment: str) -> str:
    return segment.split(":", 1)[1] if ":" in segment else segment


def same_item(given, held, tok: str) -> bool:
    if given is held or given == held:
        return True
    key = key_of_item(given)
    if key is None:
        return False
    return encode(key) == tok
