//! Port of Expander.kt: turns a `.nl` file (with `if`, `for`, `{values}`, `<include>`, `<style>` and
//! `<link>`) into plain HTML for the layout engine.
//!
//! Differences from Kotlin:
//! - Data is a `Value` instead of `Any?`.
//! - The cache is per thread (Kotlin shares it between threads) and keeps only the prepared nodes.
//! - Text lines are compiled when the nodes are built, not at the first emit (the result is the same).
//! - Conditions are tokenized every time they are evaluated.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::OnceLock;

use regex::Regex;

use crate::error::NativeLayoutError;
use crate::layout_engine::LayoutWarning;
use crate::scopes;
use crate::value::Value;

type Res<T> = Result<T, NativeLayoutError>;

#[derive(Debug, Clone)]
pub struct ExpandedScreen {
    pub html: String,
    pub line_map: Vec<usize>,
    /// Things that work but deserve a look, with the line of the `.nl` file.
    pub warnings: Vec<LayoutWarning>,
    /// The item of every repetition of a `for` that holds an element with an `id` or a class, by path
    /// (`p:7`, `p:7/o:2`). The program side finds the items here (section 5.7).
    pub items: HashMap<String, Value>,
}

/// Expands `source`. `resolve` gives the text of a file by name (for `<link>` and `<include>`).
/// `dark` selects the rules inside `@media (prefers-color-scheme: dark)`.
pub fn expand(
    source: &str,
    data: &HashMap<String, Value>,
    resolve: &dyn Fn(&str) -> Option<String>,
    dark: bool,
) -> Res<ExpandedScreen> {
    ScreenExpander::new(source, data, resolve, dark).expand()
}

/// Same as `expand` with no files and the light theme.
pub fn expand_simple(source: &str, data: &HashMap<String, Value>) -> Res<ExpandedScreen> {
    expand(source, data, &|_| None, false)
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Rc<Prepared>>> = RefCell::new(HashMap::new());
}

fn cached(key: &str) -> Option<Rc<Prepared>> {
    CACHE.with(|c| c.borrow().get(key).cloned())
}

fn store(key: String, prepared: Rc<Prepared>) {
    CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if m.len() >= 32 {
            m.clear();
        }
        m.insert(key, prepared);
    });
}

pub fn clear_cache() {
    CACHE.with(|c| c.borrow_mut().clear());
}

const KIND_TAG: i32 = 1;
const KIND_CLASS: i32 = 2;
const KIND_ID: i32 = 3;

const STATE_NONE: i32 = 0;
const STATE_PRESSED: i32 = 1;
const STATE_DISABLED: i32 = 2;
const STATE_FOCUS: i32 = 3;

const STATE_ATTRIBUTES: [&str; 3] = ["style-pressed", "style-disabled", "style-focus"];
const MAX_INCLUDE_LEVELS: usize = 8;
const VOID_NAMES: [&str; 6] = ["br", "hr", "img", "input", "meta", "link"];
const COMPARATORS: [&str; 6] = ["==", "!=", "<", ">", "<=", ">="];

struct Rule {
    kind: i32,
    name: String,
    declarations: Vec<(String, String)>,
    order: usize,
    line: usize,
    file: Option<String>,
    link_line: usize,
    state: i32,
    resolved: Vec<(String, String)>,
}

struct Attr {
    key: String,
    value: Option<String>,
}

/// `path` is where the code being emitted sits among the repetitions of the `for` loops that contain
/// it (empty outside any `for`). A component keeps the path of the place that includes it.
struct Scope {
    parent: Option<Rc<Scope>>,
    vars: HashMap<String, Value>,
    path: String,
}

impl Scope {
    fn empty() -> Scope {
        Scope { parent: None, vars: HashMap::new(), path: String::new() }
    }

    fn find(&self, name: &str) -> Option<&Value> {
        let mut s: &Scope = self;
        loop {
            if let Some(v) = s.vars.get(name) {
                return Some(v);
            }
            match &s.parent {
                Some(p) => s = &**p,
                None => return None,
            }
        }
    }

    fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }
}

enum Node {
    Text(TextNode),
    If(IfNode),
    For(ForNode),
    Include(IncludeNode),
}

struct TextNode {
    parts: Vec<Part>,
    line: usize,
}

struct Branch {
    condition: Option<String>,
    line: usize,
    nodes: Vec<Node>,
}

struct IfNode {
    #[allow(dead_code)]
    line: usize,
    branches: Vec<Branch>,
}

struct ForNode {
    line: usize,
    variable: String,
    list_name: String,
    nodes: Vec<Node>,
}

struct ParamPlan {
    name: String,
    constant: Value,
    single: Option<String>,
    parts: Option<Vec<Part>>,
}

struct IncludeNode {
    line: usize,
    component: Rc<Component>,
    params: Vec<ParamPlan>,
    key: Option<Vec<Part>>,
}

struct Control {
    kind: String,
    argument: String,
    variable: String,
}

/// Where the parts of a `.nl` file are, in lines counted from 0.
struct FileLayout {
    links: Vec<(String, usize)>,
    style_start: Option<usize>,
    style_end: usize,
    body_start: usize,
    body_end: usize,
}

/// A `.nl` file included by name. `include_line` is the line of the first `<include>` that asked for it.
struct Component {
    name: String,
    source: String,
    lines: Vec<String>,
    layout: FileLayout,
    include_line: usize,
    nodes: RefCell<Vec<Node>>,
}

struct IncludeTag {
    attrs: Vec<(String, Option<String>)>,
}

impl IncludeTag {
    fn get(&self, name: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .and_then(|(_, v)| v.clone())
    }
}

enum Part {
    Lit(String),
    Val(ValPart),
    Dyn(DynTag),
}

struct ValPart {
    inner: String,
    name: String,
    ok: bool,
    path: Vec<String>,
}

impl ValPart {
    fn new(inner: &str) -> ValPart {
        let name = inner.trim().to_string();
        let ok = rx().name.is_match(&name);
        let path = name.split('.').map(str::to_string).collect();
        ValPart { inner: inner.to_string(), name, ok, path }
    }
}

struct AttrPlan {
    orig: String,
    key: String,
    value: Option<String>,
    brace: bool,
    single: Option<String>,
    text_parts: Option<Vec<Part>>,
}

struct DynTag {
    name: String,
    lower: String,
    self_close: bool,
    rule_style: String,
    attrs: Vec<AttrPlan>,
    state_styles: [String; 3],
    /// The tag as text, for a tag with no `{value}` that carries an `id` or a class: used outside any `for`.
    pre: Option<String>,
}

struct Prepared {
    nodes: Vec<Node>,
}

struct Frame {
    open_line: usize,
    /// 0 = root, 1 = `if`, 2 = `for`
    kind: u8,
    for_var: String,
    for_list: String,
    done: Vec<Branch>,
    cur_condition: Option<String>,
    cur_line: usize,
    nodes: Vec<Node>,
}

impl Frame {
    fn new(open_line: usize, kind: u8) -> Frame {
        Frame {
            open_line,
            kind,
            for_var: String::new(),
            for_list: String::new(),
            done: Vec::new(),
            cur_condition: None,
            cur_line: open_line,
            nodes: Vec::new(),
        }
    }
}

// ---------- regular expressions ----------

struct Rx {
    include_start: Regex,
    key_attr: Regex,
    has_key: Regex,
    field_or_id: Regex,
    tag: Regex,
    tag_name: Regex,
    attr: Regex,
    brace: Regex,
    name: Regex,
    single_value: Regex,
    var: Regex,
    rule: Regex,
    comment: Regex,
    for_re: Regex,
    keyword: Regex,
    else_space: Regex,
    token: Regex,
    tag_selector: Regex,
    class_selector: Regex,
    id_selector: Regex,
    ws: Regex,
}

