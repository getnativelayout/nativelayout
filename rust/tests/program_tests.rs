//! Tests of the program side (`NativeView`), with a host that lives in memory.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use nativelayout::differ::Change;
use nativelayout::layout_host::{LayoutEvent, LayoutHost, BACK_EVENT_KEY};
use nativelayout::program::{screen_fn, NativeView, Page, ProgramError, ScreenFn};
use nativelayout::value::Value;

#[derive(Default)]
struct Shared {
    changes: Vec<Change>,
    errors: Vec<String>,
    closed: bool,
}

struct MemoryHost {
    events: VecDeque<String>,
    shared: Rc<RefCell<Shared>>,
    fields: HashMap<String, String>,
    store: HashMap<String, String>,
}

impl MemoryHost {
    fn new(events: &[&str], shared: &Rc<RefCell<Shared>>) -> MemoryHost {
        MemoryHost {
            events: events.iter().map(|e| e.to_string()).collect(),
            shared: Rc::clone(shared),
            fields: HashMap::new(),
            store: HashMap::new(),
        }
    }
}

impl LayoutHost for MemoryHost {
    fn apply_changes(&mut self, changes: &[Change]) {
        self.shared.borrow_mut().changes.extend_from_slice(changes);
    }
    fn next_event(&mut self) -> Option<LayoutEvent> {
        self.events.pop_front().map(|key| LayoutEvent { key })
    }
    fn field_text(&self, id: &str) -> Option<String> {
        self.fields.get(id).cloned()
    }
    fn field_checked(&self, id: &str) -> Option<bool> {
        self.fields.get(id).map(|v| v == "1")
    }
    fn set_value(&mut self, id: &str, text: &str) {
        self.fields.insert(id.to_string(), text.to_string());
    }
    fn save(&mut self, key: &str, text: &str) {
        self.store.insert(key.to_string(), text.to_string());
    }
    fn load(&self, key: &str) -> Option<String> {
        self.store.get(key).cloned()
    }
    fn exists(&self, key: &str) -> bool {
        self.store.contains_key(key)
    }
    fn delete(&mut self, key: &str) {
        self.store.remove(key);
    }
    fn close(&mut self) {
        self.shared.borrow_mut().closed = true;
    }
    fn show_error(&mut self, text: &str) {
        self.shared.borrow_mut().errors.push(text.to_string());
    }
}

fn nl(body: &str) -> String {
    format!("<nativelayout>\n<body>\n{}\n</body>\n</nativelayout>\n", body)
}

fn files(list: &[(&str, String)]) -> HashMap<String, String> {
    list.iter().map(|(n, s)| (n.to_string(), s.clone())).collect()
}

fn view_of(events: &[&str], files: HashMap<String, String>, fields: &[(&str, &str)]) -> (NativeView, Rc<RefCell<Shared>>) {
    let shared = Rc::new(RefCell::new(Shared::default()));
    let mut host = MemoryHost::new(events, &shared);
    for (k, v) in fields {
        host.fields.insert(k.to_string(), v.to_string());
    }
    let view = NativeView::new(Box::new(host), move |n| files.get(n).cloned());
    (view, shared)
}

fn data1(name: &str, v: Value) -> HashMap<String, Value> {
    let mut d = HashMap::new();
    d.insert(name.to_string(), v);
    d
}

#[test]
fn a_screen_function_keeps_its_state_between_taps() {
    let src = nl("    <p id=\"n\">{count}</p>\n    <button id=\"inc\">+</button>");
    let (mut ui, shared) = view_of(&["#inc", "#inc"], files(&[("counter.nl", src)]), &[]);
    let log: Rc<RefCell<Vec<i64>>> = Rc::new(RefCell::new(Vec::new()));
    let l = Rc::clone(&log);
    let app = screen_fn(move |ui: &mut NativeView| {
        let count = ui.state(|| 0i64);
        if ui.click("inc") {
            *count.borrow_mut() += 1;
        }
        l.borrow_mut().push(*count.borrow());
        ui.view("counter.nl", data1("count", Value::Int(*count.borrow())))
    });
    ui.run(app).unwrap();
    assert_eq!(*log.borrow(), vec![0, 1, 2]);
    // the first render builds everything, the next two only change the text
    let changes = &shared.borrow().changes;
    assert!(matches!(changes[0], Change::SetRoot { .. }));
    assert_eq!(changes.iter().filter(|c| matches!(c, Change::UpdateText { .. })).count(), 2);
    assert!(shared.borrow().closed);
}

