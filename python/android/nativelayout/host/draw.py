"""Drawing with Kivy: colors, shapes with rounded corners, gradients and icons."""
from __future__ import annotations

import math

from kivy.graphics import Color, Ellipse, Line, Rectangle, RoundedRectangle
from kivy.graphics.texture import Texture

from nativelayout.core import icons as IconSet
from nativelayout.core.tree import Corners, ImageFit, ImagePos, Length, NativeStyle


def rgba(c: int, opacity: float = 1.0) -> tuple[float, float, float, float]:
    """Unsigned ARGB integer -> (r, g, b, a) from 0 to 1."""
    return (((c >> 16) & 255) / 255.0, ((c >> 8) & 255) / 255.0, (c & 255) / 255.0,
            ((c >> 24) & 255) / 255.0 * opacity)


def radii_of(style: NativeStyle, px) -> list[float]:
    """[top-left, top-right, bottom-right, bottom-left] in pixels."""
    c: Corners | None = style.corners
    if c is not None:
        return [px(c.top_left), px(c.top_right), px(c.bottom_right), px(c.bottom_left)]
    r = px(style.border_radius)
    return [r, r, r, r]


_gradient_cache: dict = {}


def gradient_texture(angle: int, colors: tuple[int, ...], size: int = 48) -> Texture:
    """Small texture with the CSS gradient (angle in degrees, 0 = up, 90 = to the right)."""
    key = (angle, colors, size)
    tex = _gradient_cache.get(key)
    if tex is not None:
        return tex
    rad = math.radians(angle)
    dx, dy = math.sin(rad), -math.cos(rad)          # direction in screen coordinates (y down)
    half = (abs(dx) + abs(dy)) / 2.0 or 1.0
    stops = [rgba(c) for c in colors]
    n = len(stops) - 1
    buf = bytearray()
    for row in range(size):                         # a Kivy texture has its origin at the bottom
        sy = (size - 1 - row) / (size - 1) - 0.5
        for col in range(size):
            sx = col / (size - 1) - 0.5
            t = min(max((sx * dx + sy * dy) / (2 * half) + 0.5, 0.0), 1.0)
            f = t * n
            i = min(int(f), n - 1)
            k = f - i
            a, b = stops[i], stops[i + 1]
            for ch in range(4):
                buf.append(int(255 * (a[ch] + (b[ch] - a[ch]) * k)))
    tex = Texture.create(size=(size, size), colorfmt="rgba")
    tex.blit_buffer(bytes(buf), colorfmt="rgba", bufferfmt="ubyte")
    tex.mag_filter = tex.min_filter = "linear"
    if len(_gradient_cache) > 64:
        _gradient_cache.clear()
    _gradient_cache[key] = tex
    return tex


def draw_box(canvas, x: float, y: float, w: float, h: float, style: NativeStyle, px,
             background: int | None = None, border: int | None = None, opacity: float = 1.0,
             image: Texture | None = None) -> None:
    """Background, gradient, background image, shadow and border of a box. (x, y) is the bottom-left corner (Kivy)."""
    if w <= 0 or h <= 0:
        return
    radii = radii_of(style, px)
    r = [min(v, w / 2, h / 2) for v in radii]
    elevation = px(style.elevation)
    if elevation > 0.5 and (style.background_color or style.gradient or background):
        steps = 4
        for i in range(steps, 0, -1):
            grow = elevation * i / steps
            canvas.add(Color(0, 0, 0, 0.06 * opacity))
            canvas.add(RoundedRectangle(pos=(x - grow * 0.4, y - grow * 0.9),
                                        size=(w + grow * 0.8, h + grow * 0.8),
                                        radius=[v + grow * 0.4 for v in r]))
    if background is None and style.gradient is not None:
        g = style.gradient
        canvas.add(Color(1, 1, 1, opacity))
        canvas.add(RoundedRectangle(pos=(x, y), size=(w, h), radius=r,
                                    texture=gradient_texture(g.angle, g.colors)))
    else:
        bg = background if background is not None else style.background_color
        if bg:
            canvas.add(Color(*rgba(bg, opacity)))
            canvas.add(RoundedRectangle(pos=(x, y), size=(w, h), radius=r))
    if image is not None:
        draw_image(canvas, image, x, y, w, h, style.background_size, style.background_position, r, opacity)
    bw = px(style.border_width)
    if bw > 0:
        bw = max(1.0, bw)
        canvas.add(Color(*rgba(border if border is not None else style.border_color, opacity)))
        inset = bw / 2
        canvas.add(Line(rounded_rectangle=(x + inset, y + inset, w - bw, h - bw,
                                           *[max(v - inset, 0) for v in r]), width=bw))


