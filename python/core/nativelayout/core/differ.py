"""Diff between two native trees (port of LayoutDiffer.kt)."""
from __future__ import annotations

from dataclasses import dataclass

from .tree import (Native, NativeBox, NativeButton, NativeCheckbox, NativeField,
                   NativeImage, NativeItem, NativeStyle, NativeText, Position, Span,
                   native_children, same_as)


class Change:
    pass


@dataclass
class SetRoot(Change):
    node: Native


@dataclass
class Create(Change):
    parent_key: str
    position: int
    node: Native


@dataclass
class Remove(Change):
    key: str


@dataclass
class Replace(Change):
    key: str
    node: Native


@dataclass
class UpdateText(Change):
    key: str
    spans: list[Span]


@dataclass
class UpdateStyle(Change):
    key: str
    style: NativeStyle


@dataclass
class UpdateAttributes(Change):
    key: str
    node: Native


@dataclass
class SetItems(Change):
    key: str
    items: list[Native]


def diff(previous: Native | None, nxt: Native) -> list[Change]:
    if previous is None or previous.key != nxt.key or type(previous) is not type(nxt):
        return [SetRoot(nxt)]
    removed: list[Change] = []
    rest: list[Change] = []
    _compare(previous, nxt, removed, rest)
    return removed + rest


def _compare(a: Native, n: Native, removed: list, rest: list) -> None:
    if isinstance(a, NativeBox) and isinstance(n, NativeBox):
        if (a.virtual != n.virtual or a.overlay != n.overlay
                or a.header != n.header or a.footer != n.footer
                or a.has_absolute != n.has_absolute):
            rest.append(Replace(n.key, n))
            return
        if n.virtual:
            if a.style != n.style:
                rest.append(UpdateStyle(n.key, n.style))
            if _attributes_differ(a, n):
                rest.append(UpdateAttributes(n.key, n))
            if not _same_children(a.children, n.children):
                rest.append(SetItems(n.key, n.children))
            return
    if a.style != n.style:
        rest.append(UpdateStyle(n.key, n.style))
    if _attributes_differ(a, n):
        rest.append(UpdateAttributes(n.key, n))
    if isinstance(a, NativeText) and isinstance(n, NativeText) and a.spans != n.spans:
        rest.append(UpdateText(n.key, n.spans))
    ca, cn = native_children(a), native_children(n)
    if not ca and not cn:
        return
    _compare_children(n, ca, cn, removed, rest)


def _same_children(a: list, b: list) -> bool:
    return len(a) == len(b) and all(same_as(x, y) for x, y in zip(a, b))


def _attributes_differ(a: Native, n: Native) -> bool:
    if a.classes != n.classes:
        return True
    if isinstance(a, NativeButton) and isinstance(n, NativeButton):
        return a.label != n.label or a.disabled != n.disabled
    if isinstance(a, NativeField) and isinstance(n, NativeField):
        return (a.type != n.type or a.hint != n.hint or a.disabled != n.disabled
                or a.read_only != n.read_only or a.rows != n.rows)
    if isinstance(a, NativeCheckbox) and isinstance(n, NativeCheckbox):
        return a.disabled != n.disabled
    if isinstance(a, NativeImage) and isinstance(n, NativeImage):
        return a.source != n.source or a.description != n.description
    if isinstance(a, NativeItem) and isinstance(n, NativeItem):
        return a.marker != n.marker
    return False


def _compare_children(parent: Native, prev: list, nxt: list, removed: list, rest: list) -> None:
    new_keys = {c.key for c in nxt}
    old_by_key = {c.key: c for c in prev}
    kept_old = [c.key for c in prev if c.key in new_keys]
    kept_new = [c.key for c in nxt if c.key in old_by_key]
    if kept_old != kept_new:
        rest.append(Replace(parent.key, parent))
        return
    for a in prev:
        if a.key not in new_keys:
            removed.append(Remove(a.key))
    for i, n in enumerate(nxt):
        a = old_by_key.get(n.key)
        if a is None:
            # an absolute child lives apart from the flow: its position counts only the absolute ones
            absolute = n.style.position == Position.ABSOLUTE
            before = sum(1 for c in nxt[:i] if (c.style.position == Position.ABSOLUTE) == absolute)
            rest.append(Create(parent.key, before, n))
        elif type(a) is not type(n):
            rest.append(Replace(n.key, n))
        else:
            _compare(a, n, removed, rest)
