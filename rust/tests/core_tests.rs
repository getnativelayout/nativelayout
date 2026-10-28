//! Tests ported from the Kotlin ones (AbsoluteTest, FooterTest, IncludeTest) plus a few for the
//! Expander. They only use the public API of the crate.

use std::collections::HashMap;

use nativelayout::differ::{self, Change};
use nativelayout::expander;
use nativelayout::layout_engine::{self, LayoutResult};
use nativelayout::scopes;
use nativelayout::tree::*;
use nativelayout::value::Value;

fn build(html: &str) -> LayoutResult {
    layout_engine::build(html, 1, &HashMap::new()).unwrap()
}

fn error_of(html: &str) -> String {
    match layout_engine::build(html, 1, &HashMap::new()) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.message,
    }
}

fn as_box(n: &Native) -> &NativeBox {
    match &n.kind {
        NativeKind::Box(b) => b,
        _ => panic!("expected a box, got {}", n.kind_name()),
    }
}

fn is_text(n: &Native) -> bool {
    matches!(n.kind, NativeKind::Text { .. })
}

// ---------- AbsoluteTest ----------

#[test]
fn absolute_child_makes_the_parent_a_frame() {
    let r = build(
        r#"<div><div id="box"><p>Flow</p><p id="badge" style="position: absolute; top: 4px; right: 8px">1</p></div></div>"#,
    );
    assert!(as_box(&r.by_id["box"]).has_absolute);
    let badge = &r.by_id["badge"];
    assert_eq!(badge.style.position, Position::Absolute);
    assert_eq!(badge.style.top.unwrap().value, 4.0);
    assert_eq!(badge.style.right.unwrap().value, 8.0);
    assert!(badge.style.left.is_none());
}

