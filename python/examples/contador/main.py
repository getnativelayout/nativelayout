import os, sys
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", ".."))
sys.path[:0] = [os.path.join(root, "core"), os.path.join(root, "android")]
import nativelayout as nl

def screen():
    # sem click ainda, só pra abrir
    return nl.view("tela.nl", {
        "status_label": "Desconectado",
        "status_ip": "102.23.45.10",
        "status_loc": "Luanda",
        "btn_text": "Conectar",
        "servers": [
            {"id": "ao", "name": "Luanda", "ping": "24ms"},
            {"id": "pt", "name": "Lisboa", "ping": "89ms"},
        ]
    })

nl.run(screen, here, title="VPN")