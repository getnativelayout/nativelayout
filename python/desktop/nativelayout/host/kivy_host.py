"""Host Kivy: aplica as mudancas da arvore nativa aos widgets e entrega os eventos ao programa."""
from __future__ import annotations

import asyncio
import base64
import io
import json
import os
from dataclasses import replace

from kivy.clock import Clock
from kivy.core.image import Image as CoreImage
from kivy.metrics import dp, sp
from kivy.uix.label import Label
from kivy.uix.relativelayout import RelativeLayout
from kivy.uix.scrollview import ScrollView

from nativelayout.core import icons as IconSet
from nativelayout.core.differ import (Create, Remove, Replace, SetItems, SetRoot, UpdateAttributes,
                                      UpdateStyle, UpdateText)
from nativelayout.core.layout_host import BACK_EVENT_KEY, LayoutEvent, LayoutHost
from nativelayout.core.tree import (Align, Dim, Direction, Dock, FieldType, Justify, Length, Native,
                                    NativeBox, NativeButton, NativeCheckbox, NativeField, NativeImage,
                                    NativeItem, NativeList, NativeStyle, NativeText, Percent, Span,
                                    native_children)

from . import draw, widgets
from .flex import LNode, Metrics, lay_node, layout


class Entry:
    """Um no da arvore nativa com o seu widget."""

    def __init__(self, host: "KivyLayoutHost", node: Native, parent: "Entry | None"):
        self.host = host
        self.node = node
        self.style: NativeStyle = node.style
        self.parent = parent
        self.children: list[Entry] = []
        self.spans: list[Span] | None = None
        self.texture = None
        self.widget = None
        self.lnode: LNode | None = None
        self.inner: LNode | None = None
        self.marker_box = None

    # ---- propriedades usadas pelos widgets
    @property
    def is_layer(self) -> bool:
        return isinstance(self.node, NativeBox) and self.node.overlay

    @property
    def is_header(self) -> bool:
        return isinstance(self.node, NativeBox) and self.node.header

    @property
    def is_footer(self) -> bool:
        return isinstance(self.node, NativeBox) and self.node.footer

    @property
    def is_special(self) -> bool:
        return self.is_layer or self.is_header or self.is_footer

    @property
    def is_button(self) -> bool:
        return isinstance(self.node, NativeButton)

    def interactive(self) -> bool:
        return self.node.id is not None or bool(self.node.classes) or isinstance(self.node, NativeButton)

    def is_disabled(self) -> bool:
        n = self.node
        return isinstance(n, (NativeButton, NativeField, NativeCheckbox)) and n.disabled

    def effective_opacity(self) -> float:
        a = self.style.opacity
        if self.is_disabled():
            d = self.style.states.disabled.opacity if (self.style.states and self.style.states.disabled) else None
            a *= d if d is not None else 0.5
        return a

    def markup(self) -> str:
        n = self.node
        px = self.host.px
        if isinstance(n, NativeText):
            return widgets.markup_of(self.spans if self.spans is not None else n.spans, px)
        if isinstance(n, NativeButton):
            st = self.style
            return widgets.markup_of([Span(n.label, st.bold, st.text_color, st.font_size, st.italic,
                                           st.underline, st.strike)], px)
        return ""

    def icon_name(self) -> str | None:
        n = self.node
        if isinstance(n, NativeImage):
            name = IconSet.name_of(n.source)
            if name is not None and IconSet.shapes_of(name) is not None:
                return name
        return None


class NLArea(RelativeLayout):
    """O widget que o programa coloca no seu ecra: contem cabecalho, scroll, rodape e camadas."""


