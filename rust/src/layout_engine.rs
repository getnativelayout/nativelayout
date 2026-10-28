//! Port of LayoutEngine.kt: turns the (already expanded) HTML into the neutral tree.

use std::collections::{HashMap, HashSet};

use crate::css::{self, CssDeclaration};
use crate::error::NativeLayoutError;
use crate::html_parser::{HtmlElement, HtmlNode, HtmlParser};
use crate::icons;
use crate::tree::*;

type Res<T> = Result<T, NativeLayoutError>;

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutWarning {
    pub message: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub root: Native,
    pub warnings: Vec<LayoutWarning>,
    /// A copy of every element that has an `id`, by id. The copies carry their final `key`.
    pub by_id: HashMap<String, Native>,
}

/// `LayoutEngine.build(html, startLine, variables)`.
pub fn build(html: &str, start_line: usize, variables: &HashMap<String, String>) -> Res<LayoutResult> {
    LayoutBuilder::new(html, start_line, variables).build()
}

const SUPPORTED_TAGS: [&str; 18] = [
    "div", "span", "p", "h1", "h2", "h3", "button", "input", "textarea", "img", "ul", "li", "fieldset",
    "legend", "dialog", "header", "footer", "progress",
];
const TAGS_SUGGESTION: &str =
    "supported tags: div, span, p, h1, h2, h3, button, input, textarea, img, ul, li, fieldset, legend, dialog, header, footer, progress";
const FIELDSET_BORDER: u32 = 0xFFBD_BDBD;
const DIALOG_BACKDROP: u32 = 0x8000_0000;
const PROGRESS_TRACK: u32 = 0xFFE0_E0E0;
const PROGRESS_BAR: u32 = 0xFF2F_81F7;
/// Tags whose content can scroll with `overflow-y: auto|scroll` (the root already scrolls with the screen).
const SCROLL_TAGS: [&str; 4] = ["div", "header", "footer", "fieldset"];
const BORDER_STYLES: [&str; 8] = ["solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset"];
const DECORATION_STYLES: [&str; 5] = ["solid", "double", "dotted", "dashed", "wavy"];

struct LayoutBuilder {
    html: String,
    start_line: usize,
    variables: HashMap<String, String>,
    source: String,
    warnings: Vec<LayoutWarning>,
    /// How deep the element being converted is (the root is 0) and the tag of its parent.
    depth: usize,
    parent_tag: Option<String>,
    /// True while converting the children of a `<fieldset disabled>`.
    group_disabled: bool,
    warned: HashSet<(String, usize)>,
    reserved_ids: HashSet<String>,
    /// Every element with an `id`, as it was when it was built (before the keys are assigned).
    ids: HashMap<String, Native>,
}

// ---------- free helpers ----------

/// Matches `var(\s*(--[\w-]+)\s*)` at `i`. Gives the variable name and the index after the match.
fn match_var(chars: &[char], i: usize) -> Option<(String, usize)> {
    let pat = ['v', 'a', 'r', '('];
    if i + 4 > chars.len() || chars[i..i + 4] != pat {
        return None;
    }
    let mut j = i + 4;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if j + 2 > chars.len() || chars[j] != '-' || chars[j + 1] != '-' {
        return None;
    }
    let name_start = j;
    j += 2;
    let body_start = j;
    while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '-') {
        j += 1;
    }
    if j == body_start {
        return None;
    }
    let name: String = chars[name_start..j].iter().collect();
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if j < chars.len() && chars[j] == ')' {
        Some((name, j + 1))
    } else {
        None
    }
}

fn inherit(p: &NativeStyle) -> NativeStyle {
    let mut s = NativeStyle::default();
    s.text_color = p.text_color;
    s.font_size = p.font_size;
    s.bold = p.bold;
    s.text_align = p.text_align;
    s.italic = p.italic;
    s.underline = p.underline;
    s.strike = p.strike;
    s.letter_spacing = p.letter_spacing;
    s.line_height = p.line_height;
    s.text_transform = p.text_transform;
    s.font_family = p.font_family;
    s
}

