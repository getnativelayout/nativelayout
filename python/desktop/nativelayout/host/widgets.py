"""Kivy widgets of the host. Each draws its own background; the position comes from the flex engine."""
from __future__ import annotations

from kivy.core.text.markup import MarkupLabel as CoreLabel
from kivy.graphics import Color, Line, Rectangle, InstructionGroup
from kivy.uix.relativelayout import RelativeLayout
from kivy.uix.textinput import TextInput
from kivy.uix.widget import Widget
from kivy.utils import escape_markup

from nativelayout.core.tree import (Align, Direction, FontFamily, Justify, LineHeightFixed,
                                    LineHeightMultiple, NativeStyle, Span, TextAlign)
from nativelayout.core.tree import ImagePos

from . import draw

TAP_SLOP = 12.0   # pixels of movement after which a touch is no longer a click


def _font_name(f: FontFamily) -> str | None:
    if f == FontFamily.MONOSPACE:
        from kivy.resources import resource_find
        return resource_find("data/fonts/RobotoMono-Regular.ttf") or None
    return None


# ---------------------------------------------------------------- text
def markup_of(spans: list[Span], px) -> str:
    out = []
    for s in spans:
        t = escape_markup(s.text)
        if not t:
            continue
        c = s.color
        hexcol = "%02x%02x%02x%02x" % ((c >> 16) & 255, (c >> 8) & 255, c & 255, (c >> 24) & 255)
        t = f"[size={max(1, int(round(px(s.size))))}]{t}[/size]"
        if s.bold:
            t = f"[b]{t}[/b]"
        if s.italic:
            t = f"[i]{t}[/i]"
        if s.underline:
            t = f"[u]{t}[/u]"
        if s.strike:
            t = f"[s]{t}[/s]"
        out.append(f"[color={hexcol}]{t}[/color]")
    return "".join(out)


_measure_cache: dict = {}


def make_label(markup: str, style: NativeStyle, px, max_w: float | None, halign: str = "left") -> CoreLabel:
    size = max(1, int(round(px(style.font_size))))
    kwargs = dict(text=markup, markup=True, font_size=size, halign=halign, bold=style.bold,
                  italic=style.italic, text_size=(max_w, None))
    fn = _font_name(style.font_family)
    if fn:
        kwargs["font_name"] = fn
    lh = style.line_height
    if isinstance(lh, LineHeightMultiple):
        kwargs["line_height"] = lh.value
    elif isinstance(lh, LineHeightFixed):
        kwargs["line_height"] = px(lh.length) / max(size, 1)
    label = CoreLabel(**kwargs)
    label.refresh()
    return label


def measure_text(markup: str, style: NativeStyle, px, max_w: float | None) -> tuple[float, float]:
    """Natural size of the text; if it does not fit in `max_w`, it wraps and returns the widest line."""
    if not markup:
        return 0.0, px(style.font_size) * 1.2
    key = (markup, style.font_size, style.bold, style.italic, style.font_family, style.line_height,
           None if max_w is None else round(max_w, 1), px(style.font_size))
    hit = _measure_cache.get(key)
    if hit is not None:
        return hit
    label = make_label(markup, style, px, None)
    w, h = label.content_size if label.texture else (0, 0)
    if max_w is not None and w > max_w:
        label = make_label(markup, style, px, max_w)
        lines = getattr(label, "_cached_lines", None)
        w = max((ln.w for ln in lines), default=max_w) if lines else max_w
        h = label.content_size[1] if label.texture else h
    if len(_measure_cache) > 2000:
        _measure_cache.clear()
    _measure_cache[key] = (float(w), float(h))
    return float(w), float(h)


def text_gravity(style: NativeStyle, is_button: bool) -> tuple[str, str]:
    """(horizontal, vertical) = ('left'|'center'|'right', 'top'|'center'|'bottom'), like the Android host."""
    if is_button:
        h = {"CENTER": "center", "END": "right"}.get(style.text_align.name, "left")
        return h, "center"
    h = {"CENTER": "center", "END": "right"}.get(style.text_align.name, "left")
    if style.direction == Direction.COLUMN:
        if style.align_items != Align.STRETCH and style.text_align == TextAlign.START:
            h = {"CENTER": "center", "END": "right"}.get(style.align_items.name, "left")
        v = {"CENTER": "center", "END": "bottom"}.get(style.justify.name, "top")
    else:
        if style.justify != Justify.START:
            h = {"CENTER": "center", "END": "right"}.get(style.justify.name, "left")
        v = {"CENTER": "center", "END": "bottom"}.get(style.align_items.name, "top")
    return h, v


