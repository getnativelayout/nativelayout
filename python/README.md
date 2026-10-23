# nativelayout (Python)

Python port of the nativelayout core. It follows the Kotlin core feature for feature
(currently v1.2.0: `<include>`, identity inside `for`, `<footer>` and `dock`,
`position: absolute`, `background-image`, `object-fit`).

- `core/`     pure Python, no Kivy: parser, expander, layout engine, differ, program side
- `android/`  Kivy host for Android (`nativelayout/host`)
- `desktop/`  Kivy host for the desktop (`nativelayout/host`)
- `examples/` examples (`contador/`)

The Kivy hosts draw on a canvas; they do not build platform-native views.

`nativelayout` is a namespace package (no `__init__.py` at its root): the core lives in
`core/nativelayout/core` and each host brings `nativelayout/host`. Put `core` and the folder of the
host on `sys.path`.

## Usage

```python
import kivy  # noqa: F401  (Pydroid starts the Kivy window only if the opened file imports kivy)
from nativelayout.core.program import state, view, click
from nativelayout.host import run_app, file_loader

def screen():
    s = state({"n": 0})
    if click(".more"):
        s["n"] += 1
    return view("screen.nl", s)

run_app(file_loader("."), app=screen)
```

Dependency: Kivy 2.x. On Android, package with buildozer (`requirements = python3,kivy`).

## Tests

```
cd python/core
python -m unittest discover -s tests -t .

cd python/android
python -m unittest discover -s tests -t .
```

The core tests mirror the Kotlin ones (`IncludeTest`, `FooterTest`, `AbsoluteTest`). The host tests
cover `flex.py`, which does not need Kivy.

## Status of the host (v1.2.0)

Done: columns and rows, `flex`, `gap`, `justify`, `align`, wrap, header, footer and `dock`, dialogs,
`position: absolute` (with `z-index`), `background-image` (`background-size`, `background-position`),
`object-fit`, icons, gradients, shadows, rounded corners.

Not done yet:
- `virtual` lists draw all items (no recycling)
- animations: only the fade-in of the screen (slide-up and slide-left are missing)
- `letter-spacing` and `overflow: hidden` (clipping) are not applied
- the Android keyboard can cover fields (adjust `softinput_mode`)
- the Kivy drawing of `background-image` and `object-fit` has not been tried on a device yet
