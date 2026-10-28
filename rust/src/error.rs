//! Port of NativeLayoutError.kt, and of its two subclasses `ExpandError` (Expander.kt) and
//! `MarkupError` (HtmlParser.kt). Rust has no inheritance, so one struct carries a `kind`.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// `NativeLayoutError` itself.
    Layout,
    /// `ExpandError`.
    Expand,
    /// `MarkupError`.
    Markup,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeLayoutError {
    pub kind: ErrorKind,
    pub message: String,
    pub line: usize,
    pub context: String,
    pub word: Option<String>,
    pub suggestion: Option<String>,
}

impl NativeLayoutError {
    pub fn new(message: &str, line: usize, context: &str) -> NativeLayoutError {
        NativeLayoutError {
            kind: ErrorKind::Layout,
            message: message.to_string(),
            line,
            context: context.to_string(),
            word: None,
            suggestion: None,
        }
    }

    /// `ExpandError(message, line, source, word, suggestion)`: the context is the line of `source`.
    pub fn expand(
        message: &str,
        line: usize,
        source: &str,
        word: Option<&str>,
        suggestion: Option<&str>,
    ) -> NativeLayoutError {
        NativeLayoutError {
            kind: ErrorKind::Expand,
            message: message.to_string(),
            line,
            context: extract_line(source, line),
            word: word.map(str::to_string),
            suggestion: suggestion.map(str::to_string),
        }
    }

    /// `MarkupError(message, line, source, word, suggestion)`: the message starts with `markup: `.
    pub fn markup(
        message: &str,
        line: usize,
        source: &str,
        word: Option<&str>,
        suggestion: Option<&str>,
    ) -> NativeLayoutError {
        NativeLayoutError {
            kind: ErrorKind::Markup,
            message: format!("markup: {}", message),
            line,
            context: extract_line(source, line),
            word: word.map(str::to_string),
            suggestion: suggestion.map(str::to_string),
        }
    }

    pub fn with_word(mut self, word: &str) -> NativeLayoutError {
        self.word = Some(word.to_string());
        self
    }

    pub fn with_suggestion(mut self, suggestion: &str) -> NativeLayoutError {
        self.suggestion = Some(suggestion.to_string());
        self
    }

    pub fn format(&self) -> String {
        let mut out = format!("Error: line {}\n{}\n\n{}", self.line, self.context, self.message);
        if let Some(s) = &self.suggestion {
            out.push_str(&format!("\n\nSuggestion: {}", s));
        }
        out
    }
}

impl fmt::Display for NativeLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NativeLayoutError {}

/// The trimmed text of line `line_number` (1-based) of `source`, or an empty string if there is none.
pub fn extract_line(source: &str, line_number: usize) -> String {
    if line_number == 0 {
        return String::new();
    }
    match source.split('\n').nth(line_number - 1) {
        Some(l) => l.trim().to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_line_is_one_based_and_trimmed() {
        let src = "a\n  b  \nc";
        assert_eq!(extract_line(src, 1), "a");
        assert_eq!(extract_line(src, 2), "b");
        assert_eq!(extract_line(src, 3), "c");
        assert_eq!(extract_line(src, 4), "");
        assert_eq!(extract_line(src, 0), "");
    }

    #[test]
    fn format_without_and_with_suggestion() {
        let e = NativeLayoutError::new("bad", 3, "<p>");
        assert_eq!(e.format(), "Error: line 3\n<p>\n\nbad");
        let e = e.with_suggestion("close it");
        assert_eq!(e.format(), "Error: line 3\n<p>\n\nbad\n\nSuggestion: close it");
        assert_eq!(e.to_string(), "bad");
    }

    #[test]
    fn markup_and_expand_errors() {
        let src = "<div>\n  <p>\n</div>";
        let m = NativeLayoutError::markup("unclosed tag", 2, src, Some("p"), None);
        assert_eq!(m.kind, ErrorKind::Markup);
        assert_eq!(m.message, "markup: unclosed tag");
        assert_eq!(m.context, "<p>");
        assert_eq!(m.word.as_deref(), Some("p"));
        let x = NativeLayoutError::expand("unknown value", 1, src, None, Some("check the name"));
        assert_eq!(x.kind, ErrorKind::Expand);
        assert_eq!(x.message, "unknown value");
        assert_eq!(x.context, "<div>");
        assert_eq!(x.suggestion.as_deref(), Some("check the name"));
    }
}
