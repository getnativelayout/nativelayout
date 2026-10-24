# nativelayout for Python: guide

This guide shows how to write a program with the Python package. The language of the `.nl` files (tags, `{values}`, `if`, `for`, style) is the same as in every implementation and is described in the [specification](https://github.com/getnativelayout/nativelayout/blob/main/SPEC.md). Here is only the Python side: how to start, the verbs and what the Kivy host does.

## 1. Install and start

```
pip install "nativelayout[kivy]"
```

Python 3.10 or newer, Kivy 2.x. The core alone (`pip install nativelayout`) is pure Python and has no dependencies; it is what you use for tests and tools.

One import is enough:

```python
import kivy  # Pydroid only starts the Kivy window if the opened file imports kivy
import nativelayout as nl

def screen():
    s = nl.state({"n": 0})
    if nl.click(".more"):
        s["n"] += 1
    return nl.view("screen.nl", s)

nl.run(screen)
```

`nl.run(screen)` opens the window and reads the `.nl` files from the folder of your script. The same verbs can be imported by name (`from nativelayout import state, view, click`), but with `nl.` the reader sees at once that the function comes from the library.

## 2. The screen function

A screen is one function. It holds the state, reads the taps and returns what to show. The engine calls it, shows what it returns, waits for a tap and calls it again, until the window closes.

- In the first call there is no tap yet, so `nl.click(...)` is `False`.
- `nl.state(initial)` returns the same object on every call, so what changed survives. Call it at the top, always in the same order, never inside `if` or `for`.
- The function must return `nl.view(...)`, `nl.go(...)`, `nl.go_root(...)` or `nl.go_back()`. Anything else is an error.
- The data given to `view` is plain: dicts, lists, strings, numbers and booleans.

`state` accepts a dict, or a class: `nl.state(HomeState)` gives an instance whose attributes you read and write (`s.filter = "all"`). The Despesas example uses classes.

## 3. The verbs

| Verb | Does |
|---|---|
| `nl.state(initial)` | state of the screen: the same object on every call, a new one after `go` |
| `nl.view(name, data)` | the screen to show (a `.nl` file) with its data |
| `nl.go(function)` | jumps to another screen function; the current one stays behind it |
| `nl.go_root(function)` | like `go`, and forgets the screens behind (after a login or a logout) |
| `nl.go_back()` | returns to the screen that jumped here |
| `nl.can_go_back()` | `True` when there is a screen to return to |
| `nl.back()` | `True` when the system back button woke the program |
| `nl.click(target)` | `True` if the `id` (or `".class"`) was tapped |
| `nl.clicked(name)` | the item of the `for name in ...` where the last tap happened |
| `nl.click_index()` | position, from 0, of the tapped element of the class |
| `nl.value(id)` | text of a field, `""` if the id does not exist |
| `nl.checked(id)` | state of a checkbox, `False` if it does not exist |
| `nl.set_value(id, text)` | fills or clears a field (`"1"` / `"0"` for a checkbox) |
| `nl.save(key, text)`, `nl.load(key)`, `nl.exists(key)`, `nl.delete(key)` | store text on the device |

For an element inside a `for`, `value`, `checked` and `set_value` also take the item, as in the specification (section 5.7). These verbs only work inside a screen function; elsewhere they raise `RuntimeError`.

## 4. Several screens

```python
def login():
    s = nl.state({"failed": False})
    if nl.click("login"):
        if nl.value("pass") == "1234":
            return nl.go(panel)
        s["failed"] = True
    return nl.view("login.nl", s)

def panel():
    if nl.click("logout"):
        return nl.go_root(login)     # forgets the screens behind
    if nl.click("cancel"):
        return nl.go_back()
    return nl.view("panel.nl", {})

nl.run(login)
```

- `go` leaves the current screen: its state is dropped and the new function is called at once, with no tap yet. More than 8 jumps in a row without a `view` is an error (`go: too many jumps without showing a screen`).
- Coming back with `go_back` starts the screen again with a new state. To keep something between screens, use `save` and `load`.
- The system back button needs no code: `go_back()` is the default. Use `nl.back()` only to give the button another meaning, for example closing a dialog first.
- On the first screen, back calls the exit of the app.

## 5. Dialogs, lists and taps in a list

Dialogs and lists are written in the `.nl` file (see the specification). In Python you only decide when they appear, with the data:

```python
def home():
    s = nl.state({"confirm": ""})
    if nl.click("remove"):
        s["target"] = nl.clicked("e")["id"]     # "e" is the name used in: for e in items
        s["confirm"] = "delete"
    if nl.click("confirm-no") or (s["confirm"] != "" and nl.back()):
        s["confirm"] = ""
    return nl.view("home.nl", {"items": ITEMS, "confirm": s["confirm"]})
```

`nl.click(".chip")` is true for any element of the class; `nl.click_index()` tells which one.

## 6. Storage

`nl.save(key, text)` and `nl.load(key)` keep text in a JSON file in the data folder of the app (on Android, the private folder of the app). Store a JSON string for structured data (`json.dumps` / `json.loads`). To choose the file, pass `store_path` to `nl.run`.

## 7. Options of `nl.run`

```python
nl.run(screen, folder=None, title="nativelayout", dark=None, image_resolver=None, store_path=None)
```

| Option | Meaning |
|---|---|
| `folder` | where the `.nl` files are; by default, the folder of the running script |
| `title` | title of the window |
| `dark` | `True` / `False` to force the theme, or a function; by default it follows the system on Android |
| `image_resolver` | `function(source) -> Kivy texture or None`, for `<img src>` and `background-image: url(...)` |
| `store_path` | file used by `save` and `load` |

`nl.run` uses a screen function. The other form of the program, a loop with `await ui.render(...)`, is available through `run_app(loader, body=...)` (from `nativelayout.host`) and `NativeView`. `loader` is a folder path or a function `name -> text | None`, so files can come from anywhere (an archive, the network, memory).

## 8. Images

`<img src="photo.png">` and `background-image: url(photo.png)` ask the `image_resolver` for a texture. The Imagens example shows a resolver with `kivy.core.image.Image`. `object-fit` (contain, cover, fill), `background-size` and `background-position` work as in the specification, including with rounded corners.

## 9. Running

- **Desktop:** `python main.py`.
- **Pydroid:** the file you open must contain `import kivy`; otherwise Pydroid does not start the Kivy window. The system keyboard does not open inside Pydroid; to type in fields, build an APK (below). Pasting text into a field works.
- **Android APK:** package with buildozer and put `requirements = python3,kivy,nativelayout` in `buildozer.spec`.

## 10. Without Kivy (tests and tools)

The core has no dependency on Kivy. It parses, expands and lays out a screen to a tree you can inspect, and it is what the tests of the package use. Importing `nativelayout` never loads Kivy; only `nl.run`, `run_app`, `file_loader`, `NativeView` and `system_is_dark` do, and if Kivy is missing they say how to install it.

## 11. Status of the Kivy host

Done: columns and rows, `flex`, `gap`, `justify`, `align`, wrap, header, footer and `dock`, dialogs, `position: absolute` with `z-index`, `background-image`, `object-fit`, icons, gradients, shadows, rounded corners, `<include>`, stretch effect when scrolling past the end.

Not done yet:

- `virtual` lists draw all the items (no recycling);
- animations: only the fade-in of the screen (slide-up and slide-left are missing);
- `letter-spacing` and `overflow: hidden` (clipping) are not applied;
- the Android keyboard can cover fields (adjust `softinput_mode`).

The host draws on a Kivy canvas; it does not create platform views. The result follows the same layout rules as the Kotlin host, but small differences in text metrics can exist.

## 12. Examples

In the `examples` folder of the repository:

- `contador`: the smallest program (state, a tap, a list).
- `despesas`: a real app with three screens, a confirmation dialog, `save` / `load` and a theme, the port of the Android example.
- `imagens`: image fit, background images, absolute position and `z-index`.
