# nativelayout - Specification: The program side

[Index](../../SPEC.md) | Previous: [Values, control and data](values-and-data.md) | Next: [Errors and targets](errors-and-targets.md)

---

## 8. The program side

The file only describes. The logic (state, taps) is written in each language, with the same verbs. Each screen has its own logic file next to its `.nl`, and the entry point (`MainActivity`, `main.py`) only prepares and starts the first screen.

```
app/
  MainActivity.kt        (only prepares)
  login.nl   login.css   LoginScreen.kt
  panel.nl   panel.css   PanelScreen.kt
```

### 8.1 The screen function

A screen is one function. It holds the state, reads the taps and returns what to show. The engine calls it, shows what it returns, waits for a tap and calls it again, until the window closes.

JS (illustrative):

```js
export default function PanelScreen() {
  const s = state({ pending: 0 })

  if (click("add")) s.pending++

  return view("panel.nl", s)
}
```

Kotlin:

```kotlin
fun NativeView.PanelScreen(): Page {
    val s = state { mutableMapOf<String, Any?>("pending" to 0) }

    if (click("add")) s["pending"] = (s["pending"] as Int) + 1

    return view("panel.nl", s)
}
```

Python (`import nativelayout as nl`):

```python
def panel_screen():
    s = nl.state({"pending": 0})

    if nl.click("add"):
        s["pending"] += 1

    return nl.view("panel.nl", s)
```

Going to another screen:

```kotlin
fun NativeView.LoginScreen(): Page {
    val s = state { mutableMapOf<String, Any?>("failed" to false) }

    if (click("login")) {
        if (value("pass") == "1234") return go { PanelScreen() }
        s["failed"] = true
    }

    return view("login.nl", s)
}
```

### 8.2 Verbs

In Python the verbs are used as `nl.state`, `nl.click`, ... after `import nativelayout as nl`; the column "Python style" gives the name.

| Verb | Python style | Does |
|---|---|---|
| `state(initial)` | `state` | the state of the screen: the same object on every call, a new one after `go` |
| `view(screen, data)` | `view` | returns the screen to show, with its data |
| `go(function)` | `go` | returns a jump to another screen function; the current one stays behind it, so `goBack` can return (section 8.6) |
| `goRoot(function)` | `go_root` | like `go`, and forgets the screens behind (after a login or a logout) |
| `goBack()` | `go_back` | returns to the screen that jumped here (section 8.6) |
| `canGoBack()` | `can_go_back` | `true` when there is a screen to return to |
| `back()` | `back` | `true` when the system back button woke the program (section 8.6) |
| `click(target)` | `click` | `true` if the `id` (or `".class"`) was tapped |
| `clicked(name)` | `clicked` | the item of the `for name in ...` where the last tap happened (section 5.7) |
| `clickIndex()` | `click_index` | position, from 0, of the tapped element of the class |
| `value(id)`, `value(id, item...)` | `value` | text of a field, `""` if the id does not exist. For an element inside a `for`, give the item (section 5.7) |
| `checked(id)`, `checked(id, item...)` | `checked` | state of a checkbox, `false` if it does not exist |
| `setValue(id, text)`, `setValue(id, item..., text)` | `set_value` | fills or clears a field (`"1"` / `"0"` for a checkbox) |
| `save(key, text)`, `load(key)`, `exists(key)`, `delete(key)` | same | store text on the device |
| `render(screen, data)` | `render` | base layer (section 8.5): shows the screen, waits for a tap, returns `true`; `false` when the window closes |

### 8.3 Rules

- The function is called again after every tap. In the first call there is no tap yet, so `click` is `false`.
- `state(initial)` gives the same object on every call, so what changed survives. Call it at the top, always in the same order, never inside `if` or `for`.
- The data given to `view` is plain, as in section 7.
- `go(function)` leaves the current screen. Its state is dropped, the tree is built from scratch (section 9, point 6) and the new function is called at once, with no tap yet. More than 8 `go` in a row without a `view` is an error.
- A screen file is read once and kept, so `view` is cheap on every call.
- Coming back to a screen (with `goBack`) starts it again with a new state. To keep something between screens, use `save` and `load`.
- A screen shown with `go` or `goBack` appears with the default animation (section 4.6). A `<dialog>` that appears does the same.

Examples of `go`:

A loop that never shows a screen is an error. Here each function jumps to the other, so at the ninth jump in a row the engine stops with `go: too many jumps without showing a screen`. A function that always jumps to itself fails the same way.

```kotlin
fun NativeView.AScreen(): Page = go { BScreen() }
fun NativeView.BScreen(): Page = go { AScreen() }
```

