//! Port of IconSet.kt: the built-in icons, used as `<img src="icon:name">`.
//!
//! Every icon is drawn on a 24 x 24 grid with a 2-unit stroke and round caps and joins, in the text
//! color of the element. The shapes are plain data, so every host draws the same icons.

#[derive(Debug, Clone, PartialEq)]
pub enum IconShape {
    /// A polyline; `points` is x1, y1, x2, y2... When `closed` the last point joins the first.
    Line { points: Vec<f32>, closed: bool },
    Circle { cx: f32, cy: f32, r: f32 },
    /// A small filled dot, for "more" icons.
    Dot { cx: f32, cy: f32 },
    Rect { left: f32, top: f32, right: f32, bottom: f32, radius: f32 },
}

pub const PREFIX: &str = "icon:";
pub const GRID: f32 = 24.0;
pub const STROKE: f32 = 2.0;
pub const DOT_RADIUS: f32 = 1.6;

/// The icon name of an `<img src>`, or `None` when the source is not an `icon:` one.
pub fn name_of(source: &str) -> Option<String> {
    source.strip_prefix(PREFIX).map(|n| n.trim().to_string())
}

pub fn shapes_of(name: &str) -> Option<Vec<IconShape>> {
    table().into_iter().find(|(n, _)| *n == name).map(|(_, s)| s)
}

/// The names of the icons, in the order they are defined.
pub fn names() -> Vec<&'static str> {
    table().into_iter().map(|(n, _)| n).collect()
}

