"""Contract of the host and render session (port of LayoutHost.kt).

`next_event` is asynchronous (`suspend` in Kotlin): in Kivy it runs on asyncio.
"""
from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field

from . import scopes as Scopes
from .differ import Change, diff
from .layout_engine import LayoutEngine, LayoutWarning
from .tree import Native, native_children

# Key of the event the host sends when the system back button is pressed.
BACK_EVENT_KEY = "\u0000back"


@dataclass(frozen=True)
class LayoutEvent:
    key: str


class LayoutHost(ABC):
    @abstractmethod
    def apply_changes(self, changes: list[Change]) -> None: ...

    @abstractmethod
    async def next_event(self) -> LayoutEvent | None: ...

    @abstractmethod
    def field_text(self, id: str) -> str | None: ...

    @abstractmethod
    def field_checked(self, id: str) -> bool | None: ...

    @abstractmethod
    def set_value(self, id: str, text: str) -> None: ...

    @abstractmethod
    def save(self, key: str, text: str) -> None: ...

    @abstractmethod
    def load(self, key: str) -> str | None: ...

    @abstractmethod
    def exists(self, key: str) -> bool: ...

    @abstractmethod
    def delete(self, key: str) -> None: ...

    @abstractmethod
    def close(self) -> None: ...

    def show_error(self, text: str) -> None:
        """Shows an error in place of the screen (optional)."""

    def post_back(self) -> None:
        """Wakes the program with the back-button event (optional)."""


@dataclass
class FilledMarkup:
    html: str
    line: int
    variables: dict[str, str] = field(default_factory=dict)


@dataclass
class PreparedRender:
    """Result of `LayoutSession.prepare`: plain data, it can be built on any thread."""
    root: Native
    by_key: dict[str, Native]
    by_id: dict[str, Native]
    changes: list[Change]
    warnings: list[LayoutWarning]


class LayoutSession:
    def __init__(self, host: LayoutHost):
        self._host = host
        self._tree: Native | None = None
        self._by_key: dict[str, Native] = {}
        self._by_id: dict[str, Native] = {}
        self._last_event: LayoutEvent | None = None
        self._clicked_index = -1
        self._closed = False
        self._items: dict[str, object] = {}
        self.warnings: list[LayoutWarning] = []

    def prepare(self, m: FilledMarkup) -> PreparedRender:
        """Heavy part: builds the tree, indexes it and diffs it against the one on screen. Does not touch the host."""
        r = LayoutEngine.build(m.html, m.line, m.variables)
        changes = diff(self._tree, r.root)
        by_key: dict[str, Native] = {}
        self._index(r.root, by_key)
        return PreparedRender(r.root, by_key, r.by_id, changes, r.warnings)

    async def commit(self, p: PreparedRender) -> bool:
        """Light part: applies the changes to the views and waits for the next event (on the thread of the views)."""
        if self._closed:
            return False
        for a in p.warnings:
            if not any(w.message == a.message and w.line == a.line for w in self.warnings):
                self.warnings.append(a)
        if p.changes:
            self._host.apply_changes(p.changes)
        self._tree = p.root
        self._by_key = p.by_key
        self._by_id = p.by_id
        self._last_event = None
        self._clicked_index = -1

        event = await self._host.next_event()
        if event is None:
            self._closed = True
            self._tree = None
            self._host.close()
            return False
        self._last_event = event
        return True

    async def render(self, m: FilledMarkup) -> bool:
        return await self.commit(self.prepare(m))

    # ---- identity inside a `for` (section 5.7)
    def set_scope_items(self, items: dict[str, object]) -> None:
        self._items = items

    def _tapped_scope(self) -> str | None:
        if self._last_event is None:
            return None
        n = self._by_key.get(self._last_event.key)
        return n.scope if n is not None else None

    def clicked_item(self, name: str):
        path = self._tapped_scope()
        if path is None:
            raise RuntimeError(f"clicked('{name}'): the tapped element is not inside a 'for'")
        segments = path.split("/")
        for i in range(len(segments) - 1, -1, -1):
            if Scopes.variable_of(segments[i]) == name:
                prefix = "/".join(segments[:i + 1])
                if prefix not in self._items:
                    break
                return self._items[prefix]
        raise RuntimeError(f"clicked('{name}'): the tapped element is not inside 'for {name} in ...'")

    def scoped_id(self, id: str, given: list | None = None) -> str | None:
        given = given or []
        if not given:
            if id in self._by_id:
                return id
            prefix = id + Scopes.SEP
            sample = next((k for k in self._by_id if k.startswith(prefix)), None)
            if sample is None:
                return None
            segs = sample[len(prefix):].split("/")
            vars_ = ", ".join(Scopes.variable_of(s) for s in segs)
            raise RuntimeError(f"'{id}' is repeated: give {len(segs)} item(s), one for each of: {vars_}")
        prefix = id + Scopes.SEP
        candidates = [k for k in self._by_id if k.startswith(prefix)]
        if not candidates:
            return None
        for c in candidates:
            segments = c[len(prefix):].split("/")
            if len(segments) != len(given):
                continue
            ok = True
            path = ""
            for k, seg in enumerate(segments):
                path = seg if k == 0 else path + "/" + seg
                if path not in self._items or not Scopes.same_item(
                        given[k], self._items[path], Scopes.token_of(seg)):
                    ok = False
                    break
            if ok:
                return c
        return None

    def click(self, target: str) -> bool:
        ev = self._last_event
        if ev is None:
            return False
        node = self._by_key.get(ev.key)
        if node is None:
            return False
        if target.startswith("."):
            cls = target[1:]
            if cls not in node.classes:
                return False
            all_: list[Native] = []
            if self._tree is not None:
                self._collect_class(self._tree, cls, all_)
            self._clicked_index = next((i for i, n in enumerate(all_) if n.key == node.key), -1)
            return True
        nid = node.id
        if nid == target or (nid is not None and node.scope is not None
                             and nid.split(Scopes.SEP, 1)[0] == target):
            self._clicked_index = -1
            return True
        return False

    def click_index(self) -> int:
        return self._clicked_index

    def is_back(self) -> bool:
        return self._last_event is not None and self._last_event.key == BACK_EVENT_KEY

    def reset(self) -> None:
        """Forgets the tree on screen: the next render builds everything from scratch (the screen changed)."""
        self._tree = None

    def field_text(self, id: str, given: list | None = None) -> str | None:
        s = self.scoped_id(id, given)
        return self._host.field_text(s) if s is not None else None

    def field_checked(self, id: str, given: list | None = None) -> bool | None:
        s = self.scoped_id(id, given)
        return self._host.field_checked(s) if s is not None else None

    def set_value(self, id: str, text: str, given: list | None = None) -> None:
        s = self.scoped_id(id, given)
        if s is not None:
            self._host.set_value(s, text)

    def save(self, key: str, text: str) -> None:
        self._host.save(key, text)

    def load(self, key: str) -> str | None:
        return self._host.load(key)

    def exists(self, key: str) -> bool:
        return self._host.exists(key)

    def delete(self, key: str) -> None:
        self._host.delete(key)

    def _index(self, n: Native, result: dict[str, Native]) -> None:
        result[n.key] = n
        for f in native_children(n):
            self._index(f, result)

    def _collect_class(self, n: Native, cls: str, result: list[Native]) -> None:
        if cls in n.classes:
            result.append(n)
        for f in native_children(n):
            self._collect_class(f, cls, result)