#[test]
fn go_and_go_back_move_between_screen_functions() {
    let a_nl = nl("    <button id=\"next\">Next</button>");
    let b_nl = nl("    <button id=\"back\">Back</button>");
    let (mut ui, _shared) = view_of(&["#next", "#back"], files(&[("a.nl", a_nl), ("b.nl", b_nl)]), &[]);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let lb = Rc::clone(&log);
    let b: ScreenFn = screen_fn(move |ui: &mut NativeView| {
        lb.borrow_mut().push("b");
        if ui.click("back") {
            return ui.go_back();
        }
        ui.view("b.nl", HashMap::new())
    });
    let la = Rc::clone(&log);
    let b2 = Rc::clone(&b);
    let a: ScreenFn = screen_fn(move |ui: &mut NativeView| {
        la.borrow_mut().push("a");
        if ui.click("next") {
            return ui.go_fn(Rc::clone(&b2));
        }
        ui.view("a.nl", HashMap::new())
    });
    ui.run(a).unwrap();
    // a shows; tap "next": a jumps, b shows; tap "back": b returns, a shows again
    assert_eq!(*log.borrow(), vec!["a", "a", "b", "b", "a"]);
}

#[test]
fn after_a_jump_the_new_screen_sees_no_tap() {
    let a_nl = nl("    <button id=\"x\">X</button>");
    let (mut ui, _s) = view_of(&["#x"], files(&[("a.nl", a_nl.clone()), ("b.nl", a_nl)]), &[]);
    let seen: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
    let sb = Rc::clone(&seen);
    let b: ScreenFn = screen_fn(move |ui: &mut NativeView| {
        sb.borrow_mut().push(ui.click("x"));
        ui.view("b.nl", HashMap::new())
    });
    let b2 = Rc::clone(&b);
    let a: ScreenFn = screen_fn(move |ui: &mut NativeView| {
        if ui.click("x") {
            return ui.go_fn(Rc::clone(&b2));
        }
        ui.view("a.nl", HashMap::new())
    });
    ui.run(a).unwrap();
    // "x" was tapped on a, but b is new: it must not see that tap
    assert_eq!(*seen.borrow(), vec![false]);
}

#[test]
fn a_loop_of_jumps_is_stopped() {
    let (mut ui, shared) = view_of(&[], HashMap::new(), &[]);
    fn spin(ui: &mut NativeView) -> Page {
        ui.go(spin)
    }
    let r = ui.run(screen_fn(spin));
    match r {
        Err(ProgramError::Program(m)) => assert!(m.contains("too many jumps"), "{}", m),
        other => panic!("expected an error, got {:?}", other.is_ok()),
    }
    assert_eq!(shared.borrow().errors.len(), 1);
}

