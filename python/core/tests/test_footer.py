"""The <footer>: a region fixed outside the scroll, placed by `dock` (mirrors FooterTest.kt)."""
import unittest

from nativelayout.core import (Align, Direction, Dock, Justify, LayoutEngine, NativeBox,
                               NativeLayoutError, NativeText, diff)
from nativelayout.core.differ import UpdateStyle


def error_of(html: str) -> str:
    try:
        LayoutEngine.build(html)
    except NativeLayoutError as e:
        return str(e)
    raise AssertionError("expected an error")


class FooterTest(unittest.TestCase):
    def test_footer_comes_after_the_body_and_before_the_dialogs(self):
        root = LayoutEngine.build(
            '<div><dialog id="d"></dialog><footer id="f"></footer><p>Body</p><header id="h"></header></div>').root
        c = root.children
        self.assertTrue(c[0].header)
        self.assertIsInstance(c[1], NativeText)
        self.assertTrue(c[2].footer)
        self.assertTrue(c[3].overlay)

    def test_footer_with_only_text_stays_a_box(self):
        f = LayoutEngine.build('<div><footer id="f">Menu</footer><p>Body</p></div>').by_id["f"]
        self.assertTrue(isinstance(f, NativeBox) and f.footer)

    def test_defaults_are_a_row_centered(self):
        f = LayoutEngine.build('<div><footer id="f"></footer></div>').by_id["f"]
        self.assertEqual(Direction.ROW, f.style.direction)
        self.assertEqual(Justify.CENTER, f.style.justify)
        self.assertEqual(Align.CENTER, f.style.align_items)
        self.assertIsNone(f.style.dock)

    def test_dock_values(self):
        cases = {"bottom": Dock.BOTTOM, "top": Dock.TOP, "left": Dock.LEFT,
                 "right": Dock.RIGHT, "center": Dock.CENTER}
        for word, dock in cases.items():
            f = LayoutEngine.build(f'<div><footer id="f" style="dock: {word}"></footer></div>').by_id["f"]
            self.assertEqual(dock, f.style.dock, word)

    def test_footer_must_be_at_the_first_level(self):
        self.assertIn("direct child of the root", error_of("<div><div><footer></footer></div></div>"))

    def test_only_one_footer_is_allowed(self):
        self.assertIn("only one", error_of("<div><footer></footer><footer></footer></div>"))

    def test_changing_dock_is_a_style_change(self):
        a = LayoutEngine.build('<div><footer id="f"></footer></div>').root
        b = LayoutEngine.build('<div><footer id="f" style="dock: top"></footer></div>').root
        self.assertTrue(any(isinstance(c, UpdateStyle) for c in diff(a, b)))


if __name__ == "__main__":
    unittest.main()