fn rx() -> &'static Rx {
    static RX: OnceLock<Rx> = OnceLock::new();
    RX.get_or_init(|| Rx {
        include_start: Regex::new(r"^<include[\s/>]").unwrap(),
        key_attr: Regex::new(r#"\skey\s*=\s*"([^"]*)""#).unwrap(),
        has_key: Regex::new(r"\skey\s*=").unwrap(),
        field_or_id: Regex::new(r"<(?:input|textarea)\b|\sid\s*=").unwrap(),
        tag: Regex::new(r"<[^<>]*>").unwrap(),
        tag_name: Regex::new(r"^<([A-Za-z][A-Za-z0-9-]*)").unwrap(),
        attr: Regex::new(
            r#"([A-Za-z_:][-A-Za-z0-9_:.]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?"#,
        )
        .unwrap(),
        brace: Regex::new(r"\\[{}]|\{([^{}]*)\}").unwrap(),
        name: Regex::new(r"^[A-Za-z_][A-Za-z0-9_.]*$").unwrap(),
        single_value: Regex::new(r"^\{\s*([A-Za-z_][A-Za-z0-9_.]*)\s*\}$").unwrap(),
        var: Regex::new(r"var\(\s*(--[A-Za-z0-9_-]+)\s*\)").unwrap(),
        rule: Regex::new(r"([^{}]+)\{([^{}]*)\}").unwrap(),
        comment: Regex::new(r"/\*[\s\S]*?\*/").unwrap(),
        for_re: Regex::new(r"^for\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+([A-Za-z_][A-Za-z0-9_.]*):$").unwrap(),
        keyword: Regex::new(r"^(if|elif|for)\s").unwrap(),
        else_space: Regex::new(r"^else\s+:$").unwrap(),
        token: Regex::new(
            r#"==|!=|<=|>=|<|>|\(|\)|"[^"]*"|-?\d+(?:\.\d+)?|[A-Za-z_][A-Za-z0-9_.]*"#,
        )
        .unwrap(),
        tag_selector: Regex::new(r"^[A-Za-z][A-Za-z0-9]*$").unwrap(),
        class_selector: Regex::new(r"^\.[A-Za-z_][A-Za-z0-9_-]*$").unwrap(),
        id_selector: Regex::new(r"^#[A-Za-z_][A-Za-z0-9_-]*$").unwrap(),
        ws: Regex::new(r"\s+").unwrap(),
    })
}

fn count_newlines(s: &str) -> usize {
    s.bytes().filter(|&b| b == b'\n').count()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn format_number(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Float(d) => {
            if d.is_finite() && *d == d.floor() {
                (*d as i64).to_string()
            } else {
                d.to_string()
            }
        }
        _ => String::new(),
    }
}

fn singular(name: &str) -> String {
    let last = name.rsplit('.').next().unwrap_or(name);
    if last.chars().count() > 1 && last.ends_with('s') {
        last[..last.len() - 1].to_string()
    } else {
        "item".to_string()
    }
}

fn join_path(from: &str, rel: &str) -> String {
    let dir = match from.rfind('/') {
        Some(i) => &from[..i],
        None => "",
    };
    if dir.is_empty() {
        rel.to_string()
    } else {
        format!("{}/{}", dir, rel)
    }
}

