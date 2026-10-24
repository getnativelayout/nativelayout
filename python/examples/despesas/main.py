"""Despesas: a real example (port of the Android app). Run it on the desktop or in Pydroid: python main.py

Two screens (home, add) and a confirmation dialog, with the same .nl files and theme as the Android app.
The list is kept in the data folder of the app and read back the next time.
"""
import kivy  # Pydroid starts the Kivy window only if the opened file imports kivy

import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", ".."))
sys.path[:0] = [os.path.join(root, "core"), os.path.join(root, "android")]

import nativelayout as nl

# Change to your currency.
CURRENCY = "Kz"

CATEGORIES = ["Alimentação", "Transporte", "Casa", "Lazer", "Saúde", "Outros"]


class Store:
    items: list[dict] = []
    next_id = 1
    loaded = False


def money(v: float) -> str:
    return f"{v:.2f} {CURRENCY}"


def persist() -> None:
    nl.save("expenses", "\n".join(
        f"{e['id']}|{e['category']}|{e['amount']}|{e['title']}" for e in Store.items))


def load_store() -> None:
    if Store.loaded:
        return
    Store.loaded = True
    Store.items.clear()
    for line in (nl.load("expenses") or "").splitlines():
        p = line.split("|", 3)
        if len(p) != 4:
            continue
        try:
            id_, amount = int(p[0]), float(p[2])
        except ValueError:
            continue
        Store.items.append({"id": id_, "title": p[3], "category": p[1], "amount": amount})
        Store.next_id = max(Store.next_id, id_ + 1)


class HomeState:
    filter = "Todas"
    confirm = ""
    target_id = -1


def home():
    s = nl.state(HomeState)
    load_store()
    filters = ["Todas"] + CATEGORIES

    if nl.click("add"):
        return nl.go(add_expense)

    # Each row of the list has its own "remove": nl.clicked("e") is the expense that was tapped.
    if nl.click("remove"):
        s.target_id = nl.clicked("e")["id"]
        s.confirm = "delete"
    if nl.click(".chip"):
        i = nl.click_index()
        if 0 <= i < len(filters):
            s.filter = filters[i]

    # nl.back() is only asked while the dialog is open: then the button closes it.
    # Otherwise the screen does not ask, and the default applies (the app closes).
    if nl.click("confirm-no") or nl.click("layer") or (s.confirm != "" and nl.back()):
        s.confirm = ""
        s.target_id = -1
    if nl.click("confirm-yes"):
        Store.items[:] = [e for e in Store.items if e["id"] != s.target_id]
        persist()
        s.confirm = ""
        s.target_id = -1

    target = next((e for e in Store.items if e["id"] == s.target_id), None)
    dialog_text = f'"{target["title"]}" ({money(target["amount"])}) será removida.' if target else ""

    shown = sorted((e for e in Store.items if s.filter == "Todas" or e["category"] == s.filter),
                   key=lambda e: -e["id"])
    total = sum(e["amount"] for e in shown)
    sums: dict[str, float] = {}
    for e in shown:
        sums[e["category"]] = sums.get(e["category"], 0.0) + e["amount"]
    by_category = sorted(sums.items(), key=lambda kv: -kv[1])

    return nl.view("home.nl", {
        "confirm": s.confirm,
        "dialogText": dialog_text,
        "total": money(total),
        "count": len(shown),
        "chips": [{"id": f, "name": f, "state": "on" if f == s.filter else ""} for f in filters],
        "summary": [{"id": n, "name": n, "amount": money(a),
                     "percent": int(a / total * 100 + 0.5) if total > 0 else 0} for n, a in by_category],
        "expenses": [{"id": e["id"], "title": e["title"], "category": e["category"],
                      "amount": money(e["amount"])} for e in shown],
    })


class AddState:
    category = CATEGORIES[0]
    error = ""


def add_expense():
    s = nl.state(AddState)

    # The system back button needs no code here: nl.go_back() is the default.
    if nl.click("cancel"):
        return nl.go_back()
    if nl.click(".chip"):
        i = nl.click_index()
        if 0 <= i < len(CATEGORIES):
            s.category = CATEGORIES[i]
    if nl.click("save"):
        title = nl.value("title").strip().replace("|", " ")
        try:
            amount = float(nl.value("amount").strip().replace(",", "."))
        except ValueError:
            amount = None
        if not title:
            s.error = "Dê um nome à despesa."
        elif amount is None or amount <= 0:
            s.error = "Indique um valor maior que zero."
        else:
            Store.items.append({"id": Store.next_id, "title": title, "category": s.category, "amount": amount})
            Store.next_id += 1
            persist()
            return nl.go_back()

    return nl.view("add.nl", {
        "error": s.error,
        "categories": [{"id": c, "name": c, "state": "on" if c == s.category else ""} for c in CATEGORIES],
    })


if __name__ == "__main__":
    nl.run(home, here, title="Despesas")