def _sub_tex_coords(tex: Texture, a0: float, a1: float, b0: float, b1: float) -> tuple:
    """Texture coordinates of the part of `tex` between a0..a1 (left to right) and b0..b1 (bottom to top).

    Uses the coordinates the texture already has, so it works whatever its orientation (loaded images are flipped).
    """
    u_bl, v_bl, u_br, v_br, u_tr, v_tr, u_tl, v_tl = tex.tex_coords

    def at(a: float, b: float) -> tuple[float, float]:
        u = u_bl * (1 - a) * (1 - b) + u_br * a * (1 - b) + u_tr * a * b + u_tl * (1 - a) * b
        v = v_bl * (1 - a) * (1 - b) + v_br * a * (1 - b) + v_tr * a * b + v_tl * (1 - a) * b
        return u, v

    return (*at(a0, b0), *at(a1, b0), *at(a1, b1), *at(a0, b1))


def draw_image(canvas, tex: Texture, x: float, y: float, w: float, h: float, fit: ImageFit,
               pos: ImagePos = ImagePos.CENTER, radii: list[float] | None = None, opacity: float = 1.0) -> None:
    """Draws `tex` in the box (x, y, w, h), (x, y) being the bottom-left corner.

    COVER fills the box and cuts what does not fit, CONTAIN shows the whole image, FILL stretches it.
    `pos` says which part stays in view (cover) or on which side the image sits (contain). It follows
    the Android host (ImageBackground). COVER and FILL follow the rounded corners given in `radii`.
    """
    tw, th = tex.size
    if tw <= 0 or th <= 0 or w <= 0 or h <= 0:
        return
    canvas.add(Color(1, 1, 1, opacity))
    rounded = radii is not None and any(r > 0 for r in radii)

    def box(coords, px_, py_, pw_, ph_, round_corners):
        if round_corners:
            canvas.add(RoundedRectangle(texture=tex, tex_coords=coords, pos=(px_, py_), size=(pw_, ph_), radius=radii))
        else:
            canvas.add(Rectangle(texture=tex, tex_coords=coords, pos=(px_, py_), size=(pw_, ph_)))

    if fit == ImageFit.FILL:
        box(_sub_tex_coords(tex, 0, 1, 0, 1), x, y, w, h, rounded)
    elif fit == ImageFit.COVER:
        scale = max(w / tw, h / th)
        sw, sh = w / scale, h / scale                      # part of the image that stays in view, in image pixels
        sx = 0.0 if pos == ImagePos.LEFT else (tw - sw) if pos == ImagePos.RIGHT else (tw - sw) / 2
        sy = 0.0 if pos == ImagePos.TOP else (th - sh) if pos == ImagePos.BOTTOM else (th - sh) / 2   # from the top
        coords = _sub_tex_coords(tex, sx / tw, (sx + sw) / tw, 1 - (sy + sh) / th, 1 - sy / th)
        box(coords, x, y, w, h, rounded)
    else:
        scale = min(w / tw, h / th)
        dw, dh = tw * scale, th * scale
        dx = 0.0 if pos == ImagePos.LEFT else (w - dw) if pos == ImagePos.RIGHT else (w - dw) / 2
        dy = 0.0 if pos == ImagePos.TOP else (h - dh) if pos == ImagePos.BOTTOM else (h - dh) / 2     # from the top
        box(_sub_tex_coords(tex, 0, 1, 0, 1), x + dx, y + h - dy - dh, dw, dh, False)


def draw_icon(canvas, name: str, x: float, y: float, size: float, color: int, opacity: float = 1.0) -> None:
    """Draws a built-in icon (24x24 grid, stroke 2, round caps). (x, y) is the bottom-left corner."""
    shapes = IconSet.shapes_of(name)
    if shapes is None:
        return
    s = size / IconSet.GRID
    top = y + size

    def X(v): return x + v * s
    def Y(v): return top - v * s

    canvas.add(Color(*rgba(color, opacity)))
    for sh in shapes:
        if isinstance(sh, IconSet.Line):
            pts = []
            for i in range(0, len(sh.points), 2):
                pts += [X(sh.points[i]), Y(sh.points[i + 1])]
            canvas.add(Line(points=pts, width=IconSet.STROKE * s, cap="round", joint="round", close=sh.closed))
        elif isinstance(sh, IconSet.Circle):
            canvas.add(Line(circle=(X(sh.cx), Y(sh.cy), sh.r * s), width=IconSet.STROKE * s))
        elif isinstance(sh, IconSet.Dot):
            r = IconSet.DOT_RADIUS * s
            canvas.add(Ellipse(pos=(X(sh.cx) - r, Y(sh.cy) - r), size=(2 * r, 2 * r)))
        elif isinstance(sh, IconSet.Rect):
            canvas.add(Line(rounded_rectangle=(X(sh.left), Y(sh.bottom), (sh.right - sh.left) * s,
                                               (sh.bottom - sh.top) * s, sh.radius * s),
                            width=IconSet.STROKE * s))
