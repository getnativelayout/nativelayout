"""HTML parser of the core (port of HtmlParser.kt)."""
from __future__ import annotations

import re
import threading

from .errors import NativeLayoutError, extract_line
from .tree import Native, NativeStyle

_WHITESPACE = re.compile(r"\s+")
_CLASS_CACHE: dict[str, list[str]] = {}
_CLASS_LOCK = threading.Lock()
_HEX_RE = re.compile(r"^[0-9a-fA-F]+$")
_DEC_RE = re.compile(r"^[0-9]+$")
VOID_TAGS = frozenset({"br", "hr", "img", "input", "meta", "link"})


class MarkupError(NativeLayoutError):
    def __init__(self, message: str, line: int, source: str,
                 word: str | None = None, suggestion: str | None = None):
        super().__init__(f"markup: {message}", line, extract_line(source, line), word, suggestion)


class HtmlNode:
    line: int


class HtmlText(HtmlNode):
    def __init__(self, text: str, line: int):
        self.text = text
        self.line = line


class HtmlElement(HtmlNode):
    def __init__(self, tag: str, attributes: dict[str, str], children: list[HtmlNode], line: int):
        self.tag = tag
        self.attributes = attributes
        self.children = children
        self.line = line
        self.chunk_text: str | None = None

    @property
    def id(self) -> str | None:
        v = self.attributes.get("id")
        if v is None:
            return None
        v = v.strip()
        return v or None

    @property
    def classes(self) -> list[str]:
        raw = self.attributes.get("class")
        if raw is None:
            return []
        with _CLASS_LOCK:
            hit = _CLASS_CACHE.get(raw)
            if hit is not None:
                return hit
            parsed = [p for p in _WHITESPACE.split(raw) if p]
            if len(_CLASS_CACHE) > 1024:
                _CLASS_CACHE.clear()
            _CLASS_CACHE[raw] = parsed
            return parsed


class Chunk:
    """An element with a key whose text did not change: it is not parsed again."""
    def __init__(self, text: str, element: HtmlElement, native: Native,
                 parent_style: NativeStyle, ids: list[tuple[str, Native]]):
        self.text = text
        self.element = element
        self.native = native
        self.parent_style = parent_style
        self.ids = ids


class HtmlCachedElement(HtmlElement):
    def __init__(self, chunk: Chunk, line: int):
        super().__init__(chunk.element.tag, chunk.element.attributes,
                         chunk.element.children, line)
        self.chunk = chunk


class ChunkCache:
    def __init__(self):
        self._map: dict[str, Chunk] = {}
        self._variables: dict[str, str] | None = None
        self._lock = threading.RLock()

    def use_for(self, variables: dict[str, str]) -> None:
        with self._lock:
            if self._variables != variables:
                self._map.clear()
                self._variables = variables

    def get(self, key: str) -> Chunk | None:
        with self._lock:
            return self._map.get(key)

    def put(self, key: str, chunk: Chunk) -> None:
        with self._lock:
            if len(self._map) >= 20000:
                self._map.clear()
            self._map[key] = chunk

    def clear(self) -> None:
        with self._lock:
            self._map.clear()


class _Open:
    def __init__(self, tag: str, attributes: dict[str, str], line: int, start: int):
        self.tag = tag
        self.attributes = attributes
        self.line = line
        self.start = start
        self.children: list[HtmlNode] = []


def _key_of(attributes: dict[str, str]) -> str | None:
    v = attributes.get("key")
    if v is None:
        return None
    v = v.strip()
    return v or None


