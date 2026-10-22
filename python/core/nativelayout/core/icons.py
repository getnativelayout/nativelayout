"""Built-in icons (port of IconSet.kt). 24x24 grid, stroke 2, round caps."""
from __future__ import annotations
from dataclasses import dataclass

PREFIX = "icon:"
GRID = 24.0
STROKE = 2.0
DOT_RADIUS = 1.6


@dataclass(frozen=True)
class Line:
    points: tuple[float, ...]
    closed: bool = False


@dataclass(frozen=True)
class Circle:
    cx: float
    cy: float
    r: float


@dataclass(frozen=True)
class Dot:
    cx: float
    cy: float


@dataclass(frozen=True)
class Rect:
    left: float
    top: float
    right: float
    bottom: float
    radius: float


_SHAPES: dict[str, list] = {
    "arrow-left": [Line((19, 12, 5, 12,)), Line((12, 19, 5, 12, 12, 5,))],
    "arrow-right": [Line((5, 12, 19, 12,)), Line((12, 5, 19, 12, 12, 19,))],
    "arrow-up": [Line((12, 19, 12, 5,)), Line((5, 12, 12, 5, 19, 12,))],
    "arrow-down": [Line((12, 5, 12, 19,)), Line((5, 12, 12, 19, 19, 12,))],
    "chevron-left": [Line((15, 5, 8, 12, 15, 19,))],
    "chevron-right": [Line((9, 5, 16, 12, 9, 19,))],
    "chevron-up": [Line((5, 15, 12, 8, 19, 15,))],
    "chevron-down": [Line((5, 9, 12, 16, 19, 9,))],
    "close": [Line((6, 6, 18, 18,)), Line((18, 6, 6, 18,))],
    "check": [Line((4, 12.5, 9.5, 18, 20, 6.5,))],
    "plus": [Line((12, 5, 12, 19,)), Line((5, 12, 19, 12,))],
    "minus": [Line((5, 12, 19, 12,))],
    "menu": [Line((4, 6, 20, 6,)), Line((4, 12, 20, 12,)), Line((4, 18, 20, 18,))],
    "search": [Circle(10.5, 10.5, 6.5), Line((15.3, 15.3, 20, 20,))],
    "more-vertical": [Dot(12, 5), Dot(12, 12), Dot(12, 19)],
    "more-horizontal": [Dot(5, 12), Dot(12, 12), Dot(19, 12)],
    "home": [Line((3, 11, 12, 3, 21, 11,)), Line((5, 9.5, 5, 21, 19, 21, 19, 9.5,)), Line((10, 21, 10, 15, 14, 15, 14, 21,))],
    "user": [Circle(12, 8, 4), Line((5, 21, 5, 19.5, 5.8, 17, 7.6, 15.3, 10, 14.3, 12, 14, 14, 14.3, 16.4, 15.3, 18.2, 17, 19, 19.5, 19, 21,))],
    "trash": [Line((4, 7, 20, 7,)), Line((9, 7, 9, 4, 15, 4, 15, 7,)), Line((6, 7, 7, 21, 17, 21, 18, 7,)), Line((10, 11, 10, 17,)), Line((14, 11, 14, 17,))],
    "edit": [Line((4, 20, 5, 15, 16, 4, 20, 8, 9, 19,), True), Line((14, 6, 18, 10,))],
    "star": [Line((12, 3, 14.4, 9.3, 21.1, 9.6, 15.9, 13.9, 17.6, 20.4, 12, 16.7, 6.4, 20.4, 8.1, 13.9, 2.9, 9.6, 9.6, 9.3,), True)],
    "lock": [Rect(5, 11, 19, 21, 2), Line((8, 11, 8, 8.2, 8.6, 6.4, 10, 5.1, 12, 4.6, 14, 5.1, 15.4, 6.4, 16, 8.2, 16, 11,))],
    "eye": [Line((2, 12, 4, 9.6, 7, 7.6, 9.5, 6.6, 12, 6.3, 14.5, 6.6, 17, 7.6, 20, 9.6, 22, 12, 20, 14.4, 17, 16.4, 14.5, 17.4, 12, 17.7, 9.5, 17.4, 7, 16.4, 4, 14.4,), True), Circle(12, 12, 3)],
    "mail": [Rect(3, 5, 21, 19, 2), Line((3, 7, 12, 13, 21, 7,))],
    "info": [Circle(12, 12, 9), Line((12, 11, 12, 17,)), Dot(12, 7.5)],
    "alert": [Line((12, 3, 22, 20, 2, 20,), True), Line((12, 9, 12, 14,)), Dot(12, 17)],
    "calendar": [Rect(3, 5, 21, 21, 2), Line((3, 10, 21, 10,)), Line((8, 3, 8, 7,)), Line((16, 3, 16, 7,))],
    "logout": [Line((9, 4, 5, 4, 5, 20, 9, 20,)), Line((10, 12, 20, 12,)), Line((16, 8, 20, 12, 16, 16,))],
    "bookmark": [Line((6, 3, 18, 3, 18, 21, 12, 16, 6, 21,), True)],
    "clock": [Circle(12, 12, 9), Line((12, 7, 12, 12, 16, 14,))],
}


def name_of(source: str) -> str | None:
    return source[len(PREFIX):].strip() if source.startswith(PREFIX) else None


def shapes_of(name: str):
    return _SHAPES.get(name)


def names() -> list[str]:
    return list(_SHAPES.keys())
