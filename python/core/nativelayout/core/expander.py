"""Expander: `.nl` -> plain HTML with {value}, if/for/include and styles (port of Expander.kt)."""
from __future__ import annotations

import math
import re
import threading
from dataclasses import dataclass, field
from decimal import Decimal

from . import scopes as Scopes
from .errors import NativeLayoutError, extract_line
from .layout_engine import LayoutWarning


@dataclass
class ExpandedScreen:
    html: str
    line_map: list[int]
    warnings: list[LayoutWarning] = field(default_factory=list)
    # The item of each repetition of a `for` that has an id or a class, by path (`p:7`, `p:7/o:2`).
    items: dict[str, object] = field(default_factory=dict)


class ExpandError(NativeLayoutError):
    def __init__(self, message: str, line: int, source: str,
                 word: str | None = None, suggestion: str | None = None):
        super().__init__(message, line, extract_line(source, line), word, suggestion)


def _rx(p: str) -> re.Pattern:
    return re.compile(p, re.ASCII)


KIND_TAG, KIND_CLASS, KIND_ID = 1, 2, 3
STATE_NONE, STATE_PRESSED, STATE_DISABLED, STATE_FOCUS = 0, 1, 2, 3
STATE_ATTRIBUTES = ("style-pressed", "style-disabled", "style-focus")
MAX_INCLUDE_LEVELS = 8
VOID_NAMES = frozenset({"br", "hr", "img", "input", "meta", "link"})

INCLUDE_START = _rx(r"^<include(?=[\s/>])")
KEY_ATTR = _rx(r'\skey\s*=\s*"([^"]*)"')
HAS_KEY = _rx(r"\skey\s*=")
FIELD_OR_ID = _rx(r"<(?:input|textarea)\b|\sid\s*=")
TAG_RE = _rx(r"<[^<>]*>")
TAG_NAME_RE = _rx(r"^<([A-Za-z][A-Za-z0-9-]*)")
ATTR_RE = _rx(r"""([A-Za-z_:][-A-Za-z0-9_:.]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?""")
BRACE_RE = _rx(r"\\[{}]|\{([^{}]*)\}")
NAME_RE = _rx(r"[A-Za-z_][A-Za-z0-9_.]*")
SINGLE_VALUE_RE = _rx(r"\{\s*([A-Za-z_][A-Za-z0-9_.]*)\s*\}")
VAR_RE = _rx(r"var\(\s*(--[A-Za-z0-9_-]+)\s*\)")
RULE_RE = _rx(r"([^{}]+)\{([^{}]*)\}")
COMMENT_RE = _rx(r"/\*[\s\S]*?\*/")
FOR_RE = _rx(r"for\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+([A-Za-z_][A-Za-z0-9_.]*):")
KEYWORD_RE = _rx(r"^(if|elif|for)\s")
ELSE_SPACE_RE = _rx(r"else\s+:")
TOKEN_RE = _rx(r'==|!=|<=|>=|<|>|\(|\)|"[^"]*"|-?\d+(?:\.\d+)?|[A-Za-z_][A-Za-z0-9_.]*')
TAG_SELECTOR = _rx(r"[A-Za-z][A-Za-z0-9]*")
CLASS_SELECTOR = _rx(r"\.[A-Za-z_][A-Za-z0-9_-]*")
ID_SELECTOR = _rx(r"#[A-Za-z_][A-Za-z0-9_-]*")
COMPARATORS = frozenset({"==", "!=", "<", ">", "<=", ">="})
WS_RE = _rx(r"\s+")

_MISSING = object()


def _first(m: re.Match, *idx: int) -> str | None:
    for i in idx:
        g = m.group(i)
        if g is not None:
            return g
    return None


# ---------------------------------------------------------------- model
class Rule:
    def __init__(self, kind, name, declarations, order, line, file, link_line, state=STATE_NONE):
        self.kind = kind
        self.name = name
        self.declarations = declarations
        self.order = order
        self.line = line
        self.file = file
        self.link_line = link_line
        self.state = state
        self.resolved: list[tuple[str, str]] = []


class Attr:
    __slots__ = ("key", "value")

    def __init__(self, key: str, value: str | None):
        self.key = key
        self.value = value


class Scope:
    def __init__(self, parent: "Scope | None", vars_: dict, path: str | None = None):
        self.parent = parent
        self.vars = vars_
        self.path = path if path is not None else (parent.path if parent is not None else "")

    def find(self, name: str):
        s = self
        while s is not None:
            if name in s.vars:
                return s.vars[name]
            s = s.parent
        return _MISSING

    def has(self, name: str) -> bool:
        return name in self.vars or (self.parent.has(name) if self.parent is not None else False)


class Node:
    pass


class TextNode(Node):
    def __init__(self, text: str, line: int):
        self.text = text
        self.line = line
        self.parts: list | None = None


class Branch:
    def __init__(self, condition: str | None, line: int):
        self.condition = condition
        self.line = line
        self.nodes: list[Node] = []
        self.tokens: list[str] | None = None


class IfNode(Node):
    def __init__(self, line: int):
        self.line = line
        self.branches: list[Branch] = []


class ForNode(Node):
    def __init__(self, line: int, variable: str, list_name: str):
        self.line = line
        self.variable = variable
        self.list_name = list_name
        self.nodes: list[Node] = []


class ParamPlan:
    def __init__(self, name, constant, single, parts):
        self.name = name
        self.constant = constant
        self.single = single
        self.parts = parts


class IncludeNode(Node):
    def __init__(self, line, component, params, key):
        self.line = line
        self.component = component
        self.params = params
        self.key = key


