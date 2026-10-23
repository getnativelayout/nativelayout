"""Layout of `position: absolute` in flex.py (pure Python, no Kivy needed)."""
import importlib.util
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "core"))

from nativelayout.core import LayoutEngine  # noqa: E402

_spec = importlib.util.spec_from_file_location("nl_flex", os.path.join(HERE, "..", "nativelayout", "host", "flex.py"))
flex = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(flex)

M = flex.Metrics(1.0, 1.0)


def node(html: str, key: str) -> "flex.LNode":
    n = LayoutEngine.build(html).by_id[key]
    return flex.LNode(n.style, leaf=lambda mw: (40.0, 20.0), key=n.key)


class AbsoluteLayoutTest(unittest.TestCase):
    def test_top_left_offsets_from_the_parent_edges(self):
        n = node('<div><div id="b"><p>x</p><p id="a" style="margin:0;position:absolute;top:4px;left:6px">1</p></div></div>', "a")
        flex.layout_absolute(n, M, 200.0, 100.0)
        self.assertEqual((6.0, 4.0, 40.0, 20.0), (n.x, n.y, n.w, n.h))

    def test_right_bottom_sit_against_the_edges(self):
        n = node('<div><div><p>x</p><p id="a" style="margin:0;position:absolute;right:8px;bottom:10px">1</p></div></div>', "a")
        flex.layout_absolute(n, M, 200.0, 100.0)
        self.assertEqual((200.0 - 8.0 - 40.0, 100.0 - 10.0 - 20.0), (n.x, n.y))

    def test_left_and_right_stretch_the_width(self):
        n = node('<div><div><p>x</p><p id="a" style="margin:0;position:absolute;left:10px;right:30px;top:0">1</p></div></div>', "a")
        flex.layout_absolute(n, M, 200.0, 100.0)
        self.assertEqual((10.0, 160.0), (n.x, n.w))

    def test_margin_adds_to_the_offset(self):
        n = node('<div><div><p>x</p><p id="a" style="position:absolute;top:4px;left:4px;margin:2px">1</p></div></div>', "a")
        flex.layout_absolute(n, M, 200.0, 100.0)
        self.assertEqual((6.0, 6.0), (n.x, n.y))

    def test_fixed_size(self):
        n = node('<div><div><p>x</p><p id="a" style="margin:0;position:absolute;top:0;left:0;width:50px;height:30px">1</p></div></div>', "a")
        flex.layout_absolute(n, M, 200.0, 100.0)
        self.assertEqual((50.0, 30.0), (n.w, n.h))


if __name__ == "__main__":
    unittest.main()
