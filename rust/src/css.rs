//! Port of Css.kt: CSS declaration parser (`css::declarations`) and value readers
//! (`color`, `length`, `fraction`, `size`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::tree::{Length, Size};

#[derive(Debug, Clone, PartialEq)]
pub struct CssDeclaration {
    pub name: String,
    pub value: String,
    pub line: usize,
}

/// Splits `text` on `sep`, ignoring separators inside parentheses or quotes.
pub fn split_top_level(text: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth: u32 = 0;
    let mut quote: Option<char> = None;
    for c in text.chars() {
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
            current.push(c);
        } else if c == '(' {
            depth += 1;
            current.push(c);
        } else if c == ')' {
            if depth > 0 {
                depth -= 1;
            }
            current.push(c);
        } else if c == sep && depth == 0 {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts
}

struct Parsed {
    items: Vec<(String, String)>,
    warnings: Vec<String>,
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Rc<Parsed>>> = RefCell::new(HashMap::new());
}

/// Removes one trailing `!important` (any case) and the spaces before it.
fn strip_important(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if lower.ends_with("!important") {
        value[..value.len() - "!important".len()].trim_end().to_string()
    } else {
        value.to_string()
    }
}

fn parse(text: &str) -> Parsed {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for part in split_top_level(text, ';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let colon = match p.find(':') {
            Some(i) if i > 0 => i,
            _ => {
                warnings.push(format!("CSS: declaration without ':' was ignored ('{}')", p));
                continue;
            }
        };
        let name = p[..colon].trim().to_lowercase();
        let value = strip_important(p[colon + 1..].trim());
        if value.is_empty() {
            warnings.push(format!("CSS: property '{}' has no value and was ignored", name));
            continue;
        }
        items.push((name, value));
    }
    Parsed { items, warnings }
}

/// Reads `name: value; ...`. Each problem is reported through `warn(message, line)`.
pub fn declarations<F: FnMut(&str, usize)>(text: &str, line: usize, mut warn: F) -> Vec<CssDeclaration> {
    let cached = CACHE.with(|c| c.borrow().get(text).cloned());
    let parsed = match cached {
        Some(p) => p,
        None => {
            let p = Rc::new(parse(text));
            CACHE.with(|c| {
                let mut map = c.borrow_mut();
                if map.len() > 512 {
                    map.clear();
                }
                map.insert(text.to_string(), Rc::clone(&p));
            });
            p
        }
    };
    for w in &parsed.warnings {
        warn(w, line);
    }
    parsed
        .items
        .iter()
        .map(|(n, v)| CssDeclaration { name: n.clone(), value: v.clone(), line })
        .collect()
}

// ---------- values ----------

const OPAQUE: u32 = 0xFF00_0000;

fn named_color(name: &str) -> Option<u32> {
    Some(match name {
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "red" => 0xFF0000,
        "green" => 0x008000,
        "blue" => 0x0000FF,
        "yellow" => 0xFFFF00,
        "orange" => 0xFFA500,
        "purple" => 0x800080,
        "pink" => 0xFFC0CB,
        "gray" | "grey" => 0x808080,
        "silver" => 0xC0C0C0,
        "maroon" => 0x800000,
        "navy" => 0x000080,
        "teal" => 0x008080,
        "lime" => 0x00FF00,
        "aqua" | "cyan" => 0x00FFFF,
        "magenta" | "fuchsia" => 0xFF00FF,
        "olive" => 0x808000,
        "brown" => 0xA52A2A,
        "gold" => 0xFFD700,
        "indigo" => 0x4B0082,
        "violet" => 0xEE82EE,
        "coral" => 0xFF7F50,
        "crimson" => 0xDC143C,
        "tomato" => 0xFF6347,
        "salmon" => 0xFA8072,
        "khaki" => 0xF0E68C,
        "beige" => 0xF5F5DC,
        "ivory" => 0xFFFFF0,
        "lavender" => 0xE6E6FA,
        "turquoise" => 0x40E0D0,
        "darkgray" | "darkgrey" => 0xA9A9A9,
        "lightgray" | "lightgrey" => 0xD3D3D3,
        "darkblue" => 0x00008B,
        "darkgreen" => 0x006400,
        "darkred" => 0x8B0000,
        "lightblue" => 0xADD8E6,
        "lightgreen" => 0x90EE90,
        "whitesmoke" => 0xF5F5F5,
        _ => return None,
    })
}

