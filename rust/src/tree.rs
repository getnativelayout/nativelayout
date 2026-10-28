//! Port of NativeTree.kt: the neutral tree that the layout engine produces and the hosts read.
//!
//! Kotlin has `sealed class Native` with subclasses. Here `Native` is one struct with the fields
//! they share, plus a `kind` enum with what is specific to each one. The tree is owned top-down
//! (children in `Vec`), with no pointers to the parent, exactly like the Kotlin tree.

// ---------- units ----------

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Length {
    pub value: f32,
    pub scalable: bool,
}

impl Length {
    pub const ZERO: Length = Length { value: 0.0, scalable: false };

    pub fn rem(value: f32) -> Length {
        Length { value, scalable: true }
    }

    pub fn px(value: f32) -> Length {
        Length { value, scalable: false }
    }

    pub fn to_pixels(&self, density: f32, font_scale: f32) -> f32 {
        if self.scalable {
            self.value * 16.0 * density * font_scale
        } else {
            self.value * density
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Dim(Length),
    Percent(f32),
    Auto,
}

// ---------- enums ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Column,
    Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    Between,
    Around,
    Evenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Where a `<footer>` sits: `Bottom` (the default) reserves its space, the others float over the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dock {
    Bottom,
    Top,
    Left,
    Right,
    Center,
}

/// `position`: `Static` stays in the flow of its parent; `Absolute` leaves the flow and is placed
/// by top, right, bottom and left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Absolute,
}

/// How an image fills its box: `Cover` fills and crops, `Contain` fits whole, `Fill` stretches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    Cover,
    Contain,
    Fill,
}

/// Which part of a `cover` or `contain` image stays in view or where it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePos {
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Password,
    Number,
    Area,
}

impl FieldType {
    /// The name the Kotlin enum prints (used by `describe`).
    pub fn name(&self) -> &'static str {
        match self {
            FieldType::Text => "TEXT",
            FieldType::Password => "PASSWORD",
            FieldType::Number => "NUMBER",
            FieldType::Area => "AREA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontFamily {
    #[default]
    Default,
    Sans,
    Serif,
    Monospace,
}

// ---------- values ----------

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sides {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

/// The four corners of a rounded box, in the CSS order: top-left, top-right, bottom-right, bottom-left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    pub top_left: Length,
    pub top_right: Length,
    pub bottom_right: Length,
    pub bottom_left: Length,
}

/// Colors are 0xAARRGGBB.
pub mod colors {
    pub const BLACK: u32 = 0xFF00_0000;
    pub const WHITE: u32 = 0xFFFF_FFFF;
    pub const BUTTON_GRAY: u32 = 0xFFE0_E0E0;
    pub const BORDER_GRAY: u32 = 0xFF76_7676;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// Unitless number: a multiple of the font size.
    Multiple(f32),
    Fixed(Length),
}

/// linear-gradient: `angle` in CSS degrees (0 = to top, 90 = to right, 180 = to bottom).
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    pub angle: i32,
    pub colors: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimKind {
    None,
    Fade,
    SlideUp,
    SlideLeft,
}

/// Entrance animation of a screen (`body`) or of a layer (`dialog`). `duration_ms` is in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Animation {
    pub kind: AnimKind,
    pub duration_ms: i32,
}

impl Animation {
    /// Same as Kotlin `Animation(kind)`: 200 ms.
    pub fn new(kind: AnimKind) -> Animation {
        Animation { kind, duration_ms: 200 }
    }
}

/// What a screen and a layer do when they appear and the CSS says nothing: a short fade.
pub mod default_animations {
    use super::{AnimKind, Animation};

    pub const SCREEN: Animation = Animation { kind: AnimKind::Fade, duration_ms: 220 };
    pub const LAYER: Animation = Animation { kind: AnimKind::Fade, duration_ms: 160 };
}

