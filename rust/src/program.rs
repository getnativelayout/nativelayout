//! Port of NativeView.kt (and of `program.py`): the verbs of the program side, on top of the engine.
//!
//! It does not depend on the platform. Each host gives a `LayoutHost`, a `loader` and the dark theme.
//!
//! Two styles, as in the spec:
//!
//! - **screen functions**: `fn panel(ui: &mut NativeView) -> Page { let s = ui.state(|| 0); ...; ui.view("panel.nl", data) }`.
//!   The function is called again after every tap, until the window closes (`NativeView::run`).
//! - **base layer**: `while ui.render(&screen, &data)? { if ui.click("add") { ... } }` (`NativeView::run_body`).
//!
//! Differences from Kotlin, which is asynchronous (coroutines and a worker thread):
//! - Everything is synchronous. `LayoutHost::next_event` blocks until the host has an event.
//! - A verb that fails (`view` of a file that does not exist, `clicked` outside a `for`, a field of a
//!   repeated element given without its items) does not return an error, so screen functions stay
//!   short. The first error is kept, and the loop returns it as soon as the function returns.
//! - `state` gives an `Rc<RefCell<T>>` instead of the object itself.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::error::NativeLayoutError;
use crate::expander;
use crate::layout_engine::LayoutWarning;
use crate::layout_host::{FilledMarkup, LayoutHost, LayoutSession};
use crate::value::Value;

const MAX_HOPS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum ProgramError {
    /// A mistake in a `.nl` file or in the markup.
    Layout(NativeLayoutError),
    /// A mistake of the program: screen not found, too many jumps, no screen to return to...
    Program(String),
}

impl ProgramError {
    /// The text shown in place of the screen.
    pub fn message(&self) -> String {
        match self {
            ProgramError::Layout(e) => e.format(),
            ProgramError::Program(m) => m.clone(),
        }
    }
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::Layout(e) => write!(f, "{}", e.message),
            ProgramError::Program(m) => write!(f, "{}", m),
        }
    }
}

impl std::error::Error for ProgramError {}

impl From<NativeLayoutError> for ProgramError {
    fn from(e: NativeLayoutError) -> ProgramError {
        ProgramError::Layout(e)
    }
}

/// A screen file, read once. `NativeView::screen` makes one.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    source: String,
}

enum PageKind {
    View { screen: Rc<Screen>, data: HashMap<String, Value> },
    Go { next: ScreenFn, root: bool },
    Back,
    /// A verb failed; the error is waiting in the `NativeView`.
    Failed,
}

/// What a screen function returns: a screen to show (`view`) or a jump to another one (`go`, `go_back`).
pub struct Page(PageKind);

/// A screen function: it is called again after every tap.
pub type ScreenFn = Rc<dyn Fn(&mut NativeView) -> Page>;

/// Makes a `ScreenFn` from a closure. Write the type of the argument: `|ui: &mut NativeView| ...`.
pub fn screen_fn<F: Fn(&mut NativeView) -> Page + 'static>(f: F) -> ScreenFn {
    Rc::new(f)
}

pub struct NativeView {
    session: LayoutSession<Box<dyn LayoutHost>>,
    loader: Box<dyn Fn(&str) -> Option<String>>,
    is_dark: Box<dyn Fn() -> bool>,
    closed: bool,
    screens: HashMap<String, Rc<Screen>>,
    slots: Vec<Box<dyn Any>>,
    slot_index: usize,
    fresh: bool,
    /// Screens left behind by `go`: `go_back` returns to the last one. Each one starts again with a new state.
    history: Vec<ScreenFn>,
    back_handled: bool,
    last_shown: Option<(Rc<Screen>, HashMap<String, Value>)>,
    error: Option<ProgramError>,
    /// Called when the system back button is pressed on the first screen and the program did not handle it.
    pub on_exit: Option<Box<dyn FnMut()>>,
}

impl NativeView {
    /// `loader` gives the text of a file by name (screens, components and style files).
    pub fn new(host: Box<dyn LayoutHost>, loader: impl Fn(&str) -> Option<String> + 'static) -> NativeView {
        NativeView {
            session: LayoutSession::new(host),
            loader: Box::new(loader),
            is_dark: Box::new(|| false),
            closed: false,
            screens: HashMap::new(),
            slots: Vec::new(),
            slot_index: 0,
            fresh: false,
            history: Vec::new(),
            back_handled: false,
            last_shown: None,
            error: None,
            on_exit: None,
        }
    }

    /// Tells what `@media (prefers-color-scheme: dark)` follows. It is read on every render.
    pub fn with_dark(mut self, is_dark: impl Fn() -> bool + 'static) -> NativeView {
        self.is_dark = Box::new(is_dark);
        self
    }

    /// The warnings of the engine and of the Expander, without repeats.
    pub fn warnings(&self) -> &[LayoutWarning] {
        &self.session.warnings
    }

    // ---- base layer

