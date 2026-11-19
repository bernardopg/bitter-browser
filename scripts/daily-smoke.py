#!/usr/bin/env python3
"""Drive the native GTK/WebKit browser against a loopback HTTP fixture."""
import argparse
import json
import os
import re
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

def eventually(check, message, timeout=12):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        result = check()
        if result:
            return result
        time.sleep(0.025)
    raise AssertionError(message)


class Fixture(BaseHTTPRequestHandler):
    paths = Counter()
    lock = threading.Lock()
    release_slow = threading.Event()

    @classmethod
    def count(cls, path):
        with cls.lock:
            return cls.paths[path]

    def do_GET(self):
        with self.lock:
            self.paths[self.path] += 1
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "/landing")
            self.end_headers()
            return
        if self.path == "/slow":
            self.release_slow.wait(10)
        if self.path == "/home":
            body = """<a href="/background" style="position:absolute; top:10px; left:10px">Open in background</a>
<a href="/target" target="_blank" style="position:absolute; top:60px; left:10px">Open in a new tab</a>"""
        else:
            body = "<p>Local browsing fixture</p>"
        name = self.path.removeprefix("/").title()
        self.send_response(404 if self.path == "/missing" else 200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        try:
            self.wfile.write(f"<!doctype html><title>{name}</title>{body}".encode())
        except BrokenPipeError:
            pass

    def log_message(self, *_args):
        pass


class Browser:
    def __init__(self, binary, directory, screenshots):
        self.binary = binary
        self.directory = directory
        self.screenshots = screenshots
        self.profile = directory / "data" / "bitter-browser"
        self.profile.mkdir(parents=True)
        self.session = self.profile / "session.json"
        self.process = None
        self.window = None
        self.log = directory / "browser.log"
        self.env = os.environ | {
            "HOME": str(directory / "home"),
            "XDG_CONFIG_HOME": str(directory / "config"),
            "XDG_DATA_HOME": str(directory / "data"),
            "XDG_CACHE_HOME": str(directory / "cache"),
            "XDG_RUNTIME_DIR": str(directory / "runtime"),
            "GDK_BACKEND": "x11",
            "GSK_RENDERER": "cairo",
            "GTK_A11Y": "none",
        }
        for key in ("HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"):
            Path(self.env[key]).mkdir(mode=0o700)

    def xdotool(self, *args):
        return subprocess.check_output(["xdotool", *args], text=True).strip()

    def start(self):
        start = time.monotonic()
        with self.log.open("ab") as log:
            self.process = subprocess.Popen([self.binary], env=self.env, stdout=log, stderr=log)

        def window_id():
            status = self.process.poll()
            assert status is None, f"Browser exited {status}\n{self.log.read_text()}"
            search = subprocess.run(
                ["xdotool", "search", "--onlyvisible", "--pid", str(self.process.pid), "--name", "^Bitter Browser$"],
                text=True, capture_output=True,
            )
            return search.stdout.splitlines()[0] if search.returncode == 0 else None

        self.window = eventually(window_id, "Browser window did not open")
        elapsed = time.monotonic() - start
        self.xdotool("windowmove", self.window, "60", "60")
        self.xdotool("windowfocus", "--sync", self.window)
        return elapsed

    def key(self, *keys):
        self.xdotool("key", "--clearmodifiers", *keys)

    def click(self, x, y, modifiers=None):
        values = ("mousemove", "--window", self.window, str(x), str(y), "click", "1")
        if modifiers:
            self.xdotool("keydown", modifiers, *values, "keyup", modifiers)
        else:
            self.xdotool(*values)

    def enter(self, uri):
        self.key("ctrl+l")
        self.xdotool("type", "--clearmodifiers", "--delay", "0", uri)
        self.key("Return")

    def matching_history(self, uri, title):
        path = self.profile / "history.sqlite"
        try:
            with sqlite3.connect(path, timeout=0.1) as db:
                return db.execute("SELECT title FROM history WHERE url = ?", (uri,)).fetchone() == (title,)
        except sqlite3.OperationalError:
            return False

    def frame(self, name):
        if self.screenshots:
            geometry = dict(line.split("=", 1) for line in self.xdotool(
                "getwindowgeometry", "--shell", self.window).splitlines())
            crop = "{WIDTH}x{HEIGHT}+{X}+{Y}".format(**geometry)
            subprocess.run(["import", "-window", "root", "-crop", crop,
                            str(self.screenshots / f"{name}.png")], check=True)

    def current(self):
        try:
            return json.loads(self.session.read_text())["session"]
        except FileNotFoundError:
            return None

    def stop(self, crash=False):
        if self.process is None:
            return
        if self.process.poll() is None:
            try:
                if crash:
                    self.process.kill()
                else:
                    self.xdotool("windowfocus", "--sync", self.window)
                    self.key("ctrl+q")
                code = self.process.wait(timeout=10)
                assert crash or code == 0, f"Browser exited {code}\n{self.log.read_text()}"
            finally:
                if self.process.poll() is None:
                    self.process.kill()
                    self.process.wait(timeout=5)
        self.process = None


def tab_paths(session, workspace):
    return [tab["uri"] for tab in session["workspaces"][workspace]["tabs"]]


def run_flow(browser, base):
    def expect(path, count=1):
        return eventually(lambda: Fixture.count(path) >= count, f"No request for {path} ({count})")

    def snapshot(workspace, index):
        def check():
            saved = browser.current()
            if saved and saved["workspace"] == ("main", "second")[workspace]:
                return saved if saved["workspaces"][workspace]["selected"] == index else None
            return None
        return eventually(check, f"Workspace {workspace}, tab {index} was not saved")

    def selected_uri(uri, workspace=0, index=0):
        def check():
            saved = browser.current()
            return saved and tab_paths(saved, workspace)[index:index + 1] == [uri]
        return eventually(check, f"Selected address did not become {uri}")

    startup = browser.start()
    browser.enter(f"{base}/first")
    eventually(lambda: browser.matching_history(f"{base}/first", "First"), "First page was not ready")
    browser.enter(f"{base}/redirect")
    expect("/landing")
    eventually(lambda: browser.matching_history(f"{base}/landing", "Landing"), "Redirected title was not saved")
    assert not browser.matching_history(f"{base}/redirect", "Landing")
    browser.key("alt+Left")
    selected_uri(f"{base}/first")
    browser.key("alt+Right")
    selected_uri(f"{base}/landing")

    browser.enter(f"{base}/slow")
    expect("/slow")
    browser.frame("loading")
    browser.key("Escape")
    Fixture.release_slow.set()
    browser.enter(f"{base}/missing")
    expect("/missing")
    eventually(lambda: browser.matching_history(f"{base}/missing", "Missing"), "HTTP 404 page was not displayed")
    assert not browser.matching_history(f"{base}/slow", "Slow")

    browser.key("ctrl+t")
    browser.enter(f"{base}/home")
    expect("/home")
    eventually(lambda: browser.matching_history(f"{base}/home", "Home"), "Home page was not ready")
    browser.click(250, 75, "Control")
    eventually(lambda: len(tab_paths(browser.current(), 0)) == 3, "Background link did not create a tab")
    assert Fixture.count("/background") == 0
    snapshot(0, 1)
    browser.click(250, 126)
    expect("/target")
    eventually(lambda: len(tab_paths(browser.current(), 0)) == 4, "target=_blank did not create a tab")
    snapshot(0, 3)

    browser.key("alt+shift+Left")
    moved = snapshot(0, 2)
    assert tab_paths(moved, 0)[2:] == [f"{base}/target", f"{base}/background"], moved
    browser.key("ctrl+w")
    expect("/background")
    eventually(lambda: len(tab_paths(browser.current(), 0)) == 3, "Closed tab remained in session")
    browser.key("ctrl+shift+t")
    expect("/target", 2)
    eventually(lambda: len(tab_paths(browser.current(), 0)) == 4, "Reopened tab was not saved")
    reopened = snapshot(0, 2)
    assert tab_paths(reopened, 0)[2] == f"{base}/target", reopened

    browser.click(154, 24)
    snapshot(1, 0)
    browser.enter(f"{base}/second")
    expect("/second")
    eventually(lambda: browser.matching_history(f"{base}/second", "Second"), "Second workspace did not load")
    browser.click(58, 24)
    snapshot(0, 2)
    browser.click(154, 24)
    snapshot(1, 0)
    browser.click(58, 24)
    snapshot(0, 2)
    browser.frame("workspaces")

    browser.key("ctrl+l")
    browser.xdotool("type", "--clearmodifiers", "--delay", "0", "Second")
    time.sleep(0.25)
    browser.key("Down", "Return")
    eventually(lambda: browser.current()["workspace"] == "second", "Switch-to-tab suggestion did not synchronize workspace")
    browser.key("ctrl+d")
    eventually(lambda: (browser.profile / "bookmarks.sqlite").exists(), "Bookmarks database missing")
    browser.key("ctrl+l")
    browser.xdotool("type", "--clearmodifiers", "--delay", "0", "Firs")
    time.sleep(0.35)
    browser.frame("suggestions")
    before = Fixture.count("/first")
    browser.key("Down", "Return")
    expect("/first", before + 1)
    selected_uri(f"{base}/first", workspace=1)
    browser.key("alt+Left")
    selected_uri(f"{base}/second", workspace=1)

    # A closed loopback port exercises the network error and retry UI.
    with socket.socket() as closed:
        closed.bind(("127.0.0.1", 0))
        dead_port = closed.getsockname()[1]
    failed_uri = f"http://127.0.0.1:{dead_port}/problem"
    browser.key("ctrl+t")
    browser.enter(failed_uri)
    selected_uri(failed_uri, workspace=1, index=1)
    time.sleep(0.3)
    browser.frame("network-error")
    assert not browser.matching_history(failed_uri, failed_uri)
    with ThreadingHTTPServer(("127.0.0.1", dead_port), Fixture) as recovery:
        threading.Thread(target=recovery.serve_forever, daemon=True).start()
        try:
            browser.key("ctrl+r")
            expect("/problem")
            eventually(lambda: browser.matching_history(failed_uri, "Problem"), "Retry did not load the page")
        finally:
            recovery.shutdown()
    browser.key("ctrl+w")
    snapshot(1, 0)

    saved = browser.current()
    background = Fixture.count("/background")
    home = Fixture.count("/home")
    target = Fixture.count("/target")
    second = Fixture.count("/second")
    browser.stop(crash=True)
    restart = browser.start()
    expect("/second", second + 1)
    time.sleep(0.4)
    assert Fixture.count("/background") == background
    assert Fixture.count("/home") == home
    assert Fixture.count("/target") == target
    browser.click(58, 24)
    expect("/target", target + 1)
    assert tab_paths(browser.current(), 0) == tab_paths(saved, 0)
    browser.key("ctrl+Page_Down")
    expect("/background", background + 1)
    selected_uri(f"{base}/background", index=3)
    time.sleep(0.25)
    browser.frame("restored")
    handed_over = subprocess.run([browser.binary, f"{base}/external"], env=browser.env,
                                 capture_output=True, text=True, timeout=20)
    assert handed_over.returncode == 0, handed_over.stderr
    expect("/external")
    eventually(lambda: tab_paths(browser.current(), 0)[-1] == f"{base}/external",
               "Address from another application did not open in the running window")
    browser.stop()
    return {"startup_ms": round(startup * 1000), "restore_ms": round(restart * 1000)}


def benchmark(binary, directory, base, samples):
    runs = []
    for count in (0, 50, 100):
        for run in range(samples):
            browser = Browser(binary, directory / f"bench-{count}-{run}", None)
            if count:
                tabs = [{"uri": f"{base}/bench-{index}", "title": f"Page {index}"} for index in range(count)]
                browser.session.write_text(json.dumps({
                    "version": 1, "session": {"workspace": "main", "workspaces": [
                        {"tabs": tabs, "selected": 0}, {"tabs": [], "selected": 0},
                    ]},
                }))
            before = [Fixture.count(f"/bench-{index}") for index in range(count)]
            try:
                elapsed = browser.start()
                if count:
                    eventually(lambda: Fixture.count("/bench-0") > before[0], "Selected saved tab did not load")
                time.sleep(0.4)
                after = [Fixture.count(f"/bench-{index}") for index in range(count)]
                assert sum(a - b for a, b in zip(after, before)) == int(count > 0)
                rss = re.search(r"^VmRSS:\s+(\d+)", Path(f"/proc/{browser.process.pid}/status").read_text(), re.M)
                runs.append({"saved_tabs": count, "window_ms": round(elapsed * 1000), "browser_rss_kib": int(rss[1]),
                             "restored_page_requests": sum(a - b for a, b in zip(after, before))})
            finally:
                browser.stop()
    return runs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=lambda value: str(Path(value).resolve()))
    parser.add_argument("--screenshots", type=Path)
    parser.add_argument("--benchmark-samples", type=int, default=0)
    args = parser.parse_args()
    if args.screenshots:
        args.screenshots.mkdir(parents=True, exist_ok=True)
    if not shutil.which("xdotool"):
        parser.error("xdotool is required")
    assert os.environ.get("DISPLAY") and os.environ.get("DBUS_SESSION_BUS_ADDRESS"), "Use Xvfb and dbus-run-session"
    fixture = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    threading.Thread(target=fixture.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{fixture.server_port}"
    with tempfile.TemporaryDirectory(prefix="bitter-daily-") as root:
        directory = Path(root)
        browser = Browser(args.binary, directory / "flow", args.screenshots)
        try:
            print(json.dumps(run_flow(browser, base)), flush=True)
            if args.benchmark_samples:
                print(json.dumps(benchmark(args.binary, directory, base, args.benchmark_samples), indent=2))
            assert "panicked" not in browser.log.read_text(), browser.log.read_text()
        except Exception:
            print("Last snapshot:", browser.current(), flush=True)
            print("HTTP requests:", dict(Fixture.paths), flush=True)
            if browser.log.exists():
                print(browser.log.read_text(), flush=True)
            raise
        finally:
            browser.stop()
            fixture.shutdown()
            fixture.server_close()


if __name__ == "__main__":
    main()
