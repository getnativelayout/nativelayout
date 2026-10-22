
from __future__ import annotations

SEP = "~"
_KEY_FIELDS = ("key", "id")
_HEX = "0123456789ABCDEF"


def key_of_item(item) -> str | None:
    """Chave automatica de um item: campo `key`, ou senao `id`. So texto e inteiros."""
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
    """Letras, digitos, `_`, `-` e `.` ficam; o resto vira %XX (nunca aparece `/`, `:` ou `~`)."""
    out = []
    for c in s.encode("utf-8"):
        plain = (97 <= c <= 122) or (65 <= c <= 90) or (48 <= c <= 57) or c in (95, 45, 46)
        out.append(chr(c) if plain else "%" + _HEX[c >> 4] + _HEX[c & 15])
    return "".join(out)


def token(key: str | None, index: int) -> str:
    """Token de uma repeticao: a chave, ou `%p` + posicao quando nao ha chave."""
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
