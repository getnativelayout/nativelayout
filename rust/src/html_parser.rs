//! Port of HtmlParser.kt: a small, strict HTML reader.
//!
//! Not ported: `Chunk`, `ChunkCache` and `HtmlCachedElement`. In the Kotlin core nothing passes a
//! `ChunkCache` to the parser, so that code path is never used.

use crate::error::NativeLayoutError;

const VOID_TAGS: [&str; 6] = ["br", "hr", "img", "input", "meta", "link"];

#[derive(Debug, Clone, PartialEq)]
pub struct HtmlText {
    pub text: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HtmlElement {
    pub tag: String,
    /// In the order they were written; the first one wins when a name repeats.
    pub attributes: Vec<(String, String)>,
    pub children: Vec<HtmlNode>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HtmlNode {
    Element(HtmlElement),
    Text(HtmlText),
}

impl HtmlNode {
    pub fn line(&self) -> usize {
        match self {
            HtmlNode::Element(e) => e.line,
            HtmlNode::Text(t) => t.line,
        }
    }
}

impl HtmlElement {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn has_attr(&self, name: &str) -> bool {
        self.attributes.iter().any(|(k, _)| k == name)
    }

    pub fn id(&self) -> Option<String> {
        let v = self.attr("id")?.trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    }

    pub fn classes(&self) -> Vec<String> {
        match self.attr("class") {
            Some(raw) => raw.split_whitespace().map(str::to_string).collect(),
            None => Vec::new(),
        }
    }
}

struct Open {
    tag: String,
    attributes: Vec<(String, String)>,
    line: usize,
    children: Vec<HtmlNode>,
}

pub struct HtmlParser {
    html: Vec<char>,
    source: String,
    i: usize,
    line: usize,
    stack: Vec<Open>,
    root: Vec<HtmlNode>,
}

impl HtmlParser {
    pub fn new(html: &str, start_line: usize, source: &str) -> HtmlParser {
        HtmlParser {
            html: html.chars().collect(),
            source: source.to_string(),
            i: 0,
            line: start_line,
            stack: Vec::new(),
            root: Vec::new(),
        }
    }

    fn err(&self, message: &str, at: usize, word: Option<&str>) -> NativeLayoutError {
        NativeLayoutError::markup(message, at, &self.source, word, None)
    }

    fn add(&mut self, node: HtmlNode) {
        match self.stack.last_mut() {
            Some(o) => o.children.push(node),
            None => self.root.push(node),
        }
    }

    fn advance_to(&mut self, end: usize) {
        for k in self.i..end {
            if self.html[k] == '\n' {
                self.line += 1;
            }
        }
        self.i = end;
    }

    fn starts_with(&self, pat: &str, at: usize) -> bool {
        let mut k = at;
        for c in pat.chars() {
            if k >= self.html.len() || self.html[k] != c {
                return false;
            }
            k += 1;
        }
        true
    }

    fn find_str(&self, pat: &str, from: usize) -> Option<usize> {
        (from..self.html.len()).find(|&k| self.starts_with(pat, k))
    }

    fn find_char(&self, c: char, from: usize) -> Option<usize> {
        (from..self.html.len()).find(|&k| self.html[k] == c)
    }

    fn slice(&self, a: usize, b: usize) -> String {
        self.html[a..b].iter().collect()
    }

    pub fn parse(mut self) -> Result<Vec<HtmlNode>, NativeLayoutError> {
        let n = self.html.len();
        while self.i < n {
            let c = self.html[self.i];
            if self.starts_with("<!--", self.i) {
                let end = match self.find_str("-->", self.i + 4) {
                    Some(e) => e,
                    None => return Err(self.err("unclosed HTML comment: missing '-->'", self.line, None)),
                };
                self.advance_to(end + 3);
            } else if self.starts_with("<!", self.i) {
                let end = match self.find_char('>', self.i) {
                    Some(e) => e,
                    None => return Err(self.err("unclosed '<!': missing '>'", self.line, None)),
                };
                self.advance_to(end + 1);
            } else if self.starts_with("</", self.i) {
                self.read_close()?;
            } else if c == '<' && self.i + 1 < n && self.html[self.i + 1].is_alphabetic() {
                self.read_open()?;
            } else {
                self.read_text();
            }
        }
        if let Some(a) = self.stack.last() {
            let msg = format!("<{}> is never closed (opened on line {})", a.tag, a.line);
            let word = format!("<{}", a.tag);
            return Err(self.err(&msg, a.line, Some(word.as_str())));
        }
        Ok(self.root)
    }

    fn read_text(&mut self) {
        let n = self.html.len();
        let mut j = self.i + 1;
        while j < n {
            if self.html[j] == '<'
                && j + 1 < n
                && (self.html[j + 1].is_alphabetic() || self.html[j + 1] == '/' || self.html[j + 1] == '!')
            {
                break;
            }
            j += 1;
        }
        let raw = self.slice(self.i, j);
        let l = self.line;
        self.advance_to(j);
        if !raw.is_empty() {
            let text = decode(&raw);
            self.add(HtmlNode::Text(HtmlText { text, line: l }));
        }
    }

    fn read_close(&mut self) -> Result<(), NativeLayoutError> {
        let tag_line = self.line;
        let end = match self.find_char('>', self.i) {
            Some(e) => e,
            None => return Err(self.err("unclosed '</': missing '>'", self.line, None)),
        };
        let tag = self.slice(self.i + 2, end).trim().to_lowercase();
        self.advance_to(end + 1);
        if VOID_TAGS.contains(&tag.as_str()) {
            return Ok(());
        }
        let word = format!("</{}", tag);
        let top_tag_line = match self.stack.last() {
            None => {
                let msg = format!("</{}> has no matching opening tag", tag);
                return Err(self.err(&msg, tag_line, Some(word.as_str())));
            }
            Some(top) => (top.tag.clone(), top.line),
        };
        if top_tag_line.0 != tag {
            let msg = format!(
                "expected </{}> (opened on line {}) before </{}>",
                top_tag_line.0, top_tag_line.1, tag
            );
            return Err(self.err(&msg, tag_line, Some(word.as_str())));
        }
        let top = self.stack.pop().unwrap();
        let element = HtmlElement {
            tag: top.tag,
            attributes: top.attributes,
            children: top.children,
            line: top.line,
        };
        self.add(HtmlNode::Element(element));
        Ok(())
    }

    fn read_open(&mut self) -> Result<(), NativeLayoutError> {
        let n = self.html.len();
        let tag_line = self.line;
        let mut j = self.i + 1;
        let name_start = j;
        while j < n && (self.html[j].is_alphanumeric() || self.html[j] == '-') {
            j += 1;
        }
        let tag = self.slice(name_start, j).to_lowercase();
        let tag_word = format!("<{}", tag);
        let mut attributes: Vec<(String, String)> = Vec::new();
        let mut self_close = false;

        loop {
            while j < n && self.html[j].is_whitespace() {
                j += 1;
            }
            if j >= n {
                let msg = format!("unclosed tag <{}>: missing '>'", tag);
                return Err(self.err(&msg, tag_line, Some(tag_word.as_str())));
            }
            let ch = self.html[j];
            if ch == '>' {
                j += 1;
                break;
            }
            if ch == '/' && j + 1 < n && self.html[j + 1] == '>' {
                self_close = true;
                j += 2;
                break;
            }
            if ch == '/' {
                j += 1;
                continue;
            }

            let attr_start = j;
            while j < n
                && !self.html[j].is_whitespace()
                && self.html[j] != '='
                && self.html[j] != '>'
                && self.html[j] != '/'
            {
                j += 1;
            }
            if j == attr_start {
                let msg = format!("invalid attribute in tag <{}>", tag);
                return Err(self.err(&msg, tag_line, Some(tag_word.as_str())));
            }
            let name = self.slice(attr_start, j).to_lowercase();
            while j < n && self.html[j].is_whitespace() {
                j += 1;
            }

            let mut value = String::new();
            if j < n && self.html[j] == '=' {
                j += 1;
                while j < n && self.html[j].is_whitespace() {
                    j += 1;
                }
                if j < n && (self.html[j] == '"' || self.html[j] == '\'') {
                    let quote = self.html[j];
                    let end = match self.find_char(quote, j + 1) {
                        Some(e) => e,
                        None => {
                            let msg = format!("attribute '{}' has an unclosed quote", name);
                            return Err(self.err(&msg, tag_line, Some(name.as_str())));
                        }
                    };
                    value = self.slice(j + 1, end);
                    j = end + 1;
                } else {
                    let val_start = j;
                    while j < n && !self.html[j].is_whitespace() && self.html[j] != '>' {
                        j += 1;
                    }
                    value = self.slice(val_start, j);
                }
            }
            if !attributes.iter().any(|(k, _)| *k == name) {
                attributes.push((name, decode(&value)));
            }
        }
        self.advance_to(j);

        if tag == "script" {
            return Err(self.err("<script> is not allowed", tag_line, Some("<script")));
        }
        if tag == "style" {
            return Err(self.err(
                "<style> is not supported yet; use style=\"...\"",
                tag_line,
                Some("<style"),
            ));
        }
        if VOID_TAGS.contains(&tag.as_str()) || self_close {
            self.add(HtmlNode::Element(HtmlElement {
                tag,
                attributes,
                children: Vec::new(),
                line: tag_line,
            }));
        } else {
            self.stack.push(Open { tag, attributes, line: tag_line, children: Vec::new() });
        }
        Ok(())
    }
}

fn decode(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    let chars: Vec<char> = t.chars().collect();
    let mut sb = String::new();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c == '&' {
            if let Some(end) = (k..chars.len()).find(|&x| chars[x] == ';') {
                if end >= k + 2 && end <= k + 10 {
                    let name: String = chars[k + 1..end].iter().collect();
                    let r: Option<String> = match name.as_str() {
                        "amp" => Some("&".to_string()),
                        "lt" => Some("<".to_string()),
                        "gt" => Some(">".to_string()),
                        "quot" => Some("\"".to_string()),
                        "apos" => Some("'".to_string()),
                        "nbsp" => Some("\u{00A0}".to_string()),
                        _ => {
                            if let Some(h) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                                code_point(i32::from_str_radix(h, 16).ok())
                            } else if let Some(d) = name.strip_prefix('#') {
                                code_point(d.parse::<i32>().ok())
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(r) = r {
                        sb.push_str(&r);
                        k = end + 1;
                        continue;
                    }
                }
            }
        }
        sb.push(c);
        k += 1;
    }
    sb
}

fn code_point(n: Option<i32>) -> Option<String> {
    let n = n?;
    if n < 0 {
        return None;
    }
    char::from_u32(n as u32).map(|c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> Result<Vec<HtmlNode>, NativeLayoutError> {
        HtmlParser::new(html, 1, html).parse()
    }

    fn el(n: &HtmlNode) -> &HtmlElement {
        match n {
            HtmlNode::Element(e) => e,
            _ => panic!("expected an element"),
        }
    }

    #[test]
    fn parses_elements_attributes_and_text() {
        let nodes = parse("<div id=\"a\" class='x  y' hidden><p>Hi &amp; bye</p><br/></div>").unwrap();
        assert_eq!(nodes.len(), 1);
        let div = el(&nodes[0]);
        assert_eq!(div.tag, "div");
        assert_eq!(div.id(), Some("a".to_string()));
        assert_eq!(div.classes(), vec!["x", "y"]);
        assert_eq!(div.attr("hidden"), Some(""));
        assert_eq!(div.children.len(), 2);
        let p = el(&div.children[0]);
        match &p.children[0] {
            HtmlNode::Text(t) => assert_eq!(t.text, "Hi & bye"),
            _ => panic!("expected text"),
        }
        assert_eq!(el(&div.children[1]).tag, "br");
    }

    #[test]
    fn first_attribute_wins_and_names_are_lowercase() {
        let nodes = parse("<DIV Id=\"a\" id=\"b\"></DIV>").unwrap();
        assert_eq!(el(&nodes[0]).attr("id"), Some("a"));
    }

    #[test]
    fn tracks_lines() {
        let nodes = parse("<div>\n<p>x</p>\n</div>").unwrap();
        let div = el(&nodes[0]);
        assert_eq!(div.line, 1);
        let p = div.children.iter().find_map(|c| match c {
            HtmlNode::Element(e) => Some(e),
            _ => None,
        });
        assert_eq!(p.unwrap().line, 2);
    }

    #[test]
    fn comments_are_skipped() {
        let nodes = parse("<!-- hi --><div></div>").unwrap();
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn errors() {
        let e = parse("<div><p></div>").unwrap_err();
        assert_eq!(e.message, "markup: expected </p> (opened on line 1) before </div>");
        let e = parse("<div>").unwrap_err();
        assert_eq!(e.message, "markup: <div> is never closed (opened on line 1)");
        let e = parse("</div>").unwrap_err();
        assert_eq!(e.message, "markup: </div> has no matching opening tag");
        let e = parse("<script></script>").unwrap_err();
        assert_eq!(e.message, "markup: <script> is not allowed");
        let e = parse("<style></style>").unwrap_err();
        assert!(e.message.starts_with("markup: <style> is not supported yet"));
        let e = parse("<div class=\"x></div>").unwrap_err();
        assert_eq!(e.message, "markup: attribute 'class' has an unclosed quote");
        let e = parse("<!-- x").unwrap_err();
        assert_eq!(e.message, "markup: unclosed HTML comment: missing '-->'");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode("a &lt;b&gt; &quot;c&quot; &#65; &#x42; &nbsp;"), "a <b> \"c\" A B \u{a0}");
        assert_eq!(decode("&unknown; &#; &"), "&unknown; &#; &");
    }
}