# ---------------------------------------------------------------- base
class NLWidget(Widget):
    """A widget whose background we draw. `entry` links it to the model of the host."""

    def __init__(self, entry, **kw):
        super().__init__(size_hint=(None, None), **kw)
        self.entry = entry
        self.pressed = False
        self._touch_start = None
        self._moved = False

    # drawing: subclasses add on top of the background
    def redraw(self, px) -> None:
        e = self.entry
        st = e.style
        self.canvas.before.clear()
        self.canvas.clear()
        bg = border = None
        states = st.states
        if self.pressed and states and states.pressed:
            bg = states.pressed.background_color
            border = states.pressed.border_color
        elif e.is_disabled() and states and states.disabled:
            bg = states.disabled.background_color
            border = states.disabled.border_color
        elif getattr(self, "focused", False) and states and states.focused:
            bg = states.focused.background_color
            border = states.focused.border_color
        draw.draw_box(self.canvas.before, self.x, self.y, self.width, self.height, st, px, bg, border,
                      image=e.host.background_texture(e))
        self.draw_content(px)
        self.opacity = e.effective_opacity()

    def draw_content(self, px) -> None:
        pass

    # touches
    def wants_touch(self) -> bool:
        return self.entry.interactive() and not self.entry.is_disabled()

    def on_touch_down(self, touch):
        if not self.collide_point(*touch.pos):
            return False
        if self.entry.is_layer:
            if self.wants_touch():
                pass
            self._touch_start = touch.pos
            self._moved = False
            touch.grab(self)
            return True
        if not self.wants_touch():
            return False
        self._touch_start = touch.pos
        self._moved = False
        self.pressed = True
        touch.grab(self)
        self.entry.host.redraw_entry(self.entry)
        return True

    def on_touch_move(self, touch):
        if touch.grab_current is self and self._touch_start is not None:
            dx = touch.pos[0] - self._touch_start[0]
            dy = touch.pos[1] - self._touch_start[1]
            if abs(dx) > TAP_SLOP or abs(dy) > TAP_SLOP:
                self._moved = True
                if self.pressed:
                    self.pressed = False
                    self.entry.host.redraw_entry(self.entry)
        return False

    def on_touch_up(self, touch):
        if touch.grab_current is self:
            touch.ungrab(self)
            was = self.pressed
            self.pressed = False
            if was:
                self.entry.host.redraw_entry(self.entry)
            if was and not self._moved and self.collide_point(*touch.pos):
                self.on_tap()
            return True
        return False

    def on_tap(self) -> None:
        self.entry.host.publish(self.entry.node.key)


class NLBoxWidget(NLWidget):
    def draw_content(self, px) -> None:
        e = self.entry
        mb = e.marker_box
        if mb is None:
            return
        mx, my, mw, mh = mb
        label = make_label(escape_markup(e.node.marker + " "), e.style, px, None)
        tex = label.texture
        if tex is None:
            return
        self.canvas.add(Color(*draw.rgba(e.style.text_color)))
        self.canvas.add(Rectangle(texture=tex, pos=(self.x + mx, self.y + self.height - my - tex.size[1]),
                                  size=tex.size))


class NLTextWidget(NLWidget):
    """Text (and buttons): draws the text inside the content area, with the gravity of the style."""

    def draw_content(self, px) -> None:
        e = self.entry
        st = e.style
        markup = e.markup()
        if not markup:
            return
        pl, pr = px(st.padding.left), px(st.padding.right)
        pt, pb = px(st.padding.top), px(st.padding.bottom)
        cw = max(self.width - pl - pr, 1.0)
        ch = max(self.height - pt - pb, 1.0)
        hal, val = text_gravity(st, e.is_button)
        label = make_label(markup, st, px, cw, hal)
        tex = label.texture
        if tex is None:
            return
        tw, th = tex.size
        x = self.x + pl
        if hal == "center":
            x += (cw - tw) / 2
        elif hal == "right":
            x += cw - tw
        if val == "center":
            y = self.y + pb + (ch - th) / 2
        elif val == "bottom":
            y = self.y + pb
        else:
            y = self.y + pb + ch - th
        self.canvas.add(Color(1, 1, 1, 1))
        self.canvas.add(Rectangle(texture=tex, pos=(x, y), size=(tw, th)))