class HtmlParser:
    def __init__(self, html: str, start_line: int, source: str,
                 chunks: ChunkCache | None = None):
        self.html = html
        self.source = source
        self.chunks = chunks
        self.i = 0
        self.line = start_line
        self.stack: list[_Open] = []
        self.root: list[HtmlNode] = []

    def _err(self, message: str, at: int | None = None, word: str | None = None):
        return MarkupError(message, self.line if at is None else at, self.source, word)

    def _target(self) -> list[HtmlNode]:
        return self.root if not self.stack else self.stack[-1].children

    def _advance_to(self, end: int) -> None:
        self.line += self.html.count("\n", self.i, end)
        self.i = end

    def parse(self) -> list[HtmlNode]:
        html = self.html
        n = len(html)
        while self.i < n:
            i = self.i
            c = html[i]
            if html.startswith("<!--", i):
                end = html.find("-->", i + 4)
                if end < 0:
                    raise self._err("unclosed HTML comment: missing '-->'")
                self._advance_to(end + 3)
            elif html.startswith("<!", i):
                end = html.find(">", i)
                if end < 0:
                    raise self._err("unclosed '<!': missing '>'")
                self._advance_to(end + 1)
            elif html.startswith("</", i):
                self._read_close()
            elif c == "<" and i + 1 < n and html[i + 1].isalpha():
                self._read_open()
            else:
                self._read_text()
        if self.stack:
            a = self.stack[-1]
            raise self._err(f"<{a.tag}> is never closed (opened on line {a.line})", a.line, f"<{a.tag}")
        return self.root

    def _read_text(self) -> None:
        html = self.html
        n = len(html)
        j = self.i + 1
        while j < n:
            if html[j] == "<" and j + 1 < n and (html[j + 1].isalpha() or html[j + 1] in "/!"):
                break
            j += 1
        raw = html[self.i:j]
        l = self.line
        self._advance_to(j)
        if raw:
            self._target().append(HtmlText(self._decode(raw), l))

    def _read_close(self) -> None:
        tag_line = self.line
        end = self.html.find(">", self.i)
        if end < 0:
            raise self._err("unclosed '</': missing '>'")
        tag = self.html[self.i + 2:end].strip().lower()
        self._advance_to(end + 1)
        if tag in VOID_TAGS:
            return
        if not self.stack:
            raise self._err(f"</{tag}> has no matching opening tag", tag_line, f"</{tag}")
        top = self.stack[-1]
        if top.tag != tag:
            raise self._err(f"expected </{top.tag}> (opened on line {top.line}) before </{tag}>",
                            tag_line, f"</{tag}")
        self.stack.pop()
        element = HtmlElement(top.tag, top.attributes, list(top.children), top.line)
        if self.chunks is not None and _key_of(top.attributes) is not None:
            element.chunk_text = self.html[top.start:end + 1]
        self._target().append(element)

    def _read_open(self) -> None:
        html = self.html
        n = len(html)
        tag_start = self.i
        tag_line = self.line
        j = self.i + 1
        name_start = j
        while j < n and (html[j].isalnum() or html[j] == "-"):
            j += 1
        tag = html[name_start:j].lower()
        attributes: dict[str, str] = {}
        self_close = False

        while True:
            while j < n and html[j].isspace():
                j += 1
            if j >= n:
                raise self._err(f"unclosed tag <{tag}>: missing '>'", tag_line, f"<{tag}")
            ch = html[j]
            if ch == ">":
                j += 1
                break
            if ch == "/" and j + 1 < n and html[j + 1] == ">":
                self_close = True
                j += 2
                break
            if ch == "/":
                j += 1
                continue

            attr_start = j
            while j < n and not html[j].isspace() and html[j] not in "=>/":
                j += 1
            if j == attr_start:
                raise self._err(f"invalid attribute in tag <{tag}>", tag_line, f"<{tag}")
            name = html[attr_start:j].lower()
            while j < n and html[j].isspace():
                j += 1

            value = ""
            if j < n and html[j] == "=":
                j += 1
                while j < n and html[j].isspace():
                    j += 1
                if j < n and html[j] in "\"'":
                    quote = html[j]
                    end = html.find(quote, j + 1)
                    if end < 0:
                        raise self._err(f"attribute '{name}' has an unclosed quote", tag_line, name)
                    value = html[j + 1:end]
                    j = end + 1
                else:
                    val_start = j
                    while j < n and not html[j].isspace() and html[j] != ">":
                        j += 1
                    value = html[val_start:j]
            if name not in attributes:
                attributes[name] = self._decode(value)
        self._advance_to(j)

        if tag == "script":
            raise self._err("<script> is not allowed", tag_line, "<script")
        if tag == "style":
            raise self._err('<style> is not supported yet; use style="..."', tag_line, "<style")
        if tag in VOID_TAGS or self_close:
            self._target().append(HtmlElement(tag, attributes, [], tag_line))
            return
        key = _key_of(attributes) if self.chunks is not None else None
        hit = self.chunks.get(tag + "|" + key) if key is not None else None
        if hit is not None and html.startswith(hit.text, tag_start):
            self._advance_to(tag_start + len(hit.text))
            self._target().append(HtmlCachedElement(hit, tag_line))
        else:
            self.stack.append(_Open(tag, attributes, tag_line, tag_start))

    @staticmethod
    def _decode(t: str) -> str:
        if "&" not in t:
            return t
        out: list[str] = []
        k = 0
        n = len(t)
        while k < n:
            c = t[k]
            if c == "&":
                end = t.find(";", k)
                if k + 2 <= end <= k + 10:
                    name = t[k + 1:end]
                    r: str | None = None
                    if name == "amp":
                        r = "&"
                    elif name == "lt":
                        r = "<"
                    elif name == "gt":
                        r = ">"
                    elif name == "quot":
                        r = '"'
                    elif name == "apos":
                        r = "'"
                    elif name == "nbsp":
                        r = "\u00A0"
                    elif name[:2] in ("#x", "#X"):
                        d = name[2:]
                        if _HEX_RE.match(d) and int(d, 16) <= 0x10FFFF:
                            r = chr(int(d, 16))
                    elif name.startswith("#"):
                        d = name[1:]
                        if _DEC_RE.match(d) and int(d) <= 0x10FFFF:
                            r = chr(int(d))
                    if r is not None:
                        out.append(r)
                        k = end + 1
                        continue
            out.append(c)
            k += 1
        return "".join(out)