fn table() -> Vec<(&'static str, Vec<IconShape>)> {
    vec![
        ("arrow-left", vec![
            IconShape::Line { points: vec![19.0, 12.0, 5.0, 12.0], closed: false },
            IconShape::Line { points: vec![12.0, 19.0, 5.0, 12.0, 12.0, 5.0], closed: false },
        ]),
        ("arrow-right", vec![
            IconShape::Line { points: vec![5.0, 12.0, 19.0, 12.0], closed: false },
            IconShape::Line { points: vec![12.0, 5.0, 19.0, 12.0, 12.0, 19.0], closed: false },
        ]),
        ("arrow-up", vec![
            IconShape::Line { points: vec![12.0, 19.0, 12.0, 5.0], closed: false },
            IconShape::Line { points: vec![5.0, 12.0, 12.0, 5.0, 19.0, 12.0], closed: false },
        ]),
        ("arrow-down", vec![
            IconShape::Line { points: vec![12.0, 5.0, 12.0, 19.0], closed: false },
            IconShape::Line { points: vec![5.0, 12.0, 12.0, 19.0, 19.0, 12.0], closed: false },
        ]),
        ("chevron-left", vec![
            IconShape::Line { points: vec![15.0, 5.0, 8.0, 12.0, 15.0, 19.0], closed: false },
        ]),
        ("chevron-right", vec![
            IconShape::Line { points: vec![9.0, 5.0, 16.0, 12.0, 9.0, 19.0], closed: false },
        ]),
        ("chevron-up", vec![
            IconShape::Line { points: vec![5.0, 15.0, 12.0, 8.0, 19.0, 15.0], closed: false },
        ]),
        ("chevron-down", vec![
            IconShape::Line { points: vec![5.0, 9.0, 12.0, 16.0, 19.0, 9.0], closed: false },
        ]),
        ("close", vec![
            IconShape::Line { points: vec![6.0, 6.0, 18.0, 18.0], closed: false },
            IconShape::Line { points: vec![18.0, 6.0, 6.0, 18.0], closed: false },
        ]),
        ("check", vec![
            IconShape::Line { points: vec![4.0, 12.5, 9.5, 18.0, 20.0, 6.5], closed: false },
        ]),
        ("plus", vec![
            IconShape::Line { points: vec![12.0, 5.0, 12.0, 19.0], closed: false },
            IconShape::Line { points: vec![5.0, 12.0, 19.0, 12.0], closed: false },
        ]),
        ("minus", vec![
            IconShape::Line { points: vec![5.0, 12.0, 19.0, 12.0], closed: false },
        ]),
        ("menu", vec![
            IconShape::Line { points: vec![4.0, 6.0, 20.0, 6.0], closed: false },
            IconShape::Line { points: vec![4.0, 12.0, 20.0, 12.0], closed: false },
            IconShape::Line { points: vec![4.0, 18.0, 20.0, 18.0], closed: false },
        ]),
        ("search", vec![
            IconShape::Circle { cx: 10.5, cy: 10.5, r: 6.5 },
            IconShape::Line { points: vec![15.3, 15.3, 20.0, 20.0], closed: false },
        ]),
        ("more-vertical", vec![
            IconShape::Dot { cx: 12.0, cy: 5.0 },
            IconShape::Dot { cx: 12.0, cy: 12.0 },
            IconShape::Dot { cx: 12.0, cy: 19.0 },
        ]),
        ("more-horizontal", vec![
            IconShape::Dot { cx: 5.0, cy: 12.0 },
            IconShape::Dot { cx: 12.0, cy: 12.0 },
            IconShape::Dot { cx: 19.0, cy: 12.0 },
        ]),
        ("home", vec![
            IconShape::Line { points: vec![3.0, 11.0, 12.0, 3.0, 21.0, 11.0], closed: false },
            IconShape::Line { points: vec![5.0, 9.5, 5.0, 21.0, 19.0, 21.0, 19.0, 9.5], closed: false },
            IconShape::Line { points: vec![10.0, 21.0, 10.0, 15.0, 14.0, 15.0, 14.0, 21.0], closed: false },
        ]),
        ("user", vec![
            IconShape::Circle { cx: 12.0, cy: 8.0, r: 4.0 },
            IconShape::Line { points: vec![5.0, 21.0, 5.0, 19.5, 5.8, 17.0, 7.6, 15.3, 10.0, 14.3, 12.0, 14.0, 14.0, 14.3, 16.4, 15.3, 18.2, 17.0, 19.0, 19.5, 19.0, 21.0], closed: false },
        ]),
        ("trash", vec![
            IconShape::Line { points: vec![4.0, 7.0, 20.0, 7.0], closed: false },
            IconShape::Line { points: vec![9.0, 7.0, 9.0, 4.0, 15.0, 4.0, 15.0, 7.0], closed: false },
            IconShape::Line { points: vec![6.0, 7.0, 7.0, 21.0, 17.0, 21.0, 18.0, 7.0], closed: false },
            IconShape::Line { points: vec![10.0, 11.0, 10.0, 17.0], closed: false },
            IconShape::Line { points: vec![14.0, 11.0, 14.0, 17.0], closed: false },
        ]),
        ("edit", vec![
            IconShape::Line { points: vec![4.0, 20.0, 5.0, 15.0, 16.0, 4.0, 20.0, 8.0, 9.0, 19.0], closed: true },
            IconShape::Line { points: vec![14.0, 6.0, 18.0, 10.0], closed: false },
        ]),
        ("star", vec![
            IconShape::Line { points: vec![12.0, 3.0, 14.4, 9.3, 21.1, 9.6, 15.9, 13.9, 17.6, 20.4, 12.0, 16.7, 6.4, 20.4, 8.1, 13.9, 2.9, 9.6, 9.6, 9.3], closed: true },
        ]),
        ("lock", vec![
            IconShape::Rect { left: 5.0, top: 11.0, right: 19.0, bottom: 21.0, radius: 2.0 },
            IconShape::Line { points: vec![8.0, 11.0, 8.0, 8.2, 8.6, 6.4, 10.0, 5.1, 12.0, 4.6, 14.0, 5.1, 15.4, 6.4, 16.0, 8.2, 16.0, 11.0], closed: false },
        ]),
        ("eye", vec![
            IconShape::Line { points: vec![2.0, 12.0, 4.0, 9.6, 7.0, 7.6, 9.5, 6.6, 12.0, 6.3, 14.5, 6.6, 17.0, 7.6, 20.0, 9.6, 22.0, 12.0, 20.0, 14.4, 17.0, 16.4, 14.5, 17.4, 12.0, 17.7, 9.5, 17.4, 7.0, 16.4, 4.0, 14.4], closed: true },
            IconShape::Circle { cx: 12.0, cy: 12.0, r: 3.0 },
        ]),
        ("mail", vec![
            IconShape::Rect { left: 3.0, top: 5.0, right: 21.0, bottom: 19.0, radius: 2.0 },
            IconShape::Line { points: vec![3.0, 7.0, 12.0, 13.0, 21.0, 7.0], closed: false },
        ]),
        ("info", vec![
            IconShape::Circle { cx: 12.0, cy: 12.0, r: 9.0 },
            IconShape::Line { points: vec![12.0, 11.0, 12.0, 17.0], closed: false },
            IconShape::Dot { cx: 12.0, cy: 7.5 },
        ]),
        ("alert", vec![
            IconShape::Line { points: vec![12.0, 3.0, 22.0, 20.0, 2.0, 20.0], closed: true },
            IconShape::Line { points: vec![12.0, 9.0, 12.0, 14.0], closed: false },
            IconShape::Dot { cx: 12.0, cy: 17.0 },
        ]),
        ("calendar", vec![
            IconShape::Rect { left: 3.0, top: 5.0, right: 21.0, bottom: 21.0, radius: 2.0 },
            IconShape::Line { points: vec![3.0, 10.0, 21.0, 10.0], closed: false },
            IconShape::Line { points: vec![8.0, 3.0, 8.0, 7.0], closed: false },
            IconShape::Line { points: vec![16.0, 3.0, 16.0, 7.0], closed: false },
        ]),
        ("logout", vec![
            IconShape::Line { points: vec![9.0, 4.0, 5.0, 4.0, 5.0, 20.0, 9.0, 20.0], closed: false },
            IconShape::Line { points: vec![10.0, 12.0, 20.0, 12.0], closed: false },
            IconShape::Line { points: vec![16.0, 8.0, 20.0, 12.0, 16.0, 16.0], closed: false },
        ]),
        ("bookmark", vec![
            IconShape::Line { points: vec![6.0, 3.0, 18.0, 3.0, 18.0, 21.0, 12.0, 16.0, 6.0, 21.0], closed: true },
        ]),
        ("clock", vec![
            IconShape::Circle { cx: 12.0, cy: 12.0, r: 9.0 },
            IconShape::Line { points: vec![12.0, 7.0, 12.0, 12.0, 16.0, 14.0], closed: false },
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_shapes() {
        assert_eq!(name_of("icon:star"), Some("star".to_string()));
        assert_eq!(name_of("icon: star "), Some("star".to_string()));
        assert_eq!(name_of("photo.png"), None);
        assert_eq!(names().len(), 30);
        assert_eq!(names()[0], "arrow-left");
        assert!(shapes_of("star").is_some());
        assert!(shapes_of("nope").is_none());
        match &shapes_of("close").unwrap()[0] {
            IconShape::Line { points, closed } => {
                assert_eq!(points, &vec![6.0, 6.0, 18.0, 18.0]);
                assert!(!closed);
            }
            _ => panic!("close starts with a line"),
        }
    }
}
