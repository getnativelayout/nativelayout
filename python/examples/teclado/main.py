"""Keyboard test with plain Kivy widgets (no nativelayout).

If the system keyboard opens here but not in the Despesas example, the problem is in the nativelayout field.
If it does not open here either, the problem is in the Kivy build or in the phone.
"""
import kivy  # Pydroid starts the Kivy window only if the opened file imports kivy
from kivy.app import App
from kivy.core.window import Window
from kivy.uix.boxlayout import BoxLayout
from kivy.uix.button import Button
from kivy.uix.label import Label
from kivy.uix.scrollview import ScrollView
from kivy.uix.textinput import TextInput


class KeyboardTest(App):
    def build(self):
        root = BoxLayout(orientation="vertical", padding=20, spacing=16)

        self.info = Label(text="tap a field", size_hint_y=None, height=120)
        root.add_widget(self.info)

        root.add_widget(Label(text="A: TextInput directly in the window", size_hint_y=None, height=40))
        self.a = TextInput(multiline=False, size_hint_y=None, height=110, hint_text="A")
        root.add_widget(self.a)

        root.add_widget(Label(text="B: TextInput inside a ScrollView", size_hint_y=None, height=40))
        scroll = ScrollView(size_hint_y=None, height=240)
        inner = BoxLayout(orientation="vertical", size_hint_y=None, height=700, padding=10, spacing=10)
        self.b = TextInput(multiline=False, size_hint_y=None, height=110, hint_text="B")
        inner.add_widget(self.b)
        scroll.add_widget(inner)
        root.add_widget(scroll)

        button = Button(text="C: ask for the keyboard now", size_hint_y=None, height=110)
        button.bind(on_release=self.ask_keyboard)
        root.add_widget(button)

        for name, field in (("A", self.a), ("B", self.b)):
            field.bind(focus=lambda _w, value, n=name: self.show(f"{n} focus={value}"))
        Window.bind(keyboard_height=lambda _w, h: self.show(f"keyboard_height={h}"))
        return root

    def show(self, text):
        self.info.text = text

    def ask_keyboard(self, *_args):
        keyboard = Window.request_keyboard(lambda: None, self.a)
        self.show(f"request_keyboard -> {keyboard is not None}")


if __name__ == "__main__":
    KeyboardTest().run()
