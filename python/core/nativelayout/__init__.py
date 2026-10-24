"""nativelayout: one import for programs.

    import nativelayout as nl

    def screen():
        s = nl.state({"n": 0})
        if nl.click(".more"):
            s["n"] += 1
        return nl.view("screen.nl", s)

    nl.run(screen)

The core (`nativelayout.core`) is pure Python and needs nothing else. `run`, `run_app`, `file_loader` and the
other names of the Kivy host are loaded only when they are used, so the core works without Kivy.
"""
import os
import sys
from pkgutil import extend_path

# `nativelayout.core` and `nativelayout.host` live in separate folders of the repository.
__path__ = extend_path(__path__, __name__)

from .core.program import (Page, back, can_go_back, checked, click, click_index, clicked, delete, exists,  # noqa: E402
                           go, go_back, go_root, load, save, screen, set_value, state, value, view)

_HOST_NAMES = ("run_app", "file_loader", "NativeView", "system_is_dark")

__all__ = ["run", "Page", "back", "can_go_back", "checked", "click", "click_index", "clicked", "delete", "exists",
           "go", "go_back", "go_root", "load", "save", "screen", "set_value", "state", "value", "view",
           *_HOST_NAMES]


def _host():
    try:
        from . import host  # needs Kivy
    except ImportError as e:
        raise ImportError(
            f"the Kivy host could not be loaded ({e}). Install it with: pip install 'nativelayout[kivy]'") from e
    return host


def run(app, folder=None, **options):
    """Opens a window with the program `app` (a screen function).

    `folder` is where the .nl files are; by default, the folder of the script that is running.
    The other options (`title`, ...) go to `run_app`.
    """
    if folder is None:
        path = getattr(sys.modules.get("__main__"), "__file__", None)
        folder = os.path.dirname(os.path.abspath(path)) if path else os.getcwd()
    host = _host()
    host.run_app(host.file_loader(folder), app=app, **options)


def __getattr__(name):
    if name in _HOST_NAMES:
        return getattr(_host(), name)
    raise AttributeError(f"module 'nativelayout' has no attribute {name!r}")
