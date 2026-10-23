# nativelayout

**Create native interfaces from HTML, in any language.**

A screen is written once, in a plain text file (`.nl`) that looks like HTML, with `{values}`, `if` / `for` lines and a `<style>`. This package is the Python implementation: a pure Python core (parser, expander, layout engine and differ) and a Kivy host that draws the screen.

The same `.nl` file also runs on [Kotlin / Android](https://github.com/getnativelayout/nativelayout/tree/main/android) and in the [online editor](https://nativelayoutpad.vercel.app/).

## Install

```
pip install nativelayout            # core only, pure Python, no dependencies
pip install "nativelayout[kivy]"    # core + Kivy host
```

Python 3.10 or newer. The host needs Kivy 2.x. On Android, package the app with buildozer (`requirements = python3,kivy,nativelayout`).

## Example

`screen.nl`:

```html
<nativelayout>
<style>
body { padding: 16px; gap: 12px; }
.mais { background-color: #2F81F7; color: white; border-radius: 20px; }
</style>
<body>
<h1>Hello, {name}</h1>
<p>Total: {n}</p>
<button class="mais">+ add</button>
</body>
</nativelayout>
```

`main.py`:

```python
import kivy  # noqa: F401
from nativelayout.core.program import state, view, click
from nativelayout.host import run_app, file_loader

def screen():
    s = state({"n": 0, "name": "world"})
    if click(".mais"):
        s["n"] += 1
    return view("screen.nl", s)

run_app(file_loader("."), app=screen)
```

The file describes the screen. The logic (state and taps) stays in your Python code, in a screen function.

## What is in the package

- `nativelayout.core`: parser, expander, layout engine, differ and the program side (`state`, `view`, `click`, `go`, `value`, `save`, ...). Pure Python.
- `nativelayout.host`: the Kivy host. It draws on a canvas; it does not build platform-native views.

`nativelayout` is a namespace package, so both parts install under the same name.

## Supported

Columns and rows, `flex`, `gap`, `justify`, `align`, wrap, `header`, `footer` and `dock`, dialogs, `<include>`, identity inside `for`, `position: absolute`, `background-image`, `object-fit`, built-in icons, gradients, shadows and rounded corners.

Not done yet in the host: recycling in long lists, the slide animations (only the fade-in), `letter-spacing` and `overflow: hidden`.

## Links

- [Website](https://getnativelayout.github.io/nativelayout/)
- [Specification](https://github.com/getnativelayout/nativelayout/blob/main/SPEC.md)
- [Source and issues](https://github.com/getnativelayout/nativelayout)
- [Online editor](https://nativelayoutpad.vercel.app/)

## License

Mozilla Public License 2.0. Copyright (c) 2026 Adilson C. Rafael.