/// What changes while the element is pressed, focused or disabled. `None` means unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StateStyle {
    pub background_color: Option<u32>,
    pub text_color: Option<u32>,
    pub border_color: Option<u32>,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StateStyles {
    pub pressed: Option<StateStyle>,
    pub disabled: Option<StateStyle>,
    pub focused: Option<StateStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeStyle {
    pub text_color: u32,
    pub background_color: u32,
    pub font_size: Length,
    pub bold: bool,
    pub text_align: TextAlign,
    pub width: Size,
    pub height: Size,
    pub padding: Sides,
    pub margin: Sides,
    pub border_radius: Length,
    /// Different radius per corner. When not `None` it replaces `border_radius`.
    pub corners: Option<Corners>,
    pub border_width: Length,
    pub border_color: u32,
    pub direction: Direction,
    pub justify: Justify,
    pub align_items: Align,
    pub gap: Length,
    pub flex: f32,
    pub opacity: f32,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub letter_spacing: Length,
    pub line_height: Option<LineHeight>,
    pub text_transform: TextTransform,
    pub font_family: FontFamily,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_height: Option<Length>,
    pub overflow_hidden: bool,
    /// `overflow-y: auto|scroll` on a box: its content scrolls inside it when it does not fit.
    pub scroll_y: bool,
    pub align_self: Option<Align>,
    pub dock: Option<Dock>,
    pub position: Position,
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
    pub z_index: i32,
    /// Name of a background image, resolved by the host like the source of an `<img>`.
    pub background_image: Option<String>,
    pub background_size: ImageFit,
    pub background_position: ImagePos,
    pub object_fit: ImageFit,
    pub wrap: bool,
    pub elevation: Length,
    pub gradient: Option<Gradient>,
    /// Only read on `body` and `dialog`. `None` means the default (`default_animations`). Not inherited.
    pub animation: Option<Animation>,
    pub states: Option<StateStyles>,
}

impl Default for NativeStyle {
    fn default() -> NativeStyle {
        NativeStyle {
            text_color: colors::BLACK,
            background_color: 0,
            font_size: Length::rem(1.0),
            bold: false,
            text_align: TextAlign::Start,
            width: Size::Auto,
            height: Size::Auto,
            padding: Sides::default(),
            margin: Sides::default(),
            border_radius: Length::ZERO,
            corners: None,
            border_width: Length::ZERO,
            border_color: colors::BLACK,
            direction: Direction::Column,
            justify: Justify::Start,
            align_items: Align::Stretch,
            gap: Length::ZERO,
            flex: 0.0,
            opacity: 1.0,
            italic: false,
            underline: false,
            strike: false,
            letter_spacing: Length::ZERO,
            line_height: None,
            text_transform: TextTransform::None,
            font_family: FontFamily::Default,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            overflow_hidden: false,
            scroll_y: false,
            align_self: None,
            dock: None,
            position: Position::Static,
            top: None,
            right: None,
            bottom: None,
            left: None,
            z_index: 0,
            background_image: None,
            background_size: ImageFit::Cover,
            background_position: ImagePos::Center,
            object_fit: ImageFit::Contain,
            wrap: false,
            elevation: Length::ZERO,
            gradient: None,
            animation: None,
            states: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub color: u32,
    pub size: Length,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl Span {
    /// Same as Kotlin `Span(text, bold, color, size)`: no italic, underline or strike.
    pub fn new(text: &str, bold: bool, color: u32, size: Length) -> Span {
        Span {
            text: text.to_string(),
            bold,
            color,
            size,
            italic: false,
            underline: false,
            strike: false,
        }
    }
}

/// A float the way Kotlin prints it: `3.0`, not `3`.
fn kotlin_float(v: f32) -> String {
    if v.is_finite() && v == v.trunc() {
        format!("{:.1}", v)
    } else {
        format!("{}", v)
    }
}

// ---------- the tree ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NativeBox {
    pub children: Vec<Native>,
    pub is_virtual: bool,
    /// A `<dialog>`: a layer drawn over the whole screen, above the other blocks.
    pub overlay: bool,
    /// A `<header>`: fixed at the top of the screen, outside the scroll.
    pub header: bool,
    /// A `<footer>`: fixed outside the scroll, placed by the `dock` property (bottom by default).
    pub footer: bool,
    /// True when a child is `position: absolute`: the host gives the box a frame for them.
    pub has_absolute: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeKind {
    Box(NativeBox),
    Text { spans: Vec<Span> },
    Button { label: String, disabled: bool },
    Field {
        field_type: FieldType,
        hint: String,
        value: String,
        disabled: bool,
        read_only: bool,
        rows: i32,
    },
    Checkbox { checked: bool, disabled: bool },
    Image { source: String, description: String },
    /// A `<progress>`. `value` `None` means indeterminate (a loading animation); otherwise it is the
    /// amount done, out of `max`. `circular` draws a ring instead of a bar. The bar takes the `color`
    /// of the element and the track its `background-color`.
    Progress { value: Option<f32>, max: f32, circular: bool },
    /// `items` are always `NativeKind::Item`.
    List { items: Vec<Native> },
    Item { marker: String, children: Vec<Native> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Native {
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub style: NativeStyle,
    pub line: usize,
    pub kind: NativeKind,
    pub key: String,
    /// Value of the optional `key="..."` attribute: a stable identity among siblings.
    pub key_hint: Option<String>,
    /// Where the element sits among the repetitions of a `for` (written by the Expander), or `None`.
    pub scope: Option<String>,
}

impl Native {
    pub fn new(
        id: Option<String>,
        classes: Vec<String>,
        style: NativeStyle,
        line: usize,
        kind: NativeKind,
    ) -> Native {
        Native {
            id,
            classes,
            style,
            line,
            kind,
            key: String::new(),
            key_hint: None,
            scope: None,
        }
    }

    /// The text of a `Text` element (its spans joined), or `None` for any other kind.
    pub fn plain_text(&self) -> Option<String> {
        match &self.kind {
            NativeKind::Text { spans } => Some(spans.iter().map(|s| s.text.as_str()).collect()),
            _ => None,
        }
    }

    /// Done, from 0 to 1, for a `Progress` (0 when indeterminate); `None` for any other kind.
    pub fn progress_fraction(&self) -> Option<f32> {
        match &self.kind {
            NativeKind::Progress { value, max, .. } => Some((value.unwrap_or(0.0) / max).clamp(0.0, 1.0)),
            _ => None,
        }
    }

    /// `nativeChildren()` in Kotlin.
    pub fn native_children(&self) -> &[Native] {
        match &self.kind {
            NativeKind::Box(b) => &b.children,
            NativeKind::List { items } => items,
            NativeKind::Item { children, .. } => children,
            _ => &[],
        }
    }

    /// Same as `native_children`, to change the children in place.
    pub fn native_children_mut(&mut self) -> &mut [Native] {
        match &mut self.kind {
            NativeKind::Box(b) => &mut b.children[..],
            NativeKind::List { items } => &mut items[..],
            NativeKind::Item { children, .. } => &mut children[..],
            _ => &mut [],
        }
    }

    /// The Kotlin class name, as `describe` prints it.
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            NativeKind::Box(_) => "NativeBox",
            NativeKind::Text { .. } => "NativeText",
            NativeKind::Button { .. } => "NativeButton",
            NativeKind::Field { .. } => "NativeField",
            NativeKind::Checkbox { .. } => "NativeCheckbox",
            NativeKind::Image { .. } => "NativeImage",
            NativeKind::Progress { .. } => "NativeProgress",
            NativeKind::List { .. } => "NativeList",
            NativeKind::Item { .. } => "NativeItem",
        }
    }

    pub fn describe(&self) -> String {
        self.describe_at(0)
    }

    pub fn describe_at(&self, level: usize) -> String {
        let id_text = match &self.id {
            Some(id) => format!(" #{}", id),
            None => String::new(),
        };
        let cls_text = if self.classes.is_empty() {
            String::new()
        } else {
            format!(" .{}", self.classes.join(" "))
        };
        let detail = match &self.kind {
            NativeKind::Text { .. } => format!(" \"{}\"", self.plain_text().unwrap_or_default()),
            NativeKind::Button { label, .. } => format!(" \"{}\"", label),
            NativeKind::Field { field_type, .. } => format!(" {}", field_type.name()),
            NativeKind::Image { source, .. } => format!(" {}", source),
            NativeKind::Progress { value, .. } => match value {
                Some(v) => format!(" {}", kotlin_float(*v)),
                None => " indeterminate".to_string(),
            },
            _ => String::new(),
        };
        let mut out = String::new();
        out.push_str(&"  ".repeat(level));
        out.push_str(self.kind_name());
        out.push_str(&id_text);
        out.push_str(&cls_text);
        out.push_str(&detail);
        out.push('\n');
        for c in self.native_children() {
            out.push_str(&c.describe_at(level + 1));
        }
        out
    }

    /// `sameAs` in Kotlin: same kind, key, id, classes, style, content and children.
    pub fn same_as(&self, o: &Native) -> bool {
        if std::ptr::eq(self, o) {
            return true;
        }
        if std::mem::discriminant(&self.kind) != std::mem::discriminant(&o.kind)
            || self.key != o.key
            || self.id != o.id
            || self.classes != o.classes
            || self.style != o.style
        {
            return false;
        }
        let same = match (&self.kind, &o.kind) {
            (NativeKind::Text { spans: a }, NativeKind::Text { spans: b }) => a == b,
            (
                NativeKind::Button { label: l1, disabled: d1 },
                NativeKind::Button { label: l2, disabled: d2 },
            ) => l1 == l2 && d1 == d2,
            (
                NativeKind::Field { field_type: t1, hint: h1, value: v1, disabled: d1, read_only: r1, rows: n1 },
                NativeKind::Field { field_type: t2, hint: h2, value: v2, disabled: d2, read_only: r2, rows: n2 },
            ) => t1 == t2 && h1 == h2 && v1 == v2 && d1 == d2 && r1 == r2 && n1 == n2,
            (
                NativeKind::Checkbox { checked: c1, disabled: d1 },
                NativeKind::Checkbox { checked: c2, disabled: d2 },
            ) => c1 == c2 && d1 == d2,
            (
                NativeKind::Image { source: s1, description: d1 },
                NativeKind::Image { source: s2, description: d2 },
            ) => s1 == s2 && d1 == d2,
            (NativeKind::Item { marker: m1, .. }, NativeKind::Item { marker: m2, .. }) => m1 == m2,
            (
                NativeKind::Progress { value: v1, max: x1, circular: c1 },
                NativeKind::Progress { value: v2, max: x2, circular: c2 },
            ) => v1 == v2 && x1 == x2 && c1 == c2,
            (NativeKind::Box(a), NativeKind::Box(b)) => {
                a.is_virtual == b.is_virtual
                    && a.overlay == b.overlay
                    && a.header == b.header
                    && a.footer == b.footer
                    && a.has_absolute == b.has_absolute
            }
            _ => true,
        };
        if !same {
            return false;
        }
        let a = self.native_children();
        let b = o.native_children();
        if a.len() != b.len() {
            return false;
        }
        a.iter().zip(b.iter()).all(|(x, y)| x.same_as(y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(kind: NativeKind) -> Native {
        Native::new(None, vec![], NativeStyle::default(), 1, kind)
    }

    fn text(s: &str) -> Native {
        mk(NativeKind::Text {
            spans: vec![Span::new(s, false, colors::BLACK, Length::rem(1.0))],
        })
    }

    fn button(s: &str) -> Native {
        mk(NativeKind::Button { label: s.to_string(), disabled: false })
    }

    fn boxed(children: Vec<Native>) -> Native {
        mk(NativeKind::Box(NativeBox { children, ..Default::default() }))
    }

    #[test]
    fn to_pixels() {
        assert_eq!(Length::px(10.0).to_pixels(2.0, 1.5), 20.0);
        assert_eq!(Length::rem(1.0).to_pixels(2.0, 1.0), 32.0);
        assert_eq!(Length::ZERO.to_pixels(3.0, 3.0), 0.0);
        assert_eq!(Length::default(), Length::ZERO);
    }

    #[test]
    fn style_defaults_match_kotlin() {
        let s = NativeStyle::default();
        assert_eq!(s.text_color, 0xFF00_0000);
        assert_eq!(s.background_color, 0);
        assert_eq!(s.font_size, Length::rem(1.0));
        assert_eq!(s.width, Size::Auto);
        assert_eq!(s.direction, Direction::Column);
        assert_eq!(s.align_items, Align::Stretch);
        assert_eq!(s.opacity, 1.0);
        assert_eq!(s.flex, 0.0);
        assert_eq!(s.background_size, ImageFit::Cover);
        assert_eq!(s.object_fit, ImageFit::Contain);
        assert_eq!(s.position, Position::Static);
        assert!(s.animation.is_none() && s.states.is_none() && s.gradient.is_none());
    }

    #[test]
    fn progress_describe_same_and_fraction() {
        let p = |v: Option<f32>| mk(NativeKind::Progress { value: v, max: 12.0, circular: false });
        assert_eq!(p(Some(3.0)).describe(), "NativeProgress 3.0\n");
        assert_eq!(p(Some(0.5)).describe(), "NativeProgress 0.5\n");
        assert_eq!(p(None).describe(), "NativeProgress indeterminate\n");
        assert_eq!(p(Some(3.0)).progress_fraction(), Some(0.25));
        assert_eq!(p(None).progress_fraction(), Some(0.0));
        assert_eq!(p(Some(99.0)).progress_fraction(), Some(1.0));
        assert_eq!(button("x").progress_fraction(), None);
        assert!(p(Some(1.0)).same_as(&p(Some(1.0))));
        assert!(!p(Some(1.0)).same_as(&p(Some(2.0))));
        assert!(!p(Some(1.0)).same_as(&p(None)));
    }

    #[test]
    fn animation_defaults() {
        assert_eq!(Animation::new(AnimKind::Fade), Animation { kind: AnimKind::Fade, duration_ms: 200 });
        assert_eq!(default_animations::SCREEN.duration_ms, 220);
        assert_eq!(default_animations::LAYER.duration_ms, 160);
    }

    #[test]
    fn describe_matches_kotlin_format() {
        let mut root = boxed(vec![text("hi"), button("ok")]);
        root.id = Some("root".to_string());
        root.classes = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            root.describe(),
            "NativeBox #root .a b\n  NativeText \"hi\"\n  NativeButton \"ok\"\n"
        );
        let field = mk(NativeKind::Field {
            field_type: FieldType::Password,
            hint: String::new(),
            value: String::new(),
            disabled: false,
            read_only: false,
            rows: 1,
        });
        assert_eq!(field.describe(), "NativeField PASSWORD\n");
        let img = mk(NativeKind::Image { source: "icon:star".into(), description: String::new() });
        assert_eq!(img.describe(), "NativeImage icon:star\n");
    }

    #[test]
    fn list_children_are_the_items() {
        let item = mk(NativeKind::Item { marker: "-".into(), children: vec![text("x")] });
        let list = mk(NativeKind::List { items: vec![item] });
        assert_eq!(list.native_children().len(), 1);
        assert_eq!(list.native_children()[0].native_children().len(), 1);
        assert_eq!(list.describe(), "NativeList\n  NativeItem\n    NativeText \"x\"\n");
        assert!(button("x").native_children().is_empty());
    }

    #[test]
    fn plain_text_joins_spans() {
        let t = mk(NativeKind::Text {
            spans: vec![
                Span::new("a", false, colors::BLACK, Length::rem(1.0)),
                Span::new("b", true, colors::BLACK, Length::rem(1.0)),
            ],
        });
        assert_eq!(t.plain_text(), Some("ab".to_string()));
        assert_eq!(button("x").plain_text(), None);
    }

    #[test]
    fn same_as_compares_content_and_children() {
        let a = boxed(vec![text("hi"), button("ok")]);
        let b = boxed(vec![text("hi"), button("ok")]);
        assert!(a.same_as(&a));
        assert!(a.same_as(&b));
        assert!(!a.same_as(&boxed(vec![text("hi"), button("no")])));
        assert!(!a.same_as(&boxed(vec![text("hi")])));
        assert!(!text("x").same_as(&button("x")));
        let mut c = b.clone();
        c.key = "k".to_string();
        assert!(!a.same_as(&c));
        let mut d = b.clone();
        d.style.bold = true;
        assert!(!a.same_as(&d));
        let overlay = mk(NativeKind::Box(NativeBox { overlay: true, ..Default::default() }));
        assert!(!boxed(vec![]).same_as(&overlay));
    }
}