fn merge_lits(list: Vec<Part>) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::with_capacity(list.len());
    for p in list {
        if let Part::Lit(s) = &p {
            if let Some(Part::Lit(last)) = out.last_mut() {
                last.push_str(s);
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// Where a mistake in the structure of a file is reported: the main file or a component.
enum Fail {
    Main,
    Component { name: String, include_line: usize },
}

struct ScreenExpander<'a> {
    source: &'a str,
    data: &'a HashMap<String, Value>,
    resolve: &'a dyn Fn(&str) -> Option<String>,
    dark: bool,
    lines: Vec<String>,
    rules: Vec<Rule>,
    variables: HashMap<String, String>,
    rule_order: usize,
    out: Vec<String>,
    out_lines: Vec<usize>,
    warnings: Vec<LayoutWarning>,
    // While a component is built or emitted, errors name it and point at the line of its <include>.
    current_component: Option<String>,
    current_include_line: usize,
    components: HashMap<String, Rc<Component>>,
    component_order: Vec<String>,
    // Identity of the repetitions of a `for` (section 5.7).
    scope_counter: usize,
    marker_hits: usize,
    auto_keyed: HashSet<usize>,
    scope_log: Vec<(String, Value)>,
}

// ---------- the expander ----------

impl<'a> ScreenExpander<'a> {
    fn new(
        source: &'a str,
        data: &'a HashMap<String, Value>,
        resolve: &'a dyn Fn(&str) -> Option<String>,
        dark: bool,
    ) -> ScreenExpander<'a> {
        ScreenExpander {
            source,
            data,
            resolve,
            dark,
            lines: source.split('\n').map(str::to_string).collect(),
            rules: Vec::new(),
            variables: HashMap::new(),
            rule_order: 0,
            out: Vec::new(),
            out_lines: Vec::new(),
            warnings: Vec::new(),
            current_component: None,
            current_include_line: 0,
            components: HashMap::new(),
            component_order: Vec::new(),
            scope_counter: 0,
            marker_hits: 0,
            auto_keyed: HashSet::new(),
            scope_log: Vec::new(),
        }
    }

    fn err(&self, message: &str, line: usize, word: Option<&str>, suggestion: Option<&str>) -> NativeLayoutError {
        match &self.current_component {
            None => NativeLayoutError::expand(message, line, self.source, word, suggestion),
            Some(c) => NativeLayoutError::expand(
                &format!("in '{}': {}", c, message),
                self.current_include_line,
                self.source,
                word,
                suggestion,
            ),
        }
    }

    /// The line to report: inside a component it is the line of the `<include>`.
    fn eff_line(&self, line: usize) -> usize {
        if self.current_component.is_some() {
            self.current_include_line
        } else {
            line
        }
    }

    fn style_error(
        &self,
        message: &str,
        line: usize,
        word: Option<&str>,
        file: Option<&str>,
        link_line: usize,
    ) -> NativeLayoutError {
        match file {
            None => self.err(&format!("style: {}", message), line, word, None),
            Some(f) => self.err(&format!("style: in '{}' line {}: {}", f, line, message), link_line, word, None),
        }
    }

    fn fail_err(&self, fail: &Fail, message: &str, line: usize, word: Option<&str>) -> NativeLayoutError {
        match fail {
            Fail::Main => self.err(message, line, word, None),
            Fail::Component { name, include_line } => NativeLayoutError::expand(
                &format!("in '{}': {}", name, message),
                *include_line,
                self.source,
                word,
                None,
            ),
        }
    }

    fn include_error(&self, file_name: &str, message: &str, line: usize) -> NativeLayoutError {
        if file_name.is_empty() {
            NativeLayoutError::expand(message, line, self.source, None, None)
        } else {
            NativeLayoutError::expand(&format!("in '{}': {}", file_name, message), line, self.source, None, None)
        }
    }

    fn link_href(&self, tag: &str, line: usize, fail: &Fail) -> Res<String> {
        let mut attrs: HashMap<String, String> = HashMap::new();
        let inner = tag.strip_prefix("<link").unwrap_or(tag);
        let inner = inner.strip_suffix('>').unwrap_or(inner);
        let inner = inner.strip_suffix('/').unwrap_or(inner);
        for a in rx().attr.captures_iter(inner) {
            let value = match a.get(2).or_else(|| a.get(3)).or_else(|| a.get(4)) {
                Some(v) => v.as_str().to_string(),
                None => continue,
            };
            attrs.insert(a[1].to_lowercase(), value);
        }
        let href = match attrs.get("href") {
            Some(h) if !h.trim().is_empty() => h.clone(),
            _ => {
                return Err(self.fail_err(fail, "markup: <link> needs rel=\"stylesheet\" and a href", line, None));
            }
        };
        if attrs.get("rel").map(|r| r.to_lowercase()) != Some("stylesheet".to_string()) {
            return Err(self.fail_err(fail, "markup: <link> needs rel=\"stylesheet\" and a href", line, None));
        }
        if href.starts_with('/') || href.contains("..") || href.contains("://") {
            let msg = format!("style: the file '{}' is not allowed", href);
            return Err(self.fail_err(fail, &msg, line, Some(href.as_str())));
        }
        if !href.ends_with(".css") {
            let msg = format!("style: the file '{}' must be a .css file", href);
            return Err(self.fail_err(fail, &msg, line, Some(href.as_str())));
        }
        Ok(href)
    }

    /// Finds the `<link>`, the `<style>` and the `<body>` of a `.nl` file.
    fn scan_file(&self, file_lines: &[String], fail: &Fail) -> Res<FileLayout> {
        let n = file_lines.len();
        let mut first = 0usize;
        while first < n && file_lines[first].trim().is_empty() {
            first += 1;
        }
        let mut last: isize = n as isize - 1;
        while last >= 0 && file_lines[last as usize].trim().is_empty() {
            last -= 1;
        }
        if (first as isize) >= last
            || file_lines[first].trim() != "<nativelayout>"
            || file_lines[last as usize].trim() != "</nativelayout>"
        {
            return Err(self.fail_err(
                fail,
                "the file must start with <nativelayout> and end with </nativelayout>",
                if first < n { first + 1 } else { 1 },
                None,
            ));
        }
        let last = last as usize;

        let mut style_start: Option<usize> = None;
        let mut style_end = 0usize;
        let mut body_start: Option<usize> = None;
        let mut body_end = 0usize;
        let mut links: Vec<(String, usize)> = Vec::new();
        let mut i = first + 1;
        let mut in_comment = false;
        while i < last {
            let t = file_lines[i].trim();
            if in_comment {
                if t.contains("-->") {
                    in_comment = false;
                }
            } else if t.is_empty() {
                // nothing
            } else if t.starts_with("<!--") {
                if !t.contains("-->") {
                    in_comment = true;
                }
            } else if t.starts_with("<script") {
                return Err(self.fail_err(fail, "markup: <script> is not allowed", i + 1, None));
            } else if t == "<style>" {
                if style_start.is_some() {
                    return Err(self.fail_err(fail, "only one <style> block is allowed", i + 1, None));
                }
                let mut j = i + 1;
                while j < last && file_lines[j].trim() != "</style>" {
                    j += 1;
                }
                if j >= last {
                    return Err(self.fail_err(fail, "<style> is never closed with </style>", i + 1, None));
                }
                style_start = Some(i + 1);
                style_end = j;
                i = j;
            } else if t.starts_with("<link") {
                if body_start.is_some() {
                    return Err(self.fail_err(fail, "a <link> must come before <body>", i + 1, None));
                }
                let href = self.link_href(t, i + 1, fail)?;
                links.push((href, i + 1));
            } else if t.starts_with("<body") {
                if body_start.is_some() {
                    return Err(self.fail_err(fail, "the file must have exactly one <body>", i + 1, None));
                }
                let mut j = i + 1;
                while j < last && file_lines[j].trim() != "</body>" {
                    j += 1;
                }
                if j >= last {
                    return Err(self.fail_err(fail, "<body> is never closed with </body>", i + 1, None));
                }
                body_start = Some(i);
                body_end = j;
                i = j;
            } else {
                return Err(self.fail_err(
                    fail,
                    "only <style> and <body> are allowed inside <nativelayout>",
                    i + 1,
                    None,
                ));
            }
            i += 1;
        }
        let body_start = match body_start {
            Some(b) => b,
            None => {
                return Err(self.fail_err(fail, "the file must have exactly one <body>", first + 1, None));
            }
        };
        Ok(FileLayout { links, style_start, style_end, body_start, body_end })
    }

    // ---------- components: <include> (section 5.6)

    /// The `<include>` of a line, or `None` when the line has none. It must be alone on its line.
    fn include_tag_of(&self, raw: &str, report_line: usize, file_name: &str) -> Res<Option<IncludeTag>> {
        let t = raw.trim();
        if !rx().include_start.is_match(t) {
            return Ok(None);
        }
        match rx().tag.find(t) {
            Some(m) if m.start() == 0 && m.end() == t.len() => {}
            _ => {
                return Err(self.include_error(file_name, "markup: <include> must be alone on its line", report_line));
            }
        }
        let inner = &t["<include".len()..t.len() - 1];
        let inner = inner.strip_suffix('/').unwrap_or(inner);
        let mut attrs: Vec<(String, Option<String>)> = Vec::new();
        for a in rx().attr.captures_iter(inner) {
            let value = a
                .get(2)
                .or_else(|| a.get(3))
                .or_else(|| a.get(4))
                .map(|m| m.as_str().to_string());
            attrs.push((a[1].to_string(), value));
        }
        Ok(Some(IncludeTag { attrs }))
    }

    fn component_path(&self, from: &str, src: &str, line: usize) -> Res<String> {
        if src.trim().is_empty() || src.starts_with('/') || src.contains("..") || src.contains("://") {
            return Err(self.include_error(from, &format!("include: the file '{}' is not allowed", src), line));
        }
        if !src.ends_with(".nl") {
            return Err(self.include_error(from, &format!("include: the file '{}' must be a .nl file", src), line));
        }
        Ok(join_path(from, src))
    }

    /// Loads, in order of appearance, every component that the file includes, and the ones they include.
    /// `chain` holds the components being read now, to catch a file that includes itself.
    fn collect_includes(
        &mut self,
        file_name: &str,
        file_lines: &[String],
        layout: &FileLayout,
        chain: &[String],
        main_line: usize,
    ) -> Res<()> {
        for idx in (layout.body_start + 1)..layout.body_end {
            let line = if file_name.is_empty() { idx + 1 } else { main_line };
            let tag = match self.include_tag_of(&file_lines[idx], line, file_name)? {
                Some(t) => t,
                None => continue,
            };
            let src = match tag.get("src").filter(|s| !s.trim().is_empty()) {
                Some(s) => s,
                None => {
                    return Err(self.include_error(file_name, "markup: <include> requires a src attribute", line));
                }
            };
            let name = self.component_path(file_name, &src, line)?;
            if name == file_name || chain.contains(&name) {
                return Err(self.include_error(file_name, &format!("include: '{}' includes itself", name), line));
            }
            if chain.len() >= MAX_INCLUDE_LEVELS {
                return Err(self.include_error(
                    file_name,
                    &format!("include: too many nested includes (the limit is {})", MAX_INCLUDE_LEVELS),
                    line,
                ));
            }
            if self.components.contains_key(&name) {
                continue;
            }
            let text = match (self.resolve)(&name) {
                Some(t) => t,
                None => {
                    return Err(self.include_error(
                        file_name,
                        &format!("include: the file '{}' was not found", name),
                        line,
                    ));
                }
            };
            let comp_lines: Vec<String> = text.split('\n').map(str::to_string).collect();
            let comp_layout = self.scan_file(
                &comp_lines,
                &Fail::Component { name: name.clone(), include_line: line },
            )?;
            let comp = Rc::new(Component {
                name: name.clone(),
                source: text,
                lines: comp_lines,
                layout: comp_layout,
                include_line: line,
                nodes: RefCell::new(Vec::new()),
            });
            self.components.insert(name.clone(), Rc::clone(&comp));
            self.component_order.push(name.clone());
            let mut next_chain: Vec<String> = chain.to_vec();
            next_chain.push(name);
            self.collect_includes(&comp.name, &comp.lines, &comp.layout, &next_chain, line)?;
        }
        Ok(())
    }

    fn param_plan(&mut self, name: &str, value: Option<&String>) -> Res<ParamPlan> {
        let value = match value {
            None => {
                return Ok(ParamPlan { name: name.to_string(), constant: Value::Bool(true), single: None, parts: None });
            }
            Some(v) => v,
        };
        if !value.contains('{') {
            return Ok(ParamPlan { name: name.to_string(), constant: Value::Str(value.clone()), single: None, parts: None });
        }
        if let Some(c) = rx().single_value.captures(value.trim()) {
            return Ok(ParamPlan {
                name: name.to_string(),
                constant: Value::Null,
                single: Some(c[1].to_string()),
                parts: None,
            });
        }
        let parts = self.compile_value(value)?;
        Ok(ParamPlan { name: name.to_string(), constant: Value::Null, single: None, parts: Some(parts) })
    }

    fn make_include(&mut self, tag: &IncludeTag, file_name: &str, line: usize) -> Res<IncludeNode> {
        let src = match tag.get("src") {
            Some(s) => s,
            None => return Err(self.err("markup: <include> requires a src attribute", line, None, None)),
        };
        let component = match self.components.get(&join_path(file_name, &src)) {
            Some(c) => Rc::clone(c),
            None => {
                return Err(self.err(&format!("include: the file '{}' was not found", src), line, None, None));
            }
        };
        let mut params: Vec<ParamPlan> = Vec::new();
        let mut key: Option<Vec<Part>> = None;
        for (name, value) in &tag.attrs {
            match name.to_lowercase().as_str() {
                "src" => {}
                "key" => {
                    key = match value {
                        Some(v) => Some(self.compile_value(v)?),
                        None => None,
                    };
                }
                _ => params.push(self.param_plan(name, value.as_ref())?),
            }
        }
        Ok(IncludeNode { line, component, params, key })
    }

    fn expand(mut self) -> Res<ExpandedScreen> {
        let lines = std::mem::take(&mut self.lines);
        let layout = self.scan_file(&lines, &Fail::Main)?;
        self.collect_includes("", &lines, &layout, &[], 0)?;

        // The style files of the components come first, so the screen can override them.
        let mut component_loaded: HashSet<String> = HashSet::new();
        let mut component_sheets: Vec<(String, usize, String)> = Vec::new();
        let names: Vec<String> = self.component_order.clone();
        for name in &names {
            let comp = Rc::clone(&self.components[name]);
            for (href, _) in &comp.layout.links {
                let path = join_path(&comp.name, href);
                if !component_loaded.insert(path.clone()) {
                    continue;
                }
                let css = match (self.resolve)(&path) {
                    Some(c) => c,
                    None => {
                        return Err(NativeLayoutError::expand(
                            &format!("style: the file '{}' was not found", path),
                            comp.include_line,
                            self.source,
                            Some(path.as_str()),
                            None,
                        ));
                    }
                };
                if css.contains("@import") {
                    return Err(NativeLayoutError::expand(
                        "style: a style file cannot import another file",
                        comp.include_line,
                        self.source,
                        Some(path.as_str()),
                        None,
                    ));
                }
                component_sheets.push((path, comp.include_line, css));
            }
        }

        let mut loaded: HashSet<String> = HashSet::new();
        let mut sheets: Vec<(String, usize, String)> = Vec::new();
        for (href, link_line) in &layout.links {
            if !loaded.insert(href.clone()) {
                continue;
            }
            let css = match (self.resolve)(href) {
                Some(c) => c,
                None => {
                    return Err(self.err(
                        &format!("style: the file '{}' was not found", href),
                        *link_line,
                        Some(href.as_str()),
                        None,
                    ));
                }
            };
            if css.contains("@import") {
                return Err(self.err(
                    "style: a style file cannot import another file",
                    *link_line,
                    Some(href.as_str()),
                    None,
                ));
            }
            sheets.push((href.clone(), *link_line, css));
        }

        let mut key = String::with_capacity(self.source.len() + 64);
        key.push(if self.dark { 'd' } else { 'l' });
        key.push('\u{1}');
        key.push_str(self.source);
        for name in &names {
            let comp = &self.components[name];
            key.push('\u{2}');
            key.push_str(&comp.name);
            key.push('\u{3}');
            key.push_str(&comp.source);
        }
        for s in &component_sheets {
            key.push('\u{0}');
            key.push_str(&s.2);
        }
        key.push('\u{4}');
        for s in &sheets {
            key.push('\u{0}');
            key.push_str(&s.2);
        }

        let prepared: Rc<Prepared> = match cached(&key) {
            Some(hit) => hit,
            None => {
                for s in &component_sheets {
                    self.parse_style(&s.2, 1, Some(s.0.as_str()), s.1)?;
                }
                for name in &names {
                    let comp = Rc::clone(&self.components[name]);
                    if let Some(ss) = comp.layout.style_start {
                        let text = comp.lines[ss..comp.layout.style_end].join("\n");
                        self.parse_style(&text, ss + 1, Some(comp.name.as_str()), comp.include_line)?;
                    }
                }
                for s in &sheets {
                    self.parse_style(&s.2, 1, Some(s.0.as_str()), s.1)?;
                }
                if let Some(ss) = layout.style_start {
                    let text = lines[ss..layout.style_end].join("\n");
                    self.parse_style(&text, ss + 1, None, 0)?;
                }
                self.resolve_variables()?;
                for name in &names {
                    let comp = Rc::clone(&self.components[name]);
                    self.current_component = Some(comp.name.clone());
                    self.current_include_line = comp.include_line;
                    let nodes = self.build_nodes(&comp.lines, comp.layout.body_start, comp.layout.body_end, &comp.name)?;
                    *comp.nodes.borrow_mut() = nodes;
                }
                self.current_component = None;
                let nodes = self.build_nodes(&lines, layout.body_start, layout.body_end, "")?;
                let p = Rc::new(Prepared { nodes });
                store(key, Rc::clone(&p));
                p
            }
        };

        let scope = Rc::new(Scope { parent: None, vars: self.data.clone(), path: String::new() });
        self.emit(&prepared.nodes, &scope)?;
        let mut items: HashMap<String, Value> = HashMap::new();
        for (k, v) in &self.scope_log {
            items.insert(k.clone(), v.clone());
        }
        Ok(ExpandedScreen {
            html: self.out.join("\n"),
            line_map: self.out_lines.clone(),
            warnings: self.warnings.clone(),
            items,
        })
    }

    fn flatten_media(&self, text: &str, first_line: usize, file: Option<&str>, link_line: usize) -> Res<String> {
        let mut t = text.to_string();
        loop {
            let at = match t.find("@media") {
                Some(a) => a,
                None => return Ok(t),
            };
            let line = first_line + count_newlines(&t[..at]);
            let open = match t[at..].find('{') {
                Some(o) => at + o,
                None => {
                    return Err(self.style_error("'@media' without '{'", line, Some("@media"), file, link_line));
                }
            };
            let mut depth = 0i32;
            let mut close: Option<usize> = None;
            for (i, c) in t.bytes().enumerate().skip(open) {
                if c == b'{' {
                    depth += 1;
                } else if c == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
            }
            let close = match close {
                Some(c) => c,
                None => {
                    return Err(self.style_error(
                        "the '@media' block is never closed",
                        line,
                        Some("@media"),
                        file,
                        link_line,
                    ));
                }
            };
            let condition = rx().ws.replace_all(&t[at + 6..open].trim().to_lowercase(), " ").to_string();
            let include = match condition.replace(": ", ":").as_str() {
                "(prefers-color-scheme:dark)" => self.dark,
                "(prefers-color-scheme:light)" => !self.dark,
                _ => {
                    return Err(self.style_error(
                        &format!(
                            "the '@media {}' is not supported (use '@media (prefers-color-scheme: dark)' or '... light')",
                            condition
                        ),
                        line,
                        Some("@media"),
                        file,
                        link_line,
                    ));
                }
            };
            let replacement: String = if include {
                t[open + 1..close].to_string()
            } else {
                t[at..=close].chars().filter(|c| *c == '\n').collect()
            };
            t = format!("{}{}{}", &t[..at], replacement, &t[close + 1..]);
        }
    }

    fn parse_style(&mut self, text: &str, first_line: usize, file: Option<&str>, link_line: usize) -> Res<()> {
        let commentless = rx()
            .comment
            .replace_all(text, |c: &regex::Captures| c[0].chars().filter(|x| *x == '\n').collect::<String>())
            .to_string();
        let clean = self.flatten_media(&commentless, first_line, file, link_line)?;
        let found: Vec<(String, String, usize)> = rx()
            .rule
            .captures_iter(&clean)
            .map(|c| (c[1].to_string(), c[2].to_string(), c.get(0).map(|m| m.start()).unwrap_or(0)))
            .collect();
        for (selector_text, body, start) in found {
            let offset = start + (selector_text.len() - selector_text.trim_start().len());
            let line = first_line + count_newlines(&clean[..offset]);
            let mut declarations: Vec<(String, String)> = Vec::new();
            for part in body.split(';') {
                let idx = match part.find(':') {
                    Some(i) => i,
                    None => continue,
                };
                let name = part[..idx].trim().to_lowercase();
                let value = part[idx + 1..].replace("!important", "").trim().to_string();
                if name.is_empty() || value.is_empty() {
                    continue;
                }
                declarations.push((name, value));
            }
            for raw in selector_text.split(',') {
                let full = raw.trim().to_string();
                let mut selector = full.clone();
                let mut state = STATE_NONE;
                if let Some(colon) = full.find(':') {
                    if colon > 0 {
                        state = match full[colon + 1..].to_lowercase().as_str() {
                            "pressed" | "active" => STATE_PRESSED,
                            "disabled" => STATE_DISABLED,
                            "focus" => STATE_FOCUS,
                            _ => {
                                return Err(self.style_error(
                                    &format!(
                                        "the selector '{}' is not supported (states: :pressed, :disabled, :focus)",
                                        full
                                    ),
                                    line,
                                    Some(full.as_str()),
                                    file,
                                    link_line,
                                ));
                            }
                        };
                        selector = full[..colon].to_string();
                    }
                }
                let file_owned = file.map(str::to_string);
                if selector == ":root" && state == STATE_NONE {
                    for (n, v) in &declarations {
                        if n.starts_with("--") {
                            self.variables.insert(n.clone(), v.clone());
                        }
                    }
                } else if rx().tag_selector.is_match(&selector) {
                    self.add_rule(KIND_TAG, selector.to_lowercase(), &declarations, line, file_owned, link_line, state);
                } else if rx().class_selector.is_match(&selector) {
                    self.add_rule(KIND_CLASS, selector[1..].to_string(), &declarations, line, file_owned, link_line, state);
                } else if rx().id_selector.is_match(&selector) {
                    self.add_rule(KIND_ID, selector[1..].to_string(), &declarations, line, file_owned, link_line, state);
                } else {
                    return Err(self.style_error(
                        &format!("the selector '{}' is not supported", full),
                        line,
                        Some(full.as_str()),
                        file,
                        link_line,
                    ));
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn add_rule(
        &mut self,
        kind: i32,
        name: String,
        declarations: &[(String, String)],
        line: usize,
        file: Option<String>,
        link_line: usize,
        state: i32,
    ) {
        let order = self.rule_order;
        self.rule_order += 1;
        self.rules.push(Rule {
            kind,
            name,
            declarations: declarations.to_vec(),
            order,
            line,
            file,
            link_line,
            state,
            resolved: Vec::new(),
        });
    }

    fn substitute_vars(&self, value: &str, line: usize, file: Option<&str>, link_line: usize) -> Res<String> {
        let mut out = String::new();
        let mut last = 0;
        for c in rx().var.captures_iter(value) {
            let m = c.get(0).unwrap();
            out.push_str(&value[last..m.start()]);
            let name = &c[1];
            match self.variables.get(name) {
                Some(v) => out.push_str(v),
                None => {
                    return Err(self.style_error(
                        &format!("the variable '{}' is not defined in ':root'", name),
                        line,
                        Some(name),
                        file,
                        link_line,
                    ));
                }
            }
            last = m.end();
        }
        out.push_str(&value[last..]);
        Ok(out)
    }

    fn resolve_variables(&mut self) -> Res<()> {
        let mut rules = std::mem::take(&mut self.rules);
        for rule in rules.iter_mut() {
            let mut resolved: Vec<(String, String)> = Vec::new();
            for d in &rule.declarations {
                if d.0.starts_with("--") {
                    continue;
                }
                let v = self.substitute_vars(&d.1, rule.line, rule.file.as_deref(), rule.link_line)?;
                resolved.push((d.0.clone(), v));
            }
            rule.resolved = resolved;
        }
        self.rules = rules;
        Ok(())
    }

    fn parse_control(&self, t: &str, line: usize) -> Res<Option<Control>> {
        let ctl = |kind: &str, argument: &str, variable: &str| Control {
            kind: kind.to_string(),
            argument: argument.to_string(),
            variable: variable.to_string(),
        };
        if t == "end" {
            return Ok(Some(ctl("end", "", "")));
        }
        if t == "else:" {
            return Ok(Some(ctl("else", "", "")));
        }
        if rx().else_space.is_match(t) {
            return Err(self.err("write 'else:' without a space before ':'", line, None, None));
        }
        let keyword = match rx().keyword.captures(t) {
            Some(c) => c[1].to_string(),
            None => return Ok(None),
        };
        if !t.ends_with(':') {
            return Ok(None);
        }
        let body = &t[..t.len() - 1];
        if body.ends_with(' ') || body.ends_with('\t') {
            return Err(self.err(
                &format!("write '{}:' without a space before ':'", body.trim_end()),
                line,
                None,
                None,
            ));
        }
        if keyword == "for" {
            let m = match rx().for_re.captures(t) {
                Some(m) => m,
                None => {
                    return Err(self.err(
                        &format!("invalid 'for': '{}'", t),
                        line,
                        None,
                        Some("write 'for item in list:'"),
                    ));
                }
            };
            return Ok(Some(ctl("for", &m[2], &m[1])));
        }
        let condition = body[keyword.len()..].trim();
        if condition.is_empty() {
            return Err(self.err("invalid condition: ''", line, None, None));
        }
        Ok(Some(ctl(&keyword, condition, "")))
    }

    fn build_nodes(&mut self, file_lines: &[String], from: usize, to: usize, file_name: &str) -> Res<Vec<Node>> {
        let mut stack: Vec<Frame> = vec![Frame::new(0, 0)];
        for idx in from..=to {
            let line = idx + 1;
            let control = if idx == from || idx == to {
                None
            } else {
                self.parse_control(file_lines[idx].trim(), line)?
            };
            let control = match control {
                None => {
                    let include = if idx == from || idx == to {
                        None
                    } else {
                        let report = if file_name.is_empty() { line } else { self.current_include_line };
                        self.include_tag_of(&file_lines[idx], report, file_name)?
                    };
                    let node = match include {
                        Some(tag) => Node::Include(self.make_include(&tag, file_name, line)?),
                        None => Node::Text(TextNode { parts: self.compile_line(&file_lines[idx])?, line }),
                    };
                    stack.last_mut().unwrap().nodes.push(node);
                    continue;
                }
                Some(c) => c,
            };
            match control.kind.as_str() {
                "if" => {
                    let mut f = Frame::new(line, 1);
                    f.cur_condition = Some(control.argument.clone());
                    f.cur_line = line;
                    stack.push(f);
                }
                "elif" | "else" => {
                    let top = stack.last_mut().unwrap();
                    if top.kind != 1 || top.cur_condition.is_none() {
                        return Err(self.err(&format!("'{}' has no matching 'if'", control.kind), line, None, None));
                    }
                    let finished = Branch {
                        condition: top.cur_condition.take(),
                        line: top.cur_line,
                        nodes: std::mem::take(&mut top.nodes),
                    };
                    top.done.push(finished);
                    top.cur_condition = if control.kind == "else" { None } else { Some(control.argument.clone()) };
                    top.cur_line = line;
                    // an `else` has no condition: remember that the branch is open with a flag value
                    if control.kind == "else" {
                        top.cur_line = line;
                    }
                }
                "for" => {
                    let mut f = Frame::new(line, 2);
                    f.for_var = control.variable.clone();
                    f.for_list = control.argument.clone();
                    stack.push(f);
                }
                _ => {
                    if stack.len() == 1 {
                        return Err(self.err("'end' has no matching 'if' or 'for'", line, None, None));
                    }
                    let mut f = stack.pop().unwrap();
                    let node = if f.kind == 1 {
                        let last = Branch {
                            condition: f.cur_condition.take(),
                            line: f.cur_line,
                            nodes: std::mem::take(&mut f.nodes),
                        };
                        f.done.push(last);
                        Node::If(IfNode { line: f.open_line, branches: f.done })
                    } else {
                        Node::For(ForNode {
                            line: f.open_line,
                            variable: f.for_var,
                            list_name: f.for_list,
                            nodes: f.nodes,
                        })
                    };
                    stack.last_mut().unwrap().nodes.push(node);
                }
            }
        }
        if stack.len() > 1 {
            let open = stack.last().unwrap();
            let word = if open.kind == 1 { "if" } else { "for" };
            return Err(self.err(
                &format!("the '{}' opened on line {} is never closed with 'end'", word, open.open_line),
                open.open_line,
                None,
                None,
            ));
        }
        Ok(stack.pop().unwrap().nodes)
    }
}

// ---------- emitting ----------

impl<'a> ScreenExpander<'a> {
    fn emit(&mut self, nodes: &[Node], scope: &Rc<Scope>) -> Res<()> {
        for node in nodes {
            match node {
                Node::Text(t) => {
                    let line = self.eff_line(t.line);
                    let text = self.render_parts(&t.parts, line, scope)?;
                    self.out.push(text);
                    self.out_lines.push(line);
                }
                Node::If(n) => {
                    for branch in &n.branches {
                        let take = match &branch.condition {
                            None => true,
                            Some(c) => self.evaluate(branch, c, scope)?,
                        };
                        if take {
                            self.emit(&branch.nodes, scope)?;
                            break;
                        }
                    }
                }
                Node::For(n) => self.emit_for(n, scope)?,
                Node::Include(n) => self.emit_include(n, scope)?,
            }
        }
        Ok(())
    }

    fn emit_for(&mut self, node: &ForNode, scope: &Rc<Scope>) -> Res<()> {
        let ln = self.eff_line(node.line);
        let list_value = self.lookup(&node.list_name, ln, scope)?;
        let list = match &list_value {
            Value::List(l) => l,
            _ => {
                return Err(self.err(
                    &format!("'{}' is not a list", node.list_name),
                    ln,
                    Some(node.list_name.as_str()),
                    None,
                ));
            }
        };
        if scope.has(&node.variable) {
            return Err(self.err(
                &format!("the name '{}' already exists", node.variable),
                ln,
                Some(node.variable.as_str()),
                None,
            ));
        }
        let mut position_warned = false;
        for (index, item) in list.iter().enumerate() {
            // Ids and taps inside the item carry the path of the repetition. It is written with a
            // marker first, because the key of the item is only known after the item is emitted.
            let marker = format!("\u{1}{}\u{2}", self.scope_counter);
            self.scope_counter += 1;
            let child_path = if scope.path.is_empty() {
                marker.clone()
            } else {
                format!("{}/{}", scope.path, marker)
            };
            let first = self.out.len();
            let log_first = self.scope_log.len();
            let hits = self.marker_hits;
            let mut vars: HashMap<String, Value> = HashMap::new();
            vars.insert(node.variable.clone(), item.clone());
            let child = Rc::new(Scope { parent: Some(Rc::clone(scope)), vars, path: child_path });
            self.emit(&node.nodes, &child)?;
            let item_key = scopes::key_of_item(item);
            if self.marker_hits != hits {
                let author_key = self.first_level_key(first);
                let key_ref: Option<&str> = author_key.as_deref().or(item_key.as_deref());
                let segment = format!("{}:{}", node.variable, scopes::token(key_ref, index));
                for k in first..self.out.len() {
                    if self.out[k].contains('\u{1}') {
                        self.out[k] = self.out[k].replace(&marker, &segment);
                    }
                }
                for k in log_first..self.scope_log.len() {
                    if self.scope_log[k].0.contains('\u{1}') {
                        let replaced = self.scope_log[k].0.replace(&marker, &segment);
                        self.scope_log[k].0 = replaced;
                    }
                }
                let path = if scope.path.is_empty() {
                    segment
                } else {
                    format!("{}/{}", scope.path, segment)
                };
                self.scope_log.push((path, item.clone()));
            }
            let keyed = self.apply_key(first, item_key.as_deref());
            if !keyed && !position_warned && self.holds_fields_or_ids(first) {
                position_warned = true;
                let w = LayoutWarning {
                    message: "the items of this list have no id: using the position".to_string(),
                    line: ln,
                };
                if !self.warnings.iter().any(|x| x.message == w.message && x.line == w.line) {
                    self.warnings.push(w);
                }
            }
        }
        Ok(())
    }

    fn emit_include(&mut self, node: &IncludeNode, scope: &Rc<Scope>) -> Res<()> {
        let ln = self.eff_line(node.line);
        let mut given: HashMap<String, Value> = HashMap::new();
        for p in &node.params {
            let v = self.param_value(p, ln, scope)?;
            given.insert(p.name.clone(), v);
        }
        let key_text: Option<String> = match &node.key {
            Some(parts) => {
                let t = self.raw_text(parts, ln, scope)?.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            }
            None => None,
        };
        let first = self.out.len();
        let saved_component = self.current_component.clone();
        let saved_line = self.current_include_line;
        if saved_component.is_none() {
            self.current_include_line = node.line;
        }
        self.current_component = Some(node.component.name.clone());
        let child = Rc::new(Scope { parent: None, vars: given, path: scope.path.clone() });
        let result = {
            let comp_nodes = node.component.nodes.borrow();
            self.emit(&comp_nodes, &child)
        };
        self.current_component = saved_component;
        self.current_include_line = saved_line;
        result?;
        if let Some(k) = key_text {
            self.apply_key(first, Some(k.as_str()));
        }
        Ok(())
    }

    /// The value of a parameter of an `<include>`: a name is handed over as it is, whatever its type.
    fn param_value(&mut self, p: &ParamPlan, line: usize, scope: &Rc<Scope>) -> Res<Value> {
        if let Some(single) = &p.single {
            let parts: Vec<String> = single.split('.').map(str::to_string).collect();
            return self.lookup_path(single, &parts, line, scope);
        }
        if let Some(parts) = &p.parts {
            return Ok(Value::Str(self.raw_text(parts, line, scope)?));
        }
        Ok(p.constant.clone())
    }

    /// Parts joined as plain text, with no escaping: it is a value for the program side, not HTML.
    fn raw_text(&mut self, parts: &[Part], line: usize, scope: &Rc<Scope>) -> Res<String> {
        let mut sb = String::new();
        for p in parts {
            match p {
                Part::Lit(s) => sb.push_str(s),
                Part::Val(v) => sb.push_str(&self.val_text(v, line, scope, false)?),
                Part::Dyn(_) => {}
            }
        }
        Ok(sb)
    }

    /// The text of a `{value}`. `escape` is true for the HTML that is emitted, false for raw text.
    fn val_text(&self, v: &ValPart, line: usize, scope: &Scope, escape: bool) -> Res<String> {
        if !v.ok {
            return Err(self.err(
                &format!(
                    "arithmetic is not supported in '{{{}}}'; give the program's result as a name",
                    v.inner
                ),
                line,
                None,
                None,
            ));
        }
        let name = v.name.as_str();
        let x = self.lookup_path(name, &v.path, line, scope)?;
        Ok(match &x {
            Value::Null => String::new(),
            Value::Str(s) => {
                if escape {
                    escape_html(s)
                } else {
                    s.clone()
                }
            }
            Value::Bool(b) => b.to_string(),
            Value::Int(_) | Value::Float(_) => format_number(&x),
            Value::List(_) => {
                return Err(self.err(
                    &format!("'{{{}}}' is a list; use 'for {} in {}:'", name, singular(name), name),
                    line,
                    Some(name),
                    None,
                ));
            }
            Value::Map(_) => {
                return Err(self.err(
                    &format!("'{{{}}}' is a map; use a name inside it, like '{{{}.field}}'", name, name),
                    line,
                    Some(name),
                    None,
                ));
            }
        })
    }

    /// The `key` that the author wrote on an element at the first level of what an item emitted, if any.
    fn first_level_key(&self, first: usize) -> Option<String> {
        let mut depth: i32 = 0;
        for k in first..self.out.len() {
            for m in rx().tag.find_iter(&self.out[k]) {
                let tag = m.as_str();
                if tag.starts_with("<!") {
                    continue;
                }
                if tag.starts_with("</") {
                    depth -= 1;
                    continue;
                }
                let name = match rx().tag_name.captures(tag) {
                    Some(c) => c[1].to_lowercase(),
                    None => continue,
                };
                // a key that an inner `for` wrote by itself is not the author's key
                if depth == 0 && !self.auto_keyed.contains(&k) {
                    if let Some(c) = rx().key_attr.captures(tag) {
                        return Some(c[1].to_string());
                    }
                }
                if !VOID_NAMES.contains(&name.as_str()) && !tag.ends_with("/>") {
                    depth += 1;
                }
            }
        }
        None
    }

    /// Writes `key="..."` on the elements at the first level of what one item of a `for` emitted,
    /// unless the author already wrote a key. Returns false when there was no key to give.
    fn apply_key(&mut self, first: usize, key: Option<&str>) -> bool {
        let key = match key {
            Some(k) => k,
            None => return false,
        };
        let escaped = escape_html(key);
        let mut depth: i32 = 0;
        for k in first..self.out.len() {
            let line = self.out[k].clone();
            let mut rebuilt: Option<String> = None;
            let mut copied = 0usize;
            for m in rx().tag.find_iter(&line) {
                let tag = m.as_str();
                if tag.starts_with("<!") {
                    continue;
                }
                if tag.starts_with("</") {
                    depth -= 1;
                    continue;
                }
                let name = match rx().tag_name.captures(tag) {
                    Some(c) => c[1].to_lowercase(),
                    None => continue,
                };
                if depth == 0 && !rx().has_key.is_match(tag) {
                    let b = rebuilt.get_or_insert_with(String::new);
                    b.push_str(&line[copied..m.start()]);
                    b.push_str(&tag[..name.len() + 1]);
                    b.push_str(" key=\"");
                    b.push_str(&escaped);
                    b.push('"');
                    b.push_str(&tag[name.len() + 1..]);
                    copied = m.end();
                }
                if !VOID_NAMES.contains(&name.as_str()) && !tag.ends_with("/>") {
                    depth += 1;
                }
            }
            if let Some(mut b) = rebuilt {
                b.push_str(&line[copied..]);
                self.out[k] = b;
                self.auto_keyed.insert(k);
            }
        }
        true
    }

    /// True when what an item emitted has an `id` or a field, that is, something that follows the item.
    fn holds_fields_or_ids(&self, first: usize) -> bool {
        (first..self.out.len()).any(|k| rx().field_or_id.is_match(&self.out[k]))
    }

    fn lookup(&self, name: &str, line: usize, scope: &Scope) -> Res<Value> {
        let parts: Vec<String> = name.split('.').map(str::to_string).collect();
        self.lookup_path(name, &parts, line, scope)
    }

    /// A name that was not given: by the program, or, inside a component, by the `<include>`.
    fn missing(&self, name: &str, line: usize, word: &str) -> NativeLayoutError {
        match &self.current_component {
            None => self.err(
                &format!("the name '{}' was not given by the program", name),
                line,
                Some(word),
                None,
            ),
            Some(c) => NativeLayoutError::expand(
                &format!("include: the name '{}' was not given to '{}'", name, c),
                self.current_include_line,
                self.source,
                Some(word),
                None,
            ),
        }
    }

    fn lookup_path(&self, name: &str, parts: &[String], line: usize, scope: &Scope) -> Res<Value> {
        let mut value: Value = match scope.find(&parts[0]) {
            Some(v) => v.clone(),
            None => return Err(self.missing(name, line, &parts[0])),
        };
        for part in &parts[1..] {
            let current = value;
            value = match &current {
                Value::Map(m) if m.contains_key(part.as_str()) => m[part.as_str()].clone(),
                Value::Map(m) if part == "size" => Value::Int(m.len() as i64),
                Value::List(l) if part == "size" => Value::Int(l.len() as i64),
                Value::Str(s) if part == "size" => Value::Int(s.encode_utf16().count() as i64),
                _ => return Err(self.missing(name, line, &parts[0])),
            };
        }
        Ok(value)
    }

    fn tokenize(&self, text: &str, line: usize) -> Res<Vec<String>> {
        let mut tokens: Vec<String> = Vec::new();
        let mut pos = 0usize;
        while pos < text.len() {
            let ch = text[pos..].chars().next().unwrap();
            if ch.is_whitespace() {
                pos += ch.len_utf8();
                continue;
            }
            match rx().token.find_at(text, pos) {
                Some(m) if m.start() == pos => {
                    tokens.push(m.as_str().to_string());
                    pos = m.end();
                }
                _ => return Err(self.err(&format!("invalid condition: '{}'", text), line, None, None)),
            }
        }
        Ok(tokens)
    }

    fn evaluate(&self, branch: &Branch, condition: &str, scope: &Rc<Scope>) -> Res<bool> {
        let tokens = self.tokenize(condition, branch.line)?;
        let mut p = CondParser { ex: self, tokens, text: condition, line: branch.line, scope, pos: 0 };
        p.run()
    }
}

struct CondParser<'e, 'a> {
    ex: &'e ScreenExpander<'a>,
    tokens: Vec<String>,
    text: &'e str,
    line: usize,
    scope: &'e Scope,
    pos: usize,
}

impl<'e, 'a> CondParser<'e, 'a> {
    fn run(&mut self) -> Res<bool> {
        let value = self.parse_or()?;
        if self.pos != self.tokens.len() {
            return Err(self.bad());
        }
        Ok(value)
    }

    fn bad(&self) -> NativeLayoutError {
        self.ex.err(&format!("invalid condition: '{}'", self.text), self.line, None, None)
    }

    fn peek(&self) -> Option<String> {
        self.tokens.get(self.pos).cloned()
    }

    fn peek_is(&self, s: &str) -> bool {
        self.tokens.get(self.pos).map(|t| t == s).unwrap_or(false)
    }

    fn parse_or(&mut self) -> Res<bool> {
        let mut v = self.parse_and()?;
        while self.peek_is("or") {
            self.pos += 1;
            let r = self.parse_and()?;
            v = v || r;
        }
        Ok(v)
    }

    fn parse_and(&mut self) -> Res<bool> {
        let mut v = self.parse_not()?;
        while self.peek_is("and") {
            self.pos += 1;
            let r = self.parse_not()?;
            v = v && r;
        }
        Ok(v)
    }

    fn parse_not(&mut self) -> Res<bool> {
        if self.peek_is("not") {
            self.pos += 1;
            return Ok(!self.parse_not()?);
        }
        if self.peek_is("(") {
            self.pos += 1;
            let v = self.parse_or()?;
            if !self.peek_is(")") {
                return Err(self.bad());
            }
            self.pos += 1;
            return Ok(v);
        }
        self.parse_compare()
    }

    fn parse_compare(&mut self) -> Res<bool> {
        let left = self.operand()?;
        if let Some(op) = self.peek() {
            if COMPARATORS.contains(&op.as_str()) {
                self.pos += 1;
                let right = self.operand()?;
                return self.compare(&op, &left, &right);
            }
        }
        if let Value::Bool(b) = left {
            return Ok(b);
        }
        Err(self.ex.err(
            &format!("invalid condition: '{}'", self.text),
            self.line,
            None,
            Some("a name alone must be true or false"),
        ))
    }

    fn operand(&mut self) -> Res<Value> {
        let token = match self.peek() {
            Some(t) => t,
            None => return Err(self.bad()),
        };
        self.pos += 1;
        if token == "true" {
            return Ok(Value::Bool(true));
        }
        if token == "false" {
            return Ok(Value::Bool(false));
        }
        if token.starts_with('"') {
            return Ok(Value::Str(token[1..token.len() - 1].to_string()));
        }
        let first = token.chars().next().unwrap_or(' ');
        if first.is_ascii_digit() || first == '-' {
            return match token.parse::<f64>() {
                Ok(d) => Ok(Value::Float(d)),
                Err(_) => Err(self.bad()),
            };
        }
        if token == "and"
            || token == "or"
            || token == "not"
            || token == "("
            || token == ")"
            || COMPARATORS.contains(&token.as_str())
        {
            return Err(self.bad());
        }
        self.ex.lookup(&token, self.line, self.scope)
    }

    fn compare(&self, op: &str, a: &Value, b: &Value) -> Res<bool> {
        if let (Some(x), Some(y)) = (a.as_number(), b.as_number()) {
            return Ok(match op {
                "==" => x == y,
                "!=" => x != y,
                "<" => x < y,
                ">" => x > y,
                "<=" => x <= y,
                _ => x >= y,
            });
        }
        if op == "==" {
            return Ok(a == b);
        }
        if op == "!=" {
            return Ok(a != b);
        }
        if let (Value::Str(x), Value::Str(y)) = (a, b) {
            let c = x.cmp(y);
            return Ok(match op {
                "<" => c.is_lt(),
                ">" => c.is_gt(),
                "<=" => c.is_le(),
                _ => c.is_ge(),
            });
        }
        Err(self.ex.err(
            &format!("invalid condition: '{}'", self.text),
            self.line,
            None,
            Some(&format!("these values cannot be compared with '{}'", op)),
        ))
    }
}

// ---------- compiling lines into parts, and rendering them ----------

impl<'a> ScreenExpander<'a> {
    fn compile_text(&self, text: &str, list: &mut Vec<Part>) {
        let mut pos = 0usize;
        for m in rx().brace.captures_iter(text) {
            let whole = m.get(0).unwrap();
            if whole.start() > pos {
                list.push(Part::Lit(text[pos..whole.start()].to_string()));
            }
            match whole.as_str() {
                "\\{" => list.push(Part::Lit("{".to_string())),
                "\\}" => list.push(Part::Lit("}".to_string())),
                _ => list.push(Part::Val(ValPart::new(&m[1]))),
            }
            pos = whole.end();
        }
        if pos < text.len() {
            list.push(Part::Lit(text[pos..].to_string()));
        }
    }

    fn compile_value(&mut self, text: &str) -> Res<Vec<Part>> {
        let mut list: Vec<Part> = Vec::new();
        self.compile_text(text, &mut list);
        Ok(merge_lits(list))
    }

    fn compile_line(&mut self, text: &str) -> Res<Vec<Part>> {
        let mut list: Vec<Part> = Vec::new();
        let mut pos = 0usize;
        let found: Vec<(usize, usize, String)> = rx()
            .tag
            .find_iter(text)
            .map(|m| (m.start(), m.end(), m.as_str().to_string()))
            .collect();
        for (start, end, tag) in found {
            self.compile_text(&text[pos..start], &mut list);
            let part = self.compile_tag(&tag)?;
            list.push(part);
            pos = end;
        }
        self.compile_text(&text[pos..], &mut list);
        Ok(merge_lits(list))
    }

    fn render_parts(&mut self, parts: &[Part], line: usize, scope: &Rc<Scope>) -> Res<String> {
        if parts.len() == 1 {
            if let Part::Lit(s) = &parts[0] {
                return Ok(s.clone());
            }
        }
        let mut sb = String::new();
        for p in parts {
            match p {
                Part::Lit(s) => sb.push_str(s),
                Part::Val(v) => sb.push_str(&self.val_text(v, line, scope, true)?),
                Part::Dyn(d) => match &d.pre {
                    Some(pre) if scope.path.is_empty() => sb.push_str(pre),
                    _ => {
                        let r = self.render_dyn(d, line, scope)?;
                        sb.push_str(&r);
                    }
                },
            }
        }
        Ok(sb)
    }

    fn style_for(&self, tag: &str, classes: &[String], id: Option<&str>, state: i32) -> String {
        let mut matched: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| {
                r.state == state
                    && match r.kind {
                        KIND_TAG => r.name == tag,
                        KIND_CLASS => classes.iter().any(|c| *c == r.name),
                        _ => id.map(|i| i == r.name).unwrap_or(false),
                    }
            })
            .collect();
        matched.sort_by_key(|r| (r.kind, r.order));
        let mut parts: Vec<String> = Vec::new();
        for rule in matched {
            for d in &rule.resolved {
                parts.push(format!("{}: {}", d.0, d.1));
            }
        }
        parts.join("; ")
    }

    fn compile_tag(&mut self, tag: &str) -> Res<Part> {
        if tag.starts_with("<!") {
            return Ok(Part::Lit(tag.to_string()));
        }
        if tag.starts_with("</") {
            let closing = tag[2..tag.len() - 1].trim().to_lowercase();
            return Ok(Part::Lit(if closing == "body" { "</div>".to_string() } else { tag.to_string() }));
        }
        let name_match = match rx().tag_name.captures(tag) {
            Some(m) => m,
            None => return Ok(Part::Lit(tag.to_string())),
        };
        let name = name_match[1].to_string();
        let name_end = name_match.get(0).unwrap().end();
        let lower = name.to_lowercase();
        let self_close = tag.ends_with("/>");
        let inner = &tag[name_end..tag.len() - if self_close { 2 } else { 1 }];

        let mut attrs: Vec<AttrPlan> = Vec::new();
        let mut classes: Vec<String> = Vec::new();
        let mut id: Option<String> = None;
        let mut any_brace = false;
        for a in rx().attr.captures_iter(inner) {
            let key = a[1].to_lowercase();
            let value: Option<String> = a
                .get(2)
                .or_else(|| a.get(3))
                .or_else(|| a.get(4))
                .map(|m| m.as_str().to_string());
            let brace = value.as_ref().map(|v| v.contains('{')).unwrap_or(false);
            let mut single: Option<String> = None;
            let mut text_parts: Option<Vec<Part>> = None;
            if brace {
                any_brace = true;
                let v = value.clone().unwrap_or_default();
                single = rx().single_value.captures(v.trim()).map(|c| c[1].to_string());
                text_parts = Some(self.compile_value(&v)?);
            } else {
                if key == "class" {
                    classes = value
                        .clone()
                        .unwrap_or_default()
                        .split(' ')
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect();
                }
                if key == "id" {
                    id = value.clone();
                }
            }
            attrs.push(AttrPlan { orig: a[1].to_string(), key, value, brace, single, text_parts });
        }
        let id_ref = id.as_deref();
        let mut plan = DynTag {
            name,
            lower: lower.clone(),
            self_close,
            rule_style: self.style_for(&lower, &classes, id_ref, STATE_NONE),
            attrs,
            state_styles: [
                self.style_for(&lower, &classes, id_ref, STATE_PRESSED),
                self.style_for(&lower, &classes, id_ref, STATE_DISABLED),
                self.style_for(&lower, &classes, id_ref, STATE_FOCUS),
            ],
            pre: None,
        };
        if any_brace {
            return Ok(Part::Dyn(plan));
        }
        let empty = Rc::new(Scope::empty());
        let pre = self.render_dyn(&plan, 0, &empty)?;
        // an id or a class may need the path of a repetition, which is only known when the line is emitted
        if plan.attrs.iter().any(|a| a.key == "id" || a.key == "class") {
            plan.pre = Some(pre);
            return Ok(Part::Dyn(plan));
        }
        Ok(Part::Lit(pre))
    }

    fn render_dyn(&mut self, t: &DynTag, line: usize, scope: &Rc<Scope>) -> Res<String> {
        let mut result: Vec<Attr> = Vec::with_capacity(t.attrs.len() + 1);
        let mut inline = String::new();
        let mut scoped = false;
        for a in &t.attrs {
            let mut value: Option<String> = a.value.clone();
            if a.brace {
                if a.key == "id" || a.key == "class" {
                    return Err(self.err(
                        &format!("markup: '{}' cannot be generated by value", a.key),
                        line,
                        Some(a.key.as_str()),
                        None,
                    ));
                }
                if (a.key == "checked" || a.key == "disabled" || a.key == "readonly") && a.single.is_some() {
                    let single = a.single.clone().unwrap_or_default();
                    let flag = self.lookup(&single, line, scope)?;
                    if flag == Value::Bool(true) {
                        result.push(Attr { key: a.orig.clone(), value: Some("true".to_string()) });
                    }
                    continue;
                }
                let parts = a.text_parts.as_ref().unwrap();
                value = Some(self.render_parts(parts, line, scope)?);
            }
            if (a.key == "id" || a.key == "class") && !scope.path.is_empty() {
                scoped = true;
                if a.key == "id" {
                    if let Some(v) = &value {
                        value = Some(format!("{}{}{}", v, scopes::SEP, scope.path));
                    }
                }
            }
            if a.key == "style" {
                inline = value.unwrap_or_default();
            } else {
                result.push(Attr { key: a.orig.clone(), value });
            }
        }
        if scoped {
            // where this element sits among the repetitions: the program asks "the item of the tap" with it
            result.push(Attr { key: "scope".to_string(), value: Some(scope.path.clone()) });
            self.marker_hits += 1;
        }

        let mut parts: Vec<String> = Vec::new();
        if !t.rule_style.is_empty() {
            parts.push(t.rule_style.clone());
        }
        let inline_style = inline.trim().trim_end_matches(';').trim().to_string();
        if !inline_style.is_empty() {
            parts.push(inline_style);
        }
        if !parts.is_empty() {
            result.push(Attr { key: "style".to_string(), value: Some(parts.join("; ")) });
        }
        for k in 0..3 {
            if !t.state_styles[k].is_empty() {
                result.push(Attr {
                    key: STATE_ATTRIBUTES[k].to_string(),
                    value: Some(t.state_styles[k].clone()),
                });
            }
        }

        let mut sb = String::from("<");
        sb.push_str(if t.lower == "body" { "div" } else { t.name.as_str() });
        for a in &result {
            sb.push(' ');
            sb.push_str(&a.key);
            if let Some(v) = &a.value {
                sb.push_str("=\"");
                sb.push_str(&v.replace('"', "&quot;"));
                sb.push('"');
            }
        }
        sb.push_str(if t.self_close { " />" } else { ">" });
        Ok(sb)
    }
}
