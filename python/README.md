# nativelayout

**Describe a screen in HTML-like text and get a native interface.**

nativelayout is a declarative language for user interfaces. You write a screen once, in a plain text file (`.nl`) that looks like HTML, and each programming language shows it with its own host. There is no WebView.

A `.nl` file has:

- HTML tags you already know (`div`, `p`, `button`, `input`, `img`, ...);
- `{values}` that your code fills in;
- `if` and `for` lines, for conditions and repetition;
- a `<style>` with simple CSS selectors.

The file only describes the screen. The logic (state and taps) stays in your own code, in a screen function.

nativelayout is a specification with implementations: [Kotlin for Android](https://github.com/getnativelayout/nativelayout/tree/main/android), Python (this package) and JavaScript (used by the [online editor](https://nativelayoutpad.vercel.app/)). The same `.nl` file works unchanged in all of them.

**This package** is the Python implementation: a pure Python core (parser, expander, layout engine and differ) and a Kivy host that draws the screen.

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
from nativelayout import state, view, click, run_app, file_loader

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

Everything is also available from the top level: `from nativelayout import state, view, click, run_app, file_loader`. The Kivy host is loaded only when you use it.

## Links

- [Website](https://getnativelayout.github.io/nativelayout/)
- [Specification](https://github.com/getnativelayout/nativelayout/blob/main/SPEC.md)
- [Source and issues](https://github.com/getnativelayout/nativelayout)
- [Online editor](https://nativelayoutpad.vercel.app/)

## License

Mozilla Public License 2.0. Copyright (c) 2026 Adilson C. Rafael.
