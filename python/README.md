# nativelayout (Python)

Python port of the nativelayout core. It follows the Kotlin core feature for feature
(currently v1.2.0: `<include>`, identity inside `for`, `<footer>` and `dock`,
`position: absolute`, `background-image`, `object-fit`).

- `core/`     pure Python, no Kivy: parser, expander, layout engine, differ, program side
- `android/`  Kivy host + Android app
- `desktop/`  Kivy host + desktop app
- `examples/`

The Kivy hosts draw on a canvas; they do not build platform-native views.

`nativelayout` is a namespace package (no `__init__.py` at its root): the core lives in
`core/nativelayout/core` and each host adds `nativelayout/host` in its own folder.
To use the core, put `python/core` on `sys.path`.

## Tests

```
cd python/core
python -m unittest discover -s tests -t .
```

The tests mirror the Kotlin ones (`IncludeTest`, `FooterTest`, `AbsoluteTest`).