/// `url(name)`, with or without quotes: the name of an image. `None` when the text is not that.
fn image_name_of(v: &str) -> Option<String> {
    let t = v.trim();
    if !t.to_lowercase().starts_with("url(") || !t.ends_with(')') || t.len() < 5 {
        return None;
    }
    let name = t[4..t.len() - 1].trim().trim_matches(|c| c == '"' || c == '\'').trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// A length for top, right, bottom or left. `auto` gives `Some(None)`; no length at all gives `None`.
fn offset_of(v: &str) -> Option<Option<Length>> {
    if v.trim().to_lowercase() == "auto" {
        return Some(None);
    }
    css::length(v).map(Some)
}

fn vertical(rem: f32) -> Sides {
    Sides { top: Length::rem(rem), bottom: Length::rem(rem), ..Sides::default() }
}

fn all_sides(l: Length) -> Sides {
    Sides { top: l, right: l, bottom: l, left: l }
}

fn tag_defaults(el: &HtmlElement, b: NativeStyle) -> NativeStyle {
    let mut s = b;
    match el.tag.as_str() {
        "h1" => {
            s.font_size = Length::rem(2.0);
            s.bold = true;
            s.margin = vertical(1.3);
        }
        "h2" => {
            s.font_size = Length::rem(1.5);
            s.bold = true;
            s.margin = vertical(1.2);
        }
        "h3" => {
            s.font_size = Length::rem(1.17);
            s.bold = true;
            s.margin = vertical(1.1);
        }
        "p" => {
            s.margin = vertical(1.0);
        }
        "fieldset" => {
            s.padding = all_sides(Length::rem(0.75));
            s.margin = vertical(0.5);
            s.border_width = Length::px(1.0);
            s.border_color = FIELDSET_BORDER;
            s.border_radius = Length::px(6.0);
        }
        "legend" => {
            s.bold = true;
            s.margin = Sides { bottom: Length::rem(0.5), ..Sides::default() };
        }
        "dialog" => {
            s.background_color = DIALOG_BACKDROP;
            s.justify = Justify::Center;
            s.align_items = Align::Center;
        }
        "footer" => {
            s.direction = Direction::Row;
            s.justify = Justify::Center;
            s.align_items = Align::Center;
        }
        "progress" => {
            if el.has_attr("circular") {
                s.width = Size::Dim(Length::rem(2.0));
                s.height = Size::Dim(Length::rem(2.0));
            } else {
                s.height = Size::Dim(Length::rem(0.5));
                s.border_radius = Length::px(999.0);
            }
            s.background_color = PROGRESS_TRACK;
            s.text_color = PROGRESS_BAR;
        }
        "ul" => {
            s.margin = vertical(1.0);
            s.padding = Sides { left: Length::rem(1.5), ..Sides::default() };
        }
        "button" => {
            s.padding = Sides {
                top: Length::rem(0.5),
                right: Length::rem(1.0),
                bottom: Length::rem(0.5),
                left: Length::rem(1.0),
            };
            s.background_color = colors::BUTTON_GRAY;
            s.border_radius = Length::px(4.0);
            s.text_align = TextAlign::Center;
            s.text_color = colors::BLACK;
            s.bold = false;
        }
        "input" | "textarea" => {
            let kind = el.attr("type").unwrap_or("text").trim().to_lowercase();
            if kind != "checkbox" {
                s.padding = all_sides(Length::rem(0.5));
                s.border_width = Length::px(1.0);
                s.border_color = colors::BORDER_GRAY;
                s.border_radius = Length::px(2.0);
                s.background_color = colors::WHITE;
                s.text_color = colors::BLACK;
            }
        }
        _ => {}
    }
    s
}

fn sides_of(v: &str) -> Option<Sides> {
    let mut p: Vec<Length> = Vec::new();
    for t in css::split_top_level(v, ' ') {
        if t.trim().is_empty() {
            continue;
        }
        p.push(css::length(&t)?);
    }
    match p.len() {
        1 => Some(Sides { top: p[0], right: p[0], bottom: p[0], left: p[0] }),
        2 => Some(Sides { top: p[0], right: p[1], bottom: p[0], left: p[1] }),
        3 => Some(Sides { top: p[0], right: p[1], bottom: p[2], left: p[1] }),
        4 => Some(Sides { top: p[0], right: p[1], bottom: p[2], left: p[3] }),
        _ => None,
    }
}

/// border-radius with one to four lengths, in the CSS order. The slash form and percentages are not supported.
fn corners_of(v: &str) -> Option<Corners> {
    if v.contains('/') {
        return None;
    }
    let mut p: Vec<Length> = Vec::new();
    for t in css::split_top_level(v, ' ') {
        if t.trim().is_empty() {
            continue;
        }
        let l = css::length(&t)?;
        if l.value < 0.0 {
            return None;
        }
        p.push(l);
    }
    let c = |a: Length, b: Length, c: Length, d: Length| Corners {
        top_left: a,
        top_right: b,
        bottom_right: c,
        bottom_left: d,
    };
    match p.len() {
        1 => Some(c(p[0], p[0], p[0], p[0])),
        2 => Some(c(p[0], p[1], p[0], p[1])),
        3 => Some(c(p[0], p[1], p[2], p[1])),
        4 => Some(c(p[0], p[1], p[2], p[3])),
        _ => None,
    }
}

/// Equal corners stay a single radius; different ones go to `NativeStyle::corners`.
fn with_corners(e: &NativeStyle, c: Corners) -> NativeStyle {
    let mut r = e.clone();
    if c.top_left == c.top_right && c.top_right == c.bottom_right && c.bottom_right == c.bottom_left {
        r.border_radius = c.top_left;
        r.corners = None;
    } else {
        r.border_radius = Length::ZERO;
        r.corners = Some(c);
    }
    r
}

fn direction_angle(words: &str) -> Option<i32> {
    let mut w: Vec<&str> = words.split(' ').filter(|x| !x.is_empty()).collect();
    w.sort();
    w.dedup();
    match w.as_slice() {
        ["top"] => Some(0),
        ["right", "top"] => Some(45),
        ["right"] => Some(90),
        ["bottom", "right"] => Some(135),
        ["bottom"] => Some(180),
        ["bottom", "left"] => Some(225),
        ["left"] => Some(270),
        ["left", "top"] => Some(315),
        _ => None,
    }
}

fn gradient_of(v: &str) -> Option<Gradient> {
    let open = v.find('(')?;
    let close = v.rfind(')')?;
    if close <= open {
        return None;
    }
    let args: Vec<String> = css::split_top_level(&v[open + 1..close], ',')
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if args.len() < 2 {
        return None;
    }
    let mut angle: i32 = 180;
    let mut from = 0;
    let first = args[0].to_lowercase();
    let degrees = first.strip_suffix("deg").filter(|n| css::is_number(n));
    if let Some(n) = degrees {
        let f = css::parse_f32(n)?;
        angle = ((css::round_to_int(f) % 360) + 360) % 360;
        from = 1;
    } else if first.starts_with("to ") {
        angle = direction_angle(first.strip_prefix("to ").unwrap_or(&first).trim())?;
        from = 1;
    }
    let mut colors: Vec<u32> = Vec::new();
    for a in &args[from..] {
        let mut found: Option<u32> = None;
        for tok in css::split_top_level(a, ' ') {
            if tok.trim().is_empty() {
                continue;
            }
            if let Some(c) = css::color(&tok) {
                found = Some(c);
                break;
            }
        }
        colors.push(found?);
    }
    if colors.len() >= 2 {
        Some(Gradient { angle, colors })
    } else {
        None
    }
}

/// box-shadow becomes an elevation: half of the blur (or of the vertical offset when there is no blur).
fn elevation_of(v: &str) -> Option<Length> {
    let parts = css::split_top_level(v, ',');
    let first = parts[0].trim().to_string();
    if first.to_lowercase() == "none" {
        return Some(Length::ZERO);
    }
    let mut lengths: Vec<Length> = Vec::new();
    for tok in css::split_top_level(&first, ' ') {
        if tok.trim().is_empty() {
            continue;
        }
        if tok.to_lowercase() == "inset" {
            return None;
        }
        match css::length(&tok) {
            Some(l) => lengths.push(l),
            None => {
                if css::color(&tok).is_none() {
                    return None;
                }
            }
        }
    }
    if lengths.len() < 2 || lengths.len() > 4 {
        return None;
    }
    let r = if lengths.len() >= 3 && lengths[2].value > 0.0 { lengths[2] } else { lengths[1] };
    Some(Length { value: r.value.max(0.0) / 2.0, scalable: r.scalable })
}

fn font_family_of(v: &str) -> Option<FontFamily> {
    for raw in css::split_top_level(v, ',') {
        let name = raw.trim().trim_matches(|c| c == '"' || c == '\'').to_lowercase();
        match name.as_str() {
            "sans-serif" | "system-ui" | "roboto" | "arial" | "helvetica" | "default" => {
                return Some(FontFamily::Sans)
            }
            "serif" | "georgia" | "times" | "times new roman" => return Some(FontFamily::Serif),
            "monospace" | "courier" | "courier new" | "consolas" | "menlo" => {
                return Some(FontFamily::Monospace)
            }
            _ => {}
        }
    }
    None
}

/// `animation: <none | fade | slide-up | slide-left> [<time>]`, time in ms or s (0 to 2 s).
fn animation_of(v: &str) -> Option<Animation> {
    let lower = v.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }
    let kind = match parts[0] {
        "none" => AnimKind::None,
        "fade" => AnimKind::Fade,
        "slide-up" => AnimKind::SlideUp,
        "slide-left" => AnimKind::SlideLeft,
        _ => return None,
    };
    let mut ms: i32 = 200;
    if parts.len() == 2 {
        let t = parts[1];
        ms = if let Some(n) = t.strip_suffix("ms") {
            css::parse_f32(n)? as i32
        } else if let Some(n) = t.strip_suffix('s') {
            (css::parse_f32(n)? * 1000.0) as i32
        } else {
            return None;
        };
    }
    if !(0..=2000).contains(&ms) {
        return None;
    }
    Some(Animation { kind, duration_ms: ms })
}

fn is_space(ch: char) -> bool {
    ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r'
}

fn transformed(text: &str, t: TextTransform) -> String {
    match t {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => {
            let mut sb = String::with_capacity(text.len());
            let mut start = true;
            for ch in text.chars() {
                if start && ch.is_alphabetic() {
                    let mut up = ch.to_uppercase();
                    match (up.next(), up.next()) {
                        (Some(u), None) => sb.push(u),
                        _ => sb.push(ch),
                    }
                } else {
                    sb.push(ch);
                }
                start = is_space(ch);
            }
            sb
        }
    }
}