class NLCheckWidget(NLWidget):
    def __init__(self, entry, **kw):
        super().__init__(entry, **kw)
        self.active = False

    def draw_content(self, px) -> None:
        e = self.entry
        st = e.style
        s = min(self.width, self.height)
        x = self.x + (self.width - s) / 2
        y = self.y + (self.height - s) / 2
        box = s * 0.72
        bx, by = x + (s - box) / 2, y + (s - box) / 2
        if self.active:
            self.canvas.add(Color(*draw.rgba(0xFF2F81F7)))
            self.canvas.add(draw.RoundedRectangle(pos=(bx, by), size=(box, box), radius=[box * 0.15] * 4))
            self.canvas.add(Color(1, 1, 1, 1))
            self.canvas.add(Line(points=[bx + box * 0.2, by + box * 0.5, bx + box * 0.42, by + box * 0.28,
                                         bx + box * 0.8, by + box * 0.74],
                                 width=max(1.5, box * 0.12), cap="round", joint="round"))
        else:
            self.canvas.add(Color(*draw.rgba(0xFF767676)))
            self.canvas.add(Line(rounded_rectangle=(bx, by, box, box, box * 0.15), width=max(1.5, box * 0.09)))

    def on_tap(self) -> None:
        self.active = not self.active
        self.entry.host.redraw_entry(self.entry)
        if self.entry.interactive():
            self.entry.host.publish(self.entry.node.key)

    def wants_touch(self) -> bool:
        return not self.entry.is_disabled()


class NLImageWidget(NLWidget):
    def draw_content(self, px) -> None:
        e = self.entry
        st = e.style
        icon = e.icon_name()
        if icon is not None:
            s = min(self.width, self.height)
            draw.draw_icon(self.canvas, icon, self.x + (self.width - s) / 2, self.y + (self.height - s) / 2,
                           s, st.text_color)
            return
        tex = e.texture
        if tex is None:
            self.canvas.add(Color(*draw.rgba(0xFFDDDDDD)))
            self.canvas.add(Rectangle(pos=self.pos, size=self.size))
            return
        # object-fit: contain (default), cover or fill; the image is always centered
        draw.draw_image(self.canvas, tex, self.x, self.y, self.width, self.height, st.object_fit, ImagePos.CENTER)


class NLFieldWidget(TextInput):
    """Text field: we draw the background and the border; the TextInput only shows the text."""

    def __init__(self, entry, **kw):
        super().__init__(size_hint=(None, None), background_normal="", background_active="",
                         background_disabled_normal="",
                         background_color=(0, 0, 0, 0), write_tab=False, **kw)
        self.entry = entry
        self.focused = False
        self.bind(focus=self._on_focus)

    def _on_focus(self, _w, value) -> None:
        self.focused = bool(value)
        self.entry.host.redraw_entry(self.entry)

    def redraw(self, px) -> None:
        e = self.entry
        st = e.style
        self.canvas.before.clear()
        bg = border = None
        states = st.states
        if e.is_disabled() and states and states.disabled:
            bg, border = states.disabled.background_color, states.disabled.border_color
        elif self.focused and states and states.focused:
            bg, border = states.focused.background_color, states.focused.border_color
        draw.draw_box(self.canvas.before, self.x, self.y, self.width, self.height, st, px, bg, border,
                      image=e.host.background_texture(e))
        pad = [px(st.padding.left), px(st.padding.top), px(st.padding.right), px(st.padding.bottom)]
        self.padding = pad
        self.font_size = max(1, int(round(px(st.font_size))))
        self.foreground_color = draw.rgba(st.text_color)
        self.cursor_color = draw.rgba(st.text_color)
        self.hint_text_color = draw.rgba((st.text_color & 0x00FFFFFF) | 0x80000000)
        self.opacity = e.effective_opacity()
        self.disabled = e.is_disabled()
        fn = _font_name(st.font_family)
        if fn:
            self.font_name = fn
