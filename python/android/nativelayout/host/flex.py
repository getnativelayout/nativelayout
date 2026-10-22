"""Motor de layout flex em Python puro (sem Kivy), usado pelo host.

Replica a semantica do host Android (LinearLayout + NLBox): colunas e linhas, `flex`, `gap`,
`justify-content`, `align-items`/`align-self`, `flex-wrap`, margens, padding, min/max e percentagens.

Cada `LNode` tem o estilo, os filhos e (se for folha) uma funcao `leaf(max_w) -> (w, h)` que mede
o conteudo sem padding. Depois de `layout`, cada no tem `x, y, w, h` (caixa exterior, relativa a
caixa do pai).
"""
from __future__ import annotations

from nativelayout.core.tree import (Align, Dim, Direction, Justify, Length, NativeStyle, Percent)

INF = float("inf")


class Metrics:
    def __init__(self, density: float = 1.0, font_scale: float = 1.0):
        self.density = density
        self.font_scale = font_scale

    def px(self, l: Length | None) -> float:
        return 0.0 if l is None else l.to_pixels(self.density, self.font_scale)


class LNode:
    def __init__(self, style: NativeStyle, children: list["LNode"] | None = None, leaf=None, key=None):
        self.style = style
        self.children = children if children is not None else []
        self.leaf = leaf
        self.key = key
        self.x = self.y = self.w = self.h = 0.0


def _clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def _limits(n: LNode, m: Metrics):
    s = n.style
    minw = m.px(s.min_width) if s.min_width is not None else 0.0
    minh = m.px(s.min_height) if s.min_height is not None else 0.0
    maxw = m.px(s.max_width) if s.max_width is not None else INF
    maxh = m.px(s.max_height) if s.max_height is not None else INF
    return minw, maxw, minh, maxh


def layout(root: LNode, m: Metrics, width: float, height: float, fill_height: bool = True) -> None:
    """Posiciona a arvore dentro de `width` x `height` (o viewport). A raiz ocupa a largura toda;
    se o conteudo for mais baixo, estica ate ao viewport (como um ScrollView com fillViewport)."""
    s = root.style
    mg = s.margin
    ml, mr, mt, mb = m.px(mg.left), m.px(mg.right), m.px(mg.top), m.px(mg.bottom)
    fw = m.px(s.width.length) if isinstance(s.width, Dim) else max(width - ml - mr, 0.0)
    fh = None
    if s.flex > 0 or (isinstance(s.height, Percent) and s.height.value >= 100):
        fh = max(height - mt - mb, 0.0)
    elif isinstance(s.height, Dim):
        fh = m.px(s.height.length)
    _lay(root, m, fw, INF, fw, fh)
    if fh is None and fill_height and root.h < height - mt - mb:
        _lay(root, m, fw, INF, fw, height - mt - mb)
    root.x, root.y = ml, mt


