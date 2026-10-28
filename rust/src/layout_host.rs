//! Port of LayoutHost.kt: what a host must offer, and the session that drives it.
//!
//! Differences from Kotlin: `nextEvent` was a `suspend fun`; here `next_event` simply blocks until
//! the host has an event (the host decides how). `commit` and `render` are plain functions.

use std::collections::HashMap;

use crate::differ::{self, Change};
use crate::error::NativeLayoutError;
use crate::layout_engine::{self, LayoutWarning};
use crate::scopes;
use crate::tree::Native;
use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutEvent {
    pub key: String,
}

/// Key of the event the host sends when the system back button is pressed. It belongs to no element.
pub const BACK_EVENT_KEY: &str = "\u{0}back";

pub trait LayoutHost {
    fn apply_changes(&mut self, changes: &[Change]);

    /// Waits for the next event. `None` means the host was closed.
    fn next_event(&mut self) -> Option<LayoutEvent>;

    fn field_text(&self, id: &str) -> Option<String>;

    fn field_checked(&self, id: &str) -> Option<bool>;
    fn set_value(&mut self, id: &str, text: &str);

    fn save(&mut self, key: &str, text: &str);
    fn load(&self, key: &str) -> Option<String>;
    fn exists(&self, key: &str) -> bool;
    fn delete(&mut self, key: &str);

    fn close(&mut self);

    /// Shows an error in place of the screen. Hosts that cannot do it keep the default (nothing).
    fn show_error(&mut self, _text: &str) {}

    /// Makes the next `next_event` return the back event (`BACK_EVENT_KEY`).
    fn post_back(&mut self) {}
}

/// A boxed host is a host, so the program side does not need to be generic over it.
impl LayoutHost for Box<dyn LayoutHost> {
    fn apply_changes(&mut self, changes: &[Change]) {
        (**self).apply_changes(changes)
    }
    fn next_event(&mut self) -> Option<LayoutEvent> {
        (**self).next_event()
    }
    fn field_text(&self, id: &str) -> Option<String> {
        (**self).field_text(id)
    }
    fn field_checked(&self, id: &str) -> Option<bool> {
        (**self).field_checked(id)
    }
    fn set_value(&mut self, id: &str, text: &str) {
        (**self).set_value(id, text)
    }
    fn save(&mut self, key: &str, text: &str) {
        (**self).save(key, text)
    }
    fn load(&self, key: &str) -> Option<String> {
        (**self).load(key)
    }
    fn exists(&self, key: &str) -> bool {
        (**self).exists(key)
    }
    fn delete(&mut self, key: &str) {
        (**self).delete(key)
    }
    fn close(&mut self) {
        (**self).close()
    }
    fn show_error(&mut self, text: &str) {
        (**self).show_error(text)
    }
    fn post_back(&mut self) {
        (**self).post_back()
    }
}

#[derive(Debug, Clone)]
pub struct FilledMarkup {
    pub html: String,
    pub line: usize,
    pub variables: HashMap<String, String>,
}

impl FilledMarkup {
    pub fn new(html: &str, line: usize) -> FilledMarkup {
        FilledMarkup { html: html.to_string(), line, variables: HashMap::new() }
    }
}

/// Result of `LayoutSession::prepare`: the new tree, its indexes and the list of changes against the
/// tree that is currently on screen. It is plain data, so it can be built on any thread.
#[derive(Debug, Clone)]
pub struct PreparedRender {
    root: Native,
    by_key: HashMap<String, Native>,
    by_id: HashMap<String, Native>,
    changes: Vec<Change>,
    warnings: Vec<LayoutWarning>,
}

pub struct LayoutSession<H: LayoutHost> {
    host: H,
    tree: Option<Native>,
    by_key: HashMap<String, Native>,
    by_id: HashMap<String, Native>,
    last_event: Option<LayoutEvent>,
    clicked_index: i32,
    closed: bool,
    pub warnings: Vec<LayoutWarning>,
    items: HashMap<String, Value>,
}

