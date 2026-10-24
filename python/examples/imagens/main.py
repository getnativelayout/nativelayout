"""Test of the v1.2 host: background-image, object-fit and position: absolute.

Run it in Pydroid by opening this file. It writes its own test image (foto.png) the first time:
a color gradient with a WHITE square at the top-left corner and a BLACK square at the bottom-right
corner, so you can see which part stays in view for cover, contain and position.
"""
import kivy  # noqa: F401  (Pydroid starts the Kivy window only if the opened file imports kivy)

import os
import struct
import sys
import zlib

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", ".."))
sys.path[:0] = [os.path.join(root, "core"), os.path.join(root, "android")]

from kivy.core.image import Image as CoreImage

import nativelayout as nl


def write_test_png(path: str, w: int = 300, h: int = 150) -> None:
    rows = bytearray()
    for y in range(h):
        rows.append(0)
        for x in range(w):
            r, g, b = int(255 * x / (w - 1)), int(255 * y / (h - 1)), 160
            if x < 24 and y < 24:
                r = g = b = 255
            if x >= w - 24 and y >= h - 24:
                r = g = b = 0
            rows += bytes((r, g, b))

    def chunk(kind: bytes, data: bytes) -> bytes:
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) \
        + chunk(b"IDAT", zlib.compress(bytes(rows))) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def images(source: str):
    path = os.path.join(here, source)
    return CoreImage(path).texture if os.path.isfile(path) else None


def screen():
    return nl.view("tela.nl", nl.state({}))


photo = os.path.join(here, "foto.png")
if not os.path.isfile(photo):
    write_test_png(photo)

nl.run(screen, here, title="Images", image_resolver=images)
