#!/usr/bin/env python3
"""Captures the product screenshots used by the website, README and metainfo.

Run it inside the headless session, with the browser you want to show:

    scripts/headless-session.sh --screen 3000x1900x24 -- \
        python3 branding/screenshots/capture.py /usr/bin/bitter-ui

Every shot starts from a fresh profile with a prepared session and history.
The demo sites in sites/ are served by a local proxy under .example names,
so the address bar shows readable addresses and no request leaves the
machine. Shots are taken at GDK_SCALE=2 (2880x1800 pixels for a 1440x900
window) and written to branding/screenshots/raw/.
"""
import http.server
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SITES = HERE / "sites"
OUT = HERE / "raw"
PORT = 8899
WIDTH, HEIGHT = 2880, 1800

PAGES = {
    "fieldnotes": ("http://fieldnotes.example/", "The Quiet Web — Field Notes"),
    "northwind": ("http://docs.northwind.example/", "Streaming responses — Northwind Docs"),
    "orbit": ("http://orbit.example/", "Solar activity — Orbit"),
    "atlas": ("http://atlas.example/", "Lisbon by bike — Atlas"),
    "harbor": ("http://harbor.example/", "Bitter orange marmalade — Harbor Kitchen"),
    "studio": ("http://studio.example/", "Selected work — Studio Mira"),
}


