"""An example app that speaks HubDeck's contract (docs/APP-CONTRACT.md), for trying it and as a
model for your own: three "agents" whose state changes every few seconds.

    python scripts/example-app.py [port]        # default 4317, on 127.0.0.1 only

On start it announces itself to HubDeck (if HubDeck runs), which connects it without a click; by
hand: Settings > Connections > Your apps > Add: http://127.0.0.1:4317/hubdeck/manifest
GET /hubdeck/status, GET /hubdeck/events (Server-Sent Events), GET /hubdeck/manifest,
POST /agents/<id>/focus (an action: marks that agent as working).
"""

import json
import os
import random
import sys
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 4317
STATES = ["working", "waiting", "done", "idle"]
agents = {str(i): {"id": str(i), "label": f"Agent {i}", "state": "working", "text": ""} for i in (1, 2, 3)}
lock = threading.Lock()
listeners = []


def publish(items):
    data = ("data: " + json.dumps({"items": items}) + "\n\n").encode()
    for q in list(listeners):
        q.append(data)


def churn():
    while True:
        time.sleep(4)
        with lock:
            a = random.choice(list(agents.values()))
            a["state"] = random.choice(STATES)
            a["text"] = {"waiting": "Needs your OK", "working": "Writing code", "done": "Finished", "idle": ""}[a["state"]]
            publish([dict(a)])


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def answer(self, code, body):
        raw = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        base = f"http://127.0.0.1:{PORT}"
        if self.path == "/hubdeck/status":
            with lock:
                self.answer(200, {"app": "Example app", "items": list(agents.values())})
        elif self.path == "/hubdeck/manifest":
            self.answer(200, {
                "app": "Example app",
                "status": f"{base}/hubdeck/status",
                "events": f"{base}/hubdeck/events",
                "actions": [{"id": "focus", "label": "Focus agent", "icon": "bot", "method": "POST", "url": f"{base}/agents/{{id}}/focus"}],
            })
        elif self.path == "/hubdeck/events":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            queue = []
            listeners.append(queue)
            try:
                while True:
                    while queue:
                        self.wfile.write(queue.pop(0))
                        self.wfile.flush()
                    time.sleep(0.2)
            except OSError:
                pass
            finally:
                listeners.remove(queue)
        else:
            self.answer(404, {"error": "not found"})

    def do_POST(self):
        parts = self.path.strip("/").split("/")
        if len(parts) == 3 and parts[0] == "agents" and parts[2] == "focus" and parts[1] in agents:
            with lock:
                agents[parts[1]].update(state="working", text="Focused")
                publish([dict(agents[parts[1]])])
            self.answer(200, {"ok": True})
        else:
            self.answer(404, {"error": "not found"})


def hubdeck_dir():
    """HubDeck's data folder, where it writes its local API port and token."""
    if sys.platform == "win32":
        return os.path.join(os.environ.get("APPDATA", ""), "dev.hubdeck.app")
    if sys.platform == "darwin":
        return os.path.expanduser("~/Library/Application Support/dev.hubdeck.app")
    return os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "dev.hubdeck.app")


def announce(manifest):
    """Tell HubDeck this app is here (docs/APP-CONTRACT.md, section 5); quiet if it is not running."""
    try:
        folder = hubdeck_dir()
        port = open(os.path.join(folder, "api-http-port")).read().strip()
        token = open(os.path.join(folder, "api-token")).read().strip()
        req = urllib.request.Request(
            f"http://127.0.0.1:{port}/api/apps/announce",
            data=json.dumps({"url": manifest}).encode(),
            headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
            method="POST",
        )
        print("HubDeck:", urllib.request.urlopen(req, timeout=10).read().decode())
    except Exception as e:  # HubDeck not installed or not running, or the user disconnected this app
        print("HubDeck not reached:", e)


server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
threading.Thread(target=churn, daemon=True).start()
manifest = f"http://127.0.0.1:{PORT}/hubdeck/manifest"
print(f"Example app on {manifest}")
threading.Thread(target=announce, args=(manifest,), daemon=True).start()
server.serve_forever()
