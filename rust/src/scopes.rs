//! Port of Scopes.kt: identity of the repetitions of a `for` (section 5.7).
//!
//! An element with an `id` inside a `for` is written by the Expander as `id~path`, where the path
//! names the repetitions that contain it, from the outermost to the innermost: `buy~p:7` is the
//! element `buy` of the item 7 of `for p in ...`, and `buy~p:7/o:2` is inside a second `for o`.
//! The program never writes this form: it asks for `buy` and gives the item.

use crate::value::Value;

/// Separates the id written in the file from the path of the repetition.
pub const SEP: &str = "~";

const KEY_FIELDS: [&str; 2] = ["key", "id"];
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// The automatic key of an item: its field `key`, or else its field `id`.
/// Only text and whole numbers count. Anything else gives `None` and the position is used.
pub fn key_of_item(item: &Value) -> Option<String> {
    if !matches!(item, Value::Map(_)) {
        return None;
    }
    for field in KEY_FIELDS {
        match item.get(field) {
            Some(Value::Str(v)) => {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
            Some(Value::Int(i)) => return Some(i.to_string()),
            _ => {}
        }
    }
    None
}

/// Letters, digits, `_`, `-` and `.` stay; every other byte becomes `%XX`, so `/`, `:` and `~` never appear.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &c in s.as_bytes() {
        let plain = c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.';
        if plain {
            out.push(c as char);
        } else {
            out.push('%');
            out.push(HEX[(c >> 4) as usize] as char);
            out.push(HEX[(c & 15) as usize] as char);
        }
    }
    out
}

/// The token of a repetition: its key, or `%p` and the position when there is no key
/// (`%p` never comes out of `encode`).
pub fn token(key: Option<&str>, index: usize) -> String {
    match key {
        Some(k) => encode(k),
        None => format!("%p{}", index),
    }
}

/// The name of the `for` variable of one segment of a path (`p:7` gives `p`).
pub fn variable_of(segment: &str) -> &str {
    segment.split(':').next().unwrap_or(segment)
}

/// The token of one segment of a path (`p:7` gives `7`).
pub fn token_of(segment: &str) -> &str {
    match segment.split_once(':') {
        Some((_, t)) => t,
        None => segment,
    }
}

/// True when `given`, the item the program holds, is the repetition `held` that the Expander
/// recorded: an equal one, or one that has the same key.
pub fn same_item(given: &Value, held: &Value, token: &str) -> bool {
    if given == held {
        return true;
    }
    match key_of_item(given) {
        Some(key) => encode(&key) == token,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_keeps_plain_and_escapes_the_rest() {
        assert_eq!(encode("a-b_c.d9"), "a-b_c.d9");
        assert_eq!(encode("a b/c:d~e"), "a%20b%2Fc%3Ad%7Ee");
        assert_eq!(encode("\u{fc}"), "%C3%BC");
    }

    #[test]
    fn tokens_and_segments() {
        assert_eq!(token(Some("x y"), 3), "x%20y");
        assert_eq!(token(None, 3), "%p3");
        assert_eq!(variable_of("p:7"), "p");
        assert_eq!(token_of("p:7"), "7");
        assert_eq!(variable_of("p"), "p");
        assert_eq!(token_of("p"), "p");
    }

    #[test]
    fn key_of_item_uses_key_then_id() {
        let a = Value::map([("key", "  k1 ".into()), ("id", 5.into())]);
        assert_eq!(key_of_item(&a), Some("k1".to_string()));
        let b = Value::map([("id", 7.into())]);
        assert_eq!(key_of_item(&b), Some("7".to_string()));
        let c = Value::map([("key", "  ".into()), ("id", "z".into())]);
        assert_eq!(key_of_item(&c), Some("z".to_string()));
        let d = Value::map([("id", 1.5.into())]);
        assert_eq!(key_of_item(&d), None);
        assert_eq!(key_of_item(&Value::Str("x".into())), None);
    }

    #[test]
    fn same_item_by_equality_or_key() {
        let held = Value::map([("id", 7.into()), ("n", "a".into())]);
        let given = Value::map([("id", 7.into()), ("n", "changed".into())]);
        assert!(same_item(&held, &held, "x"));
        assert!(same_item(&given, &held, "7"));
        assert!(!same_item(&given, &held, "8"));
        assert!(!same_item(&Value::Int(1), &Value::Int(2), "1"));
    }
}