fn pack(r: u32, g: u32, b: u32, a: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

fn hex(b: u8) -> u32 {
    (b as char).to_digit(16).unwrap_or(0)
}

/// Kotlin `Float.roundToInt`: ties go toward positive infinity.
pub(crate) fn round_to_int(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

/// Like Kotlin `toFloatOrNull`: "nan" and "inf" are not numbers here.
pub(crate) fn parse_f32(s: &str) -> Option<f32> {
    s.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// A color as 0xAARRGGBB, or `None` if the text is not a color.
pub fn color(text: &str) -> Option<u32> {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    if t == "transparent" {
        return Some(0);
    }
    if let Some(rgb) = named_color(&t) {
        return Some(rgb | OPAQUE);
    }

    if let Some(h) = t.strip_prefix('#') {
        if h.is_empty() || !h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        let b = h.as_bytes();
        let dbl = |p: usize| hex(b[p]) * 17;
        let pair = |p: usize| hex(b[p]) * 16 + hex(b[p + 1]);
        return match h.len() {
            3 => Some(pack(dbl(0), dbl(1), dbl(2), 255)),
            4 => Some(pack(dbl(0), dbl(1), dbl(2), dbl(3))),
            6 => Some(pack(pair(0), pair(2), pair(4), 255)),
            8 => Some(pack(pair(0), pair(2), pair(4), pair(6))),
            _ => None,
        };
    }

    let inner = t.strip_prefix("rgba(").or_else(|| t.strip_prefix("rgb("))?;
    let inner = inner.strip_suffix(')')?;
    if inner.contains(')') {
        return None;
    }
    let parts: Vec<&str> = inner
        .split(|c: char| c == ',' || c == ' ' || c == '/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }
    let channel = |s: &str| -> Option<i32> {
        match s.strip_suffix('%') {
            Some(n) => parse_f32(n).map(|v| round_to_int(v * 2.55)),
            None => parse_f32(s).map(round_to_int),
        }
    };
    let r = channel(parts[0])?;
    let g = channel(parts[1])?;
    let b = channel(parts[2])?;
    let mut a: i32 = 255;
    if parts.len() == 4 {
        let s = parts[3];
        let v = match s.strip_suffix('%') {
            Some(n) => parse_f32(n).map(|x| x / 100.0)?,
            None => parse_f32(s)?,
        };
        a = round_to_int(v.clamp(0.0, 1.0) * 255.0);
    }
    Some(pack(
        r.clamp(0, 255) as u32,
        g.clamp(0, 255) as u32,
        b.clamp(0, 255) as u32,
        a as u32,
    ))
}

/// Matches `-?\d*\.?\d+`.
pub(crate) fn is_number(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    let digits = |x: &str| x.bytes().all(|b| b.is_ascii_digit());
    match body.split_once('.') {
        None => !body.is_empty() && digits(body),
        Some((a, b)) => digits(a) && !b.is_empty() && digits(b),
    }
}

/// `12px`, `1.5rem` or `0`.
pub fn length(text: &str) -> Option<Length> {
    let t = text.trim().to_lowercase();
    if t == "0" {
        return Some(Length::ZERO);
    }
    let (num, is_rem) = if let Some(n) = t.strip_suffix("rem") {
        (n, true)
    } else if let Some(n) = t.strip_suffix("px") {
        (n, false)
    } else {
        return None;
    };
    if !is_number(num) {
        return None;
    }
    let n = parse_f32(num)?;
    Some(if is_rem { Length::rem(n) } else { Length::px(n) })
}

/// A number from 0 to 1, or a percentage (opacity).
pub fn fraction(text: &str) -> Option<f32> {
    let t = text.trim().to_lowercase();
    let v = match t.strip_suffix('%') {
        Some(n) => parse_f32(n).map(|x| x / 100.0),
        None => parse_f32(&t),
    };
    v.map(|x| x.clamp(0.0, 1.0))
}

pub fn size(text: &str) -> Option<Size> {
    let t = text.trim().to_lowercase();
    if t == "auto" {
        return Some(Size::Auto);
    }
    if let Some(n) = t.strip_suffix('%') {
        return parse_f32(n).map(Size::Percent);
    }
    length(&t).map(Size::Dim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_respects_parens_and_quotes() {
        assert_eq!(
            split_top_level("a;b(c;d);\"x;y\";e", ';'),
            vec!["a", "b(c;d)", "\"x;y\"", "e"]
        );
    }

    #[test]
    fn declarations_and_warnings() {
        let mut warns: Vec<(String, usize)> = Vec::new();
        let d = declarations(
            "color: red; Width:10px !IMPORTANT; foo; bar:;",
            7,
            |m, l| warns.push((m.to_string(), l)),
        );
        assert_eq!(
            d,
            vec![
                CssDeclaration { name: "color".into(), value: "red".into(), line: 7 },
                CssDeclaration { name: "width".into(), value: "10px".into(), line: 7 },
            ]
        );
        assert_eq!(
            warns,
            vec![
                ("CSS: declaration without ':' was ignored ('foo')".to_string(), 7),
                ("CSS: property 'bar' has no value and was ignored".to_string(), 7),
            ]
        );
    }

    #[test]
    fn declarations_cache_keeps_warning_behavior() {
        for _ in 0..2 {
            let mut n = 0;
            let d = declarations("x;color:blue", 1, |_, _| n += 1);
            assert_eq!(d.len(), 1);
            assert_eq!(n, 1);
        }
    }

    #[test]
    fn colors() {
        assert_eq!(color("#fff"), Some(0xFFFF_FFFF));
        assert_eq!(color("#f008"), Some(0x88FF_0000));
        assert_eq!(color("#2F81F7"), Some(0xFF2F_81F7));
        assert_eq!(color("#11223344"), Some(0x4411_2233));
        assert_eq!(color("red"), Some(0xFFFF_0000));
        assert_eq!(color(" Transparent "), Some(0));
        assert_eq!(color("rgb(255, 0, 0)"), Some(0xFFFF_0000));
        assert_eq!(color("rgb(100%, 0%, 0%)"), Some(0xFFFF_0000));
        assert_eq!(color("rgba(0,0,0,0.5)"), Some(0x8000_0000));
        assert_eq!(color("rgb(300 0 -5)"), Some(0xFFFF_0000));
        assert_eq!(color("rgb(0 0 0 / 100%)"), Some(0xFF00_0000));
        assert_eq!(color("nope"), None);
        assert_eq!(color("#12"), None);
        assert_eq!(color("#ggg"), None);
        assert_eq!(color("rgb(1,2)"), None);
        assert_eq!(color(""), None);
    }

    #[test]
    fn lengths() {
        assert_eq!(length("12px"), Some(Length::px(12.0)));
        assert_eq!(length("1.5rem"), Some(Length::rem(1.5)));
        assert_eq!(length("-4px"), Some(Length::px(-4.0)));
        assert_eq!(length(".5rem"), Some(Length::rem(0.5)));
        assert_eq!(length("0"), Some(Length::ZERO));
        assert_eq!(length("12"), None);
        assert_eq!(length("px"), None);
        assert_eq!(length("1.2.3px"), None);
        assert_eq!(length("5.px"), None);
    }

    #[test]
    fn fractions() {
        assert_eq!(fraction("50%"), Some(0.5));
        assert_eq!(fraction("2"), Some(1.0));
        assert_eq!(fraction("-1"), Some(0.0));
        assert_eq!(fraction("0.25"), Some(0.25));
        assert_eq!(fraction("x"), None);
    }

    #[test]
    fn sizes() {
        assert_eq!(size("auto"), Some(Size::Auto));
        assert_eq!(size("50%"), Some(Size::Percent(50.0)));
        assert_eq!(size("10px"), Some(Size::Dim(Length::px(10.0))));
        assert_eq!(size("abc"), None);
    }
}
