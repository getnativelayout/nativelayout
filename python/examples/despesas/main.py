python
import kivy  # noqa: F401
import os
import sys
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", ".."))
sys.path[:0] = [os.path.join(root, "core"), os.path.join(root, "android")]
import nativelayout as nl

SERVERS = [
    {"id": "ao", "name": "Luanda", "flag": "🇦🇴", "ping": "24ms", "country": "AO", "ip": "102.23.45.10"},
    {"id": "pt", "name": "Lisboa", "flag": "🇵🇹", "ping": "89ms", "country": "PT", "ip": "185.32.44.12"},
    {"id": "br", "name": "São Paulo", "flag": "🇧🇷", "ping": "142ms", "country": "BR", "ip": "200.15.22.8"},
]

class Store:
    connected = False
    selected = "ao"
    loaded = False

def persist():
    nl.save("vpn", f"{Store.selected}|{1 if Store.connected else 0}")

def load_store():
    if Store.loaded:
        return
    Store.loaded = True
    data = (nl.load("vpn") or "").strip()
    if not data:
        return
    p = data.split("|")
    if len(p) >= 1:
        Store.selected = p[0]
    if len(p) >= 2:
        Store.connected = p[1] == "1"

class HomeState:
    filter = "Todas"

def get_server(sid):
    for s in SERVERS:
        if s["id"] == sid:
            return s
    return SERVERS[0]

def home():
    s = nl.state(HomeState)
    load_store()

    filters = ["Todas", "Rápidos", "Angola", "Europa"]

    if nl.click("btn_connect"):
        Store.connected = not Store.connected
        persist()

    if nl.click("server"):
        sid = nl.clicked("server")["id"]
        Store.selected = sid
        persist()

    if nl.click(".chip"):
        i = nl.click_index()
        if 0 <= i < len(filters):
            s.filter = filters[i]

    current = get_server(Store.selected)

    if s.filter == "Angola":
        shown = [x for x in SERVERS if x["id"] == "ao"]
    elif s.filter == "Europa":
        shown = [x for x in SERVERS if x["id"] == "pt"]
    elif s.filter == "Rápidos":
        shown = sorted(SERVERS, key=lambda x: int(x["ping"].replace("ms","")))
    else:
        shown = SERVERS

    return nl.view("home.nl", {
        "connected": "on" if Store.connected else "",
        "status_label": "Conectado" if Store.connected else "Desconectado",
        "status_ip": current["ip"] if Store.connected else "102.23.45.10",
        "status_loc": f'{current["name"]}, {current["country"]} - ' + ("Conectado" if Store.connected else "Desconectado"),
        "btn_text": "Desconectar" if Store.connected else "Conectar",
        "btn_state": "off" if Store.connected else "",
        "chips": [{"id": f, "name": f, "state": "on" if f == s.filter else ""} for f in filters],
        "servers": [{"id": x["id"], "name": x["name"], "flag": x["flag"], "ping": x["ping"], "country": x["country"], "state": "on" if x["id"] == Store.selected else ""} for x in shown],
    })

if __name__ == "__main__":
    nl.run(home, here, title="VPN")