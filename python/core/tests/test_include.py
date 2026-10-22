"""`<include>` and the identity of the repetitions of a `for` (mirrors IncludeTest.kt)."""
import unittest

from nativelayout.core import ExpandError, Expander, scopes


def screen(body: str) -> str:
    return "<nativelayout>\n\n<body>\n%s\n</body>\n\n</nativelayout>\n" % body


CARD = screen('    <div class="card">\n        <p>{name}</p>\n        <button id="buy">Buy</button>\n    </div>')
FILES = {"card.nl": CARD}
PRODUCTS = [{"id": 7, "name": "Tea"}, {"id": 9, "name": "Rice"}]


def expand(src, data=None, files=FILES):
    return Expander.expand(src, data or {}, lambda n: files.get(n))


class IncludeTest(unittest.TestCase):
    def test_include_takes_the_place_of_the_tag(self):
        r = expand(screen('    <include src="card.nl" name="Hello">'))
        self.assertIn("<p>Hello</p>", r.html)
        self.assertNotIn("<include", r.html)

    def test_ids_in_a_for_are_scoped_by_the_key_of_the_item(self):
        r = expand(screen('for p in products:\n    <include src="card.nl" name="{p.name}">\nend'),
                   {"products": PRODUCTS})
        self.assertIn('id="buy~p:7"', r.html)
        self.assertIn('id="buy~p:9"', r.html)
        self.assertEqual(PRODUCTS[0], r.items["p:7"])
        self.assertEqual(PRODUCTS[1], r.items["p:9"])

    def test_ids_in_a_for_without_key_use_the_position(self):
        r = expand(screen('for t in names:\n    <button id="x">{t}</button>\nend'), {"names": ["a", "b"]})
        self.assertIn('id="x~t:%p0"', r.html)
        self.assertIn('id="x~t:%p1"', r.html)

    def test_id_outside_a_for_stays_as_written(self):
        r = expand(screen('    <button id="ok">Ok</button>'))
        self.assertIn('id="ok"', r.html)
        self.assertNotIn("scope=", r.html)

    def test_nested_fors_give_nested_paths(self):
        src = screen('for a in as_:\n    for b in bs:\n        <button id="x">{b.id}</button>\n    end\nend')
        r = expand(src, {"as_": [{"id": 1}], "bs": [{"id": 2}]})
        self.assertIn('id="x~a:1/b:2"', r.html)
        self.assertIn("a:1", r.items)
        self.assertIn("a:1/b:2", r.items)

    def test_missing_parameter_names_the_component_and_the_include_line(self):
        with self.assertRaises(ExpandError) as cm:
            expand(screen('    <include src="card.nl">'))
        self.assertIn("card.nl", str(cm.exception))
        self.assertEqual(4, cm.exception.line)

    def test_include_that_includes_itself_is_an_error(self):
        loop = screen('    <include src="loop.nl">')
        with self.assertRaises(ExpandError) as cm:
            expand(loop, files={"loop.nl": loop})
        self.assertIn("includes itself", str(cm.exception))

    def test_scopes_helpers(self):
        self.assertEqual("7", scopes.key_of_item(PRODUCTS[0]))
        self.assertTrue(scopes.same_item({"id": 7, "name": "Other"}, PRODUCTS[0], "7"))


if __name__ == "__main__":
    unittest.main()