fn make_span(text: &str, e: &NativeStyle) -> Span {
    Span {
        text: transformed(text, e.text_transform),
        bold: e.bold,
        color: e.text_color,
        size: e.font_size,
        italic: e.italic,
        underline: e.underline,
        strike: e.strike,
    }
}

fn normalize(inline_run: &[Span]) -> Vec<Span> {
    let mut result: Vec<Span> = Vec::new();
    let mut last_space = true;
    for t in inline_run {
        let mut sb = String::new();
        for ch in t.text.chars() {
            if is_space(ch) {
                if !last_space {
                    sb.push(' ');
                    last_space = true;
                }
            } else {
                sb.push(ch);
                last_space = false;
            }
        }
        if !sb.is_empty() {
            let mut s = t.clone();
            s.text = sb;
            result.push(s);
        }
    }
    if !result.is_empty() {
        let (cleaned, old_len) = {
            let u = &result[result.len() - 1];
            (u.text.trim_end_matches(' ').to_string(), u.text.len())
        };
        if cleaned.is_empty() {
            result.pop();
        } else if cleaned.len() != old_len {
            let n = result.len();
            result[n - 1].text = cleaned;
        }
    }
    result
}

fn flush(result: &mut Vec<Native>, run: &mut Vec<Span>, run_line: &mut Option<usize>, parent_style: &NativeStyle) {
    let spans = normalize(run);
    if !spans.is_empty() {
        result.push(Native::new(
            None,
            Vec::new(),
            inherit(parent_style),
            run_line.unwrap_or(0),
            NativeKind::Text { spans },
        ));
    }
    run.clear();
    *run_line = None;
}