impl<H: LayoutHost> LayoutSession<H> {
    pub fn new(host: H) -> LayoutSession<H> {
        LayoutSession {
            host,
            tree: None,
            by_key: HashMap::new(),
            by_id: HashMap::new(),
            last_event: None,
            clicked_index: -1,
            closed: false,
            warnings: Vec::new(),
            items: HashMap::new(),
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    /// Heavy part: build the tree, index it and diff it against the tree on screen.
    /// It does not touch the host. Calls to `prepare` and `commit` must alternate.
    pub fn prepare(&self, m: &FilledMarkup) -> Result<PreparedRender, NativeLayoutError> {
        let r = layout_engine::build(&m.html, m.line, &m.variables)?;
        let changes = differ::diff(self.tree.as_ref(), &r.root);
        let mut map: HashMap<String, Native> = HashMap::new();
        index(&r.root, &mut map);
        Ok(PreparedRender {
            root: r.root,
            by_key: map,
            by_id: r.by_id,
            changes,
            warnings: r.warnings,
        })
    }

    /// Light part: apply the changes to the views and wait for the next event.
    /// Gives `false` when the host was closed.
    pub fn commit(&mut self, p: PreparedRender) -> bool {
        if self.closed {
            return false;
        }
        for a in &p.warnings {
            if !self.warnings.iter().any(|w| w.message == a.message && w.line == a.line) {
                self.warnings.push(a.clone());
            }
        }

        if !p.changes.is_empty() {
            self.host.apply_changes(&p.changes);
        }

        self.tree = Some(p.root);
        self.by_key = p.by_key;
        self.by_id = p.by_id;
        self.last_event = None;
        self.clicked_index = -1;

        match self.host.next_event() {
            None => {
                self.closed = true;
                self.tree = None;
                self.host.close();
                false
            }
            Some(event) => {
                self.last_event = Some(event);
                true
            }
        }
    }

    /// Prepare and commit on the calling thread.
    pub fn render(&mut self, m: &FilledMarkup) -> Result<bool, NativeLayoutError> {
        let p = self.prepare(m)?;
        Ok(self.commit(p))
    }

    // ---- identity inside a `for` (section 5.7)

    /// The items of the repetitions of the screen on show, by path (from `ExpandedScreen::items`).
    pub fn set_scope_items(&mut self, map: HashMap<String, Value>) {
        self.items = map;
    }

    /// The path of the repetition that holds the tapped element, or `None`.
    fn tapped_scope(&self) -> Option<String> {
        let ev = self.last_event.as_ref()?;
        self.by_key.get(&ev.key)?.scope.clone()
    }

    /// The item of the `for [name] in ...` that holds the tapped element.
    /// Gives an error message when the tap was not inside that `for`.
    pub fn clicked_item(&self, name: &str) -> Result<Value, String> {
        let path = match self.tapped_scope() {
            Some(p) => p,
            None => return Err(format!("clicked('{}'): the tapped element is not inside a 'for'", name)),
        };
        let segments: Vec<&str> = path.split('/').collect();
        for i in (0..segments.len()).rev() {
            if scopes::variable_of(segments[i]) == name {
                let prefix = segments[..=i].join("/");
                return match self.items.get(&prefix) {
                    Some(v) => Ok(v.clone()),
                    None => break,
                };
            }
        }
        Err(format!("clicked('{}'): the tapped element is not inside 'for {} in ...'", name, name))
    }

    /// The id of the element `id` of the repetition given by `given` (one item for each enclosing `for`,
    /// outermost first). Without items, `id` itself when it is not repeated. Gives an error message
    /// when the items do not name a repetition.
    pub fn scoped_id(&self, id: &str, given: &[Value]) -> Result<Option<String>, String> {
        if given.is_empty() {
            if self.by_id.contains_key(id) {
                return Ok(Some(id.to_string()));
            }
            let prefix = format!("{}{}", id, scopes::SEP);
            let mut keys: Vec<&String> = self.by_id.keys().collect();
            keys.sort();
            let sample = match keys.into_iter().find(|k| k.starts_with(&prefix)) {
                Some(s) => s,
                None => return Ok(None),
            };
            let rest = &sample[prefix.len()..];
            let depth = rest.split('/').count();
            let vars: Vec<&str> = rest.split('/').map(scopes::variable_of).collect();
            return Err(format!(
                "'{}' is repeated: give {} item(s), one for each of: {}",
                id,
                depth,
                vars.join(", ")
            ));
        }
        let prefix = format!("{}{}", id, scopes::SEP);
        let mut candidates: Vec<&String> = self.by_id.keys().filter(|k| k.starts_with(&prefix)).collect();
        if candidates.is_empty() {
            return Ok(None);
        }
        candidates.sort();
        for c in candidates {
            let segments: Vec<&str> = c[prefix.len()..].split('/').collect();
            if segments.len() != given.len() {
                continue;
            }
            let mut ok = true;
            let mut path = String::new();
            for k in 0..segments.len() {
                if k == 0 {
                    path = segments[0].to_string();
                } else {
                    path = format!("{}/{}", path, segments[k]);
                }
                match self.items.get(&path) {
                    Some(held) if scopes::same_item(&given[k], held, scopes::token_of(segments[k])) => {}
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return Ok(Some(c.clone()));
            }
        }
        Ok(None)
    }

    pub fn click(&mut self, target: &str) -> bool {
        let ev = match &self.last_event {
            Some(e) => e.clone(),
            None => return false,
        };
        let node = match self.by_key.get(&ev.key) {
            Some(n) => n,
            None => return false,
        };
        if let Some(cls) = target.strip_prefix('.') {
            if !node.classes.iter().any(|c| c == cls) {
                return false;
            }
            let mut all: Vec<&Native> = Vec::new();
            if let Some(t) = &self.tree {
                collect_class(t, cls, &mut all);
            }
            self.clicked_index = all.iter().position(|n| n.key == node.key).map(|p| p as i32).unwrap_or(-1);
            return true;
        }
        let matches_id = match &node.id {
            Some(nid) => {
                nid == target
                    || (node.scope.is_some() && nid.split(scopes::SEP).next().unwrap_or(nid) == target)
            }
            None => false,
        };
        if matches_id {
            self.clicked_index = -1;
            return true;
        }
        false
    }

    pub fn click_index(&self) -> i32 {
        self.clicked_index
    }

    /// True when the event that woke the program is the system back button.
    pub fn is_back(&self) -> bool {
        matches!(&self.last_event, Some(e) if e.key == BACK_EVENT_KEY)
    }

    /// Forgets the tree on screen: the next render builds everything from scratch.
    pub fn reset(&mut self) {
        self.tree = None;
    }

    pub fn field_text(&self, id: &str, given: &[Value]) -> Result<Option<String>, String> {
        Ok(self.scoped_id(id, given)?.and_then(|s| self.host.field_text(&s)))
    }

    pub fn field_checked(&self, id: &str, given: &[Value]) -> Result<Option<bool>, String> {
        Ok(self.scoped_id(id, given)?.and_then(|s| self.host.field_checked(&s)))
    }

    pub fn set_value(&mut self, id: &str, text: &str, given: &[Value]) -> Result<(), String> {
        if let Some(s) = self.scoped_id(id, given)? {
            self.host.set_value(&s, text);
        }
        Ok(())
    }

    pub fn save(&mut self, key: &str, text: &str) {
        self.host.save(key, text)
    }

    pub fn load(&self, key: &str) -> Option<String> {
        self.host.load(key)
    }

    pub fn exists(&self, key: &str) -> bool {
        self.host.exists(key)
    }

    pub fn delete(&mut self, key: &str) {
        self.host.delete(key)
    }
}

fn index(n: &Native, result: &mut HashMap<String, Native>) {
    result.insert(n.key.clone(), n.clone());
    for f in n.native_children() {
        index(f, result);
    }
}

fn collect_class<'a>(n: &'a Native, cls: &str, result: &mut Vec<&'a Native>) {
    if n.classes.iter().any(|c| c == cls) {
        result.push(n);
    }
    for f in n.native_children() {
        collect_class(f, cls, result);
    }
}
