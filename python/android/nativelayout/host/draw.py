"""Desenho com Kivy: cores, formas com cantos redondos, gradientes e icones."""
from __future__ import annotations

import math

from kivy.graphics import Color, Ellipse, Line, Rectangle, RoundedRectangle
from kivy.graphics.texture import Texture

from nativelayout.core import icons as IconSet
from nativelayout.core.tree import Corners, Length, NativeStyle


def rgba(c: int, opacity: float = 1.0) -> tuple[float, float, float, float]:
    """Inteiro ARGB sem sinal -> (r, g, b, a) de 0 a 1."""
    return (((c >> 16) & 255) / 255.0, ((c >> 8) & 255) / 255.0, (c & 255) / 255.0,
            ((c >> 24) & 255) / 255.0 * opacity)


def radii_of(style: NativeStyle, px) -> list[float]:
    """[top-left, top-right, bottom-right, bottom-left] em pixeis."""
    c: Corners | None = style.corners
    if c is not None:
        return [px(c.top_left), px(c.top_right), px(c.bottom_right), px(c.bottom_left)]
    r = px(style.border_radius)
    return [r, r, r, r]


_gradient_cache: dict = {}


def gradient_texture(angle: int, colors: tuple[int, ...], size: int = 48) -> Texture:
    """Textura pequena com o gradiente CSS (angulo em graus, 0 = para cima, 90 = para a direita)."""
    key = (angle, colors, size)
    tex = _gradient_cache.get(key)
    if tex is not None:
        return tex
    rad = math.radians(angle)
    dx, dy = math.sin(rad), -math.cos(rad)          # direcao em coordenadas de ecra (y para baixo)
    half = (abs(dx) + abs(dy)) / 2.0 or 1.0
    stops = [rgba(c) for c in colors]
    n = len(stops) - 1
    buf = bytearray()
    for row in range(size):                         # a textura Kivy tem a origem em baixo
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
             background: int | None = None, border: int | None = None, opacity: float = 1.0) -> None:
    """Fundo, gradiente, sombra e borda de uma caixa. (x, y) e o canto inferior esquerdo (Kivy)."""
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
    bw = px(style.border_width)
    if bw > 0:
        bw = max(1.0, bw)
        canvas.add(Color(*rgba(border if border is not None else style.border_color, opacity)))
        inset = bw / 2
        canvas.add(Line(rounded_rectangle=(x + inset, y + inset, w - bw, h - bw,
                                           *[max(v - inset, 0) for v in r]), width=bw))


def draw_icon(canvas, name: str, x: float, y: float, size: float, color: int, opacity: float = 1.0) -> None:
    """Desenha um icone embutido (grelha 24x24, traco 2, pontas redondas). (x, y) em baixo a esquerda."""
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
