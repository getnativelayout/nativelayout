"""Example: counter. Run on the desktop or in Pydroid: python main.py"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", ".."))
sys.path[:0] = [os.path.join(root, "core"), os.path.join(root, "desktop")]

from nativelayout.core.program import click, state, view
from nativelayout.host import file_loader, run_app


def screen():
    s = state({"n": 0, "nome": "Adilson", "tarefas": ["um", "dois", "tres"]})
    if click(".mais"):
        s["n"] += 1
    return view("tela.nl", s)


run_app(file_loader(here), app=screen, title="Contador")