//! nativelayout: Rust core. Port of the Kotlin core (package `nativelayout.core`).
//! Each module matches one Kotlin file.

pub mod css; // Css.kt
pub mod differ; // LayoutDiffer.kt
pub mod error; // NativeLayoutError.kt (+ ExpandError and MarkupError)
pub mod expander; // Expander.kt
pub mod html_parser; // HtmlParser.kt
pub mod icons; // IconSet.kt
pub mod layout_engine; // LayoutEngine.kt
pub mod layout_host; // LayoutHost.kt
pub mod program; // NativeView.kt: the verbs of the program side (state, view, click, value, ...)
pub mod scopes; // Scopes.kt
pub mod tree; // NativeTree.kt
pub mod value; // the data type of the Expander (Kotlin `Any?`)
