"""Converts HTML into a native tree (port of LayoutEngine.kt)."""
from __future__ import annotations

import math
import re
from dataclasses import dataclass, replace

from . import css as C
from . import icons as IconSet
from .css import CssDeclaration, _to_float
from .html_parser import HtmlElement, HtmlNode, HtmlParser, HtmlText, MarkupError
from .tree import (
    Align, AnimKind, Animation, Colors, Corners, Direction, Dock, FieldType, FontFamily,
    Gradient, ImageFit, ImagePos, Justify, Length, LineHeightFixed, LineHeightMultiple, Native, NativeBox,
    NativeButton, NativeCheckbox, NativeField, NativeImage, NativeItem, NativeList,
    NativeStyle, NativeText, Position, Sides, Span, StateStyle, StateStyles, TextAlign,
    TextTransform, native_children,
)


@dataclass
class LayoutWarning:
    message: str
    line: int


@dataclass
class LayoutResult:
    root: Native
    warnings: list[LayoutWarning]
    by_id: dict[str, Native]


class LayoutEngine:
    @staticmethod
    def build(html: str, start_line: int = 1, variables: dict[str, str] | None = None) -> LayoutResult:
        return _LayoutBuilder(html, start_line, variables or {}).build()


_SUPPORTED_TAGS = frozenset({
    "div", "span", "p", "h1", "h2", "h3", "button", "input", "textarea", "img", "ul", "li",
    "fieldset", "legend", "dialog", "header", "footer",
})
_TAGS_SUGGESTION = ("supported tags: div, span, p, h1, h2, h3, button, input, textarea, img, "
                    "ul, li, fieldset, legend, dialog, header, footer")
_FIELDSET_BORDER = 0xFFBDBDBD
_DIALOG_BACKDROP = 0x80000000
_BORDER_STYLES = frozenset({"solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"})
_DECORATION_STYLES = frozenset({"solid", "double", "dotted", "dashed", "wavy"})
_REGEX_VAR = re.compile(r"var\(\s*(--[\w-]+)\s*\)", re.ASCII)
_REGEX_DEG = re.compile(r"^(-?\d*\.?\d+)deg$", re.ASCII)
_REGEX_INT = re.compile(r"^[+-]?\d+$", re.ASCII)
_WS = re.compile(r"\s+")

_DIRECTION_ANGLES = {
    frozenset({"top"}): 0, frozenset({"top", "right"}): 45, frozenset({"right"}): 90,
    frozenset({"bottom", "right"}): 135, frozenset({"bottom"}): 180,
    frozenset({"bottom", "left"}): 225, frozenset({"left"}): 270, frozenset({"top", "left"}): 315,
}


def _round(x: float) -> int:
    return int(math.floor(x + 0.5))


def _int_or_none(s: str) -> int | None:
    return int(s) if _REGEX_INT.match(s) else None


def _blank(s: str) -> bool:
    return s.strip() == ""


def _vertical(rem: float) -> Sides:
    return Sides(top=Length.rem(rem), bottom=Length.rem(rem))


def _all_sides(rem: float) -> Sides:
    l = Length.rem(rem)
    return Sides(l, l, l, l)


def _inherit(p: NativeStyle) -> NativeStyle:
    return NativeStyle(
        text_color=p.text_color, font_size=p.font_size, bold=p.bold, text_align=p.text_align,
        italic=p.italic, underline=p.underline, strike=p.strike,
        letter_spacing=p.letter_spacing, line_height=p.line_height,
        text_transform=p.text_transform, font_family=p.font_family)


def _image_name_of(v: str) -> str | None:
    """`url(name)`, with or without quotes: the name of an image. None when the text is not that."""
    t = v.strip()
    if not t.lower().startswith("url(") or not t.endswith(")"):
        return None
    name = t[4:-1].strip().strip("\"'").strip()
    return name or None


def _key_of(el: HtmlElement) -> str | None:
    v = el.attributes.get("key")
    if v is None:
        return None
    v = v.strip()
    return v or None


def _scope_of(el: HtmlElement) -> str | None:
    v = el.attributes.get("scope")
    if v is None:
        return None
    v = v.strip()
    return v or None


def _transformed(text: str, t: TextTransform) -> str:
    if t == TextTransform.NONE:
        return text
    if t == TextTransform.UPPERCASE:
        return text.upper()
    if t == TextTransform.LOWERCASE:
        return text.lower()
    out = []
    start = True
    for ch in text:
        if start and ch.isalpha():
            u = ch.upper()
            out.append(u if len(u) == 1 else ch)
        else:
            out.append(ch)
        start = ch in " \t\n\r"
    return "".join(out)


def _split_ws(s: str) -> list[str]:
    return [p for p in C.split_top_level(s, " ") if not _blank(p)]