def _lay(n: LNode, m: Metrics, aw: float, ah: float, fw: float | None, fh: float | None):
    """Mede e posiciona `n`. `aw/ah`: espaco maximo; `fw/fh`: tamanho exato imposto (ou None)."""
    st = n.style
    minw, maxw, minh, maxh = _limits(n, m)
    if fw is not None:
        fw = _clamp(fw, minw, maxw)
    if fh is not None:
        fh = _clamp(fh, minh, maxh)
    pl, pr = m.px(st.padding.left), m.px(st.padding.right)
    pt, pb = m.px(st.padding.top), m.px(st.padding.bottom)
    ph, pv = pl + pr, pt + pb
    aw = min(aw, maxw)
    ah = min(ah, maxh)
    inner_aw = (fw if fw is not None else aw) - ph
    inner_ah = (fh if fh is not None else ah) - pv

    if n.leaf is not None or not n.children:
        if n.leaf is not None:
            cw, ch = n.leaf(inner_aw if inner_aw != INF else None)
        else:
            cw = ch = 0.0
        w = fw if fw is not None else min(cw + ph, aw)
        h = fh if fh is not None else min(ch + pv, ah if ah != INF else ch + pv)
        n.w = _clamp(w, minw, maxw)
        n.h = _clamp(h, minh, maxh)
        return n.w, n.h

    vertical = st.direction == Direction.COLUMN
    gap = m.px(st.gap)
    wrapped = st.wrap and not vertical

    # eixo principal (M) e cruzado (C), a partir de (largura, altura)
    def split(w, h):
        return (h, w) if vertical else (w, h)

    def join(mn, cr):
        return (cr, mn) if vertical else (mn, cr)

    inner_main = (fh if vertical else fw)
    inner_main = None if inner_main is None else inner_main - (pv if vertical else ph)
    inner_cross = (fw if vertical else fh)
    inner_cross = None if inner_cross is None else inner_cross - (ph if vertical else pv)
    avail_main = (inner_ah if vertical else inner_aw)
    avail_cross = (inner_aw if vertical else inner_ah)

    kids = n.children

    def margins(c):
        g = c.style.margin
        l, r, t, b = m.px(g.left), m.px(g.right), m.px(g.top), m.px(g.bottom)
        return (t, b, l, r) if vertical else (l, r, t, b)   # (main_before, main_after, cross_before, cross_after)

    def child_specs(c):
        cs = c.style
        main_sz, cross_sz = (cs.height, cs.width) if vertical else (cs.width, cs.height)
        align = cs.align_self or st.align_items
        mb, ma, cb, ca = margins(c)
        # cruzado
        cross_fixed = None
        cross_fill = False
        if isinstance(cross_sz, Dim):
            cross_fixed = m.px(cross_sz.length)
        elif isinstance(cross_sz, Percent):
            if cross_sz.value >= 100:
                cross_fill = True
            elif inner_cross is not None:
                cross_fixed = inner_cross * cross_sz.value / 100.0
        elif align == Align.STRETCH:
            cross_fill = True
        if cross_fill and inner_cross is not None:
            cross_fixed = max(inner_cross - cb - ca, 0.0)
            cross_fill = False
        # principal
        main_fixed = None
        if isinstance(main_sz, Dim):
            main_fixed = m.px(main_sz.length)
        elif isinstance(main_sz, Percent) and inner_main is not None:
            main_fixed = inner_main * (min(main_sz.value, 100.0)) / 100.0
        flex = cs.flex if (cs.flex > 0 and not wrapped) else 0.0
        return main_fixed, cross_fixed, cross_fill, flex, align, (mb, ma, cb, ca)

    def call(c, main_fixed, cross_fixed, avail_m, avail_c):
        fw_, fh_ = join(main_fixed, cross_fixed)
        aw_, ah_ = join(avail_m, avail_c)
        return _lay(c, m, aw_, ah_, fw_, fh_)

    specs = [child_specs(c) for c in kids]
    sizes: list[tuple[float, float]] = [(0.0, 0.0)] * len(kids)   # (main, cross) de cada filho

    if wrapped:
        return _lay_wrap(n, m, kids, specs, call, inner_aw, aw, ah, fw, fh, st, pl, pt, ph, pv, gap,
                         minw, maxw, minh, maxh)

    # passo 1: filhos sem flex
    total_main = 0.0
    flex_sum = 0.0
    for i, c in enumerate(kids):
        main_fixed, cross_fixed, cross_fill, flex, align, (mb, ma, cb, ca) = specs[i]
        if flex > 0:
            flex_sum += flex
            total_main += mb + ma
            continue
        avail_c = (avail_cross - cb - ca) if avail_cross != INF else INF
        avail_m = (avail_main - total_main - mb - ma) if avail_main != INF else INF
        w, h = call(c, main_fixed, cross_fixed, max(avail_m, 0.0) if avail_m != INF else INF, max(avail_c, 0.0) if avail_c != INF else INF)
        mn, cr = split(w, h)
        sizes[i] = (mn, cr)
        total_main += mn + mb + ma
    count = len(kids)
    gaps = gap * (count - 1) if count > 1 else 0.0
    total_main += gaps

    # passo 2: filhos com flex (so crescem se o eixo principal do pai for exato)
    if flex_sum > 0:
        free = (inner_main - total_main) if inner_main is not None else 0.0
        free = max(free, 0.0)
        for i, c in enumerate(kids):
            main_fixed, cross_fixed, cross_fill, flex, align, (mb, ma, cb, ca) = specs[i]
            if flex <= 0:
                continue
            share = free * flex / flex_sum
            avail_c = (avail_cross - cb - ca) if avail_cross != INF else INF
            w, h = call(c, share, cross_fixed, share, max(avail_c, 0.0) if avail_c != INF else INF)
            mn, cr = split(w, h)
            sizes[i] = (mn, cr)
            total_main += mn
    # dimensao principal do pai
    content_main = total_main
    if inner_main is not None:
        main_size = inner_main
    else:
        main_size = min(content_main, avail_main) if avail_main != INF else content_main
    # dimensao cruzada do pai
    max_cross = 0.0
    for i in range(count):
        _, _, _, _, _, (mb, ma, cb, ca) = specs[i]
        max_cross = max(max_cross, sizes[i][1] + cb + ca)
    cross_size = inner_cross if inner_cross is not None else max_cross
    if inner_cross is None and avail_cross != INF:
        cross_size = min(cross_size, avail_cross)

    # filhos com cruzado "stretch" que ficaram mais estreitos que o pai: voltam a ser medidos
    for i, c in enumerate(kids):
        main_fixed, cross_fixed, cross_fill, flex, align, (mb, ma, cb, ca) = specs[i]
        if cross_fill:
            target = max(cross_size - cb - ca, 0.0)
            if abs(sizes[i][1] - target) > 0.5:
                w, h = call(c, sizes[i][0], target, sizes[i][0], target)
                sizes[i] = split(w, h)

    # justify
    free = max(main_size - content_main, 0.0)
    j = st.justify
    start = 0.0
    between = gap
    if count:
        if j == Justify.CENTER:
            start = free / 2
        elif j == Justify.END:
            start = free
        elif j == Justify.BETWEEN and count > 1:
            between = gap + free / (count - 1)
        elif j == Justify.AROUND:
            start = free / (2 * count)
            between = gap + free / count
        elif j == Justify.EVENLY:
            start = free / (count + 1)
            between = gap + free / (count + 1)
    pos = start
    pm, pc = (pt, pl) if vertical else (pl, pt)
    for i, c in enumerate(kids):
        main_fixed, cross_fixed, cross_fill, flex, align, (mb, ma, cb, ca) = specs[i]
        pos += mb
        mn, cr = sizes[i]
        if align == Align.CENTER:
            cpos = (cross_size - cr - cb - ca) / 2 + cb
        elif align == Align.END:
            cpos = cross_size - cr - ca
        else:
            cpos = cb
        if vertical:
            c.x, c.y = pc + cpos, pm + pos
        else:
            c.x, c.y = pm + pos, pc + cpos
        pos += mn + ma + between

    w, h = join(main_size, cross_size)
    w += ph
    h += pv
    n.w = _clamp(fw if fw is not None else w, minw, maxw)
    n.h = _clamp(fh if fh is not None else h, minh, maxh)
    return n.w, n.h