    pub fn screen(&mut self, name: &str) -> Result<Screen, ProgramError> {
        match (self.loader)(name) {
            Some(source) => Ok(Screen { name: name.to_string(), source }),
            None => Err(ProgramError::Program(format!("screen not found: {}", name))),
        }
    }

    /// Shows the screen with the data, waits for a tap and returns `true`; `false` when the window closes.
    pub fn render(&mut self, screen: &Screen, data: &HashMap<String, Value>) -> Result<bool, ProgramError> {
        if self.closed {
            return Ok(false);
        }
        let dark = (self.is_dark)();
        let ex = {
            let loader = &self.loader;
            expander::expand(&screen.source, data, &|n| loader(n), dark)?
        };
        let prepared = self.session.prepare(&FilledMarkup::new(&ex.html, 1))?;
        self.session.set_scope_items(ex.items);
        for w in ex.warnings {
            if !self.session.warnings.iter().any(|x| x.message == w.message && x.line == w.line) {
                self.session.warnings.push(w);
            }
        }
        Ok(self.session.commit(prepared))
    }

    // ---- screen functions

    /// State of the active screen function: the same object on every call, a new one after `go`.
    /// Call it at the top, always in the same order, never inside an `if` or a `for`.
    pub fn state<T: 'static>(&mut self, initial: impl FnOnce() -> T) -> Rc<RefCell<T>> {
        let i = self.slot_index;
        self.slot_index += 1;
        if i == self.slots.len() {
            self.slots.push(Box::new(Rc::new(RefCell::new(initial()))));
        }
        match self.slots[i].downcast_ref::<Rc<RefCell<T>>>() {
            Some(rc) => Rc::clone(rc),
            None => panic!("state: it was called in another order, or with another type, than in the previous call"),
        }
    }

    /// The screen to show with its data. The screen file is read once and kept.
    pub fn view(&mut self, name: &str, data: HashMap<String, Value>) -> Page {
        let scr = match self.screens.get(name) {
            Some(s) => Rc::clone(s),
            None => match self.screen(name) {
                Ok(s) => {
                    let rc = Rc::new(s);
                    self.screens.insert(name.to_string(), Rc::clone(&rc));
                    rc
                }
                Err(e) => {
                    self.fail(e);
                    return Page(PageKind::Failed);
                }
            },
        };
        Page(PageKind::View { screen: scr, data })
    }

    /// Jump to another screen function. The state of the current one is dropped, but `go_back` can return to it.
    pub fn go<F: Fn(&mut NativeView) -> Page + 'static>(&self, next: F) -> Page {
        Page(PageKind::Go { next: Rc::new(next), root: false })
    }

    /// Same as `go`, for a function that is already a `ScreenFn`.
    pub fn go_fn(&self, next: ScreenFn) -> Page {
        Page(PageKind::Go { next, root: false })
    }

    /// Jump to another screen function and forget the screens behind (after a login or a logout, for example).
    pub fn go_root<F: Fn(&mut NativeView) -> Page + 'static>(&self, next: F) -> Page {
        Page(PageKind::Go { next: Rc::new(next), root: true })
    }

    pub fn go_root_fn(&self, next: ScreenFn) -> Page {
        Page(PageKind::Go { next, root: true })
    }

    /// The opposite of `go`: returns to the screen that jumped here. On the first screen it does nothing.
    pub fn go_back(&self) -> Page {
        Page(PageKind::Back)
    }

    /// True when there is a screen to return to with `go_back`.
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    /// True when the system back button woke the program. Ask it to handle the button yourself (to close
    /// a dialog, for example). A screen that never asks gets the default: `go_back`, or `on_exit` on the first screen.
    pub fn back(&mut self) -> bool {
        if self.fresh || !self.session.is_back() {
            return false;
        }
        self.back_handled = true;
        true
    }

    /// Call it when the system back button is pressed.
    pub fn press_back(&mut self) {
        if !self.closed {
            self.session.host_mut().post_back();
        }
    }

    /// Runs the screen function until the window closes. An error is shown in place of the screen and returned.
    pub fn run(&mut self, app: ScreenFn) -> Result<(), ProgramError> {
        let r = self.run_app(app);
        self.show_failure(&r);
        r
    }

    /// Runs a body written in the base-layer style. An error is shown in place of the screen and returned.
    pub fn run_body<F: FnOnce(&mut NativeView) -> Result<(), ProgramError>>(&mut self, body: F) -> Result<(), ProgramError> {
        let mut r = body(self);
        if r.is_ok() {
            if let Some(e) = self.error.take() {
                r = Err(e);
            }
        }
        self.show_failure(&r);
        r
    }

    fn show_failure(&mut self, r: &Result<(), ProgramError>) {
        if let Err(e) = r {
            let text = e.message();
            self.session.host_mut().show_error(&text);
        }
    }

    fn fail(&mut self, e: ProgramError) {
        if self.error.is_none() {
            self.error = Some(e);
        }
    }

    fn run_app(&mut self, app: ScreenFn) -> Result<(), ProgramError> {
        let mut current: ScreenFn = app;
        let mut hops = 0usize;
        while !self.closed {
            self.slot_index = 0;
            self.back_handled = false;
            let mut page = current(self);
            if let Some(e) = self.error.take() {
                return Err(e);
            }
            if !self.fresh && !self.back_handled && self.session.is_back() {
                if !self.history.is_empty() {
                    page = Page(PageKind::Back);
                } else if let Some(f) = self.on_exit.as_mut() {
                    f();
                }
            }
            if matches!(page.0, PageKind::Back) && self.history.is_empty() {
                // nothing to return to: the screen stays as it is
                if let Some(f) = self.on_exit.as_mut() {
                    f();
                }
                match &self.last_shown {
                    Some((screen, data)) => {
                        page = Page(PageKind::View { screen: Rc::clone(screen), data: data.clone() });
                    }
                    None => {
                        return Err(ProgramError::Program("goBack: there is no screen to return to".to_string()));
                    }
                }
            }
            match page.0 {
                PageKind::Go { next, root } => {
                    hops += 1;
                    if hops > MAX_HOPS {
                        return Err(ProgramError::Program("go: too many jumps without showing a screen".to_string()));
                    }
                    if root {
                        self.history.clear();
                    } else {
                        self.history.push(Rc::clone(&current));
                    }
                    current = next;
                    self.after_jump();
                }
                PageKind::Back => {
                    hops += 1;
                    if hops > MAX_HOPS {
                        return Err(ProgramError::Program("go: too many jumps without showing a screen".to_string()));
                    }
                    match self.history.pop() {
                        Some(previous) => current = previous,
                        None => {
                            return Err(ProgramError::Program("goBack: there is no screen to return to".to_string()));
                        }
                    }
                    self.after_jump();
                }
                PageKind::View { screen, data } => {
                    hops = 0;
                    self.last_shown = Some((Rc::clone(&screen), data.clone()));
                    if !self.render(&screen, &data)? {
                        return Ok(());
                    }
                    self.fresh = false;
                }
                PageKind::Failed => {
                    return Err(ProgramError::Program("a screen function failed".to_string()));
                }
            }
        }
        Ok(())
    }

    fn after_jump(&mut self) {
        self.last_shown = None;
        self.slots.clear();
        self.fresh = true;
        self.session.reset();
    }

    // ---- taps and fields (after a jump there is no tap yet: they answer "nothing")

    pub fn click(&mut self, target: &str) -> bool {
        !self.fresh && self.session.click(target)
    }

    pub fn click_index(&self) -> i32 {
        if self.fresh {
            -1
        } else {
            self.session.click_index()
        }
    }

    /// The item of the `for [name] in ...` that holds the element that was just tapped (section 5.7).
    pub fn clicked(&mut self, name: &str) -> Option<Value> {
        if self.fresh {
            return None;
        }
        match self.session.clicked_item(name) {
            Ok(v) => Some(v),
            Err(m) => {
                self.fail(ProgramError::Program(m));
                None
            }
        }
    }

    /// Text of a field, `""` if the id does not exist.
    pub fn value(&mut self, id: &str) -> String {
        self.value_in(id, &[])
    }

    /// `value` of the element `id` of the item(s) given, one for each enclosing `for`, outermost first.
    pub fn value_in(&mut self, id: &str, items: &[Value]) -> String {
        if self.fresh {
            return String::new();
        }
        match self.session.field_text(id, items) {
            Ok(v) => v.unwrap_or_default(),
            Err(m) => {
                self.fail(ProgramError::Program(m));
                String::new()
            }
        }
    }

    /// State of a checkbox, `false` if it does not exist.
    pub fn checked(&mut self, id: &str) -> bool {
        self.checked_in(id, &[])
    }

    pub fn checked_in(&mut self, id: &str, items: &[Value]) -> bool {
        if self.fresh {
            return false;
        }
        match self.session.field_checked(id, items) {
            Ok(v) => v.unwrap_or(false),
            Err(m) => {
                self.fail(ProgramError::Program(m));
                false
            }
        }
    }

    /// Fills or clears a field (`"1"` / `"0"` for a checkbox).
    pub fn set_value(&mut self, id: &str, text: &str) {
        self.set_value_in(id, &[], text);
    }

    /// Fills the element `id` of the item(s) given: `set_value_in("qty", &[p], "1")`.
    pub fn set_value_in(&mut self, id: &str, items: &[Value], text: &str) {
        if let Err(m) = self.session.set_value(id, text, items) {
            self.fail(ProgramError::Program(m));
        }
    }

    pub fn save(&mut self, key: &str, text: &str) {
        self.session.save(key, text)
    }

    pub fn load(&self, key: &str) -> Option<String> {
        self.session.load(key)
    }

    pub fn exists(&self, key: &str) -> bool {
        self.session.exists(key)
    }

    pub fn delete(&mut self, key: &str) {
        self.session.delete(key)
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.session.host_mut().close();
    }
}