#[test]
fn a_box_without_absolute_children_is_not_a_frame() {
    let r = build(r#"<div><div id="box"><p>Flow</p><p>Two</p></div></div>"#);
    assert!(!as_box(&r.by_id["box"]).has_absolute);
}

#[test]
fn a_single_absolute_text_is_not_collapsed_into_its_parent() {
    let r = build(r#"<div><div id="box"><p style="position: absolute; top: 0">X</p></div></div>"#);
    assert!(as_box(&r.by_id["box"]).has_absolute);
}

#[test]
fn relative_is_accepted_and_does_nothing() {
    let r = build(r#"<div><p id="p" style="position: relative">x</p></div>"#);
    assert_eq!(r.by_id["p"].style.position, Position::Static);
}

#[test]
fn absolute_is_ignored_with_a_warning_where_it_makes_no_sense() {
    let r = build(r#"<div><header id="h" style="position: absolute"></header></div>"#);
    assert_eq!(r.by_id["h"].style.position, Position::Static);
    assert!(r.warnings.iter().any(|w| w.message.contains("position: absolute")));
}

#[test]
fn absolute_inside_a_virtual_container_is_an_error() {
    assert!(error_of(r#"<div><div virtual><p style="position: absolute">x</p></div></div>"#).contains("virtual"));
}

#[test]
fn z_index_is_read() {
    let r = build(r#"<div><div><p id="p" style="position: absolute; z-index: 3">x</p></div></div>"#);
    assert_eq!(r.by_id["p"].style.z_index, 3);
}

#[test]
fn background_image_name_and_options() {
    let r = build(
        r#"<div><div id="d" style="background-image: url(cover.jpg); background-size: contain; background-position: top"></div></div>"#,
    );
    let s = &r.by_id["d"].style;
    assert_eq!(s.background_image.as_deref(), Some("cover.jpg"));
    assert_eq!(s.background_size, ImageFit::Contain);
    assert_eq!(s.background_position, ImagePos::Top);
}

#[test]
fn background_shorthand_with_a_color_clears_the_image() {
    let r = build(r#"<div><div id="d" style="background-image: url(a.jpg); background: #fff"></div></div>"#);
    assert!(r.by_id["d"].style.background_image.is_none());
}

#[test]
fn object_fit() {
    let r = build(r#"<div><img id="i" src="x" style="object-fit: cover"></div>"#);
    assert_eq!(r.by_id["i"].style.object_fit, ImageFit::Cover);
}

#[test]
fn an_absolute_child_is_created_among_the_absolute_ones() {
    let a = build(r#"<div><div id="b"><p>one</p><p>two</p></div></div>"#).root;
    let b = build(
        r#"<div><div id="b"><p>one</p><p>two</p><p id="x" style="position: absolute; top: 0">x</p></div></div>"#,
    )
    .root;
    // the box becomes a frame, so it is replaced as a whole
    assert!(differ::diff(Some(&a), &b).iter().any(|c| matches!(c, Change::Replace { .. })));
}

// ---------- FooterTest ----------

#[test]
fn footer_comes_after_the_body_and_before_the_dialogs() {
    let r = build(
        r#"<div><dialog id="d"></dialog><footer id="f"></footer><p>Body</p><header id="h"></header></div>"#,
    );
    let c = r.root.native_children();
    assert!(as_box(&c[0]).header);
    assert!(is_text(&c[1]));
    assert!(as_box(&c[2]).footer);
    assert!(as_box(&c[3]).overlay);
}

#[test]
fn footer_with_only_text_stays_a_box() {
    let r = build(r#"<div><footer id="f">Menu</footer><p>Body</p></div>"#);
    assert!(as_box(&r.by_id["f"]).footer);
}

#[test]
fn footer_defaults_are_a_row_centered() {
    let r = build(r#"<div><footer id="f"></footer></div>"#);
    let s = &r.by_id["f"].style;
    assert_eq!(s.direction, Direction::Row);
    assert_eq!(s.justify, Justify::Center);
    assert_eq!(s.align_items, Align::Center);
    assert!(s.dock.is_none());
}

#[test]
fn dock_values() {
    let cases = [
        ("bottom", Dock::Bottom),
        ("top", Dock::Top),
        ("left", Dock::Left),
        ("right", Dock::Right),
        ("center", Dock::Center),
    ];
    for (word, dock) in cases {
        let html = format!(r#"<div><footer id="f" style="dock: {}"></footer></div>"#, word);
        let r = build(&html);
        assert_eq!(r.by_id["f"].style.dock, Some(dock), "{}", word);
    }
}

#[test]
fn footer_must_be_at_the_first_level() {
    assert!(error_of("<div><div><footer></footer></div></div>").contains("direct child of the root"));
}

#[test]
fn only_one_footer_is_allowed() {
    assert!(error_of("<div><footer></footer><footer></footer></div>").contains("only one"));
}

#[test]
fn changing_dock_is_a_style_change() {
    let a = build(r#"<div><footer id="f"></footer></div>"#).root;
    let b = build(r#"<div><footer id="f" style="dock: top"></footer></div>"#).root;
    let changes = differ::diff(Some(&a), &b);
    assert!(changes.iter().any(|c| matches!(c, Change::UpdateStyle { .. })));
}

// ---------- IncludeTest ----------

fn screen(body: &str) -> String {
    format!("<nativelayout>\n\n<body>\n{}\n</body>\n\n</nativelayout>\n", body)
}

fn card() -> String {
    screen("    <div class=\"card\">\n        <p>{name}</p>\n        <button id=\"buy\">Buy</button>\n    </div>")
}

fn expand_with_files(src: &str, data: &HashMap<String, Value>) -> Result<expander::ExpandedScreen, nativelayout::error::NativeLayoutError> {
    let files: HashMap<String, String> = [("card.nl".to_string(), card())].into_iter().collect();
    expander::expand(src, data, &|n| files.get(n).cloned(), false)
}

fn products() -> Vec<Value> {
    vec![
        Value::map([("id", 7.into()), ("name", "Tea".into())]),
        Value::map([("id", 9.into()), ("name", "Rice".into())]),
    ]
}

#[test]
fn include_takes_the_place_of_the_tag() {
    let r = expand_with_files(&screen("    <include src=\"card.nl\" name=\"Hello\">"), &HashMap::new()).unwrap();
    assert!(r.html.contains("<p>Hello</p>"), "{}", r.html);
    assert!(!r.html.contains("<include"));
}

#[test]
fn ids_in_a_for_are_scoped_by_the_key_of_the_item() {
    let src = screen("for p in products:\n    <include src=\"card.nl\" name=\"{p.name}\">\nend");
    let mut data = HashMap::new();
    data.insert("products".to_string(), Value::List(products()));
    let r = expand_with_files(&src, &data).unwrap();
    assert!(r.html.contains("id=\"buy~p:7\""), "{}", r.html);
    assert!(r.html.contains("id=\"buy~p:9\""), "{}", r.html);
    assert_eq!(r.items["p:7"], products()[0]);
    assert_eq!(r.items["p:9"], products()[1]);
}

#[test]
fn ids_in_a_for_without_key_use_the_position() {
    let src = screen("for t in names:\n    <button id=\"x\">{t}</button>\nend");
    let mut data = HashMap::new();
    data.insert("names".to_string(), Value::List(vec!["a".into(), "b".into()]));
    let r = expand_with_files(&src, &data).unwrap();
    assert!(r.html.contains("id=\"x~t:%p0\""), "{}", r.html);
    assert!(r.html.contains("id=\"x~t:%p1\""), "{}", r.html);
}

#[test]
fn id_outside_a_for_stays_as_written() {
    let r = expand_with_files(&screen("    <button id=\"ok\">Ok</button>"), &HashMap::new()).unwrap();
    assert!(r.html.contains("id=\"ok\""));
    assert!(!r.html.contains("scope="));
}

#[test]
fn nested_fors_give_nested_paths() {
    let src = screen("for a in as:\n    for b in bs:\n        <button id=\"x\">{b.id}</button>\n    end\nend");
    let mut data = HashMap::new();
    data.insert("as".to_string(), Value::List(vec![Value::map([("id", 1.into())])]));
    data.insert("bs".to_string(), Value::List(vec![Value::map([("id", 2.into())])]));
    let r = expand_with_files(&src, &data).unwrap();
    assert!(r.html.contains("id=\"x~a:1/b:2\""), "{}", r.html);
    assert!(r.items.contains_key("a:1"));
    assert!(r.items.contains_key("a:1/b:2"));
}

#[test]
fn missing_parameter_names_the_component_and_the_include_line() {
    let e = expand_with_files(&screen("    <include src=\"card.nl\">"), &HashMap::new()).unwrap_err();
    assert!(e.message.contains("card.nl"), "{}", e.message);
    assert_eq!(e.line, 4);
}

#[test]
fn include_that_includes_itself_is_an_error() {
    let looping = screen("    <include src=\"loop.nl\">");
    let l2 = looping.clone();
    let e = expander::expand(&looping, &HashMap::new(), &move |n| if n == "loop.nl" { Some(l2.clone()) } else { None }, false)
        .unwrap_err();
    assert!(e.message.contains("includes itself"), "{}", e.message);
}

#[test]
fn session_finds_the_item_and_the_scoped_field() {
    assert_eq!(scopes::key_of_item(&products()[0]), Some("7".to_string()));
    let other = Value::map([("id", 7.into()), ("name", "Other".into())]);
    assert!(scopes::same_item(&other, &products()[0], "7"));
}

// ---------- the Expander on its own ----------

#[test]
fn if_elif_else() {
    let src = screen("if n > 5:\n    <p>big</p>\nelif n == 3:\n    <p>three</p>\nelse:\n    <p>small</p>\nend");
    for (n, expected) in [(9, "big"), (3, "three"), (1, "small")] {
        let mut data = HashMap::new();
        data.insert("n".to_string(), Value::Int(n));
        let r = expander::expand_simple(&src, &data).unwrap();
        assert_eq!(r.html, format!("<div>\n    <p>{}</p>\n</div>", expected));
    }
}

#[test]
fn conditions_with_and_or_not() {
    let src = screen("if not done and n >= 2 or flag:\n    <p>yes</p>\nelse:\n    <p>no</p>\nend");
    let run = |done: bool, n: i64, flag: bool| {
        let mut data = HashMap::new();
        data.insert("done".to_string(), Value::Bool(done));
        data.insert("n".to_string(), Value::Int(n));
        data.insert("flag".to_string(), Value::Bool(flag));
        expander::expand_simple(&src, &data).unwrap().html
    };
    assert!(run(false, 3, false).contains("yes"));
    assert!(run(true, 3, false).contains("no"));
    assert!(run(true, 0, true).contains("yes"));
}

#[test]
fn for_repeats_and_escapes_values() {
    let src = screen("for t in names:\n    <p>{t}</p>\nend");
    let mut data = HashMap::new();
    data.insert("names".to_string(), Value::List(vec!["a".into(), "<b>".into()]));
    let r = expander::expand_simple(&src, &data).unwrap();
    assert_eq!(r.html, "<div>\n    <p>a</p>\n    <p>&lt;b&gt;</p>\n</div>");
}

#[test]
fn numbers_print_without_a_trailing_zero() {
    let src = screen("    <p>{a} {b} {c}</p>");
    let mut data = HashMap::new();
    data.insert("a".to_string(), Value::Int(5));
    data.insert("b".to_string(), Value::Float(2.0));
    data.insert("c".to_string(), Value::Float(2.5));
    let r = expander::expand_simple(&src, &data).unwrap();
    assert!(r.html.contains("<p>5 2 2.5</p>"), "{}", r.html);
}

#[test]
fn a_missing_name_is_an_error_with_the_line() {
    let e = expander::expand_simple(&screen("    <p>{nope}</p>"), &HashMap::new()).unwrap_err();
    assert!(e.message.contains("nope"));
    assert_eq!(e.line, 4);
}

#[test]
fn for_without_end_is_an_error() {
    let e = expander::expand_simple(&screen("for t in names:\n    <p>{t}</p>"), &HashMap::new()).unwrap_err();
    assert!(e.message.contains("never closed with 'end'"), "{}", e.message);
}

#[test]
fn style_rules_become_inline_styles_and_variables_resolve() {
    let src = "<nativelayout>\n<style>\n:root { --c: #112233; }\n.big { font-size: 2rem; color: var(--c); }\n</style>\n<body>\n    <p class=\"big\">x</p>\n</body>\n</nativelayout>\n";
    let r = expander::expand_simple(src, &HashMap::new()).unwrap();
    assert!(r.html.contains("style=\"font-size: 2rem; color: #112233\""), "{}", r.html);
}

#[test]
fn the_expanded_html_builds_a_tree() {
    let src = screen("    <p id=\"t\">Hello</p>\n    <button id=\"b\">Go</button>");
    let r = expander::expand_simple(&src, &HashMap::new()).unwrap();
    let tree = layout_engine::build(&r.html, 1, &HashMap::new()).unwrap();
    assert!(tree.by_id.contains_key("t"));
    assert!(tree.by_id.contains_key("b"));
    assert_eq!(tree.root.native_children().len(), 2);
}
