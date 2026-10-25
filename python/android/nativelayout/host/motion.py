"""Entrance motion of a screen in the Kivy host: a damped spring instead of a plain fade.

The screen fades in and rises a few dp, and the rise settles like a spring (a small overshoot, then rest),
the way a spring in framer-motion does. Everything about it is in this file: `kivy_host.py` only calls
`enter_screen`. To go back to a plain fade, make `enter_screen` set only the opacity.

`spring_curve` has no dependency on Kivy, so it can be tested without a window.
"""
from __future__ import annotations

import math

DAMPING = 0.62          # below 1: it overshoots a little and then settles; closer to 0 bounces more
DECAY = 5.3             # e^-5.3 is about 0.5%: the spring is at rest at the end of the time
RISE_DP = 16            # how far below its place the screen starts
FADE_SECONDS = 0.22
MOVE_SECONDS = 0.45     # time the spring needs to settle


def spring_curve(damping: float = DAMPING, decay: float = DECAY):
    """Returns f(x) for x in [0, 1]: 0 at the start, 1 at the end, with the overshoot of a damped spring."""
    if not 0.0 < damping < 1.0:
        raise ValueError("damping must be between 0 and 1")
    w0 = decay / damping
    wd = w0 * math.sqrt(1.0 - damping * damping)
    k = decay / wd

    def f(x: float) -> float:
        if x <= 0.0:
            return 0.0
        if x >= 1.0:
            return 1.0
        return 1.0 - math.exp(-decay * x) * (math.cos(wd * x) + k * math.sin(wd * x))

    return f


class _State:
    """The matrix that moves the scroll view and the animations running on it."""

    def __init__(self, host) -> None:
        from kivy.graphics import PopMatrix, PushMatrix, Translate
        canvas = host.scroll.canvas
        self.shift = Translate(0, 0)
        canvas.before.add(PushMatrix())
        canvas.before.add(self.shift)
        canvas.after.add(PopMatrix())
        self.running: list = []


def enter_screen(host) -> None:
    """Starts the entrance of the screen that was just built in `host.content`."""
    from kivy.animation import Animation
    from kivy.metrics import dp

    state = getattr(host, "_motion", None)
    if state is None:
        state = host._motion = _State(host)
    for a in state.running:
        a.cancel_all(host.content)
        a.cancel_all(state.shift)
    state.running.clear()

    host.content.opacity = 0.0
    state.shift.y = -dp(RISE_DP)
    fade = Animation(opacity=1.0, duration=FADE_SECONDS, t="out_quad")
    rise = Animation(y=0.0, duration=MOVE_SECONDS, t=spring_curve())
    fade.start(host.content)
    rise.start(state.shift)
    state.running += [fade, rise]