def _lay_wrap(n, m, kids, specs, call, inner_aw, aw, ah, fw, fh, st, pl, pt, ph, pv, gap,
              minw, maxw, minh, maxh):
    """Linha com `flex-wrap: wrap`: o flex nao cresce, cada filho mantem a sua largura."""
    avail = inner_aw if inner_aw != INF else INF
    lines: list[list[tuple[int, float, float]]] = [[]]
    line_w = 0.0
    for i, c in enumerate(kids):
        main_fixed, cross_fixed, cross_fill, flex, align, (mb, ma, cb, ca) = specs[i]
        w, h = call(c, main_fixed, cross_fixed, avail if avail != INF else INF, INF)
        cw = w + mb + ma
        ch = h + cb + ca
        if lines[-1] and line_w + gap + cw > avail:
            lines.append([])
            line_w = 0.0
        line_w += (gap if lines[-1] else 0.0) + cw
        lines[-1].append((i, w, h))
    y = 0.0
    max_w = 0.0
    inner_w = (fw - ph) if fw is not None else None
    for line in lines:
        if not line:
            continue
        lh = max(h + specs[i][5][2] + specs[i][5][3] for i, w, h in line)
        tw = sum(w + specs[i][5][0] + specs[i][5][1] for i, w, h in line) + gap * (len(line) - 1)
        free = max((inner_w if inner_w is not None else tw) - tw, 0.0)
        j = st.justify
        x = free / 2 if j == Justify.CENTER else free if j == Justify.END else 0.0
        between = gap
        k = len(line)
        if j == Justify.BETWEEN and k > 1:
            between = gap + free / (k - 1)
        elif j == Justify.AROUND:
            x = free / (2 * k)
            between = gap + free / k
        elif j == Justify.EVENLY:
            x = free / (k + 1)
            between = gap + free / (k + 1)
        for i, w, h in line:
            c = kids[i]
            mb, ma, cb, ca = specs[i][5]
            align = specs[i][4]
            x += mb
            if align == Align.CENTER:
                cy = (lh - h - cb - ca) / 2 + cb
            elif align == Align.END:
                cy = lh - h - ca
            else:
                cy = cb
            c.x, c.y = pl + x, pt + y + cy
            x += w + ma + between
        max_w = max(max_w, tw)
        y += lh + gap
    total_h = max(y - gap, 0.0)
    w = (fw if fw is not None else max_w + ph)
    h = (fh if fh is not None else total_h + pv)
    n.w = _clamp(w, minw, maxw)
    n.h = _clamp(h, minh, maxh)
    return n.w, n.h


def lay_node(n: LNode, m: Metrics, aw: float, ah: float, fw: float | None, fh: float | None):
    """Mede e posiciona um no com limites/tamanho exato dados (usado por cabecalho, rodape e camadas)."""
    return _lay(n, m, aw, ah, fw, fh)