#[test]
fn the_base_layer_renders_and_reads_fields() {
    let src = nl("    <input id=\"name\">\n    <button id=\"go\">Go</button>");
    let (mut ui, _s) = view_of(&["#go"], files(&[("form.nl", src)]), &[("name", "Ana")]);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sc = Rc::clone(&seen);
    ui.run_body(move |ui: &mut NativeView| {
        let scr = ui.screen("form.nl")?;
        while ui.render(&scr, &HashMap::new())? {
            sc.borrow_mut().push(ui.value("name"));
            assert!(ui.click("go"));
            assert_eq!(ui.click_index(), -1);
            ui.set_value("name", "Bia");
            sc.borrow_mut().push(ui.value("name"));
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(*seen.borrow(), vec!["Ana", "Bia"]);
}

#[test]
fn a_missing_screen_is_an_error_shown_by_the_host() {
    let (mut ui, shared) = view_of(&[], HashMap::new(), &[]);
    let app = screen_fn(|ui: &mut NativeView| ui.view("nope.nl", HashMap::new()));
    let r = ui.run(app);
    match r {
        Err(ProgramError::Program(m)) => assert_eq!(m, "screen not found: nope.nl"),
        _ => panic!("expected an error"),
    }
    assert_eq!(shared.borrow().errors, vec!["screen not found: nope.nl".to_string()]);
}

#[test]
fn a_markup_error_is_shown_with_its_line() {
    let bad = nl("    <p>{missing}</p>");
    let (mut ui, shared) = view_of(&[], files(&[("bad.nl", bad)]), &[]);
    let r = ui.run(screen_fn(|ui: &mut NativeView| ui.view("bad.nl", HashMap::new())));
    assert!(matches!(r, Err(ProgramError::Layout(_))));
    assert!(shared.borrow().errors[0].contains("missing"));
    assert!(shared.borrow().errors[0].starts_with("Error: line 4"));
}

#[test]
fn the_back_button_on_the_first_screen_calls_on_exit() {
    let src = nl("    <p>hi</p>");
    let (mut ui, _s) = view_of(&[BACK_EVENT_KEY], files(&[("a.nl", src)]), &[]);
    let exits = Rc::new(RefCell::new(0));
    let e = Rc::clone(&exits);
    ui.on_exit = Some(Box::new(move || *e.borrow_mut() += 1));
    ui.run(screen_fn(|ui: &mut NativeView| ui.view("a.nl", HashMap::new()))).unwrap();
    assert_eq!(*exits.borrow(), 1);
}

#[test]
fn a_screen_can_handle_the_back_button_itself() {
    let src = nl("    <p>hi</p>");
    let (mut ui, _s) = view_of(&[BACK_EVENT_KEY], files(&[("a.nl", src)]), &[]);
    let exits = Rc::new(RefCell::new(0));
    let e = Rc::clone(&exits);
    ui.on_exit = Some(Box::new(move || *e.borrow_mut() += 1));
    let handled = Rc::new(RefCell::new(false));
    let h = Rc::clone(&handled);
    ui.run(screen_fn(move |ui: &mut NativeView| {
        if ui.back() {
            *h.borrow_mut() = true;
        }
        ui.view("a.nl", HashMap::new())
    }))
    .unwrap();
    assert!(*handled.borrow());
    assert_eq!(*exits.borrow(), 0);
}

#[test]
fn clicked_gives_the_item_of_the_for() {
    let src = nl("for p in products:\n    <button id=\"buy\">{p.name}</button>\nend");
    let products = Value::List(vec![
        Value::map([("id", 7.into()), ("name", "Tea".into())]),
        Value::map([("id", 9.into()), ("name", "Rice".into())]),
    ]);
    let (mut ui, _s) = view_of(&["#buy~p:9"], files(&[("shop.nl", src)]), &[]);
    let got: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(Vec::new()));
    let g = Rc::clone(&got);
    let p2 = products.clone();
    ui.run(screen_fn(move |ui: &mut NativeView| {
        if ui.click("buy") {
            if let Some(item) = ui.clicked("p") {
                g.borrow_mut().push(item);
            }
        }
        ui.view("shop.nl", data1("products", p2.clone()))
    }))
    .unwrap();
    let expected = match &products {
        Value::List(l) => l[1].clone(),
        _ => unreachable!(),
    };
    assert_eq!(*got.borrow(), vec![expected]);
}

#[test]
fn storage_goes_through_the_host() {
    let (mut ui, _s) = view_of(&[], HashMap::new(), &[]);
    assert!(!ui.exists("k"));
    ui.save("k", "v");
    assert!(ui.exists("k"));
    assert_eq!(ui.load("k"), Some("v".to_string()));
    ui.delete("k");
    assert_eq!(ui.load("k"), None);
}

#[test]
fn close_stops_render() {
    let src = nl("    <p>x</p>");
    let (mut ui, shared) = view_of(&["#a"], files(&[("a.nl", src)]), &[]);
    let scr = ui.screen("a.nl").unwrap();
    ui.close();
    assert!(!ui.render(&scr, &HashMap::new()).unwrap());
    assert!(shared.borrow().closed);
}
