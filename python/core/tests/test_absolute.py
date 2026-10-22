"""`position: absolute`, `background-image` and `object-fit` (mirrors AbsoluteTest.kt)."""
import unittest

from nativelayout.core import (ImageFit, ImagePos, LayoutEngine, NativeBox, NativeLayoutError,
                               Position, diff)
from nativelayout.core.differ import Replace


def error_of(html: str) -> str:
    try:
        LayoutEngine.build(html)
    except NativeLayoutError as e:
        return str(e)
    raise AssertionError("expected an error")


class AbsoluteTest(unittest.TestCase):
    def test_absolute_child_makes_the_parent_a_frame(self):
        r = LayoutEngine.build('<div><div id="box"><p>Flow</p>'
                               '<p id="badge" style="position: absolute; top: 4px; right: 8px">1</p></div></div>')
        self.assertTrue(r.by_id["box"].has_absolute)
        badge = r.by_id["badge"]
        self.assertEqual(Position.ABSOLUTE, badge.style.position)
        self.assertAlmostEqual(4.0, badge.style.top.value, places=3)
        self.assertAlmostEqual(8.0, badge.style.right.value, places=3)
        self.assertIsNone(badge.style.left)

    def test_a_box_without_absolute_children_is_not_a_frame(self):
        box = LayoutEngine.build('<div><div id="box"><p>Flow</p><p>More</p></div></div>').by_id["box"]
        self.assertFalse(box.has_absolute)

    def test_a_single_absolute_text_is_not_collapsed_into_its_parent(self):
        box = LayoutEngine.build('<div><div id="box"><p style="position: absolute; top: 0">X</p></div></div>').by_id["box"]
        self.assertTrue(isinstance(box, NativeBox) and box.has_absolute)

    def test_relative_is_accepted_and_does_nothing(self):
        s = LayoutEngine.build('<div><p id="p" style="position: relative">x</p></div>').by_id["p"].style
        self.assertEqual(Position.STATIC, s.position)

    def test_absolute_is_ignored_with_a_warning_where_it_makes_no_sense(self):
        r = LayoutEngine.build('<div><header id="h" style="position: absolute"></header></div>')
        self.assertEqual(Position.STATIC, r.by_id["h"].style.position)
        self.assertTrue(any("position: absolute" in w.message for w in r.warnings))

    def test_absolute_inside_a_virtual_container_is_an_error(self):
        self.assertIn("virtual", error_of('<div><div virtual><p style="position: absolute">x</p></div></div>'))

    def test_z_index_is_read(self):
        s = LayoutEngine.build('<div><div><p id="p" style="position: absolute; z-index: 3">x</p></div></div>').by_id["p"].style
        self.assertEqual(3, s.z_index)

    def test_background_image_name_and_options(self):
        s = LayoutEngine.build('<div><div id="d" style="background-image: url(cover.jpg); '
                               'background-size: contain; background-position: top"></div></div>').by_id["d"].style
        self.assertEqual("cover.jpg", s.background_image)
        self.assertEqual(ImageFit.CONTAIN, s.background_size)
        self.assertEqual(ImagePos.TOP, s.background_position)

    def test_background_shorthand_with_a_color_clears_the_image(self):
        s = LayoutEngine.build('<div><div id="d" style="background-image: url(a.jpg); background: #fff"></div></div>').by_id["d"].style
        self.assertIsNone(s.background_image)

    def test_object_fit(self):
        s = LayoutEngine.build('<div><img id="i" src="x" style="object-fit: cover"></div>').by_id["i"].style
        self.assertEqual(ImageFit.COVER, s.object_fit)

    def test_a_box_that_becomes_a_frame_is_replaced_as_a_whole(self):
        a = LayoutEngine.build('<div><div id="b"><p>one</p><p>two</p></div></div>').root
        b = LayoutEngine.build('<div><div id="b"><p>one</p><p>two</p>'
                               '<p id="x" style="position: absolute; top: 0">x</p></div></div>').root
        self.assertTrue(any(isinstance(c, Replace) for c in diff(a, b)))


if __name__ == "__main__":
    unittest.main()
