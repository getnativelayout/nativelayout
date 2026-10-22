"""Program side: the verbs of the spec (section 8) on top of the engine.

The part of `NativeView` that does not depend on the platform. Each host (Kivy/android, Kivy/desktop)
provides the `LayoutHost`, the `loader` and the dark theme.

Two styles, as in the spec:
  - screen functions: `def panel(): s = state({...}); if click("add"): ...; return view("panel.nl", s)`
  - base layer:       `while await ui.render(screen, data): ...`
"""
from __future__ import annotations

import asyncio
import contextvars
import traceback
from concurrent.futures import ThreadPoolExecutor
from typing import Callable

from .errors import NativeLayoutError
from .expander import Expander
from .layout_host import FilledMarkup, LayoutHost, LayoutSession

MAX_HOPS = 8


class Page:
    """What a screen function returns: `view(...)`, `go(...)` or `go_back()`."""


class ViewPage(Page):
    def __init__(self, screen: "Screen", data: dict):
        self.screen = screen
        self.data = data


class GoPage(Page):
    def __init__(self, next_: Callable[[], Page], root: bool = False):
        self.next = next_
        self.root = root


class BackPage(Page):
    pass


_BACK = BackPage()


class Screen:
    def __init__(self, name: str, source: str):
        self.name = name
        self.source = source


_current: contextvars.ContextVar["NativeViewBase | None"] = contextvars.ContextVar(
    "nativelayout_current", default=None)