class Sites(http.server.BaseHTTPRequestHandler):
    """Serves sites/<name>/ for any host whose first label (or second, for docs.) is <name>."""

    def do_GET(self):
        host = (self.headers.get("Host") or "").split(":")[0]
        labels = [label for label in host.split(".") if label not in ("docs", "example", "www")]
        path = self.path.split("://", 1)[-1]
        path = "/" + path.split("/", 1)[1] if "/" in path else "/"
        path = path.split("?", 1)[0]
        name = labels[0] if labels else ""
        if path == "/shared.css":
            target = SITES / "shared.css"
        else:
            target = SITES / name / (path.lstrip("/") or "index.html")
        if not target.resolve().is_relative_to(SITES) or not target.is_file():
            self.send_error(404)
            return
        body = target.read_bytes()
        kind = {".html": "text/html; charset=utf-8", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png"}
        self.send_response(200)
        self.send_header("Content-Type", kind.get(target.suffix, "application/octet-stream"))
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def xdotool(*args):
    return subprocess.run(["xdotool", *args], text=True, capture_output=True).stdout.strip()


class Shot:
    def __init__(self, binary, theme):
        self.binary = binary
        self.dir = Path(tempfile.mkdtemp(prefix="bitter-shot-"))
        self.data = self.dir / "data" / "bitter-browser"
        self.data.mkdir(parents=True)
        config = self.dir / "config" / "bitter-browser"
        config.mkdir(parents=True)
        (config / "config.toml").write_text(f'version = 1\n\n[appearance]\ntheme = "{theme}"\n')
        for name in ("home", "cache", "runtime", "schemas"):
            (self.dir / name).mkdir(mode=0o700)
        subprocess.run(["glib-compile-schemas", "--targetdir", str(self.dir / "schemas"),
                        str(HERE.parent.parent / "crates/bitter-ui/data")], check=True)
        self.env = os.environ | {
            "HOME": str(self.dir / "home"),
            "XDG_CONFIG_HOME": str(self.dir / "config"),
            "XDG_DATA_HOME": str(self.dir / "data"),
            "XDG_CACHE_HOME": str(self.dir / "cache"),
            "XDG_RUNTIME_DIR": str(self.dir / "runtime"),
            "GSETTINGS_SCHEMA_DIR": str(self.dir / "schemas"),
            "GSETTINGS_BACKEND": "keyfile",
            "GDK_BACKEND": "x11",
            "GDK_SCALE": "2",
            "GDK_DEBUG": "no-portals",
            "GTK_A11Y": "none",
            "BITTER_OFFLINE_FILTER_LISTS": "1",
            "http_proxy": f"http://127.0.0.1:{PORT}",
            "no_proxy": "",
        }
        self.process = None
        self.window = None

    def session(self, workspace, main, main_selected, second, second_selected):
        def tabs(names):
            return [{"uri": PAGES[n][0] if n in PAGES else n,
                     "title": PAGES[n][1] if n in PAGES else n} for n in names]
        (self.data / "session.json").write_text(json.dumps({"version": 1, "session": {
            "workspace": workspace,
            "workspaces": [{"tabs": tabs(main), "selected": main_selected},
                           {"tabs": tabs(second), "selected": second_selected}],
        }}))

    def start(self):
        log = (self.dir / "browser.log").open("ab")
        self.process = subprocess.Popen([self.binary], env=self.env, stdout=log, stderr=log)
        for _ in range(100):
            found = xdotool("search", "--onlyvisible", "--pid", str(self.process.pid), "--name", "^Bitter Browser$")
            if found:
                self.window = found.splitlines()[0]
                break
            time.sleep(0.2)
        assert self.window, (self.dir / "browser.log").read_text()
        xdotool("windowmove", self.window, "0", "0")
        xdotool("windowsize", self.window, str(WIDTH), str(HEIGHT))
        xdotool("windowactivate", "--sync", self.window)
        time.sleep(4)

    def history(self, visits):
        """Adds history rows after the first start created the database."""
        with sqlite3.connect(self.data / "history.sqlite") as db:
            now = int(time.time() * 1_000_000)
            for offset, (name, count) in enumerate(visits):
                url, title = PAGES[name] if name in PAGES else (name, name)
                db.execute("INSERT OR REPLACE INTO history (url, title, visit_count, last_visit_time) "
                           "VALUES (?, ?, ?, ?)", (url, title, count, now - offset * 3_600_000_000))

    def key(self, *keys):
        xdotool("key", "--clearmodifiers", *keys)
        time.sleep(1.2)

    def capture(self, name):
        """Captures the screen area of the browser window, so dialogs and
        the command palette (separate windows) are included."""
        time.sleep(1.5)
        OUT.mkdir(exist_ok=True)
        geometry = dict(line.split("=", 1) for line in
                        xdotool("getwindowgeometry", "--shell", self.window).splitlines())
        crop = "{WIDTH}x{HEIGHT}+{X}+{Y}".format(**geometry)
        subprocess.run(["import", "-window", "root", "-crop", crop, "+repage",
                        str(OUT / f"{name}.png")], check=True)
        print("captured", name, flush=True)

    def center_popups(self):
        """Centers windows the browser opened over the main one (the command
        palette). Xvfb has no window manager to place them."""
        main = dict(line.split("=", 1) for line in
                    xdotool("getwindowgeometry", "--shell", self.window).splitlines())
        for found in xdotool("search", "--onlyvisible", "--pid", str(self.process.pid), "--name", "").split():
            if found == self.window:
                continue
            g = dict(line.split("=", 1) for line in xdotool("getwindowgeometry", "--shell", found).splitlines())
            x = int(main["X"]) + (int(main["WIDTH"]) - int(g["WIDTH"])) // 2
            y = int(main["Y"]) + int(int(main["HEIGHT"]) * 0.16)
            xdotool("windowmove", found, str(x), str(y))
        time.sleep(1)

    def visit_tabs(self, count):
        """Selects every tab of the workspace once, so each one loads and
        shows its site icon, and ends on the tab it started from."""
        for _ in range(count):
            self.key("ctrl+Page_Down")
            time.sleep(1.5)

    def type(self, text):
        xdotool("type", "--clearmodifiers", "--delay", "40", text)
        time.sleep(1)

    def click(self, x, y):
        xdotool("mousemove", "--window", self.window, str(x), str(y), "click", "1")
        time.sleep(1.5)

    def stop(self):
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(10)
            except subprocess.TimeoutExpired:
                self.process.kill()


MAIN = ["fieldnotes", "northwind", "orbit", "atlas"]
SECOND = ["harbor", "studio", "bitter://history"]
VISITS = [("fieldnotes", 14), ("northwind", 11), ("orbit", 9), ("atlas", 7),
          ("harbor", 6), ("studio", 5)]


def run(binary):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Sites)
    threading.Thread(target=server.serve_forever, daemon=True).start()

    def shot(name, theme, workspace, main_selected, second_selected, actions=(), seed=False,
             main=MAIN, second=SECOND):
        s = Shot(binary, theme)
        try:
            s.session(workspace, main, main_selected, second, second_selected)
            if seed:
                s.start()
                s.stop()
                s.history(VISITS)
                s.session(workspace, main, main_selected, second, second_selected)
            s.start()
            for action in actions:
                action(s)
            s.capture(name)
        finally:
            s.stop()

    shot("reading-dark", "dark", "main", 0, 0, actions=[lambda s: s.visit_tabs(len(MAIN))])
    shot("workspaces-light", "light", "second", 0, 0,
         second=SECOND[:2], actions=[lambda s: s.visit_tabs(2)])
    shot("palette-dark", "dark", "main", 1, 0,
         actions=[lambda s: s.visit_tabs(len(MAIN)), lambda s: s.key("ctrl+l"),
                  lambda s: s.key("ctrl+k"), lambda s: s.type("tab"), lambda s: s.center_popups()])
    shot("newtab-light", "light", "main", 4, 0, seed=True,
         main=MAIN + ["bitter://newtab"], actions=[lambda s: s.visit_tabs(len(MAIN) + 1)])
    shot("history-dark", "dark", "second", 2, 2, seed=True,
         actions=[lambda s: s.visit_tabs(len(SECOND))])
    # The Privacy page of Preferences: its tab is the second in the dialog header.
    shot("privacy-light", "light", "main", 2, 0,
         actions=[lambda s: s.visit_tabs(len(MAIN)), lambda s: s.key("ctrl+comma"),
                  lambda s: time.sleep(2), lambda s: s.click(1296, 269), lambda s: s.click(1296, 269)])
    server.shutdown()


if __name__ == "__main__":
    run(sys.argv[1] if len(sys.argv) > 1 else "/usr/bin/bitter-ui")