class Frame:
    def __init__(self, open_line: int, nodes: list, if_node: IfNode | None):
        self.open_line = open_line
        self.nodes = nodes
        self.if_node = if_node


class Control:
    def __init__(self, type_: str, argument: str, variable: str = ""):
        self.type = type_
        self.argument = argument
        self.variable = variable


@dataclass
class FileLayout:
    links: list[tuple[str, int]]
    style_start: int
    style_end: int
    body_start: int
    body_end: int


class Component:
    def __init__(self, name, source, lines, layout, include_line):
        self.name = name
        self.source = source
        self.lines = lines
        self.layout = layout
        self.include_line = include_line
        self.nodes: list[Node] = []


class IncludeTag:
    def __init__(self, attrs: list[tuple[str, str | None]]):
        self.attrs = attrs

    def get(self, name: str) -> str | None:
        n = name.lower()
        for k, v in self.attrs:
            if k.lower() == n:
                return v
        return None


class Part:
    pass


class Lit(Part):
    def __init__(self, s: str):
        self.s = s


class Val(Part):
    def __init__(self, inner: str):
        self.inner = inner
        self.name = inner.strip()
        self.ok = NAME_RE.fullmatch(self.name) is not None
        self.path = self.name.split(".")


class AttrPlan:
    def __init__(self, orig, key, value, brace, single, text_parts):
        self.orig = orig
        self.key = key
        self.value = value
        self.brace = brace
        self.single = single
        self.text_parts = text_parts


class DynTag(Part):
    def __init__(self, name, lower, self_close, rule_style, attrs, state_styles):
        self.name = name
        self.lower = lower
        self.self_close = self_close
        self.rule_style = rule_style
        self.attrs = attrs
        self.state_styles = state_styles
        self.pre: str | None = None


class Prepared:
    def __init__(self, rules, nodes):
        self.rules = rules
        self.nodes = nodes


# ---------------------------------------------------------------- cache
_cache: dict[str, Prepared] = {}
_cache_lock = threading.Lock()


class Expander:
    @staticmethod
    def expand(source: str, data: dict, resolve=None, dark: bool = False) -> ExpandedScreen:
        return _ScreenExpander(source, data, resolve or (lambda name: None), dark).expand()

    @staticmethod
    def clear_cache() -> None:
        with _cache_lock:
            _cache.clear()


def _cached(key: str):
    with _cache_lock:
        return _cache.get(key)


def _store(key: str, prepared: Prepared) -> None:
    with _cache_lock:
        if len(_cache) >= 32:
            _cache.clear()
        _cache[key] = prepared