class _LayoutBuilder:
    def __init__(self, html: str, start_line: int, variables: dict[str, str]):
        self.html = html
        self.start_line = start_line
        self.variables = variables
        self.source = "\n" * max(start_line - 1, 0) + html
        self.warnings: list[LayoutWarning] = []
        self.depth = 0
        self.parent_tag: str | None = None
        self.group_disabled = False
        self.warned: set[tuple[str, int]] = set()
        self.ids: dict[str, Native] = {}
        self.reserved_ids: set[str] = set()

    # -- errors and warnings --------------------------------------------------
    def _err(self, message: str, line: int, word: str | None = None, suggestion: str | None = None):
        return MarkupError(message, line, self.source, word, suggestion)

    def _warn(self, message: str, line: int) -> None:
        k = (message, line)
        if k not in self.warned:
            self.warned.add(k)
            self.warnings.append(LayoutWarning(message, line))

    # -- input ---------------------------------------------------------
    def build(self) -> LayoutResult:
        nodes = HtmlParser(self.html, self.start_line, self.source).parse()
        relevant = [n for n in nodes if not (isinstance(n, HtmlText) and _blank(n.text))]
        if len(relevant) == 1 and isinstance(relevant[0], HtmlElement):
            root_el = relevant[0]
        else:
            if len(relevant) > 1:
                line = relevant[1].line
            elif relevant:
                line = relevant[0].line
            else:
                line = self.start_line
            raise self._err("must have a single root tag", line)
        root = self._convert_element(root_el, NativeStyle())
        self._assign_keys(root, "")
        return LayoutResult(root, list(self.warnings), dict(self.ids))

    def _assign_keys(self, n: Native, path: str) -> None:
        n.key = f"#{n.id}" if n.id is not None else (path if path else "/")
        seen: set[str] = set()
        for i, f in enumerate(native_children(n)):
            hint = f.key_hint
            if hint is not None:
                if hint in seen:
                    raise self._err(f"duplicate key '{hint}'", f.line, hint)
                seen.add(hint)
            self._assign_keys(f, f"{path}/{('~' + hint) if hint is not None else i}")

    # -- variables and declarations ----------------------------------------
    def _substitute_variables(self, text: str, line: int) -> str:
        if "var(" not in text:
            return text

        def rep(m):
            name = m.group(1)
            if name not in self.variables:
                raise self._err(f"the variable '{name}' is not defined", line, name)
            return self.variables[name]
        return _REGEX_VAR.sub(rep, text)

    def _declarations_of(self, el: HtmlElement) -> list[CssDeclaration]:
        s = el.attributes.get("style")
        if s is None:
            return []
        return C.declarations(self._substitute_variables(s, el.line), el.line, self._warn)

    # -- style ----------------------------------------------------------
    def _tag_defaults(self, el: HtmlElement, b: NativeStyle) -> NativeStyle:
        t = el.tag
        if t == "h1":
            return replace(b, font_size=Length.rem(2.0), bold=True, margin=_vertical(1.3))
        if t == "h2":
            return replace(b, font_size=Length.rem(1.5), bold=True, margin=_vertical(1.2))
        if t == "h3":
            return replace(b, font_size=Length.rem(1.17), bold=True, margin=_vertical(1.1))
        if t == "p":
            return replace(b, margin=_vertical(1.0))
        if t == "fieldset":
            return replace(b, padding=_all_sides(0.75), margin=_vertical(0.5),
                           border_width=Length.px(1.0), border_color=_FIELDSET_BORDER,
                           border_radius=Length.px(6.0))
        if t == "legend":
            return replace(b, bold=True, margin=Sides(bottom=Length.rem(0.5)))
        if t == "dialog":
            return replace(b, background_color=_DIALOG_BACKDROP, justify=Justify.CENTER,
                           align_items=Align.CENTER)
        if t == "footer":
            return replace(b, direction=Direction.ROW, justify=Justify.CENTER,
                           align_items=Align.CENTER)
        if t == "ul":
            return replace(b, margin=_vertical(1.0), padding=Sides(left=Length.rem(1.5)))
        if t == "button":
            return replace(b, padding=Sides(Length.rem(0.5), Length.rem(1.0), Length.rem(0.5), Length.rem(1.0)),
                           background_color=Colors.BUTTON_GRAY, border_radius=Length.px(4.0),
                           text_align=TextAlign.CENTER, text_color=Colors.BLACK, bold=False)
        if t in ("input", "textarea"):
            if (el.attributes.get("type") or "text").strip().lower() == "checkbox":
                return b
            return replace(b, padding=_all_sides(0.5), border_width=Length.px(1.0),
                           border_color=Colors.BORDER_GRAY, border_radius=Length.px(2.0),
                           background_color=Colors.WHITE, text_color=Colors.BLACK)
        return b

    def _compute_style(self, el: HtmlElement, parent_style: NativeStyle) -> NativeStyle:
        e = self._tag_defaults(el, _inherit(parent_style))
        for d in self._declarations_of(el):
            e = self._apply_declaration(e, d)
        pressed = self._state_of(el, "style-pressed", False)
        disabled = self._state_of(el, "style-disabled", True)
        focused = self._state_of(el, "style-focus", False)
        if pressed is not None or disabled is not None or focused is not None:
            e = replace(e, states=StateStyles(pressed, disabled, focused))
        return e

    def _fix_position(self, el: HtmlElement, s: NativeStyle, level: int) -> NativeStyle:
        """`position: absolute` only makes sense inside a box of the flow: elsewhere it is ignored with a warning."""
        if s.position != Position.ABSOLUTE:
            return s
        if el.tag in ("header", "footer", "dialog"):
            why = f"<{el.tag}> has its own place"
        elif el.tag == "li":
            why = "<li> stays in its list"
        elif level == 0:
            why = "the root of the screen has no parent"
        else:
            return s
        self._warn(f"CSS: 'position: absolute' is ignored: {why}", el.line)
        return replace(s, position=Position.STATIC)

    def _state_of(self, el: HtmlElement, attribute: str, allow_opacity: bool) -> StateStyle | None:
        text = el.attributes.get(attribute)
        if text is None:
            return None
        background = color = border = opacity = None
        for d in C.declarations(text, el.line, self._warn):
            v = d.value.strip()

            def bad(d=d):
                self._warn(f"CSS: invalid value in '{d.name}: {d.value}', ignored", d.line)

            if d.name in ("background", "background-color"):
                c = C.color(v)
                if c is None:
                    bad()
                else:
                    background = c
            elif d.name == "color":
                c = C.color(v)
                if c is None:
                    bad()
                else:
                    color = c
            elif d.name == "border-color":
                c = C.color(v)
                if c is None:
                    bad()
                else:
                    border = c
            elif d.name == "opacity":
                if not allow_opacity:
                    self._warn("CSS: 'opacity' is only supported in :disabled", d.line)
                else:
                    f = C.fraction(v)
                    if f is None:
                        bad()
                    else:
                        opacity = f
            else:
                self._warn(f"CSS: '{d.name}' is not supported in :pressed, :disabled or :focus "
                           "(use background, color, border-color; opacity only in :disabled)", d.line)
        if background is None and color is None and border is None and opacity is None:
            return None
        return StateStyle(background, color, border, opacity)

    @staticmethod
    def _sides_of(v: str) -> Sides | None:
        p = []
        for t in _split_ws(v):
            l = C.length(t)
            if l is None:
                return None
            p.append(l)
        n = len(p)
        if n == 1:
            return Sides(p[0], p[0], p[0], p[0])
        if n == 2:
            return Sides(p[0], p[1], p[0], p[1])
        if n == 3:
            return Sides(p[0], p[1], p[2], p[1])
        if n == 4:
            return Sides(p[0], p[1], p[2], p[3])
        return None

    @staticmethod
    def _corners_of(v: str) -> Corners | None:
        if "/" in v:
            return None
        p = []
        for t in _split_ws(v):
            l = C.length(t)
            if l is None or l.value < 0.0:
                return None
            p.append(l)
        n = len(p)
        if n == 1:
            return Corners(p[0], p[0], p[0], p[0])
        if n == 2:
            return Corners(p[0], p[1], p[0], p[1])
        if n == 3:
            return Corners(p[0], p[1], p[2], p[1])
        if n == 4:
            return Corners(p[0], p[1], p[2], p[3])
        return None

    @staticmethod
    def _with_corners(e: NativeStyle, c: Corners) -> NativeStyle:
        if c.top_left == c.top_right == c.bottom_right == c.bottom_left:
            return replace(e, border_radius=c.top_left, corners=None)
        return replace(e, border_radius=Length.ZERO, corners=c)

    @staticmethod
    def _direction_angle(words: str) -> int | None:
        return _DIRECTION_ANGLES.get(frozenset(w for w in words.split(" ") if w))

    def _gradient_of(self, v: str) -> Gradient | None:
        op = v.find("(")
        cl = v.rfind(")")
        if op < 0 or cl <= op:
            return None
        args = [a.strip() for a in C.split_top_level(v[op + 1:cl], ",") if a.strip()]
        if len(args) < 2:
            return None
        angle = 180
        start = 0
        first = args[0].lower()
        deg = _REGEX_DEG.match(first)
        if deg is not None:
            angle = (_round(float(deg.group(1))) % 360 + 360) % 360
            start = 1
        elif first.startswith("to "):
            a = self._direction_angle(first[3:].strip())
            if a is None:
                return None
            angle = a
            start = 1
        colors: list[int] = []
        for i in range(start, len(args)):
            found = None
            for tok in _split_ws(args[i]):
                c = C.color(tok)
                if c is not None:
                    found = c
                    break
            if found is None:
                return None
            colors.append(found)
        return Gradient(angle, tuple(colors)) if len(colors) >= 2 else None

    @staticmethod
    def _elevation_of(v: str) -> Length | None:
        first = C.split_top_level(v, ",")[0].strip()
        if first.lower() == "none":
            return Length.ZERO
        lengths: list[Length] = []
        for tok in _split_ws(first):
            if tok.lower() == "inset":
                return None
            ln = C.length(tok)
            if ln is not None:
                lengths.append(ln)
            elif C.color(tok) is None:
                return None
        if not 2 <= len(lengths) <= 4:
            return None
        ref = lengths[2] if len(lengths) >= 3 and lengths[2].value > 0.0 else lengths[1]
        return Length(max(ref.value, 0.0) / 2.0, ref.scalable)

    @staticmethod
    def _font_family_of(v: str) -> FontFamily | None:
        for raw in C.split_top_level(v, ","):
            name = raw.strip().strip("\"'").lower()
            if name in ("sans-serif", "system-ui", "roboto", "arial", "helvetica", "default"):
                return FontFamily.SANS
            if name in ("serif", "georgia", "times", "times new roman"):
                return FontFamily.SERIF
            if name in ("monospace", "courier", "courier new", "consolas", "menlo"):
                return FontFamily.MONOSPACE
        return None

    @staticmethod
    def _animation_of(v: str) -> Animation | None:
        parts = [p for p in _WS.split(v.lower()) if p]
        if not parts or len(parts) > 2:
            return None
        kinds = {"none": AnimKind.NONE, "fade": AnimKind.FADE,
                 "slide-up": AnimKind.SLIDE_UP, "slide-left": AnimKind.SLIDE_LEFT}
        kind = kinds.get(parts[0])
        if kind is None:
            return None
        ms = 200
        if len(parts) == 2:
            t = parts[1]
            if t.endswith("ms"):
                f = _to_float(t[:-2])
                r = None if f is None else int(f)
            elif t.endswith("s"):
                f = _to_float(t[:-1])
                r = None if f is None else int(f * 1000.0)
            else:
                r = None
            if r is None:
                return None
            ms = r
        if ms < 0 or ms > 2000:
            return None
        return Animation(kind, ms)

    def _apply_declaration(self, e: NativeStyle, d: CssDeclaration) -> NativeStyle:
        v = d.value.strip()
        vl = v.lower()
        name = d.name

        def invalid() -> NativeStyle:
            self._warn(f"CSS: invalid value in '{d.name}: {d.value}', ignored", d.line)
            return e

        def unsupported(msg: str) -> NativeStyle:
            self._warn(msg, d.line)
            return e

        def ln() -> Length | None:
            return C.length(v)

        if name == "color":
            c = C.color(v)
            return invalid() if c is None else replace(e, text_color=c)
        if name in ("background", "background-color", "background-image"):
            if vl.startswith("linear-gradient(") and vl.endswith(")"):
                g = self._gradient_of(v)
                return invalid() if g is None else replace(e, gradient=g, background_image=None)
            image = _image_name_of(v)
            if image is not None:
                return replace(e, background_image=image)
            if name == "background-image":
                return replace(e, gradient=None, background_image=None) if vl == "none" else invalid()
            c = C.color(v)
            if c is None:
                return invalid()
            return replace(e, background_color=c, gradient=None,
                           background_image=None if name == "background" else e.background_image)
        if name == "font-size":
            l = ln()
            return replace(e, font_size=l) if l is not None and l.value > 0.0 else invalid()
        if name == "font-weight":
            n = _int_or_none(vl)
            if vl in ("bold", "bolder"):
                return replace(e, bold=True)
            if vl in ("normal", "lighter"):
                return replace(e, bold=False)
            if n is not None:
                return replace(e, bold=n >= 600)
            return invalid()
        if name == "padding":
            s = self._sides_of(v)
            return invalid() if s is None else replace(e, padding=s)
        if name == "margin":
            s = self._sides_of(v)
            return invalid() if s is None else replace(e, margin=s)
        if name in ("padding-top", "padding-right", "padding-bottom", "padding-left"):
            l = ln()
            return invalid() if l is None else replace(
                e, padding=replace(e.padding, **{name.split("-")[1]: l}))
        if name in ("margin-top", "margin-right", "margin-bottom", "margin-left"):
            l = ln()
            return invalid() if l is None else replace(
                e, margin=replace(e.margin, **{name.split("-")[1]: l}))
        if name == "border-width":
            l = ln()
            return replace(e, border_width=l) if l is not None and l.value >= 0.0 else invalid()
        if name == "border-color":
            c = C.color(v)
            return invalid() if c is None else replace(e, border_color=c)
        if name == "border-style":
            if vl in ("none", "hidden"):
                return replace(e, border_width=Length.ZERO)
            return e if vl in _BORDER_STYLES else invalid()
        if name == "text-align":
            if vl in ("left", "start"):
                return replace(e, text_align=TextAlign.START)
            if vl == "center":
                return replace(e, text_align=TextAlign.CENTER)
            if vl in ("right", "end"):
                return replace(e, text_align=TextAlign.END)
            if vl == "justify":
                self._warn("CSS: 'text-align: justify' is not supported, using start", d.line)
                return replace(e, text_align=TextAlign.START)
            return invalid()
        if name == "opacity":
            f = C.fraction(v)
            return invalid() if f is None else replace(e, opacity=f)
        if name == "font-style":
            if vl in ("italic", "oblique"):
                return replace(e, italic=True)
            if vl == "normal":
                return replace(e, italic=False)
            return invalid()
        if name in ("text-decoration", "text-decoration-line"):
            underline = strike = none = False
            ok = True
            for tok in _split_ws(vl):
                if tok == "none":
                    none = True
                elif tok == "underline":
                    underline = True
                elif tok == "line-through":
                    strike = True
                elif tok == "overline":
                    self._warn("CSS: 'text-decoration: overline' is not supported, ignored", d.line)
                elif tok in _DECORATION_STYLES or C.color(tok) is not None or C.length(tok) is not None:
                    pass
                else:
                    ok = False
            if not ok:
                return invalid()
            if none and not underline and not strike:
                return replace(e, underline=False, strike=False)
            return replace(e, underline=underline, strike=strike)
        if name == "letter-spacing":
            if vl == "normal":
                return replace(e, letter_spacing=Length.ZERO)
            l = ln()
            return invalid() if l is None else replace(e, letter_spacing=l)
        if name == "line-height":
            number = _to_float(vl)
            percent = _to_float(vl[:-1]) if vl.endswith("%") else None
            l = ln()
            if vl == "normal":
                return replace(e, line_height=None)
            if number is not None and number > 0.0:
                return replace(e, line_height=LineHeightMultiple(number))
            if percent is not None and percent > 0.0:
                return replace(e, line_height=LineHeightMultiple(percent / 100.0))
            if l is not None and l.value > 0.0:
                return replace(e, line_height=LineHeightFixed(l))
            return invalid()
        if name == "text-transform":
            m = {"none": TextTransform.NONE, "uppercase": TextTransform.UPPERCASE,
                 "lowercase": TextTransform.LOWERCASE, "capitalize": TextTransform.CAPITALIZE}
            return replace(e, text_transform=m[vl]) if vl in m else invalid()
        if name == "font-family":
            fam = self._font_family_of(v)
            if fam is not None:
                return replace(e, font_family=fam)
            return unsupported(f"CSS: no known family in 'font-family: {v}' (use sans-serif, serif or monospace), ignored")
        if name in ("overflow", "overflow-x", "overflow-y"):
            if vl in ("hidden", "clip"):
                return replace(e, overflow_hidden=True)
            if vl == "visible":
                return replace(e, overflow_hidden=False)
            return unsupported(f"CSS: 'overflow: {vl}' is not supported (use hidden or visible)")
        if name == "box-shadow":
            el_ = self._elevation_of(v)
            return invalid() if el_ is None else replace(e, elevation=el_)
        if name in ("min-width", "min-height"):
            field = name.replace("-", "_")
            if vl in ("0", "auto"):
                return replace(e, **{field: None})
            l = ln()
            return invalid() if l is None else replace(e, **{field: l})
        if name in ("max-width", "max-height"):
            field = name.replace("-", "_")
            if vl == "none":
                return replace(e, **{field: None})
            l = ln()
            return invalid() if l is None else replace(e, **{field: l})
        if name == "align-self":
            if vl == "auto":
                return replace(e, align_self=None)
            if vl in ("flex-start", "start"):
                return replace(e, align_self=Align.START)
            if vl == "center":
                return replace(e, align_self=Align.CENTER)
            if vl in ("flex-end", "end"):
                return replace(e, align_self=Align.END)
            if vl == "stretch":
                return replace(e, align_self=Align.STRETCH)
            return invalid()
        if name == "flex-wrap":
            if vl == "wrap":
                return replace(e, wrap=True)
            if vl == "nowrap":
                return replace(e, wrap=False)
            return unsupported(f"CSS: 'flex-wrap: {vl}' is not supported (use wrap or nowrap)")
        if name == "border-radius":
            c = self._corners_of(v)
            return invalid() if c is None else self._with_corners(e, c)
        if name in ("border-top-left-radius", "border-top-right-radius",
                    "border-bottom-right-radius", "border-bottom-left-radius"):
            l = ln()
            if l is None or l.value < 0.0:
                return invalid()
            c = e.corners or Corners(e.border_radius, e.border_radius, e.border_radius, e.border_radius)
            field = {"border-top-left-radius": "top_left", "border-top-right-radius": "top_right",
                     "border-bottom-right-radius": "bottom_right"}.get(name, "bottom_left")
            return self._with_corners(e, replace(c, **{field: l}))
        if name == "border":
            if vl in ("none", "0"):
                return replace(e, border_width=Length.ZERO)
            width = color = None
            no_border = False
            ok = True
            for tok in _split_ws(v):
                tl = tok.lower()
                w = C.length(tok)
                if w is not None:
                    width = w
                elif tl in ("none", "hidden"):
                    no_border = True
                elif tl in _BORDER_STYLES:
                    pass
                else:
                    c = C.color(tok)
                    if c is not None:
                        color = c
                    else:
                        ok = False
            if not ok:
                return invalid()
            r = replace(e, border_width=Length.ZERO if no_border else (width or Length.px(3.0)))
            return replace(r, border_color=color) if color is not None else r
        if name == "display":
            if vl in ("flex", "block"):
                return e
            return unsupported(f"CSS: 'display: {vl}' is not supported in v1 (use display:flex)")
        if name == "flex-direction":
            if vl == "row":
                return replace(e, direction=Direction.ROW)
            if vl == "column":
                return replace(e, direction=Direction.COLUMN)
            return unsupported(f"CSS: 'flex-direction: {vl}' is not supported in v1 (use row or column)")
        if name == "flex":
            if vl == "none":
                g = 0.0
            elif vl == "auto":
                g = 1.0
            else:
                g = _to_float(vl.split(" ")[0])
            return replace(e, flex=g) if g is not None and g >= 0.0 else invalid()
        if name == "justify-content":
            m = {"flex-start": Justify.START, "start": Justify.START, "center": Justify.CENTER,
                 "flex-end": Justify.END, "end": Justify.END, "space-between": Justify.BETWEEN,
                 "space-around": Justify.AROUND, "space-evenly": Justify.EVENLY}
            if vl in m:
                return replace(e, justify=m[vl])
            return unsupported(f"CSS: 'justify-content: {vl}' is not supported (use flex-start, "
                               "center, flex-end, space-between, space-around or space-evenly)")
        if name == "position":
            if vl in ("static", "relative"):
                return replace(e, position=Position.STATIC)
            if vl == "absolute":
                return replace(e, position=Position.ABSOLUTE)
            return unsupported(f"CSS: 'position: {vl}' is not supported in v1 (use static, relative or absolute)")
        if name in ("top", "right", "bottom", "left"):
            if vl == "auto":
                return replace(e, **{name: None})
            l = ln()
            return invalid() if l is None else replace(e, **{name: l})
        if name == "z-index":
            if vl == "auto":
                return replace(e, z_index=0)
            n = _int_or_none(vl)
            return invalid() if n is None else replace(e, z_index=max(-1000, min(1000, n)))
        if name == "background-size":
            m = {"cover": ImageFit.COVER, "contain": ImageFit.CONTAIN,
                 "100% 100%": ImageFit.FILL, "fill": ImageFit.FILL}
            if vl in m:
                return replace(e, background_size=m[vl])
            return unsupported(f"CSS: 'background-size: {vl}' is not supported in v1 "
                               "(use cover, contain or 100% 100%)")
        if name == "background-position":
            m = {"center": ImagePos.CENTER, "center center": ImagePos.CENTER,
                 "top": ImagePos.TOP, "top center": ImagePos.TOP, "center top": ImagePos.TOP,
                 "bottom": ImagePos.BOTTOM, "bottom center": ImagePos.BOTTOM, "center bottom": ImagePos.BOTTOM,
                 "left": ImagePos.LEFT, "left center": ImagePos.LEFT, "center left": ImagePos.LEFT,
                 "right": ImagePos.RIGHT, "right center": ImagePos.RIGHT, "center right": ImagePos.RIGHT}
            if vl in m:
                return replace(e, background_position=m[vl])
            return unsupported(f"CSS: 'background-position: {vl}' is not supported in v1 "
                               "(use center, top, bottom, left or right)")
        if name == "object-fit":
            m = {"cover": ImageFit.COVER, "contain": ImageFit.CONTAIN, "scale-down": ImageFit.CONTAIN,
                 "fill": ImageFit.FILL}
            if vl in m:
                return replace(e, object_fit=m[vl])
            return unsupported(f"CSS: 'object-fit: {vl}' is not supported in v1 (use cover, contain or fill)")
        if name == "dock":
            m = {"bottom": Dock.BOTTOM, "top": Dock.TOP, "left": Dock.LEFT,
                 "right": Dock.RIGHT, "center": Dock.CENTER}
            return replace(e, dock=m[vl]) if vl in m else invalid()
        if name == "align-items":
            m = {"flex-start": Align.START, "start": Align.START, "center": Align.CENTER,
                 "flex-end": Align.END, "end": Align.END, "stretch": Align.STRETCH}
            if vl in m:
                return replace(e, align_items=m[vl])
            return unsupported(f"CSS: 'align-items: {vl}' is not supported in v1 (use flex-start, center or flex-end)")
        if name == "gap":
            l = ln()
            return invalid() if l is None else replace(e, gap=l)
        if name in ("width", "height"):
            s = C.size(v)
            return invalid() if s is None else replace(e, **{name: s})
        if name == "animation":
            a = self._animation_of(v)
            if a is not None:
                return replace(e, animation=a)
            return unsupported(f"CSS: invalid value in 'animation: {d.value}' (use none, fade, slide-up "
                               "or slide-left, then an optional time like 200ms), ignored")
        return unsupported(f"CSS: property '{d.name}' is not supported in v1 and was ignored")

    # -- text -----------------------------------------------------------
    def _is_inline(self, el: HtmlElement) -> bool:
        return (el.tag == "span" and el.id is None and "class" not in el.attributes
                and not any(isinstance(c, HtmlElement) and not self._is_inline(c) for c in el.children))

    @staticmethod
    def _span(text: str, e: NativeStyle) -> Span:
        return Span(_transformed(text, e.text_transform), e.bold, e.text_color, e.font_size,
                    e.italic, e.underline, e.strike)

    def _collect_inline(self, el: HtmlElement, parent_style: NativeStyle, run: list[Span]) -> None:
        style = self._compute_style(el, parent_style)
        for f in el.children:
            if isinstance(f, HtmlText):
                run.append(self._span(f.text, style))
            elif isinstance(f, HtmlElement):
                self._collect_inline(f, style, run)

    @staticmethod
    def _normalize(run: list[Span]) -> list[Span]:
        result: list[Span] = []
        last_space = True
        for t in run:
            sb = []
            for ch in t.text:
                if ch in " \t\n\r":
                    if not last_space:
                        sb.append(" ")
                        last_space = True
                else:
                    sb.append(ch)
                    last_space = False
            if sb:
                result.append(replace(t, text="".join(sb)))
        if result:
            u = result[-1]
            cleaned = u.text.rstrip(" ")
            if not cleaned:
                result.pop()
            elif len(cleaned) != len(u.text):
                result[-1] = replace(u, text=cleaned)
        return result

    def _text_of(self, el: HtmlElement, style: NativeStyle) -> str:
        run: list[Span] = []
        for f in el.children:
            if isinstance(f, HtmlText):
                run.append(self._span(f.text, style))
            elif isinstance(f, HtmlElement):
                if self._is_inline(f):
                    self._collect_inline(f, style, run)
                else:
                    raise self._err(f"<{el.tag}> can only contain text", f.line, f"<{f.tag}")
        return "".join(s.text for s in self._normalize(run))

    # -- children ----------------------------------------------------------
    def _convert_children(self, nodes: list[HtmlNode], parent_style: NativeStyle, owner: str) -> list[Native]:
        saved = self.parent_tag
        self.parent_tag = owner
        self.depth += 1
        try:
            return self._convert_children_of(nodes, parent_style)
        finally:
            self.depth -= 1
            self.parent_tag = saved

    def _convert_children_of(self, nodes: list[HtmlNode], parent_style: NativeStyle) -> list[Native]:
        result: list[Native] = []
        run: list[Span] = []
        run_line = -1

        def flush():
            nonlocal run_line
            spans = self._normalize(run)
            if spans:
                result.append(NativeText(id=None, classes=[], style=_inherit(parent_style),
                                         line=run_line, spans=spans))
            run.clear()
            run_line = -1

        for n in nodes:
            if isinstance(n, HtmlText):
                if run_line < 0:
                    run_line = n.line
                run.append(self._span(n.text, parent_style))
            elif isinstance(n, HtmlElement):
                if self._is_inline(n):
                    if run_line < 0:
                        run_line = n.line
                    self._collect_inline(n, parent_style, run)
                else:
                    flush()
                    result.append(self._convert_element(n, parent_style))
        flush()
        return result

    def _register_id(self, el: HtmlElement) -> str | None:
        i = el.id
        if i is None:
            return None
        if i in self.ids or i in self.reserved_ids:
            raise self._err(f"duplicate id '{i}'", el.line, i)
        self.reserved_ids.add(i)
        return i

    # -- elements -------------------------------------------------------
    def _convert_element(self, el: HtmlElement, parent_style: NativeStyle) -> Native:
        tag = el.tag
        if tag not in _SUPPORTED_TAGS:
            raise self._err(f"tag '{tag}' is not supported", el.line, f"<{tag}", _TAGS_SUGGESTION)
        my_depth = self.depth
        my_parent = self.parent_tag
        style = self._fix_position(el, self._compute_style(el, parent_style), my_depth)
        id_ = self._register_id(el)
        classes = el.classes
        disabled = "disabled" in el.attributes or self.group_disabled
        line = el.line

        if tag == "button":
            node: Native = NativeButton(id=id_, classes=classes, style=style, line=line,
                                        label=self._text_of(el, style), disabled=disabled)
        elif tag == "input":
            node = self._convert_input(el, style, id_, classes, disabled)
        elif tag == "textarea":
            content = "".join(c.text for c in el.children if isinstance(c, HtmlText))
            if content.startswith("\n"):
                content = content[1:]
            rows_raw = el.attributes.get("rows")
            rows_i = _int_or_none(rows_raw.strip()) if rows_raw is not None else None
            rows = max(rows_i, 1) if rows_i is not None else 3
            node = NativeField(id=id_, classes=classes, style=style, line=line, type=FieldType.AREA,
                               hint=el.attributes.get("placeholder", ""), value=content,
                               disabled=disabled, read_only="readonly" in el.attributes, rows=rows)
        elif tag == "img":
            src = (el.attributes.get("src") or "").strip()
            if not src:
                raise self._err("<img> requires a src attribute", line, "<img")
            icon = IconSet.name_of(src)
            if icon is not None and IconSet.shapes_of(icon) is None:
                raise self._err(f"icon '{icon}' does not exist", line, src,
                                "icons: " + ", ".join(IconSet.names()))
            node = NativeImage(id=id_, classes=classes, style=style, line=line,
                               source=src, description=el.attributes.get("alt", ""))
        elif tag == "ul":
            node = self._convert_list(el, style, id_, classes)
        elif tag == "li":
            raise self._err("<li> can only be inside <ul>", line, "<li")
        else:
            node = self._convert_container(el, tag, style, id_, classes, my_depth, my_parent)

        node.key_hint = _key_of(el)
        node.scope = _scope_of(el)
        if id_ is not None:
            self.ids[id_] = node
        return node

    def _convert_container(self, el, tag, style, id_, classes, my_depth, my_parent) -> Native:
        line = el.line
        if tag == "legend" and my_parent != "fieldset":
            raise self._err("<legend> can only be inside <fieldset>", line, "<legend")
        if tag == "dialog" and my_depth != 1:
            raise self._err("<dialog> must be a direct child of the root tag", line, "<dialog",
                            "put the <dialog> at the first level of the screen, next to the other blocks")
        if tag == "header" and my_depth != 1:
            raise self._err("<header> must be a direct child of the root tag", line, "<header",
                            "put the <header> at the first level of the screen; it stays fixed at the top")
        if tag == "footer" and my_depth != 1:
            raise self._err("<footer> must be a direct child of the root tag", line, "<footer",
                            "put the <footer> at the first level of the screen; it stays fixed outside the scroll")
        saved_group = self.group_disabled
        if tag == "fieldset" and "disabled" in el.attributes:
            self.group_disabled = True
        try:
            found = self._convert_children(el.children, style, tag)
        finally:
            self.group_disabled = saved_group

        def special(n):
            return isinstance(n, NativeBox) and (n.header or n.footer or n.overlay)

        if my_depth == 0 and any(special(n) for n in found):
            headers = [n for n in found if isinstance(n, NativeBox) and n.header]
            if len(headers) > 1:
                raise self._err("a screen can have only one <header>", headers[1].line, "<header")
            footers = [n for n in found if isinstance(n, NativeBox) and n.footer]
            if len(footers) > 1:
                raise self._err("a screen can have only one <footer>", footers[1].line, "<footer")
            children = (headers + [n for n in found if not special(n)] + footers
                        + [n for n in found if isinstance(n, NativeBox) and n.overlay])
        else:
            children = found
        virtual = "virtual" in el.attributes
        if virtual:
            for c in children:
                if isinstance(c, NativeBox) and c.overlay:
                    raise self._err("<dialog> cannot be inside a 'virtual' container", c.line, "<dialog")
                if c.style.position == Position.ABSOLUTE:
                    raise self._err("'position: absolute' cannot be used inside a 'virtual' container", c.line)
        only = children[0] if len(children) == 1 else None
        is_group = tag in ("dialog", "fieldset", "header", "footer")
        if (not virtual and not is_group and isinstance(only, NativeText)
                and only.style.position != Position.ABSOLUTE):
            return NativeText(id=id_, classes=classes, style=style, line=line, spans=only.spans)
        return NativeBox(id=id_, classes=classes, style=style, line=line, children=children,
                         virtual=virtual, overlay=tag == "dialog", header=tag == "header",
                         footer=tag == "footer",
                         has_absolute=not virtual and any(c.style.position == Position.ABSOLUTE for c in children))

    def _convert_input(self, el, style, id_, classes, disabled) -> Native:
        type_ = (el.attributes.get("type") or "text").strip().lower()
        if type_ == "checkbox":
            return NativeCheckbox(id=id_, classes=classes, style=style, line=el.line,
                                  checked="checked" in el.attributes, disabled=disabled)
        if type_ in ("text", "password", "number"):
            ft = {"password": FieldType.PASSWORD, "number": FieldType.NUMBER}.get(type_, FieldType.TEXT)
            return NativeField(id=id_, classes=classes, style=style, line=el.line, type=ft,
                               hint=el.attributes.get("placeholder", ""),
                               value=el.attributes.get("value", ""), disabled=disabled,
                               read_only="readonly" in el.attributes, rows=1)
        raise self._err(f'<input type="{type_}"> is not supported', el.line, "type",
                        "supported types: text, password, number, checkbox")

    def _convert_list(self, el, style, id_, classes) -> NativeList:
        items: list[NativeItem] = []
        for f in el.children:
            if isinstance(f, HtmlText):
                if not _blank(f.text):
                    raise self._err("<ul> can only contain <li>", f.line)
            elif isinstance(f, HtmlElement):
                if f.tag != "li":
                    raise self._err(f"<ul> can only contain <li>, found <{f.tag}>", f.line, f"<{f.tag}")
                li_style = self._fix_position(f, self._compute_style(f, style), 2)
                li_id = self._register_id(f)
                item = NativeItem(id=li_id, classes=f.classes, style=li_style, line=f.line,
                                  marker="\u2022",
                                  children=self._convert_children(f.children, li_style, "li"))
                item.key_hint = _key_of(f)
                item.scope = _scope_of(f)
                if li_id is not None:
                    self.ids[li_id] = item
                items.append(item)
        return NativeList(id=id_, classes=classes, style=style, line=el.line, items=items)