A few jumps in a row are fine. Every `view` resets the count, so up to 8 jumps can come before a screen is shown:

```kotlin
fun NativeView.SplashScreen(): Page {
    if (!exists("token")) return go { LoginScreen() }
    return go { HomeScreen() }
}
```

The usual case is a jump that depends on a tap:

```kotlin
fun NativeView.LoginScreen(): Page {
    val s = state { mutableMapOf<String, Any?>("failed" to false) }

    if (click("login")) {
        if (value("pass") == "1234") return go { HomeScreen() }
        s["failed"] = true
    }

    return view("login.nl", s)
}
```

### 8.4 The entry point

The entry point creates the area and the loader and names the first screen function. It has no logic. The parameter names are provisional; use named arguments.

```kotlin
class MainActivity : Activity() {
    private lateinit var ui: NativeView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val area = FrameLayout(this).apply {
            setBackgroundColor(Color.parseColor("#0D1117"))
        }
        setContentView(area)

        ui = NativeView(
            context = this,
            area = area,
            loader = { name -> runCatching { assets.open(name).bufferedReader().use { it.readText() } }.getOrNull() },
            app = { LoginScreen() }
        )
    }

    override fun onDestroy() {
        ui.close()
        super.onDestroy()
    }
}
```

### 8.5 Base layer: the `render` loop

The screen function runs on top of this loop, which stays available for programs that want to control it themselves.

Kotlin (Android):

```kotlin
class MainActivity : Activity() {
    private lateinit var ui: NativeView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ui = NativeView(this) {
            val panel = screen("panel.nl")
            var pending = 0

            while (render(panel, mapOf("pending" to pending))) {
                if (click("add")) pending++
            }
        }
    }

    override fun onDestroy() {
        ui.close()
        super.onDestroy()
    }
}
```

Python (`run_app` with a `body`; `render` is a coroutine because the Kivy loop is asynchronous):

```python
from nativelayout.host import run_app

async def body(ui):
    panel = ui.screen("panel.nl")
    pending = 0

    while await ui.render(panel, {"pending": pending}):
        if ui.click("add"):
            pending += 1

run_app(".", body=body)
```

### 8.6 Going back and the back button

`go` keeps a stack: the function that jumps stays behind the new one. `goBack()` is the opposite of `go`: it drops the current screen and shows the one that jumped here, starting it again with a new state.

```kotlin
fun NativeView.AddScreen(): Page {
    if (click("cancel")) return goBack()
    if (click("save")) { saveTask(value("title")); return goBack() }
    return view("add.nl", mapOf())
}
```

- `go` adds the current screen to the stack. `goRoot` empties it: after a login, the back button must not return to the login screen.
- `goBack()` on the first screen (empty stack) leaves the screen as it is and calls the exit callback of the host, if there is one.
- `canGoBack()` tells whether the stack has a screen.
- **The system back button.** The host tells the program, it does not handle it alone. `back()` is `true` when the button woke the program. A screen that never asks for it gets the default: `goBack()`, or the exit callback on the first screen. A screen that asks for it takes over, for example to close a dialog first:

```kotlin
fun NativeView.HomeScreen(): Page {
    val s = state { mutableMapOf<String, Any?>("confirm" to false) }

    if (back() || click("cancel")) {
        if (s["confirm"] == true) s["confirm"] = false      // the dialog closes
        else return goBack()                                // otherwise the default
    }
    ...
}
```

- `back()` and the default apply once per tap: the button is one more event in the queue, like a tap, and `click(...)` is `false` for it.
- Each host connects the system button its own way. On Android the activity calls `ui.pressBack()` (see `android/README.md`).

---

## 9. render

1. The first call builds the native components.
2. The next calls compare with the previous screen and change only what differs. Elements are matched by `id`, by `key` or by position (section 5.3). The text typed in a field with an `id` is kept.
3. `value` and `checked` in the file are default values: they are applied when the field is created and again only when the value in the data is different from the previous `render`. What the user typed in between is not touched.
4. The program waits inside `render` until there is a tap on an element with an `id` or a class. Typing does not wake it.
5. Each tap goes into a queue and each `render` consumes one, in order.
6. Passing a different screen resets the tree and builds it again.
7. When the window closes, `render` returns `false` and the loop ends.

---

[Index](../../SPEC.md) | Previous: [Values, control and data](values-and-data.md) | Next: [Errors and targets](errors-and-targets.md)

License: [CC BY-SA 4.0](../../LICENSE-SPEC). Copyright (c) 2026 Adilson C. Rafael.
