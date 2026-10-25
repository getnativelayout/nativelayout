"""Entrance motion of a screen in the Kivy host, with the springs and the curve of framer-motion.

The numbers are the defaults of framer-motion: a transform (here, the rise of the screen) uses a spring
with stiffness 500 and damping 25; a scale uses stiffness 550 and damping 30; opacity uses the curve
cubic-bezier(.25, .1, .35, 1). The spring is the damped oscillator of its `spring` generator, and it runs
until it is at rest (speed below 10 px/s and distance below 0.5 px, like there), not for a fixed time.

Everything is in this file: `kivy_host.py` only calls `enter_screen`. `Spring` and `cubic_bezier` have no
dependency on Kivy, so they can be tested without a window.
"""
from __future__ import annotations

import math

MOVE_STIFFNESS = 500.0
MOVE_DAMPING = 25.0
SCALE_STIFFNESS = 550.0
SCALE_DAMPING = 30.0
OPACITY_CURVE = (0.25, 0.1, 0.35, 1.0)
RISE_DP = 16
FADE_SECONDS = 0.22          # the duration of `animation: fade` in the core (DefaultAnimations)

REST_SPEED = 10.0            # px per second
REST_DELTA = 0.5             # px
GRANULAR_SPEED = 0.01        # for distances under 5 (a scale, for example)
GRANULAR_DELTA = 0.005
MAX_SECONDS = 10.0


class Spring:
    """A damped spring from 0 to 1 with no initial speed (underdamped: damping below the critical one)."""

    def __init__(self, stiffness: float, damping: float, mass: float = 1.0) -> None:
        self.w0 = math.sqrt(stiffness / mass)
        self.zeta = damping / (2.0 * math.sqrt(stiffness * mass))
        if not 0.0 < self.zeta < 1.0:
            raise ValueError("the spring must be underdamped (damping below 2 * sqrt(stiffness * mass))")
        self.wd = self.w0 * math.sqrt(1.0 - self.zeta * self.zeta)

    def position(self, t: float) -> float:
        if t <= 0.0:
            return 0.0
        decay = math.exp(-self.zeta * self.w0 * t)
        k = self.zeta * self.w0 / self.wd
        return 1.0 - decay * (math.cos(self.wd * t) + k * math.sin(self.wd * t))

    def velocity(self, t: float) -> float:
        if t <= 0.0:
            return 0.0
        return math.exp(-self.zeta * self.w0 * t) * (self.w0 * self.w0 / self.wd) * math.sin(self.wd * t)

    def rest_time(self, distance: float) -> float:
        """Seconds until the spring is at rest when it travels [distance] (in px): the end of the motion."""
        distance = abs(distance)
        granular = distance < 5.0
        speed = GRANULAR_SPEED if granular else REST_SPEED
        delta = GRANULAR_DELTA if granular else REST_DELTA
        step = 1.0 / 120.0
        t = step
        while t < MAX_SECONDS:
            if abs(self.velocity(t)) * distance <= speed and abs(1.0 - self.position(t)) * distance <= delta:
                return t
            t += step
        return MAX_SECONDS

    def curve(self, distance: float):
        """Returns (seconds, f): f(x) for x in [0, 1] is the position over the whole motion, 1 at the end."""
        seconds = self.rest_time(distance)

        def f(x: float) -> float:
            if x <= 0.0:
                return 0.0
            if x >= 1.0:
                return 1.0
            return self.position(x * seconds)

        return seconds, f


def cubic_bezier(x1: float, y1: float, x2: float, y2: float):
    """The CSS curve cubic-bezier(x1, y1, x2, y2): f(x) for x in [0, 1]."""
    def bez(a: float, b: float, s: float) -> float:
        return 3 * a * s * (1 - s) ** 2 + 3 * b * s * s * (1 - s) + s ** 3

    def f(x: float) -> float:
        if x <= 0.0:
            return 0.0
        if x >= 1.0:
            return 1.0
        lo, hi = 0.0, 1.0
        for _ in range(40):
            mid = (lo + hi) / 2.0
            if bez(x1, x2, mid) < x:
                lo = mid
            else:
                hi = mid
        return bez(y1, y2, (lo + hi) / 2.0)

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

    distance = dp(RISE_DP)
    seconds, move = Spring(MOVE_STIFFNESS, MOVE_DAMPING).curve(distance)
    host.content.opacity = 0.0
    state.shift.y = -distance
    fade = Animation(opacity=1.0, duration=FADE_SECONDS, t=cubic_bezier(*OPACITY_CURVE))
    rise = Animation(y=0.0, duration=seconds, t=move)
    fade.start(host.content)
    rise.start(state.shift)
    state.running += [fade, rise]