class KivyLayoutHost(LayoutHost):
    def __init__(self, area: NLArea, image_resolver=None, store_path: str | None = None):
        self.area = area
        self.image_resolver = image_resolver or (lambda source: None)
        self._store_path = store_path
        self._store: dict | None = None

        self.entries: dict[str, Entry] = {}
        self.root: Entry | None = None
        self._structure_dirty = True
        self._scheduled = False
        self._closed = False

        self._queue: list[LayoutEvent] = []
        self._pending: asyncio.Future | None = None
        self._loop: asyncio.AbstractEventLoop | None = None

        self.scroll = ScrollView(do_scroll_x=False, bar_width=dp(3), size_hint=(None, None))
        self.content = RelativeLayout(size_hint=(None, None))
        self.scroll.add_widget(self.content)
        self.header_layer = RelativeLayout(size_hint=(None, None))
        self.footer_layer = RelativeLayout(size_hint=(None, None))
        self.float_layer = RelativeLayout(size_hint=(None, None))
        self.overlay_layer = RelativeLayout(size_hint=(None, None))
        for w in (self.scroll, self.header_layer, self.footer_layer, self.float_layer, self.overlay_layer):
            area.add_widget(w)
        area.bind(size=lambda *_: self.schedule_layout())

    # ---------------------------------------------------------------- unidades
    @property
    def metrics(self) -> Metrics:
        d = dp(1)
        return Metrics(d, sp(1) / d)

    def px(self, length: Length | None) -> float:
        return self.metrics.px(length)

    # ---------------------------------------------------------------- LayoutHost
    def apply_changes(self, changes) -> None:
        if self._closed:
            return
        for a in changes:
            if isinstance(a, SetRoot):
                self._set_root(a.node)
            elif isinstance(a, Create):
                parent = self.entries.get(a.parent_key)
                if parent is not None:
                    child = self._build(a.node, parent)
                    parent.children.insert(min(a.position, len(parent.children)), child)
            elif isinstance(a, Remove):
                self._remove(a.key)
            elif isinstance(a, Replace):
                self._replace(a)
            elif isinstance(a, UpdateText):
                e = self.entries.get(a.key)
                if e is not None:
                    e.spans = a.spans
            elif isinstance(a, UpdateStyle):
                e = self.entries.get(a.key)
                if e is not None:
                    e.style = a.style
            elif isinstance(a, UpdateAttributes):
                e = self.entries.get(a.key)
                if e is not None:
                    e.node = a.node
                    self._configure(e)
            elif isinstance(a, SetItems):
                e = self.entries.get(a.key)
                if e is not None:
                    for c in e.children:
                        self._discard(c)
                    e.children = [self._build(n, e) for n in a.items]
        self._structure_dirty = True
        self.schedule_layout()

    async def next_event(self) -> LayoutEvent | None:
        if self._queue:
            return self._queue.pop(0)
        if self._closed:
            return None
        self._loop = asyncio.get_running_loop()
        self._pending = self._loop.create_future()
        return await self._pending

    def _find(self, id: str) -> Entry | None:
        return self.entries.get("#" + id)

    def field_text(self, id: str) -> str | None:
        e = self._find(id)
        return e.widget.text if e is not None and isinstance(e.node, NativeField) else None

    def field_checked(self, id: str) -> bool | None:
        e = self._find(id)
        return e.widget.active if e is not None and isinstance(e.node, NativeCheckbox) else None

    def set_value(self, id: str, text: str) -> None:
        e = self._find(id)
        if e is None:
            return
        if isinstance(e.node, NativeField):
            e.widget.text = text
            e.widget.cursor = (len(text), 0) if "\n" not in text else e.widget.cursor
        elif isinstance(e.node, NativeCheckbox):
            e.widget.active = text.strip().lower() in ("1", "true", "on")
            self.redraw_entry(e)

    def show_error(self, text: str) -> None:
        self._reset()
        self.root = None
        label = Label(text=text, color=(1, 0.3, 0.3, 1), halign="left", valign="top",
                      size_hint=(None, None), markup=False)
        label.bind(width=lambda w, v: setattr(w, "text_size", (v - dp(24), None)))
        label.bind(texture_size=lambda w, s: setattr(w, "height", s[1] + dp(24)))
        label.width = self.area.width
        self.content.clear_widgets()
        self.content.add_widget(label)
        self.scroll.size = self.area.size
        self.scroll.pos = (0, 0)
        self.content.size = (self.area.width, max(label.height, self.area.height))
        label.pos = (0, self.content.height - label.height)

    # ---- armazenamento (JSON no directorio de dados da app)
    def _path(self) -> str:
        if self._store_path:
            return self._store_path
        try:
            from kivy.app import App
            base = App.get_running_app().user_data_dir
        except Exception:
            base = os.getcwd()
        return os.path.join(base, "nativelayout.json")

    def _data(self) -> dict:
        if self._store is None:
            try:
                with open(self._path(), "r", encoding="utf-8") as f:
                    self._store = json.load(f)
            except Exception:
                self._store = {}
        return self._store

    def _flush(self) -> None:
        try:
            path = self._path()
            os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                json.dump(self._data(), f, ensure_ascii=False)
        except Exception:
            pass

    def save(self, key: str, text: str) -> None:
        self._data()[key] = text
        self._flush()

    def load(self, key: str) -> str | None:
        return self._data().get(key)

    def exists(self, key: str) -> bool:
        return key in self._data()

    def delete(self, key: str) -> None:
        self._data().pop(key, None)
        self._flush()

    def close(self) -> None:
        if self._closed:
            return
        self._closed = True
        self._queue.clear()
        self._reset()
        pending, self._pending = self._pending, None
        if pending is not None and not pending.done():
            self._loop.call_soon_threadsafe(lambda: pending.done() or pending.set_result(None))

    def post_back(self) -> None:
        self.publish(BACK_EVENT_KEY)

    def publish(self, key: str) -> None:
        if self._closed:
            return
        event = LayoutEvent(key)
        pending = self._pending
        if pending is not None and not pending.done():
            self._pending = None
            self._loop.call_soon_threadsafe(lambda: pending.done() or pending.set_result(event))
        else:
            self._queue.append(event)

    # ---------------------------------------------------------------- modelo
    def _reset(self) -> None:
        self.entries.clear()
        self.root = None
        for layer in (self.content, self.header_layer, self.footer_layer, self.float_layer, self.overlay_layer):
            layer.clear_widgets()

    def _set_root(self, node: Native) -> None:
        self._reset()
        self.root = self._build(node, None)
        self.content.opacity = 0.0
        from kivy.animation import Animation
        Animation(opacity=1.0, duration=0.22, t="out_quad").start(self.content)

    def _remove(self, key: str) -> None:
        e = self.entries.get(key)
        if e is None:
            return
        if e is self.root:
            self._reset()
            return
        if e.parent is not None and e in e.parent.children:
            e.parent.children.remove(e)
        self._discard(e)

    def _replace(self, a: Replace) -> None:
        old = self.entries.get(a.key)
        if old is None:
            return
        if old is self.root:
            self._set_root(a.node)
            return
        parent = old.parent
        index = parent.children.index(old)
        self._discard(old)
        parent.children[index] = self._build(a.node, parent)

    def _discard(self, e: Entry) -> None:
        self.entries.pop(e.node.key, None)
        for c in e.children:
            self._discard(c)

    def _build(self, node: Native, parent: Entry | None) -> Entry:
        e = Entry(self, node, parent)
        if isinstance(node, (NativeBox, NativeList, NativeItem)):
            e.widget = widgets.NLBoxWidget(e)
        elif isinstance(node, (NativeText, NativeButton)):
            e.widget = widgets.NLTextWidget(e)
        elif isinstance(node, NativeField):
            e.widget = widgets.NLFieldWidget(e)
            e.widget.text = node.value
        elif isinstance(node, NativeCheckbox):
            e.widget = widgets.NLCheckWidget(e)
            e.widget.active = node.checked
        else:
            e.widget = widgets.NLImageWidget(e)
        self.entries[node.key] = e
        self._configure(e)
        for c in native_children(node):
            e.children.append(self._build(c, e))
        return e

    def _configure(self, e: Entry) -> None:
        n = e.node
        w = e.widget
        if isinstance(n, NativeField):
            w.hint_text = n.hint
            w.multiline = n.type == FieldType.AREA
            w.password = n.type == FieldType.PASSWORD
            w.input_filter = "float" if n.type == FieldType.NUMBER else None
            w.readonly = n.read_only
        elif isinstance(n, NativeImage):
            e.texture = self._load_texture(n.source) if e.icon_name() is None else None

    def _load_texture(self, source: str):
        try:
            if source.startswith("data:"):
                head, _, payload = source.partition(",")
                ext = head[5:].split(";")[0].split("/")[-1] or "png"
                return CoreImage(io.BytesIO(base64.b64decode(payload)), ext=ext).texture
            r = self.image_resolver(source)
            return getattr(r, "texture", r)
        except Exception:
            return None

    # ---------------------------------------------------------------- layout
    def schedule_layout(self) -> None:
        if not self._scheduled:
            self._scheduled = True
            Clock.schedule_once(self._run_layout, 0)

    def redraw_entry(self, e: Entry) -> None:
        if e.widget is not None:
            e.widget.redraw(self.px)

    def _lnode(self, e: Entry, m: Metrics) -> LNode:
        n = e.node
        px = m.px
        st = e.style
        if isinstance(n, (NativeText, NativeButton)):
            l = LNode(st, leaf=lambda mw, e=e: widgets.measure_text(e.markup(), e.style, px, mw), key=n.key)
        elif isinstance(n, NativeField):
            def meas(mw, e=e):
                size = px(e.style.font_size)
                tw, _ = widgets.measure_text(widgets.escape_markup(n.hint or " "), e.style, px, None)
                return max(tw, dp(64)), size * 1.3 * max(n.rows, 1)
            l = LNode(st, leaf=meas, key=n.key)
        elif isinstance(n, NativeCheckbox):
            l = LNode(st, leaf=lambda mw: (dp(32), dp(32)), key=n.key)
        elif isinstance(n, NativeImage):
            def meas(mw, e=e):
                if e.icon_name() is not None:
                    s = IconSet.GRID * m.density
                    return s, s
                if e.texture is None:
                    return 0.0, 0.0
                w, h = e.texture.size
                if mw is not None and w > mw:
                    h, w = h * mw / w, mw
                return float(w), float(h)
            l = LNode(st, leaf=meas, key=n.key)
        elif isinstance(n, NativeItem):
            marker = LNode(NativeStyle(), leaf=lambda mw, e=e: widgets.measure_text(
                widgets.escape_markup(n.marker + " "), e.style, px, None))
            content_style = NativeStyle(direction=st.direction, justify=st.justify, align_items=st.align_items,
                                        gap=st.gap, flex=1.0)
            inner = LNode(content_style, [self._lnode(c, m) for c in e.children if not c.is_special])
            row_style = replace(st, direction=Direction.ROW, justify=Justify.START, align_items=Align.START,
                                gap=Length.ZERO, wrap=False)
            l = LNode(row_style, [marker, inner], key=n.key)
            e.lnode, e.inner, e.marker_box = l, inner, marker
            return l
        else:
            l = LNode(st, [self._lnode(c, m) for c in e.children if not c.is_special], key=n.key)
        e.lnode = l
        e.inner = l
        e.marker_box = None
        return l

    def _place(self, e: Entry, ox: float, oy: float, region_h: float, regions: list, px) -> None:
        """Poe os widgets (coordenadas Kivy, origem em baixo) a partir das posicoes do motor."""
        l = e.lnode
        x, y = ox + l.x, oy + l.y
        w = e.widget
        w.size = (max(l.w, 0.0), max(l.h, 0.0))
        w.pos = (x, region_h - y - l.h)
        if isinstance(e.node, NativeItem):
            m = e.marker_box
            e.marker_box = (m.x, m.y, m.w, m.h)
        regions.append(e)
        inner = e.inner
        cx, cy = x, y
        if inner is not None and inner is not l:
            cx, cy = x + inner.x, y + inner.y
        for c in e.children:
            if not c.is_special:
                self._place(c, cx, cy, region_h, regions, px)

    def _run_layout(self, *_):
        self._scheduled = False
        root = self.root
        W, H = self.area.size
        if root is None or W <= 0 or H <= 0:
            return
        m = self.metrics
        px = m.px

        def sub(entry):
            out = [entry]
            for c in entry.children:
                out += sub(c)
            return out

        # reiniciar marcadores dos itens antes de os recalcular
        header = next((c for c in root.children if c.is_header), None)
        footers = [c for c in root.children if c.is_footer]
        layers = [c for c in root.children if c.is_layer]

        self._lnode(root, m)
        for c in header and [header] or []:
            self._lnode(c, m)
        for c in footers + layers:
            self._lnode(c, m)

        # cabecalho: fixo por cima do scroll
        header_h = 0.0
        header_regions: list[Entry] = []
        if header is not None:
            hs = header.style
            ml, mr, mt, mb = (px(hs.margin.left), px(hs.margin.right), px(hs.margin.top), px(hs.margin.bottom))
            fw = px(hs.width.length) if isinstance(hs.width, Dim) else W - ml - mr
            lay_node(header.lnode, m, fw, float("inf"), fw, px(hs.height.length) if isinstance(hs.height, Dim) else None)
            header.lnode.x, header.lnode.y = ml, mt
            header_h = header.lnode.h + mt + mb
            self._place(header, 0, 0, header_h, header_regions, px)

        # rodapes com dock bottom: sob o scroll
        bottom_footers = [f for f in footers if (f.style.dock or Dock.BOTTOM) == Dock.BOTTOM]
        float_footers = [f for f in footers if f not in bottom_footers]
        footer_h = 0.0
        for f in bottom_footers:
            fs = f.style
            ml, mr, mt, mb = (px(fs.margin.left), px(fs.margin.right), px(fs.margin.top), px(fs.margin.bottom))
            fw = px(fs.width.length) if isinstance(fs.width, Dim) else W - ml - mr
            lay_node(f.lnode, m, fw, float("inf"), fw, px(fs.height.length) if isinstance(fs.height, Dim) else None)
            f.lnode.x, f.lnode.y = ml, mt
            footer_h = max(footer_h, f.lnode.h + mt + mb)
        footer_regions: list[Entry] = []
        for f in bottom_footers:
            self._place(f, 0, 0, footer_h, footer_regions, px)

        # corpo
        viewport_h = max(H - header_h - footer_h, 0.0)
        layout(root.lnode, m, W, viewport_h)
        content_h = max(root.lnode.y + root.lnode.h + px(root.style.margin.bottom), viewport_h)
        body_regions: list[Entry] = []
        self._place(root, 0, 0, content_h, body_regions, px)

        # rodapes flutuantes e camadas (dialogs)
        float_regions: list[Entry] = []
        for f in float_footers:
            self._layout_floating(f, W, H, m, float_regions, px)
        overlay_regions: list[Entry] = []
        for lyr in layers:
            lay_node(lyr.lnode, m, W, H, W, H)
            lyr.lnode.x = lyr.lnode.y = 0.0
            self._place(lyr, 0, 0, H, overlay_regions, px)

        # regioes
        self.scroll.size = (W, viewport_h)
        self.scroll.pos = (0, footer_h)
        self.content.size = (W, content_h)
        self.header_layer.size, self.header_layer.pos = (W, header_h), (0, H - header_h)
        self.footer_layer.size, self.footer_layer.pos = (W, footer_h), (0, 0)
        self.float_layer.size = self.overlay_layer.size = (W, H)
        self.float_layer.pos = self.overlay_layer.pos = (0, 0)

        if self._structure_dirty:
            for layer, items in ((self.content, body_regions), (self.header_layer, header_regions),
                                 (self.footer_layer, footer_regions), (self.float_layer, float_regions),
                                 (self.overlay_layer, overlay_regions)):
                layer.clear_widgets()
                for e in items:
                    layer.add_widget(e.widget)
            self._structure_dirty = False
        for items in (body_regions, header_regions, footer_regions, float_regions, overlay_regions):
            for e in items:
                e.widget.redraw(px)

    def _layout_floating(self, f: Entry, W: float, H: float, m: Metrics, regions: list, px) -> None:
        fs = f.style
        dock = fs.dock or Dock.BOTTOM
        ml, mr, mt, mb = (px(fs.margin.left), px(fs.margin.right), px(fs.margin.top), px(fs.margin.bottom))
        wide = dock in (Dock.BOTTOM, Dock.TOP)
        fw = None
        if isinstance(fs.width, Dim):
            fw = px(fs.width.length)
        elif (isinstance(fs.width, Percent) and fs.width.value >= 100) or wide:
            fw = W - ml - mr
        fh = None
        if isinstance(fs.height, Dim):
            fh = px(fs.height.length)
        elif isinstance(fs.height, Percent) and fs.height.value >= 100:
            fh = H - mt - mb
        lay_node(f.lnode, m, W - ml - mr, H - mt - mb, fw, fh)
        w, h = f.lnode.w, f.lnode.h
        a = fs.align_self
        if dock in (Dock.TOP, Dock.BOTTOM):
            y = mt if dock == Dock.TOP else H - mb - h
            x = (W - w) / 2 if a == Align.CENTER else (W - mr - w) if a == Align.END else ml
        elif dock in (Dock.LEFT, Dock.RIGHT):
            x = ml if dock == Dock.LEFT else W - mr - w
            y = mt if a == Align.START else (H - mb - h) if a == Align.END else (H - h) / 2
        else:
            x, y = (W - w) / 2, (H - h) / 2
        f.lnode.x, f.lnode.y = x, y
        self._place(f, 0, 0, H, regions, px)