# ---------------------------------------------------------------- utilities
def _escape_html(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def _is_num(v) -> bool:
    return isinstance(v, (int, float)) and not isinstance(v, bool)


def _format_number(n) -> str:
    if isinstance(n, float):
        if math.isfinite(n) and n == math.floor(n) and abs(n) < 9.2e18:
            return str(int(n))
        if not math.isfinite(n):
            return "NaN" if math.isnan(n) else ("Infinity" if n > 0 else "-Infinity")
        a = abs(n)
        if a >= 1e7 or (a < 1e-3 and a != 0.0):
            sign, digits, exp = Decimal(repr(n)).normalize().as_tuple()
            ds = "".join(map(str, digits))
            e10 = exp + len(ds) - 1
            return ("-" if sign else "") + ds[0] + "." + (ds[1:] or "0") + "E" + str(e10)
        return repr(n)
    return str(n)


def _singular(name: str) -> str:
    last = name.rpartition(".")[2]
    return last[:-1] if len(last) > 1 and last.endswith("s") else "item"


def _equals(a, b) -> bool:
    if isinstance(a, bool) != isinstance(b, bool):
        return False
    return a == b


def _join_path(frm: str, rel: str) -> str:
    d = frm.rpartition("/")[0] if "/" in frm else ""
    return rel if not d else f"{d}/{rel}"


def _merge_lits(lst: list[Part]) -> list[Part]:
    out: list[Part] = []
    for p in lst:
        if isinstance(p, Lit) and out and isinstance(out[-1], Lit):
            out[-1] = Lit(out[-1].s + p.s)
        else:
            out.append(p)
    return out


# ---------------------------------------------------------------- expander
class _ScreenExpander:
    def __init__(self, source: str, data: dict, resolve, dark: bool):
        self.source = source
        self.data = data
        self.resolve = resolve
        self.dark = dark
        self.lines = source.split("\n")
        self.rules: list[Rule] = []
        self.variables: dict[str, str] = {}
        self.rule_order = 0
        self.out: list[str] = []
        self.out_lines: list[int] = []
        self.warnings: list[LayoutWarning] = []
        self.current_component: str | None = None
        self.current_include_line = 0
        self.components: dict[str, Component] = {}
        self.scope_counter = 0
        self.marker_hits = 0
        self.auto_keyed: set[int] = set()
        self.scope_log: list[tuple[str, object]] = []

    # -- errors
    def err(self, message: str, line: int, word: str | None = None, suggestion: str | None = None):
        c = self.current_component
        if c is None:
            return ExpandError(message, line, self.source, word, suggestion)
        return ExpandError(f"in '{c}': {message}", self.current_include_line, self.source, word, suggestion)

    def eff_line(self, line: int) -> int:
        return self.current_include_line if self.current_component is not None else line

    def style_error(self, message, line, word, file, link_line):
        if file is None:
            return self.err(f"style: {message}", line, word)
        return self.err(f"style: in '{file}' line {line}: {message}", link_line, word)

    def main_fail(self, message, line, word):
        raise self.err(message, line, word)

    def component_fail(self, name: str, include_line: int):
        def fail(message, _line, word):
            raise ExpandError(f"in '{name}': {message}", include_line, self.source, word)
        return fail

    def include_error(self, file_name: str, message: str, line: int):
        if not file_name:
            return ExpandError(message, line, self.source)
        return ExpandError(f"in '{file_name}': {message}", line, self.source)

    # -- .nl file
    @staticmethod
    def _link_href(tag: str, line: int, fail) -> str:
        attrs: dict[str, str] = {}
        inner = tag.removeprefix("<link").removesuffix(">").removesuffix("/")
        for a in ATTR_RE.finditer(inner):
            value = _first(a, 2, 3, 4)
            if value is None:
                continue
            attrs[a.group(1).lower()] = value
        href = attrs.get("href")
        rel = attrs.get("rel")
        if href is None or href.strip() == "" or (rel.lower() if rel is not None else None) != "stylesheet":
            fail('markup: <link> needs rel="stylesheet" and a href', line, None)
        if href.startswith("/") or ".." in href or "://" in href:
            fail(f"style: the file '{href}' is not allowed", line, href)
        if not href.endswith(".css"):
            fail(f"style: the file '{href}' must be a .css file", line, href)
        return href

    def _scan_file(self, file_lines: list[str], fail) -> FileLayout:
        first = 0
        while first < len(file_lines) and file_lines[first].strip() == "":
            first += 1
        last = len(file_lines) - 1
        while last >= 0 and file_lines[last].strip() == "":
            last -= 1
        if (first >= last or file_lines[first].strip() != "<nativelayout>"
                or file_lines[last].strip() != "</nativelayout>"):
            fail("the file must start with <nativelayout> and end with </nativelayout>",
                 first + 1 if first < len(file_lines) else 1, None)

        style_start = style_end = body_start = body_end = -1
        links: list[tuple[str, int]] = []
        i = first + 1
        in_comment = False
        while i < last:
            t = file_lines[i].strip()
            if in_comment:
                if "-->" in t:
                    in_comment = False
            elif t == "":
                pass
            elif t.startswith("<!--"):
                if "-->" not in t:
                    in_comment = True
            elif t.startswith("<script"):
                fail("markup: <script> is not allowed", i + 1, None)
            elif t == "<style>":
                if style_start >= 0:
                    fail("only one <style> block is allowed", i + 1, None)
                j = i + 1
                while j < last and file_lines[j].strip() != "</style>":
                    j += 1
                if j >= last:
                    fail("<style> is never closed with </style>", i + 1, None)
                style_start = i + 1
                style_end = j
                i = j
            elif t.startswith("<link"):
                if body_start >= 0:
                    fail("a <link> must come before <body>", i + 1, None)
                links.append((self._link_href(t, i + 1, fail), i + 1))
            elif t.startswith("<body"):
                if body_start >= 0:
                    fail("the file must have exactly one <body>", i + 1, None)
                j = i + 1
                while j < last and file_lines[j].strip() != "</body>":
                    j += 1
                if j >= last:
                    fail("<body> is never closed with </body>", i + 1, None)
                body_start = i
                body_end = j
                i = j
            else:
                fail("only <style> and <body> are allowed inside <nativelayout>", i + 1, None)
            i += 1
        if body_start < 0:
            fail("the file must have exactly one <body>", first + 1, None)
        return FileLayout(links, style_start, style_end, body_start, body_end)

    # -- components: <include>
    def _include_tag_of(self, raw: str, report_line: int, file_name: str) -> IncludeTag | None:
        t = raw.strip()
        if INCLUDE_START.search(t) is None:
            return None
        m = TAG_RE.search(t)
        if m is None or m.start() != 0 or m.end() - 1 != len(t) - 1:
            raise self.include_error(file_name, "markup: <include> must be alone on its line", report_line)
        inner = t[len("<include"):len(t) - 1].removesuffix("/")
        attrs = [(a.group(1), _first(a, 2, 3, 4)) for a in ATTR_RE.finditer(inner)]
        return IncludeTag(attrs)

    def _component_path(self, frm: str, src: str, line: int) -> str:
        if src.strip() == "" or src.startswith("/") or ".." in src or "://" in src:
            raise self.include_error(frm, f"include: the file '{src}' is not allowed", line)
        if not src.endswith(".nl"):
            raise self.include_error(frm, f"include: the file '{src}' must be a .nl file", line)
        return _join_path(frm, src)

    def _collect_includes(self, file_name: str, file_lines: list[str], layout: FileLayout,
                          chain: list[str], main_line: int) -> None:
        for idx in range(layout.body_start + 1, layout.body_end):
            line = idx + 1 if not file_name else main_line
            tag = self._include_tag_of(file_lines[idx], line, file_name)
            if tag is None:
                continue
            src = tag.get("src")
            if src is None or src.strip() == "":
                raise self.include_error(file_name, "markup: <include> requires a src attribute", line)
            name = self._component_path(file_name, src, line)
            if name == file_name or name in chain:
                raise self.include_error(file_name, f"include: '{name}' includes itself", line)
            if len(chain) >= MAX_INCLUDE_LEVELS:
                raise self.include_error(
                    file_name, f"include: too many nested includes (the limit is {MAX_INCLUDE_LEVELS})", line)
            if name in self.components:
                continue
            text = self.resolve(name)
            if text is None:
                raise self.include_error(file_name, f"include: the file '{name}' was not found", line)
            comp_lines = text.split("\n")
            comp_layout = self._scan_file(comp_lines, self.component_fail(name, line))
            self.components[name] = Component(name, text, comp_lines, comp_layout, line)
            self._collect_includes(name, comp_lines, comp_layout, chain + [name], line)

    def _param_plan(self, name: str, value: str | None) -> ParamPlan:
        if value is None:
            return ParamPlan(name, True, None, None)
        if "{" not in value:
            return ParamPlan(name, value, None, None)
        m = SINGLE_VALUE_RE.fullmatch(value.strip())
        if m is not None:
            return ParamPlan(name, None, m.group(1), None)
        return ParamPlan(name, None, None, self._compile_value(value))

    def _make_include(self, tag: IncludeTag, file_name: str, line: int) -> IncludeNode:
        src = tag.get("src")
        if src is None:
            raise self.err("markup: <include> requires a src attribute", line)
        component = self.components.get(_join_path(file_name, src))
        if component is None:
            raise self.err(f"include: the file '{src}' was not found", line)
        params: list[ParamPlan] = []
        key = None
        for name, value in tag.attrs:
            n = name.lower()
            if n == "src":
                pass
            elif n == "key":
                key = self._compile_value(value) if value is not None else None
            else:
                params.append(self._param_plan(name, value))
        return IncludeNode(line, component, params, key)

    # -- input
    def expand(self) -> ExpandedScreen:
        layout = self._scan_file(self.lines, self.main_fail)
        self._collect_includes("", self.lines, layout, [], 0)
        style_start, style_end = layout.style_start, layout.style_end

        component_loaded: set[str] = set()
        component_sheets: list[tuple[str, int, str]] = []
        for comp in self.components.values():
            for href, _ in comp.layout.links:
                path = _join_path(comp.name, href)
                if path in component_loaded:
                    continue
                component_loaded.add(path)
                css = self.resolve(path)
                if css is None:
                    raise ExpandError(f"style: the file '{path}' was not found", comp.include_line, self.source, path)
                if "@import" in css:
                    raise ExpandError("style: a style file cannot import another file",
                                      comp.include_line, self.source, path)
                component_sheets.append((path, comp.include_line, css))

        loaded: set[str] = set()
        sheets: list[tuple[str, int, str]] = []
        for href, link_line in layout.links:
            if href in loaded:
                continue
            loaded.add(href)
            css = self.resolve(href)
            if css is None:
                raise self.err(f"style: the file '{href}' was not found", link_line, href)
            if "@import" in css:
                raise self.err("style: a style file cannot import another file", link_line, href)
            sheets.append((href, link_line, css))

        key = ["d" if self.dark else "l", "\x01", self.source]
        for comp in self.components.values():
            key += ["\x02", comp.name, "\x03", comp.source]
        for s in component_sheets:
            key += ["\x00", s[2]]
        key.append("\x04")
        for s in sheets:
            key += ["\x00", s[2]]
        cache_key = "".join(key)

        hit = _cached(cache_key)
        if hit is not None:
            prepared = hit
            self.rules = hit.rules
        else:
            for s in component_sheets:
                self._parse_style(s[2], 1, s[0], s[1])
            for comp in self.components.values():
                l = comp.layout
                if l.style_start >= 0:
                    self._parse_style("\n".join(comp.lines[l.style_start:l.style_end]),
                                      l.style_start + 1, comp.name, comp.include_line)
            for s in sheets:
                self._parse_style(s[2], 1, s[0], s[1])
            if style_start >= 0:
                self._parse_style("\n".join(self.lines[style_start:style_end]), style_start + 1)
            self._resolve_variables()
            for comp in self.components.values():
                self.current_component = comp.name
                self.current_include_line = comp.include_line
                comp.nodes = self._build_nodes(comp.lines, comp.layout.body_start,
                                               comp.layout.body_end, comp.name)
            self.current_component = None
            prepared = Prepared(self.rules, self._build_nodes(self.lines, layout.body_start,
                                                              layout.body_end, ""))
            _store(cache_key, prepared)
        self._emit(prepared.nodes, Scope(None, self.data))
        return ExpandedScreen("\n".join(self.out), self.out_lines, list(self.warnings), dict(self.scope_log))

    # -- styles
    def _flatten_media(self, text: str, first_line: int, file, link_line) -> str:
        t = text
        while True:
            at = t.find("@media")
            if at < 0:
                return t
            line = first_line + t.count("\n", 0, at)
            op = t.find("{", at)
            if op < 0:
                raise self.style_error("'@media' without '{'", line, "@media", file, link_line)
            depth = 0
            close = -1
            i = op
            while i < len(t):
                c = t[i]
                if c == "{":
                    depth += 1
                elif c == "}":
                    depth -= 1
                    if depth == 0:
                        close = i
                        break
                i += 1
            if close < 0:
                raise self.style_error("the '@media' block is never closed", line, "@media", file, link_line)
            condition = WS_RE.sub(" ", t[at + 6:op].strip().lower())
            norm = condition.replace(": ", ":")
            if norm == "(prefers-color-scheme:dark)":
                include = self.dark
            elif norm == "(prefers-color-scheme:light)":
                include = not self.dark
            else:
                raise self.style_error(
                    f"the '@media {condition}' is not supported "
                    "(use '@media (prefers-color-scheme: dark)' or '... light')",
                    line, "@media", file, link_line)
            replacement = t[op + 1:close] if include else "".join(c for c in t[at:close + 1] if c == "\n")
            t = t[:at] + replacement + t[close + 1:]

    def _parse_style(self, text: str, first_line: int, file: str | None = None, link_line: int = 0) -> None:
        commentless = COMMENT_RE.sub(lambda m: "".join(c for c in m.group(0) if c == "\n"), text)
        clean = self._flatten_media(commentless, first_line, file, link_line)
        for m in RULE_RE.finditer(clean):
            selector_text = m.group(1)
            offset = m.start(1) + (len(selector_text) - len(selector_text.lstrip()))
            line = first_line + clean.count("\n", 0, offset)
            declarations: list[tuple[str, str]] = []
            for part in m.group(2).split(";"):
                idx = part.find(":")
                if idx < 0:
                    continue
                name = part[:idx].strip().lower()
                value = part[idx + 1:].replace("!important", "").strip()
                if not name or not value:
                    continue
                declarations.append((name, value))
            for raw in selector_text.split(","):
                full = raw.strip()
                selector = full
                state = STATE_NONE
                colon = full.find(":")
                if colon > 0:
                    s = full[colon + 1:].lower()
                    if s in ("pressed", "active"):
                        state = STATE_PRESSED
                    elif s == "disabled":
                        state = STATE_DISABLED
                    elif s == "focus":
                        state = STATE_FOCUS
                    else:
                        raise self.style_error(
                            f"the selector '{full}' is not supported (states: :pressed, :disabled, :focus)",
                            line, full, file, link_line)
                    selector = full[:colon]
                if selector == ":root" and state == STATE_NONE:
                    for d in declarations:
                        if d[0].startswith("--"):
                            self.variables[d[0]] = d[1]
                elif TAG_SELECTOR.fullmatch(selector):
                    self.rules.append(Rule(KIND_TAG, selector.lower(), declarations, self.rule_order, line, file, link_line, state))
                    self.rule_order += 1
                elif CLASS_SELECTOR.fullmatch(selector):
                    self.rules.append(Rule(KIND_CLASS, selector[1:], declarations, self.rule_order, line, file, link_line, state))
                    self.rule_order += 1
                elif ID_SELECTOR.fullmatch(selector):
                    self.rules.append(Rule(KIND_ID, selector[1:], declarations, self.rule_order, line, file, link_line, state))
                    self.rule_order += 1
                else:
                    raise self.style_error(f"the selector '{full}' is not supported", line, full, file, link_line)

    def _substitute_vars(self, value: str, rule: Rule) -> str:
        def rep(m):
            name = m.group(1)
            if name not in self.variables:
                raise self.style_error(f"the variable '{name}' is not defined in ':root'",
                                       rule.line, name, rule.file, rule.link_line)
            return self.variables[name]
        return VAR_RE.sub(rep, value)

    def _resolve_variables(self) -> None:
        for rule in self.rules:
            rule.resolved = [(n, self._substitute_vars(v, rule))
                             for n, v in rule.declarations if not n.startswith("--")]

    # -- control lines
    def _parse_control(self, t: str, line: int) -> Control | None:
        if t == "end":
            return Control("end", "")
        if t == "else:":
            return Control("else", "")
        if ELSE_SPACE_RE.fullmatch(t):
            raise self.err("write 'else:' without a space before ':'", line)
        km = KEYWORD_RE.match(t)
        if km is None:
            return None
        keyword = km.group(1)
        if not t.endswith(":"):
            return None
        body = t[:-1]
        if body.endswith(" ") or body.endswith("\t"):
            raise self.err(f"write '{body.rstrip()}:' without a space before ':'", line)
        if keyword == "for":
            m = FOR_RE.fullmatch(t)
            if m is None:
                raise self.err(f"invalid 'for': '{t}'", line, None, "write 'for item in list:'")
            return Control("for", m.group(2), m.group(1))
        condition = body[len(keyword):].strip()
        if not condition:
            raise self.err("invalid condition: ''", line)
        return Control(keyword, condition)

    def _build_nodes(self, file_lines: list[str], frm: int, to: int, file_name: str) -> list[Node]:
        root: list[Node] = []
        stack: list[Frame] = [Frame(0, root, None)]
        for idx in range(frm, to + 1):
            line = idx + 1
            edge = idx == frm or idx == to
            control = None if edge else self._parse_control(file_lines[idx].strip(), line)
            top = stack[-1]
            if control is None:
                include = None if edge else self._include_tag_of(
                    file_lines[idx], line if not file_name else self.current_include_line, file_name)
                if include is not None:
                    top.nodes.append(self._make_include(include, file_name, line))
                else:
                    top.nodes.append(TextNode(file_lines[idx], line))
                continue
            if control.type == "if":
                node = IfNode(line)
                branch = Branch(control.argument, line)
                node.branches.append(branch)
                top.nodes.append(node)
                stack.append(Frame(line, branch.nodes, node))
            elif control.type in ("elif", "else"):
                if_node = top.if_node
                if if_node is None:
                    raise self.err(f"'{control.type}' has no matching 'if'", line)
                if if_node.branches[-1].condition is None:
                    raise self.err(f"'{control.type}' has no matching 'if'", line)
                branch = Branch(None if control.type == "else" else control.argument, line)
                if_node.branches.append(branch)
                top.nodes = branch.nodes
            elif control.type == "for":
                node = ForNode(line, control.variable, control.argument)
                top.nodes.append(node)
                stack.append(Frame(line, node.nodes, None))
            else:
                if len(stack) == 1:
                    raise self.err("'end' has no matching 'if' or 'for'", line)
                stack.pop()
        if len(stack) > 1:
            open_ = stack[-1]
            word = "if" if open_.if_node is not None else "for"
            raise self.err(f"the '{word}' opened on line {open_.open_line} is never closed with 'end'",
                           open_.open_line)
        return root

    # -- emission
    def _emit(self, nodes: list[Node], scope: Scope) -> None:
        for node in nodes:
            if isinstance(node, TextNode):
                if node.parts is None:
                    node.parts = self._compile_line(node.text)
                line = self.eff_line(node.line)
                self.out.append(self._render_parts(node.parts, line, scope))
                self.out_lines.append(line)
            elif isinstance(node, IfNode):
                for branch in node.branches:
                    cond = branch.condition
                    if cond is None or self._evaluate(branch, cond, scope):
                        self._emit(branch.nodes, scope)
                        break
            elif isinstance(node, ForNode):
                self._emit_for(node, scope)
            elif isinstance(node, IncludeNode):
                self._emit_include(node, scope)

    def _emit_for(self, node: ForNode, scope: Scope) -> None:
        ln = self.eff_line(node.line)
        lst = self._lookup(node.list_name, ln, scope)
        if not isinstance(lst, list):
            raise self.err(f"'{node.list_name}' is not a list", ln, node.list_name)
        if scope.has(node.variable):
            raise self.err(f"the name '{node.variable}' already exists", ln, node.variable)
        position_warned = False
        for index, item in enumerate(lst):
            # ids and taps inside the item carry the path of the repetition; it is written with a marker
            # because the key of the item is only known after the item has been emitted.
            marker = f"\x01{self.scope_counter}\x02"
            self.scope_counter += 1
            child_path = marker if not scope.path else scope.path + "/" + marker
            first = len(self.out)
            log_first = len(self.scope_log)
            hits = self.marker_hits
            self._emit(node.nodes, Scope(scope, {node.variable: item}, child_path))
            item_key = Scopes.key_of_item(item)
            if self.marker_hits != hits:
                fl = self._first_level_key(first)
                segment = node.variable + ":" + Scopes.token(fl if fl is not None else item_key, index)
                for k in range(first, len(self.out)):
                    if "\x01" in self.out[k]:
                        self.out[k] = self.out[k].replace(marker, segment)
                for k in range(log_first, len(self.scope_log)):
                    p, it = self.scope_log[k]
                    if "\x01" in p:
                        self.scope_log[k] = (p.replace(marker, segment), it)
                self.scope_log.append((segment if not scope.path else scope.path + "/" + segment, item))
            keyed = self._apply_key(first, item_key)
            if not keyed and not position_warned and self._holds_fields_or_ids(first):
                position_warned = True
                w = LayoutWarning("the items of this list have no id: using the position", ln)
                if not any(x.message == w.message and x.line == w.line for x in self.warnings):
                    self.warnings.append(w)

    def _emit_include(self, node: IncludeNode, scope: Scope) -> None:
        ln = self.eff_line(node.line)
        given: dict[str, object] = {}
        for p in node.params:
            given[p.name] = self._param_value(p, ln, scope)
        key_text = None
        if node.key is not None:
            kt = self._raw_text(node.key, ln, scope).strip()
            key_text = kt or None
        first = len(self.out)
        saved_component = self.current_component
        saved_line = self.current_include_line
        if saved_component is None:
            self.current_include_line = node.line
        self.current_component = node.component.name
        try:
            self._emit(node.component.nodes, Scope(None, given, scope.path))
        finally:
            self.current_component = saved_component
            self.current_include_line = saved_line
        if key_text is not None:
            self._apply_key(first, key_text)

    def _param_value(self, p: ParamPlan, line: int, scope: Scope):
        if p.single is not None:
            return self._lookup_path(p.single, p.single.split("."), line, scope)
        if p.parts is not None:
            return self._raw_text(p.parts, line, scope)
        return p.constant

    def _raw_text(self, parts: list[Part], line: int, scope: Scope) -> str:
        sb = []
        for p in parts:
            if isinstance(p, Lit):
                sb.append(p.s)
            elif isinstance(p, Val):
                sb.append(self._raw_value(p, line, scope))
        return "".join(sb)

    def _value_text(self, v: Val, line: int, scope: Scope, escape: bool) -> str:
        if not v.ok:
            raise self.err(f"arithmetic is not supported in '{{{v.inner}}}'; give the program's result as a name", line)
        name = v.name
        x = self._lookup_path(name, v.path, line, scope)
        if x is None:
            return ""
        if isinstance(x, str):
            return _escape_html(x) if escape else x
        if isinstance(x, bool):
            return "true" if x else "false"
        if _is_num(x):
            return _format_number(x)
        if isinstance(x, list):
            raise self.err(f"'{{{name}}}' is a list; use 'for {_singular(name)} in {name}:'", line, name)
        if isinstance(x, dict):
            raise self.err(f"'{{{name}}}' is a map; use a name inside it, like '{{{name}.field}}'", line, name)
        return _escape_html(str(x)) if escape else str(x)

    def _raw_value(self, v: Val, line: int, scope: Scope) -> str:
        return self._value_text(v, line, scope, False)

    def _val_text(self, v: Val, line: int, scope: Scope) -> str:
        return self._value_text(v, line, scope, True)

    # -- keys of the repetitions
    def _first_level_key(self, first: int) -> str | None:
        depth = 0
        for k in range(first, len(self.out)):
            for m in TAG_RE.finditer(self.out[k]):
                tag = m.group(0)
                if tag.startswith("<!"):
                    continue
                if tag.startswith("</"):
                    depth -= 1
                    continue
                nm = TAG_NAME_RE.search(tag)
                if nm is None:
                    continue
                name = nm.group(1).lower()
                # a key that an inner `for` wrote by itself is not the key of the author
                if depth == 0 and k not in self.auto_keyed:
                    km = KEY_ATTR.search(tag)
                    if km is not None:
                        return km.group(1)
                if name not in VOID_NAMES and not tag.endswith("/>"):
                    depth += 1
        return None

    def _apply_key(self, first: int, key: str | None) -> bool:
        if key is None:
            return False
        escaped = _escape_html(key)
        depth = 0
        for k in range(first, len(self.out)):
            line = self.out[k]
            rebuilt: list[str] | None = None
            copied = 0
            for m in TAG_RE.finditer(line):
                tag = m.group(0)
                if tag.startswith("<!"):
                    continue
                if tag.startswith("</"):
                    depth -= 1
                    continue
                nm = TAG_NAME_RE.search(tag)
                if nm is None:
                    continue
                name = nm.group(1).lower()
                if depth == 0 and HAS_KEY.search(tag) is None:
                    if rebuilt is None:
                        rebuilt = []
                    rebuilt.append(line[copied:m.start()])
                    n = len(name) + 1
                    rebuilt.append(tag[:n] + ' key="' + escaped + '"' + tag[n:])
                    copied = m.end()
                if name not in VOID_NAMES and not tag.endswith("/>"):
                    depth += 1
            if rebuilt is not None:
                rebuilt.append(line[copied:])
                self.out[k] = "".join(rebuilt)
                self.auto_keyed.add(k)
        return True

    def _holds_fields_or_ids(self, first: int) -> bool:
        return any(FIELD_OR_ID.search(self.out[k]) for k in range(first, len(self.out)))

    # -- names
    def _lookup(self, name: str, line: int, scope: Scope):
        return self._lookup_path(name, name.split("."), line, scope)

    def _missing(self, name: str, line: int, word: str):
        c = self.current_component
        if c is None:
            return self.err(f"the name '{name}' was not given by the program", line, word)
        return ExpandError(f"include: the name '{name}' was not given to '{c}'",
                           self.current_include_line, self.source, word)

    def _lookup_path(self, name: str, parts: list[str], line: int, scope: Scope):
        value = scope.find(parts[0])
        if value is _MISSING:
            raise self._missing(name, line, parts[0])
        for part in parts[1:]:
            cur = value
            if isinstance(cur, dict) and part in cur:
                value = cur[part]
            elif part == "size" and isinstance(cur, (dict, list, str)):
                value = len(cur)
            else:
                raise self._missing(name, line, parts[0])
        return value

    # -- conditions
    def _tokenize(self, text: str, line: int) -> list[str]:
        tokens: list[str] = []
        pos = 0
        while pos < len(text):
            if text[pos].isspace():
                pos += 1
                continue
            m = TOKEN_RE.match(text, pos)
            if m is None:
                raise self.err(f"invalid condition: '{text}'", line)
            tokens.append(m.group(0))
            pos = m.end()
        return tokens

    def _evaluate(self, branch: Branch, condition: str, scope: Scope) -> bool:
        if branch.tokens is None:
            branch.tokens = self._tokenize(condition, branch.line)
        return _ConditionParser(self, branch.tokens, condition, branch.line, scope).run()

    # -- compiling lines
    def _compile_text(self, text: str, lst: list[Part]) -> None:
        pos = 0
        for m in BRACE_RE.finditer(text):
            if m.start() > pos:
                lst.append(Lit(text[pos:m.start()]))
            v = m.group(0)
            if v == "\\{":
                lst.append(Lit("{"))
            elif v == "\\}":
                lst.append(Lit("}"))
            else:
                lst.append(Val(m.group(1)))
            pos = m.end()
        if pos < len(text):
            lst.append(Lit(text[pos:]))

    def _compile_value(self, text: str) -> list[Part]:
        lst: list[Part] = []
        self._compile_text(text, lst)
        return _merge_lits(lst)

    def _compile_line(self, text: str) -> list[Part]:
        lst: list[Part] = []
        pos = 0
        for m in TAG_RE.finditer(text):
            self._compile_text(text[pos:m.start()], lst)
            lst.append(self._compile_tag(m.group(0)))
            pos = m.end()
        self._compile_text(text[pos:], lst)
        return _merge_lits(lst)

    def _render_parts(self, parts: list[Part], line: int, scope: Scope) -> str:
        if len(parts) == 1 and isinstance(parts[0], Lit):
            return parts[0].s
        sb = []
        for p in parts:
            if isinstance(p, Lit):
                sb.append(p.s)
            elif isinstance(p, Val):
                sb.append(self._val_text(p, line, scope))
            elif isinstance(p, DynTag):
                sb.append(p.pre if p.pre is not None and not scope.path else self._render_dyn(p, line, scope))
        return "".join(sb)

    def _style_for(self, tag: str, classes: list[str], id_: str | None, state: int = STATE_NONE) -> str:
        def match(r: Rule) -> bool:
            if r.state != state:
                return False
            if r.kind == KIND_TAG:
                return r.name == tag
            if r.kind == KIND_CLASS:
                return r.name in classes
            return id_ is not None and r.name == id_
        matched = sorted((r for r in self.rules if match(r)), key=lambda r: (r.kind, r.order))
        return "; ".join(f"{n}: {v}" for r in matched for n, v in r.resolved)

    def _compile_tag(self, tag: str) -> Part:
        if tag.startswith("<!"):
            return Lit(tag)
        if tag.startswith("</"):
            closing = tag[2:len(tag) - 1].strip().lower()
            return Lit("</div>" if closing == "body" else tag)
        nm = TAG_NAME_RE.search(tag)
        if nm is None:
            return Lit(tag)
        name = nm.group(1)
        lower = name.lower()
        self_close = tag.endswith("/>")
        inner = tag[nm.end():len(tag) - (2 if self_close else 1)]

        attrs: list[AttrPlan] = []
        classes: list[str] = []
        id_: str | None = None
        any_brace = False
        for a in ATTR_RE.finditer(inner):
            key = a.group(1).lower()
            value = _first(a, 2, 3, 4)
            brace = value is not None and "{" in value
            single = None
            text_parts = None
            if brace:
                any_brace = True
                sm = SINGLE_VALUE_RE.fullmatch(value.strip())
                single = sm.group(1) if sm is not None else None
                text_parts = self._compile_value(value)
            else:
                if key == "class":
                    classes = [c for c in (value or "").split(" ") if c]
                if key == "id":
                    id_ = value
            attrs.append(AttrPlan(a.group(1), key, value, brace, single, text_parts))
        plan = DynTag(name, lower, self_close, self._style_for(lower, classes, id_), attrs,
                      (self._style_for(lower, classes, id_, STATE_PRESSED),
                       self._style_for(lower, classes, id_, STATE_DISABLED),
                       self._style_for(lower, classes, id_, STATE_FOCUS)))
        if any_brace:
            return plan
        pre = self._render_dyn(plan, 0, Scope(None, {}))
        # an id or a class may need the path of a repetition, only known when the line is emitted
        if any(a.key in ("id", "class") for a in attrs):
            plan.pre = pre
            return plan
        return Lit(pre)

    def _render_dyn(self, t: DynTag, line: int, scope: Scope) -> str:
        result: list[Attr] = []
        inline = ""
        scoped = False
        for a in t.attrs:
            value = a.value
            if a.brace:
                if a.key in ("id", "class"):
                    raise self.err(f"markup: '{a.key}' cannot be generated by value", line, a.key)
                if a.key in ("checked", "disabled", "readonly") and a.single is not None:
                    flag = self._lookup(a.single, line, scope)
                    if flag is True:
                        result.append(Attr(a.orig, "true"))
                    continue
                value = self._render_parts(a.text_parts, line, scope)
            if a.key in ("id", "class") and scope.path:
                scoped = True
                if a.key == "id" and value is not None:
                    value = value + Scopes.SEP + scope.path
            if a.key == "style":
                inline = value or ""
            else:
                result.append(Attr(a.orig, value))
        if scoped:
            result.append(Attr("scope", scope.path))
            self.marker_hits += 1

        parts: list[str] = []
        if t.rule_style:
            parts.append(t.rule_style)
        inline_style = inline.strip().rstrip(";").strip()
        if inline_style:
            parts.append(inline_style)
        if parts:
            result.append(Attr("style", "; ".join(parts)))
        for k in range(3):
            if t.state_styles[k]:
                result.append(Attr(STATE_ATTRIBUTES[k], t.state_styles[k]))

        sb = ["<", "div" if t.lower == "body" else t.name]
        for a in result:
            sb.append(" " + a.key)
            if a.value is not None:
                sb.append('="' + a.value.replace('"', "&quot;") + '"')
        sb.append(" />" if t.self_close else ">")
        return "".join(sb)


class _ConditionParser:
    def __init__(self, owner: _ScreenExpander, tokens: list[str], text: str, line: int, scope: Scope):
        self.o = owner
        self.tokens = tokens
        self.text = text
        self.line = line
        self.scope = scope
        self.pos = 0

    def run(self) -> bool:
        value = self.parse_or()
        if self.pos != len(self.tokens):
            raise self.bad()
        return value

    def bad(self):
        return self.o.err(f"invalid condition: '{self.text}'", self.line)

    def peek(self) -> str | None:
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def parse_or(self) -> bool:
        v = self.parse_and()
        while self.peek() == "or":
            self.pos += 1
            r = self.parse_and()
            v = v or r
        return v

    def parse_and(self) -> bool:
        v = self.parse_not()
        while self.peek() == "and":
            self.pos += 1
            r = self.parse_not()
            v = v and r
        return v

    def parse_not(self) -> bool:
        if self.peek() == "not":
            self.pos += 1
            return not self.parse_not()
        if self.peek() == "(":
            self.pos += 1
            v = self.parse_or()
            if self.peek() != ")":
                raise self.bad()
            self.pos += 1
            return v
        return self.parse_compare()

    def parse_compare(self) -> bool:
        left = self.operand()
        op = self.peek()
        if op is not None and op in COMPARATORS:
            self.pos += 1
            right = self.operand()
            return self.compare(op, left, right)
        if isinstance(left, bool):
            return left
        raise self.o.err(f"invalid condition: '{self.text}'", self.line, None,
                         "a name alone must be true or false")

    def operand(self):
        token = self.peek()
        if token is None:
            raise self.bad()
        self.pos += 1
        if token == "true":
            return True
        if token == "false":
            return False
        if token.startswith('"'):
            return token[1:len(token) - 1]
        if token[0].isdigit() or token[0] == "-":
            return float(token)
        if token in ("and", "or", "not", "(", ")") or token in COMPARATORS:
            raise self.bad()
        return self.o._lookup(token, self.line, self.scope)

    def compare(self, op: str, a, b) -> bool:
        x = float(a) if _is_num(a) else None
        y = float(b) if _is_num(b) else None
        if x is not None and y is not None:
            return {"==": x == y, "!=": x != y, "<": x < y, ">": x > y, "<=": x <= y}.get(op, x >= y)
        if op == "==":
            return _equals(a, b)
        if op == "!=":
            return not _equals(a, b)
        if isinstance(a, str) and isinstance(b, str):
            if op == "<":
                return a < b
            if op == ">":
                return a > b
            if op == "<=":
                return a <= b
            return a >= b
        raise self.o.err(f"invalid condition: '{self.text}'", self.line, None,
                         f"these values cannot be compared with '{op}'")
