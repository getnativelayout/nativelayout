//! Port of LayoutDiffer.kt: compares two trees and lists what changed.

use std::collections::{HashMap, HashSet};

use crate::tree::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    SetRoot { node: Native },
    Create { parent_key: String, position: usize, node: Native },
    Remove { key: String },
    Replace { key: String, node: Native },
    UpdateText { key: String, spans: Vec<Span> },
    UpdateStyle { key: String, style: NativeStyle },
    UpdateAttributes { key: String, node: Native },
    SetItems { key: String, items: Vec<Native> },
}

fn same_class(a: &Native, b: &Native) -> bool {
    std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
}

pub fn diff(previous: Option<&Native>, next: &Native) -> Vec<Change> {
    let prev = match previous {
        Some(p) if p.key == next.key && same_class(p, next) => p,
        _ => return vec![Change::SetRoot { node: next.clone() }],
    };
    let mut removed: Vec<Change> = Vec::new();
    let mut rest: Vec<Change> = Vec::new();
    compare(prev, next, &mut removed, &mut rest);
    removed.extend(rest);
    removed
}

fn compare(a: &Native, n: &Native, removed: &mut Vec<Change>, rest: &mut Vec<Change>) {
    if let (NativeKind::Box(ab), NativeKind::Box(nb)) = (&a.kind, &n.kind) {
        if ab.is_virtual != nb.is_virtual
            || ab.overlay != nb.overlay
            || ab.header != nb.header
            || ab.footer != nb.footer
            || ab.has_absolute != nb.has_absolute
        {
            rest.push(Change::Replace { key: n.key.clone(), node: n.clone() });
            return;
        }
        if nb.is_virtual {
            if a.style != n.style {
                rest.push(Change::UpdateStyle { key: n.key.clone(), style: n.style.clone() });
            }
            if attributes_differ(a, n) {
                rest.push(Change::UpdateAttributes { key: n.key.clone(), node: n.clone() });
            }
            if !same_children(&ab.children, &nb.children) {
                rest.push(Change::SetItems { key: n.key.clone(), items: nb.children.clone() });
            }
            return;
        }
    }
    if a.style != n.style {
        rest.push(Change::UpdateStyle { key: n.key.clone(), style: n.style.clone() });
    }
    if attributes_differ(a, n) {
        rest.push(Change::UpdateAttributes { key: n.key.clone(), node: n.clone() });
    }
    if let (NativeKind::Text { spans: sa }, NativeKind::Text { spans: sn }) = (&a.kind, &n.kind) {
        if sa != sn {
            rest.push(Change::UpdateText { key: n.key.clone(), spans: sn.clone() });
        }
    }
    let children_a = a.native_children();
    let children_n = n.native_children();
    if children_a.is_empty() && children_n.is_empty() {
        return;
    }
    compare_children(n, children_a, children_n, removed, rest);
}

fn same_children(a: &[Native], b: &[Native]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.same_as(y))
}

fn attributes_differ(a: &Native, n: &Native) -> bool {
    if a.classes != n.classes {
        return true;
    }
    match (&a.kind, &n.kind) {
        (NativeKind::Button { label: l1, disabled: d1 }, NativeKind::Button { label: l2, disabled: d2 }) => {
            l1 != l2 || d1 != d2
        }
        (
            NativeKind::Field { field_type: t1, hint: h1, disabled: d1, read_only: r1, rows: n1, .. },
            NativeKind::Field { field_type: t2, hint: h2, disabled: d2, read_only: r2, rows: n2, .. },
        ) => t1 != t2 || h1 != h2 || d1 != d2 || r1 != r2 || n1 != n2,
        (NativeKind::Checkbox { disabled: d1, .. }, NativeKind::Checkbox { disabled: d2, .. }) => d1 != d2,
        (
            NativeKind::Image { source: s1, description: d1 },
            NativeKind::Image { source: s2, description: d2 },
        ) => s1 != s2 || d1 != d2,
        (NativeKind::Item { marker: m1, .. }, NativeKind::Item { marker: m2, .. }) => m1 != m2,
        _ => false,
    }
}

fn compare_children(
    parent: &Native,
    previous_children: &[Native],
    next_children: &[Native],
    removed: &mut Vec<Change>,
    rest: &mut Vec<Change>,
) {
    let new_keys: HashSet<&str> = next_children.iter().map(|c| c.key.as_str()).collect();
    let mut old_by_key: HashMap<&str, &Native> = HashMap::new();
    for c in previous_children {
        old_by_key.insert(c.key.as_str(), c);
    }

    let kept_old: Vec<&str> = previous_children
        .iter()
        .filter(|c| new_keys.contains(c.key.as_str()))
        .map(|c| c.key.as_str())
        .collect();
    let kept_new: Vec<&str> = next_children
        .iter()
        .filter(|c| old_by_key.contains_key(c.key.as_str()))
        .map(|c| c.key.as_str())
        .collect();
    if kept_old != kept_new {
        rest.push(Change::Replace { key: parent.key.clone(), node: parent.clone() });
        return;
    }

    for a in previous_children {
        if !new_keys.contains(a.key.as_str()) {
            removed.push(Change::Remove { key: a.key.clone() });
        }
    }

    for (i, n) in next_children.iter().enumerate() {
        match old_by_key.get(n.key.as_str()) {
            None => {
                // an absolute child lives apart from the flow: its position counts only the absolute ones
                let absolute = n.style.position == Position::Absolute;
                let before = next_children[..i]
                    .iter()
                    .filter(|x| (x.style.position == Position::Absolute) == absolute)
                    .count();
                rest.push(Change::Create { parent_key: parent.key.clone(), position: before, node: n.clone() });
            }
            Some(a) => {
                if !same_class(a, n) {
                    rest.push(Change::Replace { key: n.key.clone(), node: n.clone() });
                } else {
                    compare(a, n, removed, rest);
                }
            }
        }
    }
}
