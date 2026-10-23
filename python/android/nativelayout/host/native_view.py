"""NativeView for Kivy (Android and desktop): the widget of the screen and the program that drives it."""
from __future__ import annotations

import asyncio
import os

from nativelayout.core.program import NativeViewBase

from .kivy_host import KivyLayoutHost, NLArea


def system_is_dark() -> bool:
    """Dark theme of the system (Android only; on the desktop it returns False)."""
    try:
        from jnius import autoclass
        activity = autoclass("org.kivy.android.PythonActivity").mActivity
        mode = activity.getResources().getConfiguration().uiMode
        return (mode & 0x30) == 0x20
    except Exception:
        return False


def file_loader(base_dir: str):
    """`loader(name) -> text | None` that reads files of a folder (without leaving it)."""
    base = os.path.abspath(base_dir)

    def load(name: str) -> str | None:
        path = os.path.abspath(os.path.join(base, name))
        if not path.startswith(base + os.sep) or not os.path.isfile(path):
            return None
        with open(path, "r", encoding="utf-8") as f:
            return f.read()
    return load


class NativeView(NativeViewBase):
    """`NativeView(loader, app=first_screen)` or `NativeView(loader, body=async_def_body)`.

    `ui.widget` is the Kivy widget to put in the window. `ui.start()` starts the program (it needs the
    asyncio loop of Kivy: `App.async_run`). The Android back button calls `press_back()`.
    """

    def __init__(self, loader, app=None, body=None, dark=None, image_resolver=None, store_path=None):
        self.widget = NLArea()
        host = KivyLayoutHost(self.widget, image_resolver, store_path)
        super().__init__(host, loader, app=app, body=body,
                         is_dark=(dark if callable(dark) else (lambda: bool(dark))) if dark is not None else system_is_dark)
        self.on_exit = self._default_exit
        from kivy.core.window import Window
        Window.bind(on_keyboard=self._on_keyboard)
        self._task: asyncio.Future | None = None

    def start(self) -> None:
        self._task = asyncio.ensure_future(self.run())

    def _on_keyboard(self, _window, key, *_args) -> bool:
        if key == 27:           # Android back button (Esc on the desktop)
            self.press_back()
            return True
        return False

    @staticmethod
    def _default_exit() -> None:
        from kivy.app import App
        app = App.get_running_app()
        if app is not None:
            app.stop()


def run_app(loader, app=None, body=None, title: str = "nativelayout", **options) -> None:
    """Opens a Kivy window with the program. A shortcut for scripts and examples."""
    from kivy.app import App

    class _App(App):
        def build(self):
            self.title = title
            self.ui = NativeView(loader, app=app, body=body, **options)
            return self.ui.widget

        def on_start(self):
            self.ui.start()

        def on_stop(self):
            self.ui.close()

    asyncio.run(_App().async_run(async_lib="asyncio"))