class NativeViewBase:
    def __init__(self, host: LayoutHost, loader: Callable[[str], str | None],
                 app: Callable[[], Page] | None = None, body=None,
                 is_dark: Callable[[], bool] = lambda: False):
        self._host = host
        self._loader = loader
        self._app = app
        self._body = body
        self._is_dark = is_dark
        self._session = LayoutSession(host)
        self._worker = ThreadPoolExecutor(max_workers=1, thread_name_prefix="nativelayout-prepare")
        self._closed = False
        self._screens: dict[str, Screen] = {}
        self._slots: list = []
        self._slot_index = 0
        self._fresh = False
        self._history: list[Callable[[], Page]] = []
        self._back_handled = False
        self._last_shown: ViewPage | None = None
        # called when the back button is pressed on the first screen and the program did not handle it
        self.on_exit: Callable[[], None] | None = None

    # ---- base layer
    def screen(self, name: str) -> Screen:
        source = self._loader(name)
        if source is None:
            raise ValueError(f"screen not found: {name}")
        return Screen(name, source)

    async def render(self, screen: Screen, data: dict | None = None) -> bool:
        if not isinstance(screen, Screen):
            raise TypeError("render: expected a screen")
        if self._closed:
            return False
        data = data if data is not None else {}

        def work():
            ex = Expander.expand(screen.source, data, lambda n: self._loader(n), self._is_dark())
            return self._session.prepare(FilledMarkup(ex.html, 1)), ex

        loop = asyncio.get_running_loop()
        prepared, expanded = await loop.run_in_executor(self._worker, work)
        self._session.set_scope_items(expanded.items)
        for w in expanded.warnings:
            if not any(x.message == w.message and x.line == w.line for x in self._session.warnings):
                self._session.warnings.append(w)
        return await self._session.commit(prepared)

    # ---- screen functions
    def state(self, initial):
        i = self._slot_index
        self._slot_index += 1
        if i == len(self._slots):
            self._slots.append(initial() if callable(initial) else initial)
        return self._slots[i]

    def view(self, name: str, data: dict | None = None) -> Page:
        scr = self._screens.get(name)
        if scr is None:
            scr = self._screens[name] = self.screen(name)
        return ViewPage(scr, data if data is not None else {})

    def go(self, next_: Callable[[], Page]) -> Page:
        return GoPage(next_)

    def go_root(self, next_: Callable[[], Page]) -> Page:
        return GoPage(next_, root=True)

    def go_back(self) -> Page:
        return _BACK

    def can_go_back(self) -> bool:
        return bool(self._history)

    def back(self) -> bool:
        if self._fresh or not self._session.is_back():
            return False
        self._back_handled = True
        return True

    def press_back(self) -> None:
        if not self._closed:
            self._host.post_back()

    async def run(self) -> None:
        """Runs the program until the window closes. An error is shown in place of the screen."""
        try:
            if self._app is not None:
                await self._run_app(self._app)
            elif self._body is not None:
                token = _current.set(self)
                try:
                    await self._body(self)
                finally:
                    _current.reset(token)
        except Exception as error:
            traceback.print_exc()
            self._host.show_error(error.format() if isinstance(error, NativeLayoutError)
                                  else (str(error) or repr(error)))

    async def _run_app(self, app: Callable[[], Page]) -> None:
        current = app
        hops = 0
        while not self._closed:
            self._slot_index = 0
            self._back_handled = False
            token = _current.set(self)
            try:
                page = current()
            finally:
                _current.reset(token)
            if not self._fresh and not self._back_handled and self._session.is_back():
                if self._history:
                    page = _BACK
                elif self.on_exit is not None:
                    self.on_exit()
            if isinstance(page, BackPage) and not self._history:
                # nothing to go back to: the screen stays as it is
                if self.on_exit is not None:
                    self.on_exit()
                if self._last_shown is None:
                    raise RuntimeError("goBack: there is no screen to return to")
                page = self._last_shown
            if isinstance(page, GoPage):
                hops += 1
                if hops > MAX_HOPS:
                    raise RuntimeError("go: too many jumps without showing a screen")
                if page.root:
                    self._history.clear()
                else:
                    self._history.append(current)
                current = page.next
                self._last_shown = None
                self._slots.clear()
                self._fresh = True
                self._session.reset()
            elif isinstance(page, BackPage):
                hops += 1
                if hops > MAX_HOPS:
                    raise RuntimeError("go: too many jumps without showing a screen")
                current = self._history.pop()
                self._last_shown = None
                self._slots.clear()
                self._fresh = True
                self._session.reset()
            elif isinstance(page, ViewPage):
                hops = 0
                self._last_shown = page
                if not await self.render(page.screen, page.data):
                    return
                self._fresh = False
            else:
                raise TypeError("a screen function must return view(...), go(...) or go_back()")

    # ---- taps and fields (after a jump there is no tap yet: they answer "nothing")
    def click(self, target: str) -> bool:
        return (not self._fresh) and self._session.click(target)

    def click_index(self) -> int:
        return -1 if self._fresh else self._session.click_index()

    def clicked(self, name: str):
        return None if self._fresh else self._session.clicked_item(name)

    def value(self, id: str, *items) -> str:
        if self._fresh:
            return ""
        r = self._session.field_text(id, list(items))
        return r if r is not None else ""

    def checked(self, id: str, *items) -> bool:
        if self._fresh:
            return False
        r = self._session.field_checked(id, list(items))
        return r if r is not None else False

    def set_value(self, id: str, *args) -> None:
        """`set_value(id, text)` or `set_value(id, item..., text)`: the last argument is the text."""
        if not args or not isinstance(args[-1], str):
            raise TypeError("set_value(id, item..., text): the last argument must be the text")
        self._session.set_value(id, args[-1], list(args[:-1]))

    def save(self, key: str, text: str) -> None:
        self._session.save(key, text)

    def load(self, key: str) -> str | None:
        return self._session.load(key)

    def exists(self, key: str) -> bool:
        return self._session.exists(key)

    def delete(self, key: str) -> None:
        self._session.delete(key)

    def close(self) -> None:
        self._closed = True
        self._worker.shutdown(wait=False)
        self._host.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False


# ---- free verbs, bound to the NativeView that is calling the screen function
def _ui() -> NativeViewBase:
    ui = _current.get()
    if ui is None:
        raise RuntimeError("this verb can only be used inside a screen function")
    return ui


def screen(name): return _ui().screen(name)
def state(initial): return _ui().state(initial)
def view(name, data=None): return _ui().view(name, data)
def go(next_): return _ui().go(next_)
def go_root(next_): return _ui().go_root(next_)
def go_back(): return _ui().go_back()
def can_go_back(): return _ui().can_go_back()
def back(): return _ui().back()
def click(target): return _ui().click(target)
def click_index(): return _ui().click_index()
def clicked(name): return _ui().clicked(name)
def value(id, *items): return _ui().value(id, *items)
def checked(id, *items): return _ui().checked(id, *items)
def set_value(id, *args): return _ui().set_value(id, *args)
def save(key, text): return _ui().save(key, text)
def load(key): return _ui().load(key)
def exists(key): return _ui().exists(key)
def delete(key): return _ui().delete(key)