fn key_of(el: &HtmlElement) -> Option<String> {
    let v = el.attr("key")?.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn scope_of(el: &HtmlElement) -> Option<String> {
    let v = el.attr("scope")?.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn is_inline(el: &HtmlElement) -> bool {
    el.tag == "span"
        && el.id().is_none()
        && !el.has_attr("class")
        && !el.children.iter().any(|c| match c {
            HtmlNode::Element(e) => !is_inline(e),
            _ => false,
        })
}

fn collect_ids(n: &Native, map: &mut HashMap<String, Native>) {
    if let Some(id) = &n.id {
        map.insert(id.clone(), n.clone());
    }
    for c in n.native_children() {
        collect_ids(c, map);
    }
}

// ---------- the builder ----------

impl LayoutBuilder {
    fn new(html: &str, start_line: usize, variables: &HashMap<String, String>) -> LayoutBuilder {
        let source = format!("{}{}", "\n".repeat(start_line.saturating_sub(1)), html);
        LayoutBuilder {
            html: html.to_string(),
            start_line,
            variables: variables.clone(),
            source,
            warnings: Vec::new(),
            depth: 0,
            parent_tag: None,
            group_disabled: false,
            warned: HashSet::new(),
            reserved_ids: HashSet::new(),
            ids: HashMap::new(),
        }
    }

    fn err(&self, message: &str, line: usize, word: Option<&str>, suggestion: Option<&str>) -> NativeLayoutError {
        NativeLayoutError::markup(message, line, &self.source, word, suggestion)
    }

    fn warn(&mut self, message: &str, line: usize) {
        if self.warned.insert((message.to_string(), line)) {
            self.warnings.push(LayoutWarning { message: message.to_string(), line });
        }
    }

    fn build(mut self) -> Res<LayoutResult> {
        let nodes = HtmlParser::new(&self.html, self.start_line, &self.source).parse()?;
        let relevant: Vec<&HtmlNode> = nodes
            .iter()
            .filter(|n| !matches!(n, HtmlNode::Text(t) if t.text.trim().is_empty()))
            .collect();
        let root_el = if relevant.len() == 1 {
            match relevant[0] {
                HtmlNode::Element(e) => Some(e),
                _ => None,
            }
        } else {
            None
        };
        let root_el = match root_el {
            Some(e) => e,
            None => {
                let line = relevant
                    .get(1)
                    .map(|n| n.line())
                    .or_else(|| relevant.first().map(|n| n.line()))
                    .unwrap_or(self.start_line);
                return Err(self.err("must have a single root tag", line, None, None));
            }
        };
        let mut root = self.convert_element(root_el, &NativeStyle::default())?;
        self.assign_keys(&mut root, "")?;
        // In Kotlin `byId` holds the very objects of the tree, so an element that was folded into its
        // parent (a lone <p id="x"> in a <div>) is still there, without a key. The ones that are in
        // the tree carry their key.
        let mut by_id = self.ids;
        let mut in_tree = HashMap::new();
        collect_ids(&root, &mut in_tree);
        for (k, v) in in_tree {
            by_id.insert(k, v);
        }
        Ok(LayoutResult { root, warnings: self.warnings, by_id })
    }

    fn assign_keys(&self, n: &mut Native, path: &str) -> Res<()> {
        n.key = match &n.id {
            Some(id) => format!("#{}", id),
            None => {
                if path.is_empty() {
                    "/".to_string()
                } else {
                    path.to_string()
                }
            }
        };
        let mut seen: HashSet<String> = HashSet::new();
        for (i, f) in n.native_children_mut().iter_mut().enumerate() {
            let hint = f.key_hint.clone();
            if let Some(h) = &hint {
                if !seen.insert(h.clone()) {
                    return Err(self.err(&format!("duplicate key '{}'", h), f.line, Some(h.as_str()), None));
                }
            }
            let seg = match &hint {
                Some(h) => format!("~{}", h),
                None => i.to_string(),
            };
            self.assign_keys(f, &format!("{}/{}", path, seg))?;
        }
        Ok(())
    }

    fn substitute_variables(&self, text: &str, line: usize) -> Res<String> {
        if !text.contains("var(") {
            return Ok(text.to_string());
        }
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if let Some((name, end)) = match_var(&chars, i) {
                match self.variables.get(&name) {
                    Some(v) => out.push_str(v),
                    None => {
                        let msg = format!("the variable '{}' is not defined", name);
                        return Err(self.err(&msg, line, Some(name.as_str()), None));
                    }
                }
                i = end;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        Ok(out)
    }

    fn declarations_of(&mut self, el: &HtmlElement) -> Res<Vec<CssDeclaration>> {
        let s = match el.attr("style") {
            Some(s) => s.to_string(),
            None => return Ok(Vec::new()),
        };
        let text = self.substitute_variables(&s, el.line)?;
        Ok(css::declarations(&text, el.line, |m, l| self.warn(m, l)))
    }

    /// `position: absolute` only makes sense inside a box of the flow: elsewhere it is ignored with a warning.
    fn fix_position(&mut self, el: &HtmlElement, s: NativeStyle, level: usize) -> NativeStyle {
        if s.position != Position::Absolute {
            return s;
        }
        let why: Option<String> = if el.tag == "header" || el.tag == "footer" || el.tag == "dialog" {
            Some(format!("<{}> has its own place", el.tag))
        } else if el.tag == "li" {
            Some("<li> stays in its list".to_string())
        } else if level == 0 {
            Some("the root of the screen has no parent".to_string())
        } else {
            None
        };
        let why = match why {
            Some(w) => w,
            None => return s,
        };
        self.warn(&format!("CSS: 'position: absolute' is ignored: {}", why), el.line);
        let mut r = s;
        r.position = Position::Static;
        r
    }

    /// `overflow-y: auto|scroll` is kept only where it can work: not on the root, and only on boxes.
    fn scrollable(&mut self, el: &HtmlElement, s: NativeStyle, depth: usize) -> NativeStyle {
        if !s.scroll_y {
            return s;
        }
        let reason = if depth == 0 {
            "the screen already scrolls".to_string()
        } else if !SCROLL_TAGS.contains(&el.tag.as_str()) {
            format!("it works on div, header, footer and fieldset, not on <{}>", el.tag)
        } else {
            return s;
        };
        self.warn(&format!("CSS: 'overflow-y: auto|scroll' is ignored here ({})", reason), el.line);
        let mut r = s;
        r.scroll_y = false;
        r.overflow_hidden = false;
        r
    }

    fn compute_style(&mut self, el: &HtmlElement, parent_style: &NativeStyle) -> Res<NativeStyle> {
        let mut e = tag_defaults(el, inherit(parent_style));
        for d in self.declarations_of(el)? {
            e = self.apply_declaration(&e, &d);
        }
        let pressed = self.state_of(el, "style-pressed", false);
        let disabled = self.state_of(el, "style-disabled", true);
        let focused = self.state_of(el, "style-focus", false);
        if pressed.is_some() || disabled.is_some() || focused.is_some() {
            e.states = Some(StateStyles { pressed, disabled, focused });
        }
        Ok(e)
    }

    /// Declarations that the expander wrote for :pressed, :disabled and :focus.
    fn state_of(&mut self, el: &HtmlElement, attribute: &str, allow_opacity: bool) -> Option<StateStyle> {
        let text = el.attr(attribute)?.to_string();
        let mut background: Option<u32> = None;
        let mut color: Option<u32> = None;
        let mut border: Option<u32> = None;
        let mut opacity: Option<f32> = None;
        for d in css::declarations(&text, el.line, |m, l| self.warn(m, l)) {
            let v = d.value.trim().to_string();
            let bad = format!("CSS: invalid value in '{}: {}', ignored", d.name, d.value);
            match d.name.as_str() {
                "background" | "background-color" => match css::color(&v) {
                    Some(c) => background = Some(c),
                    None => self.warn(&bad, d.line),
                },
                "color" => match css::color(&v) {
                    Some(c) => color = Some(c),
                    None => self.warn(&bad, d.line),
                },
                "border-color" => match css::color(&v) {
                    Some(c) => border = Some(c),
                    None => self.warn(&bad, d.line),
                },
                "opacity" => {
                    if !allow_opacity {
                        self.warn("CSS: 'opacity' is only supported in :disabled", d.line);
                    } else {
                        match css::fraction(&v) {
                            Some(f) => opacity = Some(f),
                            None => self.warn(&bad, d.line),
                        }
                    }
                }
                _ => {
                    let msg = format!(
                        "CSS: '{}' is not supported in :pressed, :disabled or :focus (use background, color, border-color; opacity only in :disabled)",
                        d.name
                    );
                    self.warn(&msg, d.line);
                }
            }
        }
        if background.is_none() && color.is_none() && border.is_none() && opacity.is_none() {
            return None;
        }
        Some(StateStyle {
            background_color: background,
            text_color: color,
            border_color: border,
            opacity,
        })
    }

    fn invalid(&mut self, e: &NativeStyle, d: &CssDeclaration) -> NativeStyle {
        let msg = format!("CSS: invalid value in '{}: {}', ignored", d.name, d.value);
        self.warn(&msg, d.line);
        e.clone()
    }

    fn unsupported(&mut self, e: &NativeStyle, d: &CssDeclaration, msg: String) -> NativeStyle {
        self.warn(&msg, d.line);
        e.clone()
    }

    fn apply_declaration(&mut self, e: &NativeStyle, d: &CssDeclaration) -> NativeStyle {
        let v = d.value.trim().to_string();
        let vl = v.to_lowercase();
        let len = css::length(&v);
        let mut r = e.clone();

        match d.name.as_str() {
            "color" => match css::color(&v) {
                Some(c) => {
                    r.text_color = c;
                    r
                }
                None => self.invalid(e, d),
            },
            "background" | "background-color" | "background-image" => {
                let is_gradient = vl.starts_with("linear-gradient(") && vl.ends_with(')');
                let image = image_name_of(&v);
                if is_gradient {
                    match gradient_of(&v) {
                        Some(g) => {
                            r.gradient = Some(g);
                            r.background_image = None;
                            r
                        }
                        None => self.invalid(e, d),
                    }
                } else if let Some(img) = image {
                    r.background_image = Some(img);
                    r
                } else if d.name == "background-image" {
                    if vl == "none" {
                        r.gradient = None;
                        r.background_image = None;
                        r
                    } else {
                        self.invalid(e, d)
                    }
                } else {
                    match css::color(&v) {
                        Some(c) => {
                            r.background_color = c;
                            r.gradient = None;
                            if d.name == "background" {
                                r.background_image = None;
                            }
                            r
                        }
                        None => self.invalid(e, d),
                    }
                }
            }
            "font-size" => match len.filter(|l| l.value > 0.0) {
                Some(l) => {
                    r.font_size = l;
                    r
                }
                None => self.invalid(e, d),
            },
            "font-weight" => {
                let n = vl.parse::<i32>().ok();
                if vl == "bold" || vl == "bolder" {
                    r.bold = true;
                    r
                } else if vl == "normal" || vl == "lighter" {
                    r.bold = false;
                    r
                } else if let Some(n) = n {
                    r.bold = n >= 600;
                    r
                } else {
                    self.invalid(e, d)
                }
            }
            "padding" => match sides_of(&v) {
                Some(s) => {
                    r.padding = s;
                    r
                }
                None => self.invalid(e, d),
            },
            "margin" => match sides_of(&v) {
                Some(s) => {
                    r.margin = s;
                    r
                }
                None => self.invalid(e, d),
            },
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => match len {
                Some(l) => {
                    match d.name.as_str() {
                        "padding-top" => r.padding.top = l,
                        "padding-right" => r.padding.right = l,
                        "padding-bottom" => r.padding.bottom = l,
                        _ => r.padding.left = l,
                    }
                    r
                }
                None => self.invalid(e, d),
            },
            "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => match len {
                Some(l) => {
                    match d.name.as_str() {
                        "margin-top" => r.margin.top = l,
                        "margin-right" => r.margin.right = l,
                        "margin-bottom" => r.margin.bottom = l,
                        _ => r.margin.left = l,
                    }
                    r
                }
                None => self.invalid(e, d),
            },
            "border-width" => match len.filter(|l| l.value >= 0.0) {
                Some(l) => {
                    r.border_width = l;
                    r
                }
                None => self.invalid(e, d),
            },
            "border-color" => match css::color(&v) {
                Some(c) => {
                    r.border_color = c;
                    r
                }
                None => self.invalid(e, d),
            },
            "border-style" => {
                if vl == "none" || vl == "hidden" {
                    r.border_width = Length::ZERO;
                    r
                } else if BORDER_STYLES.contains(&vl.as_str()) {
                    r
                } else {
                    self.invalid(e, d)
                }
            }
            "text-align" => match vl.as_str() {
                "left" | "start" => {
                    r.text_align = TextAlign::Start;
                    r
                }
                "center" => {
                    r.text_align = TextAlign::Center;
                    r
                }
                "right" | "end" => {
                    r.text_align = TextAlign::End;
                    r
                }
                "justify" => {
                    self.warn("CSS: 'text-align: justify' is not supported, using start", d.line);
                    r.text_align = TextAlign::Start;
                    r
                }
                _ => self.invalid(e, d),
            },
            "opacity" => match css::fraction(&v) {
                Some(f) => {
                    r.opacity = f;
                    r
                }
                None => self.invalid(e, d),
            },
            "font-style" => match vl.as_str() {
                "italic" | "oblique" => {
                    r.italic = true;
                    r
                }
                "normal" => {
                    r.italic = false;
                    r
                }
                _ => self.invalid(e, d),
            },
            "text-decoration" | "text-decoration-line" => {
                let mut underline = false;
                let mut strike = false;
                let mut none = false;
                let mut ok = true;
                for tok in css::split_top_level(&vl, ' ') {
                    if tok.trim().is_empty() {
                        continue;
                    }
                    let t = tok.as_str();
                    if t == "none" {
                        none = true;
                    } else if t == "underline" {
                        underline = true;
                    } else if t == "line-through" {
                        strike = true;
                    } else if t == "overline" {
                        self.warn("CSS: 'text-decoration: overline' is not supported, ignored", d.line);
                    } else if DECORATION_STYLES.contains(&t) || css::color(t).is_some() || css::length(t).is_some() {
                        // style, color and thickness do not change what is drawn
                    } else {
                        ok = false;
                    }
                }
                if !ok {
                    self.invalid(e, d)
                } else if none && !underline && !strike {
                    r.underline = false;
                    r.strike = false;
                    r
                } else {
                    r.underline = underline;
                    r.strike = strike;
                    r
                }
            }
            "letter-spacing" => {
                if vl == "normal" {
                    r.letter_spacing = Length::ZERO;
                    r
                } else {
                    match len {
                        Some(l) => {
                            r.letter_spacing = l;
                            r
                        }
                        None => self.invalid(e, d),
                    }
                }
            }
            "line-height" => {
                let number = css::parse_f32(&vl);
                let percent = match vl.strip_suffix('%') {
                    Some(n) => css::parse_f32(n),
                    None => None,
                };
                if vl == "normal" {
                    r.line_height = None;
                    r
                } else if let Some(n) = number.filter(|n| *n > 0.0) {
                    r.line_height = Some(LineHeight::Multiple(n));
                    r
                } else if let Some(p) = percent.filter(|p| *p > 0.0) {
                    r.line_height = Some(LineHeight::Multiple(p / 100.0));
                    r
                } else if let Some(l) = len.filter(|l| l.value > 0.0) {
                    r.line_height = Some(LineHeight::Fixed(l));
                    r
                } else {
                    self.invalid(e, d)
                }
            }
            "text-transform" => match vl.as_str() {
                "none" => {
                    r.text_transform = TextTransform::None;
                    r
                }
                "uppercase" => {
                    r.text_transform = TextTransform::Uppercase;
                    r
                }
                "lowercase" => {
                    r.text_transform = TextTransform::Lowercase;
                    r
                }
                "capitalize" => {
                    r.text_transform = TextTransform::Capitalize;
                    r
                }
                _ => self.invalid(e, d),
            },
            "font-family" => match font_family_of(&v) {
                Some(f) => {
                    r.font_family = f;
                    r
                }
                None => {
                    let msg = format!(
                        "CSS: no known family in 'font-family: {}' (use sans-serif, serif or monospace), ignored",
                        v
                    );
                    self.unsupported(e, d, msg)
                }
            },
            "overflow" | "overflow-x" | "overflow-y" => {
                let vertical = d.name != "overflow-x";
                if vl == "auto" || vl == "scroll" {
                    if vertical {
                        r.scroll_y = true;
                        r.overflow_hidden = true;
                        r
                    } else {
                        let msg = format!(
                            "CSS: '{}: {}' is not supported (only vertical scrolling exists), ignored",
                            d.name, vl
                        );
                        self.unsupported(e, d, msg)
                    }
                } else if (vl == "hidden" || vl == "clip" || vl == "visible") && !vertical && e.scroll_y {
                    // overflow-x does not undo a vertical scroll
                    r
                } else if vl == "hidden" || vl == "clip" {
                    r.overflow_hidden = true;
                    r.scroll_y = false;
                    r
                } else if vl == "visible" {
                    r.overflow_hidden = false;
                    r.scroll_y = false;
                    r
                } else {
                    let msg = format!(
                        "CSS: '{}: {}' is not supported (use auto, scroll, hidden or visible)",
                        d.name, vl
                    );
                    self.unsupported(e, d, msg)
                }
            }
            "box-shadow" => match elevation_of(&v) {
                Some(l) => {
                    r.elevation = l;
                    r
                }
                None => self.invalid(e, d),
            },
            "min-width" => {
                if vl == "0" || vl == "auto" {
                    r.min_width = None;
                    r
                } else {
                    match len {
                        Some(l) => {
                            r.min_width = Some(l);
                            r
                        }
                        None => self.invalid(e, d),
                    }
                }
            }
            "min-height" => {
                if vl == "0" || vl == "auto" {
                    r.min_height = None;
                    r
                } else {
                    match len {
                        Some(l) => {
                            r.min_height = Some(l);
                            r
                        }
                        None => self.invalid(e, d),
                    }
                }
            }
            "max-width" => {
                if vl == "none" {
                    r.max_width = None;
                    r
                } else {
                    match len {
                        Some(l) => {
                            r.max_width = Some(l);
                            r
                        }
                        None => self.invalid(e, d),
                    }
                }
            }
            "max-height" => {
                if vl == "none" {
                    r.max_height = None;
                    r
                } else {
                    match len {
                        Some(l) => {
                            r.max_height = Some(l);
                            r
                        }
                        None => self.invalid(e, d),
                    }
                }
            }
            "align-self" => match vl.as_str() {
                "auto" => {
                    r.align_self = None;
                    r
                }
                "flex-start" | "start" => {
                    r.align_self = Some(Align::Start);
                    r
                }
                "center" => {
                    r.align_self = Some(Align::Center);
                    r
                }
                "flex-end" | "end" => {
                    r.align_self = Some(Align::End);
                    r
                }
                "stretch" => {
                    r.align_self = Some(Align::Stretch);
                    r
                }
                _ => self.invalid(e, d),
            },
            "flex-wrap" => match vl.as_str() {
                "wrap" => {
                    r.wrap = true;
                    r
                }
                "nowrap" => {
                    r.wrap = false;
                    r
                }
                _ => {
                    let msg = format!("CSS: 'flex-wrap: {}' is not supported (use wrap or nowrap)", vl);
                    self.unsupported(e, d, msg)
                }
            },
            "border-radius" => match corners_of(&v) {
                Some(c) => with_corners(e, c),
                None => self.invalid(e, d),
            },
            "border-top-left-radius"
            | "border-top-right-radius"
            | "border-bottom-right-radius"
            | "border-bottom-left-radius" => match len.filter(|l| l.value >= 0.0) {
                Some(l) => {
                    let mut c = e.corners.unwrap_or(Corners {
                        top_left: e.border_radius,
                        top_right: e.border_radius,
                        bottom_right: e.border_radius,
                        bottom_left: e.border_radius,
                    });
                    match d.name.as_str() {
                        "border-top-left-radius" => c.top_left = l,
                        "border-top-right-radius" => c.top_right = l,
                        "border-bottom-right-radius" => c.bottom_right = l,
                        _ => c.bottom_left = l,
                    }
                    with_corners(e, c)
                }
                None => self.invalid(e, d),
            },
            "border" => {
                if vl == "none" || vl == "0" {
                    r.border_width = Length::ZERO;
                    r
                } else {
                    let mut width: Option<Length> = None;
                    let mut color: Option<u32> = None;
                    let mut no_border = false;
                    let mut ok = true;
                    for tok in css::split_top_level(&v, ' ') {
                        if tok.trim().is_empty() {
                            continue;
                        }
                        let tl = tok.to_lowercase();
                        if let Some(w) = css::length(&tok) {
                            width = Some(w);
                        } else if tl == "none" || tl == "hidden" {
                            no_border = true;
                        } else if BORDER_STYLES.contains(&tl.as_str()) {
                            // the style does not change what is drawn
                        } else {
                            match css::color(&tok) {
                                Some(c) => color = Some(c),
                                None => ok = false,
                            }
                        }
                    }
                    if !ok {
                        self.invalid(e, d)
                    } else {
                        r.border_width = if no_border { Length::ZERO } else { width.unwrap_or(Length::px(3.0)) };
                        if let Some(c) = color {
                            r.border_color = c;
                        }
                        r
                    }
                }
            }
            "display" => match vl.as_str() {
                "flex" | "block" => r,
                _ => {
                    let msg = format!("CSS: 'display: {}' is not supported in v1 (use display:flex)", vl);
                    self.unsupported(e, d, msg)
                }
            },
            "flex-direction" => match vl.as_str() {
                "row" => {
                    r.direction = Direction::Row;
                    r
                }
                "column" => {
                    r.direction = Direction::Column;
                    r
                }
                _ => {
                    let msg = format!("CSS: 'flex-direction: {}' is not supported in v1 (use row or column)", vl);
                    self.unsupported(e, d, msg)
                }
            },
            "flex" => {
                let g: Option<f32> = match vl.as_str() {
                    "none" => Some(0.0),
                    "auto" => Some(1.0),
                    _ => css::parse_f32(vl.split(' ').next().unwrap_or("")),
                };
                match g.filter(|g| *g >= 0.0) {
                    Some(g) => {
                        r.flex = g;
                        r
                    }
                    None => self.invalid(e, d),
                }
            }
            "justify-content" => match vl.as_str() {
                "flex-start" | "start" => {
                    r.justify = Justify::Start;
                    r
                }
                "center" => {
                    r.justify = Justify::Center;
                    r
                }
                "flex-end" | "end" => {
                    r.justify = Justify::End;
                    r
                }
                "space-between" => {
                    r.justify = Justify::Between;
                    r
                }
                "space-around" => {
                    r.justify = Justify::Around;
                    r
                }
                "space-evenly" => {
                    r.justify = Justify::Evenly;
                    r
                }
                _ => {
                    let msg = format!(
                        "CSS: 'justify-content: {}' is not supported (use flex-start, center, flex-end, space-between, space-around or space-evenly)",
                        vl
                    );
                    self.unsupported(e, d, msg)
                }
            },
            "position" => match vl.as_str() {
                "static" | "relative" => {
                    r.position = Position::Static;
                    r
                }
                "absolute" => {
                    r.position = Position::Absolute;
                    r
                }
                _ => {
                    let msg = format!("CSS: 'position: {}' is not supported in v1 (use static, relative or absolute)", vl);
                    self.unsupported(e, d, msg)
                }
            },
            "top" | "right" | "bottom" | "left" => match offset_of(&v) {
                Some(o) => {
                    match d.name.as_str() {
                        "top" => r.top = o,
                        "right" => r.right = o,
                        "bottom" => r.bottom = o,
                        _ => r.left = o,
                    }
                    r
                }
                None => self.invalid(e, d),
            },
            "z-index" => {
                if vl == "auto" {
                    r.z_index = 0;
                    r
                } else if let Ok(n) = vl.parse::<i32>() {
                    r.z_index = n.clamp(-1000, 1000);
                    r
                } else {
                    self.invalid(e, d)
                }
            }
            "background-size" => match vl.as_str() {
                "cover" => {
                    r.background_size = ImageFit::Cover;
                    r
                }
                "contain" => {
                    r.background_size = ImageFit::Contain;
                    r
                }
                "100% 100%" | "fill" => {
                    r.background_size = ImageFit::Fill;
                    r
                }
                _ => {
                    let msg = format!(
                        "CSS: 'background-size: {}' is not supported in v1 (use cover, contain or 100% 100%)",
                        vl
                    );
                    self.unsupported(e, d, msg)
                }
            },
            "background-position" => match vl.as_str() {
                "center" | "center center" => {
                    r.background_position = ImagePos::Center;
                    r
                }
                "top" | "top center" | "center top" => {
                    r.background_position = ImagePos::Top;
                    r
                }
                "bottom" | "bottom center" | "center bottom" => {
                    r.background_position = ImagePos::Bottom;
                    r
                }
                "left" | "left center" | "center left" => {
                    r.background_position = ImagePos::Left;
                    r
                }
                "right" | "right center" | "center right" => {
                    r.background_position = ImagePos::Right;
                    r
                }
                _ => {
                    let msg = format!(
                        "CSS: 'background-position: {}' is not supported in v1 (use center, top, bottom, left or right)",
                        vl
                    );
                    self.unsupported(e, d, msg)
                }
            },
            "object-fit" => match vl.as_str() {
                "cover" => {
                    r.object_fit = ImageFit::Cover;
                    r
                }
                "contain" | "scale-down" => {
                    r.object_fit = ImageFit::Contain;
                    r
                }
                "fill" => {
                    r.object_fit = ImageFit::Fill;
                    r
                }
                _ => {
                    let msg = format!("CSS: 'object-fit: {}' is not supported in v1 (use cover, contain or fill)", vl);
                    self.unsupported(e, d, msg)
                }
            },
            "dock" => match vl.as_str() {
                "bottom" => {
                    r.dock = Some(Dock::Bottom);
                    r
                }
                "top" => {
                    r.dock = Some(Dock::Top);
                    r
                }
                "left" => {
                    r.dock = Some(Dock::Left);
                    r
                }
                "right" => {
                    r.dock = Some(Dock::Right);
                    r
                }
                "center" => {
                    r.dock = Some(Dock::Center);
                    r
                }
                _ => self.invalid(e, d),
            },
            "align-items" => match vl.as_str() {
                "flex-start" | "start" => {
                    r.align_items = Align::Start;
                    r
                }
                "center" => {
                    r.align_items = Align::Center;
                    r
                }
                "flex-end" | "end" => {
                    r.align_items = Align::End;
                    r
                }
                "stretch" => {
                    r.align_items = Align::Stretch;
                    r
                }
                _ => {
                    let msg = format!(
                        "CSS: 'align-items: {}' is not supported in v1 (use flex-start, center or flex-end)",
                        vl
                    );
                    self.unsupported(e, d, msg)
                }
            },
            "gap" => match len {
                Some(l) => {
                    r.gap = l;
                    r
                }
                None => self.invalid(e, d),
            },
            "width" => match css::size(&v) {
                Some(s) => {
                    r.width = s;
                    r
                }
                None => self.invalid(e, d),
            },
            "height" => match css::size(&v) {
                Some(s) => {
                    r.height = s;
                    r
                }
                None => self.invalid(e, d),
            },
            "animation" => match animation_of(&v) {
                Some(a) => {
                    r.animation = Some(a);
                    r
                }
                None => {
                    let msg = format!(
                        "CSS: invalid value in 'animation: {}' (use none, fade, slide-up or slide-left, then an optional time like 200ms), ignored",
                        d.value
                    );
                    self.unsupported(e, d, msg)
                }
            },
            _ => {
                let msg = format!("CSS: property '{}' is not supported in v1 and was ignored", d.name);
                self.unsupported(e, d, msg)
            }
        }
    }
}

impl LayoutBuilder {
    fn collect_inline(&mut self, el: &HtmlElement, parent_style: &NativeStyle, run: &mut Vec<Span>) -> Res<()> {
        let style = self.compute_style(el, parent_style)?;
        for f in &el.children {
            match f {
                HtmlNode::Text(t) => run.push(make_span(&t.text, &style)),
                HtmlNode::Element(c) => self.collect_inline(c, &style, run)?,
            }
        }
        Ok(())
    }

    fn text_of(&mut self, el: &HtmlElement, style: &NativeStyle) -> Res<String> {
        let mut run: Vec<Span> = Vec::new();
        for f in &el.children {
            match f {
                HtmlNode::Text(t) => run.push(make_span(&t.text, style)),
                HtmlNode::Element(c) => {
                    if is_inline(c) {
                        self.collect_inline(c, style, &mut run)?;
                    } else {
                        let msg = format!("<{}> can only contain text", el.tag);
                        let word = format!("<{}", c.tag);
                        return Err(self.err(&msg, c.line, Some(word.as_str()), None));
                    }
                }
            }
        }
        Ok(normalize(&run).iter().map(|s| s.text.as_str()).collect())
    }

    fn convert_children(&mut self, nodes: &[HtmlNode], parent_style: &NativeStyle, owner: &str) -> Res<Vec<Native>> {
        let saved_parent = self.parent_tag.take();
        self.parent_tag = Some(owner.to_string());
        self.depth += 1;
        let r = self.convert_children_of(nodes, parent_style);
        self.depth -= 1;
        self.parent_tag = saved_parent;
        r
    }

    fn convert_children_of(&mut self, nodes: &[HtmlNode], parent_style: &NativeStyle) -> Res<Vec<Native>> {
        let mut result: Vec<Native> = Vec::new();
        let mut run: Vec<Span> = Vec::new();
        let mut run_line: Option<usize> = None;

        for n in nodes {
            match n {
                HtmlNode::Text(t) => {
                    if run_line.is_none() {
                        run_line = Some(t.line);
                    }
                    run.push(make_span(&t.text, parent_style));
                }
                HtmlNode::Element(el) => {
                    if is_inline(el) {
                        if run_line.is_none() {
                            run_line = Some(el.line);
                        }
                        self.collect_inline(el, parent_style, &mut run)?;
                    } else {
                        flush(&mut result, &mut run, &mut run_line, parent_style);
                        let c = self.convert_element(el, parent_style)?;
                        result.push(c);
                    }
                }
            }
        }
        flush(&mut result, &mut run, &mut run_line, parent_style);
        Ok(result)
    }

    fn register_id(&mut self, el: &HtmlElement) -> Res<Option<String>> {
        let id = match el.id() {
            Some(i) => i,
            None => return Ok(None),
        };
        if self.reserved_ids.contains(&id) {
            return Err(self.err(&format!("duplicate id '{}'", id), el.line, Some(id.as_str()), None));
        }
        self.reserved_ids.insert(id.clone());
        Ok(Some(id))
    }

    fn convert_element(&mut self, el: &HtmlElement, parent_style: &NativeStyle) -> Res<Native> {
        let tag = el.tag.as_str();
        if !SUPPORTED_TAGS.contains(&tag) {
            let word = format!("<{}", tag);
            return Err(self.err(
                &format!("tag '{}' is not supported", tag),
                el.line,
                Some(word.as_str()),
                Some(TAGS_SUGGESTION),
            ));
        }
        let my_depth = self.depth;
        let my_parent = self.parent_tag.clone();
        let computed = self.compute_style(el, parent_style)?;
        let positioned = self.fix_position(el, computed, my_depth);
        let style = self.scrollable(el, positioned, my_depth);
        let id = self.register_id(el)?;
        let classes = el.classes();
        let disabled = el.has_attr("disabled") || self.group_disabled;

        let mut node: Native = match tag {
            "button" => {
                let label = self.text_of(el, &style)?;
                Native::new(id, classes, style.clone(), el.line, NativeKind::Button { label, disabled })
            }
            "input" => self.convert_input(el, &style, id, classes, disabled)?,
            "textarea" => {
                let mut content = String::new();
                for c in &el.children {
                    if let HtmlNode::Text(t) = c {
                        content.push_str(&t.text);
                    }
                }
                let content = match content.strip_prefix('\n') {
                    Some(c) => c.to_string(),
                    None => content,
                };
                let rows = el
                    .attr("rows")
                    .and_then(|r| r.trim().parse::<i32>().ok())
                    .map(|r| r.max(1))
                    .unwrap_or(3);
                Native::new(
                    id,
                    classes,
                    style.clone(),
                    el.line,
                    NativeKind::Field {
                        field_type: FieldType::Area,
                        hint: el.attr("placeholder").unwrap_or("").to_string(),
                        value: content,
                        disabled,
                        read_only: el.has_attr("readonly"),
                        rows,
                    },
                )
            }
            "img" => {
                let src = match el.attr("src").map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    Some(s) => s.to_string(),
                    None => return Err(self.err("<img> requires a src attribute", el.line, Some("<img"), None)),
                };
                if let Some(icon) = icons::name_of(&src) {
                    if icons::shapes_of(&icon).is_none() {
                        let suggestion = format!("icons: {}", icons::names().join(", "));
                        return Err(self.err(
                            &format!("icon '{}' does not exist", icon),
                            el.line,
                            Some(src.as_str()),
                            Some(suggestion.as_str()),
                        ));
                    }
                }
                Native::new(
                    id,
                    classes,
                    style.clone(),
                    el.line,
                    NativeKind::Image {
                        source: src,
                        description: el.attr("alt").unwrap_or("").to_string(),
                    },
                )
            }
            "progress" => {
                let max = match el.attr("max").map(|t| t.trim()) {
                    None => 1.0,
                    Some(t) => match css::parse_f32(t).filter(|v| *v > 0.0) {
                        Some(v) => v,
                        None => {
                            return Err(self.err(
                                "<progress> 'max' must be a number greater than 0",
                                el.line,
                                Some("max"),
                                None,
                            ));
                        }
                    },
                };
                let value = match el.attr("value").map(|t| t.trim()) {
                    None => None,
                    Some(t) => match css::parse_f32(t) {
                        Some(v) => Some(v),
                        None => {
                            return Err(self.err(
                                "<progress> 'value' must be a number",
                                el.line,
                                Some("value"),
                                Some("leave 'value' out for an indeterminate loading"),
                            ));
                        }
                    },
                };
                Native::new(
                    id,
                    classes,
                    style.clone(),
                    el.line,
                    NativeKind::Progress { value, max, circular: el.has_attr("circular") },
                )
            }
            "ul" => self.convert_list(el, &style, id, classes)?,
            "li" => return Err(self.err("<li> can only be inside <ul>", el.line, Some("<li"), None)),
            _ => {
                if tag == "legend" && my_parent.as_deref() != Some("fieldset") {
                    return Err(self.err("<legend> can only be inside <fieldset>", el.line, Some("<legend"), None));
                }
                if tag == "dialog" && my_depth != 1 {
                    return Err(self.err(
                        "<dialog> must be a direct child of the root tag",
                        el.line,
                        Some("<dialog"),
                        Some("put the <dialog> at the first level of the screen, next to the other blocks"),
                    ));
                }
                if tag == "header" && my_depth != 1 {
                    return Err(self.err(
                        "<header> must be a direct child of the root tag",
                        el.line,
                        Some("<header"),
                        Some("put the <header> at the first level of the screen; it stays fixed at the top"),
                    ));
                }
                if tag == "footer" && my_depth != 1 {
                    return Err(self.err(
                        "<footer> must be a direct child of the root tag",
                        el.line,
                        Some("<footer"),
                        Some("put the <footer> at the first level of the screen; it stays fixed outside the scroll"),
                    ));
                }
                let saved_group = self.group_disabled;
                if tag == "fieldset" && el.has_attr("disabled") {
                    self.group_disabled = true;
                }
                let found = self.convert_children(&el.children, &style, tag);
                self.group_disabled = saved_group;
                let found = found?;

                // the <header> is fixed at the top and comes first; the dialogs of the screen are layers:
                // they come last, in the order they were written
                let is_special = |n: &Native| match &n.kind {
                    NativeKind::Box(b) => b.header || b.footer || b.overlay,
                    _ => false,
                };
                let children: Vec<Native> = if my_depth == 0 && found.iter().any(|n| is_special(n)) {
                    let mut headers: Vec<Native> = Vec::new();
                    let mut others: Vec<Native> = Vec::new();
                    let mut footers: Vec<Native> = Vec::new();
                    let mut overlays: Vec<Native> = Vec::new();
                    for n in found {
                        let class = match &n.kind {
                            NativeKind::Box(b) if b.header => 1,
                            NativeKind::Box(b) if b.footer => 2,
                            NativeKind::Box(b) if b.overlay => 3,
                            _ => 0,
                        };
                        match class {
                            1 => headers.push(n),
                            2 => footers.push(n),
                            3 => overlays.push(n),
                            _ => others.push(n),
                        }
                    }
                    if headers.len() > 1 {
                        return Err(self.err(
                            "a screen can have only one <header>",
                            headers[1].line,
                            Some("<header"),
                            None,
                        ));
                    }
                    if footers.len() > 1 {
                        return Err(self.err(
                            "a screen can have only one <footer>",
                            footers[1].line,
                            Some("<footer"),
                            None,
                        ));
                    }
                    let mut v = headers;
                    v.extend(others);
                    v.extend(footers);
                    v.extend(overlays);
                    v
                } else {
                    found
                };

                let is_virtual = el.has_attr("virtual");
                if is_virtual {
                    for c in &children {
                        if let NativeKind::Box(b) = &c.kind {
                            if b.overlay {
                                return Err(self.err(
                                    "<dialog> cannot be inside a 'virtual' container",
                                    c.line,
                                    Some("<dialog"),
                                    None,
                                ));
                            }
                        }
                        if c.style.position == Position::Absolute {
                            return Err(self.err(
                                "'position: absolute' cannot be used inside a 'virtual' container",
                                c.line,
                                None,
                                None,
                            ));
                        }
                    }
                }
                let is_group = tag == "dialog" || tag == "fieldset" || tag == "header" || tag == "footer";
                let only_spans: Option<Vec<Span>> = if children.len() == 1 {
                    match &children[0].kind {
                        NativeKind::Text { spans } if children[0].style.position != Position::Absolute => {
                            Some(spans.clone())
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                match only_spans {
                    Some(spans) if !is_virtual && !is_group => {
                        Native::new(id, classes, style.clone(), el.line, NativeKind::Text { spans })
                    }
                    _ => {
                        let has_absolute =
                            !is_virtual && children.iter().any(|c| c.style.position == Position::Absolute);
                        let mut box_style = style.clone();
                        if style.scroll_y && (is_virtual || has_absolute) {
                            self.warn(
                                "CSS: 'overflow-y: auto|scroll' cannot be used on a 'virtual' box or on a box with 'position: absolute' children, ignored",
                                el.line,
                            );
                            box_style.scroll_y = false;
                            box_style.overflow_hidden = false;
                        }
                        Native::new(
                            id,
                            classes,
                            box_style,
                            el.line,
                            NativeKind::Box(NativeBox {
                                children,
                                is_virtual,
                                overlay: tag == "dialog",
                                header: tag == "header",
                                footer: tag == "footer",
                                has_absolute,
                            }),
                        )
                    }
                }
            }
        };
        node.key_hint = key_of(el);
        node.scope = scope_of(el);
        if let Some(i) = &node.id {
            self.ids.insert(i.clone(), node.clone());
        }
        Ok(node)
    }

    fn convert_input(
        &mut self,
        el: &HtmlElement,
        style: &NativeStyle,
        id: Option<String>,
        classes: Vec<String>,
        disabled: bool,
    ) -> Res<Native> {
        let kind = el.attr("type").unwrap_or("text").trim().to_lowercase();
        match kind.as_str() {
            "checkbox" => Ok(Native::new(
                id,
                classes,
                style.clone(),
                el.line,
                NativeKind::Checkbox { checked: el.has_attr("checked"), disabled },
            )),
            "text" | "password" | "number" => {
                let field_type = match kind.as_str() {
                    "password" => FieldType::Password,
                    "number" => FieldType::Number,
                    _ => FieldType::Text,
                };
                Ok(Native::new(
                    id,
                    classes,
                    style.clone(),
                    el.line,
                    NativeKind::Field {
                        field_type,
                        hint: el.attr("placeholder").unwrap_or("").to_string(),
                        value: el.attr("value").unwrap_or("").to_string(),
                        disabled,
                        read_only: el.has_attr("readonly"),
                        rows: 1,
                    },
                ))
            }
            _ => Err(self.err(
                &format!("<input type=\"{}\"> is not supported", kind),
                el.line,
                Some("type"),
                Some("supported types: text, password, number, checkbox"),
            )),
        }
    }

    fn convert_list(
        &mut self,
        el: &HtmlElement,
        style: &NativeStyle,
        id: Option<String>,
        classes: Vec<String>,
    ) -> Res<Native> {
        let mut items: Vec<Native> = Vec::new();
        for f in &el.children {
            match f {
                HtmlNode::Text(t) => {
                    if !t.text.trim().is_empty() {
                        return Err(self.err("<ul> can only contain <li>", t.line, None, None));
                    }
                }
                HtmlNode::Element(li) => {
                    if li.tag != "li" {
                        let word = format!("<{}", li.tag);
                        return Err(self.err(
                            &format!("<ul> can only contain <li>, found <{}>", li.tag),
                            li.line,
                            Some(word.as_str()),
                            None,
                        ));
                    }
                    let computed = self.compute_style(li, style)?;
                    let li_style = self.fix_position(li, computed, 2);
                    let li_id = self.register_id(li)?;
                    let children = self.convert_children(&li.children, &li_style, "li")?;
                    let mut item = Native::new(
                        li_id,
                        li.classes(),
                        li_style,
                        li.line,
                        NativeKind::Item { marker: "\u{2022}".to_string(), children },
                    );
                    item.key_hint = key_of(li);
                    item.scope = scope_of(li);
                    if let Some(i) = &item.id {
                        self.ids.insert(i.clone(), item.clone());
                    }
                    items.push(item);
                }
            }
        }
        Ok(Native::new(id, classes, style.clone(), el.line, NativeKind::List { items }))
    }
}
