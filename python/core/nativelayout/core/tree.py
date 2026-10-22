"""Native tree and style (port of NativeTree.kt).

Colors: unsigned ARGB integers (0xAARRGGBB). 0 = transparent.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum, auto


@dataclass(frozen=True)
class Length:
    value: float
    scalable: bool = False

    def to_pixels(self, density: float, font_scale: float) -> float:
        if self.scalable:
            return self.value * 16.0 * density * font_scale
        return self.value * density

    @staticmethod
    def rem(value: float) -> "Length":
        return Length(value, True)

    @staticmethod
    def px(value: float) -> "Length":
        return Length(value, False)


Length.ZERO = Length(0.0)


@dataclass(frozen=True)
class Dim:
    length: Length


@dataclass(frozen=True)
class Percent:
    value: float


class _Auto:
    _inst = None

    def __new__(cls):
        if cls._inst is None:
            cls._inst = super().__new__(cls)
        return cls._inst

    def __repr__(self):
        return "Auto"


AUTO = _Auto()
Size = Dim | Percent | _Auto


class Direction(Enum):
    COLUMN = auto()
    ROW = auto()


class Justify(Enum):
    START = auto()
    CENTER = auto()
    END = auto()
    BETWEEN = auto()
    AROUND = auto()
    EVENLY = auto()


class Align(Enum):
    START = auto()
    CENTER = auto()
    END = auto()
    STRETCH = auto()


class Dock(Enum):
    """Where a <footer> sits: BOTTOM (the default) takes its space, the others float over the screen."""
    BOTTOM = auto()
    TOP = auto()
    LEFT = auto()
    RIGHT = auto()
    CENTER = auto()


class Position(Enum):
    """`position`: STATIC stays in the flow of its parent; ABSOLUTE leaves it and is placed by top, right, bottom, left."""
    STATIC = auto()
    ABSOLUTE = auto()


class ImageFit(Enum):
    """How an image fills its box: COVER fills and crops, CONTAIN fits whole, FILL stretches."""
    COVER = auto()
    CONTAIN = auto()
    FILL = auto()


class ImagePos(Enum):
    """Which part of a cover or contain image stays in view, or the side where it sits."""
    CENTER = auto()
    TOP = auto()
    BOTTOM = auto()
    LEFT = auto()
    RIGHT = auto()


class TextAlign(Enum):
    START = auto()
    CENTER = auto()
    END = auto()


class FieldType(Enum):
    TEXT = auto()
    PASSWORD = auto()
    NUMBER = auto()
    AREA = auto()


class TextTransform(Enum):
    NONE = auto()
    UPPERCASE = auto()
    LOWERCASE = auto()
    CAPITALIZE = auto()


class FontFamily(Enum):
    DEFAULT = auto()
    SANS = auto()
    SERIF = auto()
    MONOSPACE = auto()


class AnimKind(Enum):
    NONE = auto()
    FADE = auto()
    SLIDE_UP = auto()
    SLIDE_LEFT = auto()


@dataclass(frozen=True)
class Sides:
    top: Length = Length.ZERO
    right: Length = Length.ZERO
    bottom: Length = Length.ZERO
    left: Length = Length.ZERO


@dataclass(frozen=True)
class Corners:
    """CSS order: top-left, top-right, bottom-right, bottom-left."""
    top_left: Length = Length.ZERO
    top_right: Length = Length.ZERO
    bottom_right: Length = Length.ZERO
    bottom_left: Length = Length.ZERO


class Colors:
    BLACK = 0xFF000000
    WHITE = 0xFFFFFFFF
    BUTTON_GRAY = 0xFFE0E0E0
    BORDER_GRAY = 0xFF767676


@dataclass(frozen=True)
class LineHeightMultiple:
    value: float


@dataclass(frozen=True)
class LineHeightFixed:
    length: Length


LineHeight = LineHeightMultiple | LineHeightFixed


@dataclass(frozen=True)
class Gradient:
    """angle in CSS degrees (0 = to top, 90 = to right, 180 = to bottom)."""
    angle: int
    colors: tuple[int, ...]


@dataclass(frozen=True)
class Animation:
    kind: AnimKind
    duration_ms: int = 200


class DefaultAnimations:
    SCREEN = Animation(AnimKind.FADE, 220)
    LAYER = Animation(AnimKind.FADE, 160)


@dataclass(frozen=True)
class StateStyle:
    background_color: int | None = None
    text_color: int | None = None
    border_color: int | None = None
    opacity: float | None = None


@dataclass(frozen=True)
class StateStyles:
    pressed: StateStyle | None = None
    disabled: StateStyle | None = None
    focused: StateStyle | None = None


@dataclass(frozen=True)
class NativeStyle:
    text_color: int = Colors.BLACK
    background_color: int = 0
    font_size: Length = field(default_factory=lambda: Length.rem(1.0))
    bold: bool = False
    text_align: TextAlign = TextAlign.START
    width: Size = AUTO
    height: Size = AUTO
    padding: Sides = field(default_factory=Sides)
    margin: Sides = field(default_factory=Sides)
    border_radius: Length = Length.ZERO
    corners: Corners | None = None
    border_width: Length = Length.ZERO
    border_color: int = Colors.BLACK
    direction: Direction = Direction.COLUMN
    justify: Justify = Justify.START
    align_items: Align = Align.STRETCH
    gap: Length = Length.ZERO
    flex: float = 0.0
    opacity: float = 1.0
    italic: bool = False
    underline: bool = False
    strike: bool = False
    letter_spacing: Length = Length.ZERO
    line_height: LineHeight | None = None
    text_transform: TextTransform = TextTransform.NONE
    font_family: FontFamily = FontFamily.DEFAULT
    min_width: Length | None = None
    max_width: Length | None = None
    min_height: Length | None = None
    max_height: Length | None = None
    overflow_hidden: bool = False
    align_self: Align | None = None
    dock: Dock | None = None
    position: Position = Position.STATIC
    top: Length | None = None
    right: Length | None = None
    bottom: Length | None = None
    left: Length | None = None
    z_index: int = 0
    # name of a background image, resolved by the host like the source of an <img>
    background_image: str | None = None
    background_size: ImageFit = ImageFit.COVER
    background_position: ImagePos = ImagePos.CENTER
    object_fit: ImageFit = ImageFit.CONTAIN
    wrap: bool = False
    elevation: Length = Length.ZERO
    gradient: Gradient | None = None
    animation: Animation | None = None
    states: StateStyles | None = None


@dataclass(frozen=True)
class Span:
    text: str
    bold: bool
    color: int
    size: Length
    italic: bool = False
    underline: bool = False
    strike: bool = False


@dataclass(eq=False, kw_only=True)
class Native:
    id: str | None
    classes: list[str]
    style: NativeStyle
    line: int
    key: str = ""
    key_hint: str | None = None
    scope: str | None = None


@dataclass(eq=False, kw_only=True)
class NativeBox(Native):
    children: list[Native]
    virtual: bool = False
    overlay: bool = False   # <dialog>
    header: bool = False    # <header>
    footer: bool = False    # <footer>
    # True when a child is `position: absolute`: the host gives the box a frame for them
    has_absolute: bool = False


@dataclass(eq=False, kw_only=True)
class NativeText(Native):
    spans: list[Span]

    @property
    def plain_text(self) -> str:
        return "".join(s.text for s in self.spans)


@dataclass(eq=False, kw_only=True)
class NativeButton(Native):
    label: str
    disabled: bool


@dataclass(eq=False, kw_only=True)
class NativeField(Native):
    type: FieldType
    hint: str
    value: str
    disabled: bool
    read_only: bool
    rows: int


@dataclass(eq=False, kw_only=True)
class NativeCheckbox(Native):
    checked: bool
    disabled: bool


@dataclass(eq=False, kw_only=True)
class NativeImage(Native):
    source: str
    description: str


@dataclass(eq=False, kw_only=True)
class NativeItem(Native):
    marker: str
    children: list[Native]


@dataclass(eq=False, kw_only=True)
class NativeList(Native):
    items: list[NativeItem]


def native_children(n: Native) -> list[Native]:
    if isinstance(n, NativeBox):
        return n.children
    if isinstance(n, NativeList):
        return n.items
    if isinstance(n, NativeItem):
        return n.children
    return []


def describe(n: Native, level: int = 0) -> str:
    id_text = f" #{n.id}" if n.id is not None else ""
    cls_text = " ." + " ".join(n.classes) if n.classes else ""
    if isinstance(n, NativeText):
        detail = f' "{n.plain_text}"'
    elif isinstance(n, NativeButton):
        detail = f' "{n.label}"'
    elif isinstance(n, NativeField):
        detail = f" {n.type.name}"
    elif isinstance(n, NativeImage):
        detail = f" {n.source}"
    else:
        detail = ""
    out = "  " * level + type(n).__name__ + id_text + cls_text + detail + "\n"
    return out + "".join(describe(c, level + 1) for c in native_children(n))


def same_as(a: Native, o: Native) -> bool:
    if a is o:
        return True
    if (type(a) is not type(o) or a.key != o.key or a.id != o.id
            or a.classes != o.classes or a.style != o.style):
        return False
    if isinstance(a, NativeText):
        same = a.spans == o.spans
    elif isinstance(a, NativeButton):
        same = a.label == o.label and a.disabled == o.disabled
    elif isinstance(a, NativeField):
        same = (a.type == o.type and a.hint == o.hint and a.value == o.value
                and a.disabled == o.disabled and a.read_only == o.read_only and a.rows == o.rows)
    elif isinstance(a, NativeCheckbox):
        same = a.checked == o.checked and a.disabled == o.disabled
    elif isinstance(a, NativeImage):
        same = a.source == o.source and a.description == o.description
    elif isinstance(a, NativeItem):
        same = a.marker == o.marker
    elif isinstance(a, NativeBox):
        same = (a.virtual == o.virtual and a.overlay == o.overlay
                and a.header == o.header and a.footer == o.footer
                and a.has_absolute == o.has_absolute)
    else:
        same = True
    if not same:
        return False
    ca, cb = native_children(a), native_children(o)
    if len(ca) != len(cb):
        return False
    return all(same_as(x, y) for x, y in zip(ca, cb))
