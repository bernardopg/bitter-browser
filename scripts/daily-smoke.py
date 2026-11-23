#!/usr/bin/env python3
"""Drive the native GTK/WebKit browser against a loopback HTTP fixture."""
import argparse
import ctypes
import ctypes.util
import json
import os
import re
import shutil
import socket
import sqlite3
import ssl
import subprocess
import tempfile
import threading
import time
import struct
import zlib
import urllib.parse
from collections import Counter
from http.client import HTTPSConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from queue import Queue

def solid_png(size, rgb):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    rows = b"".join(b"\0" + bytes(rgb) * size for _ in range(size))
    header = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")


FAVICON = solid_png(16, (232, 84, 42))
DOWNLOAD_BODY = b"Bitter Browser download fixture\n" * 64
UPLOAD_PAGE = """<style>input{position:absolute;top:20px;left:20px;width:300px;height:60px}</style>
<input type="file" id="file">
<script>
document.getElementById("file").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  const text = await file.text();
  fetch("/uploaded?" + encodeURIComponent([file.name, file.size, text.length].join("|")));
});
</script>"""
PROBE_PAGE = """<style>body{margin:0;font:16px sans-serif}button{position:absolute;top:120px;left:10px}</style>
<p>alpha beta alpha gamma alpha</p>
<button onclick="document.documentElement.requestFullscreen()">Full screen</button>
<script>
let last = "";
setInterval(() => {
  const state = [getSelection().toString(), innerWidth, !!document.fullscreenElement].join("|");
  if (state !== last) { last = state; fetch("/report?" + encodeURIComponent(state)).catch(() => {}); }
}, 50);
</script>"""


PLATFORM_PAGE = """<style>body{margin:0;font:16px sans-serif}button,a{position:absolute;left:10px;height:30px}</style>
<button style="top:10px" onclick="window.open('/popup-child', 'child', 'width=500,height=400')">Pop up</button>
<button style="top:70px" onclick="Notification.requestPermission().then(p => ev('requested', p))">Notify</button>
<a style="top:130px" href="/linked">A link to follow</a>
<ins class="adsbygoogle" style="display:block;position:absolute;top:200px;width:60px;height:60px">Ad</ins>
<script>
const ev = (name, value) => fetch("/event?" + encodeURIComponent(name + "=" + value)).catch(() => {});
addEventListener("message", event => ev("message", event.data));
ev("notification", Notification.permission);
ev("cookie", document.cookie.includes("visited=1") ? "kept" : "none");
document.cookie = "visited=1; max-age=86400; path=/";
navigator.permissions.query({name: "notifications"}).then(state => ev("query", state.state));
let height = 0;
setInterval(() => { if (innerHeight !== height) { height = innerHeight; ev("height", height); } }, 100);
setTimeout(() => ev("ad", getComputedStyle(document.querySelector(".adsbygoogle")).display), 300);
</script>"""
POPUP_CHILD = """<p>Pop-up</p><script>
fetch("/event?" + encodeURIComponent("opener=" + !!window.opener));
if (window.opener) window.opener.postMessage("hello from the pop-up", "*");
setTimeout(() => window.close(), 1200);
</script>"""


XLIB = ctypes.CDLL(ctypes.util.find_library("X11"))
XTST = ctypes.CDLL(ctypes.util.find_library("Xtst"))
XLIB.XOpenDisplay.restype = ctypes.c_void_p
XLIB.XOpenDisplay.argtypes = [ctypes.c_char_p]
XLIB.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
XLIB.XKeysymToKeycode.restype = ctypes.c_ubyte
XLIB.XFlush.argtypes = [ctypes.c_void_p]
XLIB.XCloseDisplay.argtypes = [ctypes.c_void_p]
XTST.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
F11 = 0xFFC8
F9 = 0xFFC6
F12 = 0xFFC9
F5 = 0xFFC2


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

    reports = []
    uploads = []
    events = []

    @classmethod
    def event(cls, name, since=0):
        """The newest value reported for `name` after `since` events."""
        with cls.lock:
            return next((value for key, value in reversed(cls.events[since:]) if key == name), None)

    @classmethod
    def event_mark(cls):
        with cls.lock:
            return len(cls.events)

    @classmethod
    def report(cls, predicate, since=0):
        with cls.lock:
            return next((r for r in reversed(cls.reports[since:]) if predicate(r)), None)

    @classmethod
    def mark(cls):
        with cls.lock:
            return len(cls.reports)

    @classmethod
    def count(cls, path):
        with cls.lock:
            return cls.paths[path]

    def do_GET(self):
        with self.lock:
            self.paths[self.path] += 1
        if self.path.startswith("/report?"):
            selection, width, fullscreen = urllib.parse.unquote(self.path[8:]).split("|")
            with self.lock:
                self.reports.append((selection, int(width), fullscreen == "true"))
            self.send_response(204)
            self.end_headers()
            return
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "/landing")
            self.end_headers()
            return
        if self.path == "/favicon.ico":
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(FAVICON)))
            self.end_headers()
            self.wfile.write(FAVICON)
            return
        if self.path in ("/download", "/archive.zip"):
            body = DOWNLOAD_BODY
            self.send_response(200)
            if self.path == "/download":
                self.send_header("Content-Type", "text/plain")
                self.send_header("Content-Disposition", 'attachment; filename="../../report:final.txt"')
            else:
                self.send_header("Content-Type", "application/zip")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/slow":
            self.release_slow.wait(10)
        if self.path.startswith("/event?"):
            name, value = urllib.parse.unquote(self.path[7:]).split("=", 1)
            with self.lock:
                self.events.append((name, value))
            self.send_response(204)
            self.end_headers()
            return
        if self.path.startswith("/uploaded?"):
            with self.lock:
                self.uploads.append(urllib.parse.unquote(self.path[10:]))
            self.send_response(204)
            self.end_headers()
            return
        if self.path == "/upload":
            body = UPLOAD_PAGE
        elif self.path == "/probe":
            body = PROBE_PAGE
        elif self.path == "/platform":
            body = PLATFORM_PAGE
        elif self.path == "/popup-child":
            body = POPUP_CHILD
        elif self.path == "/home":
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
            "GSETTINGS_BACKEND": "keyfile",
            # Use in-process GTK dialogs so the harness can drive them.
            "GDK_DEBUG": "no-portals",
            "GSETTINGS_SCHEMA_DIR": str(directory / "schemas"),
            # Block with the built-in list only; never fetch EasyList in tests.
            "BITTER_OFFLINE_FILTER_LISTS": "1",
        }
        (directory / "schemas").mkdir()
        subprocess.run(["glib-compile-schemas", "--targetdir", self.env["GSETTINGS_SCHEMA_DIR"],
                        str(Path(__file__).resolve().parent.parent / "crates/bitter-ui/data")],
                       check=True)
        for key in ("HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"):
            Path(self.env[key]).mkdir(mode=0o700)
        self.downloads = directory / "home" / "Fetched"
        (directory / "config" / "user-dirs.dirs").write_text('XDG_DOWNLOAD_DIR="$HOME/Fetched"\n')

    def xdotool(self, *args):
        return subprocess.check_output(["xdotool", *args], text=True).strip()

    def focus_window(self, window, required=True):
        for attempt in range(3):
            for cmd in (["windowfocus", "--sync", window], ["windowactivate", "--sync", window]):
                try:
                    self.xdotool(*cmd)
                    return
                except subprocess.CalledProcessError:
                    pass
            if attempt < 2:
                time.sleep(0.2)
        if required:
            self.xdotool("windowfocus", "--sync", window)

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
        self.focus_window(self.window)
        return elapsed

    def dialog(self):
        """The newest visible window of the browser other than the main one."""
        def find():
            found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.process.pid)],
                                   text=True, capture_output=True).stdout.split()
            others = [window for window in found if window != self.window]
            return others[-1] if others else None
        try:
            window = eventually(find, "No dialog window opened")
        except AssertionError:
            if self.screenshots:
                subprocess.run(["import", "-window", "root", str(self.screenshots / "no-dialog.png")])
            print("Windows:", subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "."],
                                             text=True, capture_output=True).stdout.split(),
                  "browser", self.window, "pid", self.process.pid, "downloads",
                  list(self.downloads.glob("*")), flush=True)
            raise
        self.focus_window(window)
        time.sleep(0.5)
        return window

    def size(self):
        geometry = dict(line.split("=", 1) for line in self.xdotool(
            "getwindowgeometry", "--shell", self.window).splitlines())
        return int(geometry["WIDTH"]), int(geometry["HEIGHT"])

    def key(self, *keys):
        self.xdotool("key", "--clearmodifiers", *keys)

    def plain_key(self, keysym):
        """Press a key without modifiers through XTest.

        Some xdotool builds pick a key level that needs Alt, so `xdotool key
        F11` arrives as Alt+F11 on Ubuntu's Xvfb.
        """
        display = XLIB.XOpenDisplay(None)
        assert display, "Cannot open the X display"
        try:
            keycode = XLIB.XKeysymToKeycode(display, keysym)
            assert keycode, f"No keycode for keysym {keysym:#x}"
            for pressed in (True, False):
                XTST.XTestFakeKeyEvent(display, keycode, pressed, 0)
            XLIB.XFlush(display)
        finally:
            XLIB.XCloseDisplay(display)

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
                    self.focus_window(self.window, required=False)
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

    version = subprocess.run([browser.binary, "--version"], env=browser.env, capture_output=True,
                             text=True, timeout=10)
    assert version.returncode == 0 and re.fullmatch(r"Bitter Browser \d+\.\d+\.\d+\n", version.stdout), version
    usage = subprocess.run([browser.binary, "--help"], env=browser.env, capture_output=True, text=True, timeout=10)
    assert usage.returncode == 0 and "[URL…]" in usage.stdout and "--version" in usage.stdout, usage
    startup = browser.start()
    browser.enter(f"{base}/first")
    eventually(lambda: browser.matching_history(f"{base}/first", "First"), "First page was not ready")
    expect("/favicon.ico")
    check_cancelled_address_draft(browser, f"{base}/first")
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
    check_last_tab_lifecycle(browser, f"{base}/second")
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

    def downloaded(*names):
        def check():
            files = sorted(path.name for path in browser.downloads.glob("*"))
            return files == sorted(names) and all(
                (browser.downloads / name).read_bytes() == DOWNLOAD_BODY for name in names)
        return eventually(check, f"Downloads did not become {names}")

    browser.enter(f"{base}/download")
    downloaded("report_final.txt")
    browser.enter(f"{base}/download")
    downloaded("report_final.txt", "report_final (1).txt")
    browser.enter(f"{base}/archive.zip")
    downloaded("report_final.txt", "report_final (1).txt", "archive.zip")
    time.sleep(0.3)
    browser.frame("downloads")
    assert tab_paths(browser.current(), 0)[-1] == f"{base}/external", "A download replaced the page"

    def step(action, predicate, message):
        since = Fixture.mark()
        action()
        return eventually(lambda: Fixture.report(predicate, since), message)

    def keys(*names):
        return lambda: [browser.key(name) for name in names]

    _, width, _ = step(lambda: browser.enter(f"{base}/probe"), lambda r: r[1] > 0,
                       "Probe page did not report")
    # Slow runners need time to reveal the find bar before typing.
    step(lambda: (browser.key("ctrl+f"), time.sleep(1.0),
                  browser.xdotool("type", "--clearmodifiers", "--delay", "60", "ALPHA")),
         lambda r: r[0].lower() == "alpha", "Find did not select a match")
    time.sleep(0.2)
    browser.frame("find")
    browser.key("Escape")
    step(keys("ctrl+plus", "ctrl+plus"), lambda r: r[1] < width * 0.85,
         "Zoom in did not enlarge the page")
    step(keys("ctrl+0"), lambda r: r[1] == width, "Zoom reset did not restore the page")
    step(lambda: browser.plain_key(F11), lambda r: r[1] > width + 150,
         "Full screen did not hide the browser chrome")
    step(keys("Escape"), lambda r: r[1] == width, "Leaving full screen did not restore the chrome")
    step(lambda: browser.click(270, 187), lambda r: r[2], "The page could not enter full screen")
    step(keys("Escape"), lambda r: not r[2] and r[1] == width, "The page did not leave full screen")
    step(lambda: browser.plain_key(F9), lambda r: r[1] > width + 150, "F9 did not hide the sidebar")
    step(lambda: browser.plain_key(F9), lambda r: r[1] == width, "F9 did not show the sidebar again")
    browser.stop()
    return {"startup_ms": round(startup * 1000), "restore_ms": round(restart * 1000)}


def untrusted_certificate(browser, base, directory):
    """An untrusted TLS peer must never receive an HTTP request."""
    certificate = directory / "certificate.pem"
    key = directory / "key.pem"
    subprocess.run([
        "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
        "-subj", "/CN=127.0.0.1", "-addext", "subjectAltName=IP:127.0.0.1",
        "-keyout", str(key), "-out", str(certificate),
    ], check=True, capture_output=True)

    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    # In TLS 1.2 the server observes rejection before its handshake completes.
    context.maximum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(certificate, key)
    handshakes = Queue()

    class TlsFixture(ThreadingHTTPServer):
        def get_request(self):
            connection, address = super().get_request()
            try:
                connection.settimeout(3)
                secure = context.wrap_socket(connection, server_side=True)
            except OSError as error:
                connection.close()
                handshakes.put(error)
                raise
            handshakes.put(None)
            return secure, address

    path = "/untrusted-certificate"
    with TlsFixture(("127.0.0.1", 0), Fixture) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        uri = f"https://127.0.0.1:{server.server_port}{path}"
        try:
            # Trust the certificate in this control client only. Prove that
            # the same fixture can complete TLS and answer an HTTP request.
            control = HTTPSConnection("127.0.0.1", server.server_port, timeout=3,
                                      context=ssl.create_default_context(cafile=certificate))
            try:
                control.request("GET", "/tls-control")
                response = control.getresponse()
                assert response.status == 200
                response.read()
                assert handshakes.get(timeout=3) is None
            finally:
                control.close()
            browser.start()
            browser.enter(uri)
            for attempt in ("navigation", "retry"):
                error = handshakes.get(timeout=12)
                # TLS backends can send a certificate alert or reset the
                # socket when cancelling a failed certificate handshake.
                assert isinstance(error, ConnectionResetError) or (
                    isinstance(error, ssl.SSLError)
                    and error.reason in {"TLSV1_ALERT_UNKNOWN_CA", "SSLV3_ALERT_BAD_CERTIFICATE"}
                ), (attempt, error)
                assert Fixture.count(path) == 0, "The untrusted TLS peer received an HTTP request"
                assert not browser.matching_history(uri, "Untrusted-Certificate")
                if attempt == "navigation":
                    eventually(
                        lambda: (snapshot := browser.current())
                        and uri in tab_paths(snapshot, 0),
                        "The failed TLS navigation was not recorded in the session",
                    )
                    browser.frame("untrusted-certificate")
                    browser.key("ctrl+r")
            healthy = f"{base}/after-tls-error"
            browser.enter(healthy)
            eventually(lambda: browser.matching_history(healthy, "After-Tls-Error"),
                       "The browser could not navigate after the TLS failure")
            assert Fixture.count(path) == 0
        finally:
            browser.stop()
            server.shutdown()
    return {"untrusted_certificate_rejected": True, "retry_rejected": True,
            "navigation_after_tls_failure": True}


def dialogs_and_persistence(binary, directory, base, screenshots):
    """Save dialog, uploads, and zoom and download list after a restart."""
    browser = Browser(binary, directory, screenshots)
    config = directory / "config" / "bitter-browser"
    config.mkdir(parents=True)
    (config / "config.toml").write_text("version = 1\n\n[downloads]\nask_location = true\n")
    chosen = directory / "chosen" / "picked name.txt"
    chosen.parent.mkdir()
    records = browser.profile / "downloads.json"
    try:
        browser.start()
        time.sleep(1)
        browser.enter(f"{base}/download")
        browser.dialog()
        browser.key("ctrl+a")
        browser.xdotool("type", "--clearmodifiers", "--delay", "10", str(chosen))
        browser.key("Return")
        eventually(lambda: chosen.exists() and chosen.read_bytes() == DOWNLOAD_BODY,
                   "The chosen destination was not used")
        assert not list(browser.downloads.glob("*")), "Ask mode also saved to the download directory"
        eventually(lambda: records.exists() and str(chosen) in records.read_text(),
                   "The download list was not saved")

        browser.enter(f"{base}/upload")
        time.sleep(1)
        browser.click(300, 88)
        browser.dialog()
        browser.key("ctrl+l")
        browser.xdotool("type", "--clearmodifiers", "--delay", "10", str(chosen))
        time.sleep(0.5)
        # Drop any inline completion GTK appended after the cursor.
        browser.key("shift+End", "Delete")
        if screenshots:
            subprocess.run(["import", "-window", "root", str(screenshots / "upload-dialog.png")])
        browser.key("Return")
        size = len(DOWNLOAD_BODY)
        eventually(lambda: f"picked name.txt|{size}|{size}" in Fixture.uploads,
                   f"The page did not receive the uploaded file: {Fixture.uploads}")

        since = Fixture.mark()
        browser.enter(f"{base}/probe")
        _, width, _ = eventually(lambda: Fixture.report(lambda r: r[1] > 0, since), "Probe did not load")
        since = Fixture.mark()
        browser.key("ctrl+plus")
        eventually(lambda: Fixture.report(lambda r: r[1] < width, since), "Zoom in failed")
        time.sleep(0.3)
        browser.stop()
        zoom = json.loads((browser.profile / "zoom.json").read_text())
        assert zoom["sites"] == [["127.0.0.1", 1.1]], zoom

        since = Fixture.mark()
        browser.start()
        zoomed = eventually(lambda: Fixture.report(lambda r: r[1] > 0, since),
                            "Restored probe did not load")
        assert zoomed[1] < width, f"Zoom was not restored: {zoomed[1]} >= {width}"
        browser.key("ctrl+j")
        time.sleep(0.6)
        browser.frame("downloads-restored")
        browser.key("Escape")
    finally:
        browser.stop()
    return {"uploaded": True, "restored_zoom_width": zoomed[1], "default_width": width}


def check_cancelled_address_draft(browser, uri):
    path = urllib.parse.urlsplit(uri).path
    requests = Fixture.count(path)
    browser.key("ctrl+l", "ctrl+a")
    # Internal addresses never show the suggestions popover.
    browser.xdotool("type", "--clearmodifiers", "--delay", "0", "bitter://history")
    browser.key("Escape", "Return")
    eventually(lambda: Fixture.count(path) == requests + 1,
               "Escape did not restore the current address without suggestions")


def web_platform(binary, directory, base, screenshots):
    """Pop-ups, permission prompts, the context menu, developer tools,
    content blocking and the site and privacy panels."""
    browser = Browser(binary, directory, screenshots)
    settings = browser.profile / "site-settings.json"
    page = f"{base}/platform"
    # Page coordinates to window coordinates: sidebar and toolbar.
    left, top = 226, 59

    def event(name, since, expected=None, message=None):
        return eventually(
            lambda: (value := Fixture.event(name, since)) is not None
            and (expected is None or value == expected) and value,
            message or f"No {name} event{'' if expected is None else ' = ' + expected}: {Fixture.events[since:]}")

    def main_tabs():
        session = browser.current()
        return tab_paths(session, 0) if session else []

    try:
        browser.start()
        since = Fixture.event_mark()
        browser.enter(page)
        event("notification", since, "default")
        event("query", since, "prompt")
        # The built-in list hides `.adsbygoogle`; it compiles on first start.
        hidden = None
        for _ in range(20):
            since = Fixture.event_mark()
            hidden = event("ad", since)
            if hidden == "none":
                break
            time.sleep(0.5)
            browser.plain_key(F5)
        assert hidden == "none", f"Element hiding did not apply: display {hidden}"
        start_height = int(event("height", 0))

        since = Fixture.event_mark()
        browser.click(left + 40, top + 25)
        event("opener", since, "true", "The pop-up did not keep window.opener")
        event("message", since, "hello from the pop-up", "The opener did not receive postMessage")
        eventually(lambda: len(main_tabs()) == 2 and "popup-child" in main_tabs()[1],
                   f"The pop-up did not open as the next tab: {main_tabs()}")
        browser.frame("popup")
        eventually(lambda: main_tabs() == [page], f"window.close() did not close the pop-up: {main_tabs()}")

        since = Fixture.event_mark()
        browser.click(left + 40, top + 85)
        time.sleep(0.8)
        browser.frame("permission-bar")
        width, _ = browser.size()
        browser.click(width - 52, top + 25)
        event("requested", since, "granted", "Allowing notifications did not reach the page")
        eventually(lambda: settings.exists() and '"notifications"' in settings.read_text(),
                   "The notification answer was not remembered")
        saved = json.loads(settings.read_text())["permissions"]
        assert saved == [{"origin": base, "permission": "notifications", "allowed": True}], saved

        since = Fixture.event_mark()
        browser.plain_key(F5)
        event("notification", since, "granted", "The remembered answer was not given to the page")
        event("query", since, "granted", "navigator.permissions did not report the answer")

        browser.xdotool("mousemove", "--window", browser.window, str(left + 50), str(top + 140))
        time.sleep(0.5)
        browser.frame("link-status")
        browser.xdotool("click", "3")
        time.sleep(0.8)
        browser.frame("context-menu")
        # The menu opens at the pointer; the second item opens a new tab.
        browser.click(left + 150, top + 140 + 56)
        eventually(lambda: len(main_tabs()) == 2 and main_tabs()[1].endswith("/linked")
                   and browser.current()["workspaces"][0]["selected"] == 1,
                   f"Open Link in New Tab did not open a tab: {browser.current()}")
        browser.key("ctrl+w")
        eventually(lambda: main_tabs() == [page], f"Could not close the linked tab: {main_tabs()}")

        since = Fixture.event_mark()
        browser.plain_key(F12)
        eventually(lambda: (value := Fixture.event("height", since)) and int(value) < start_height - 100,
                   "F12 did not open the Web Inspector")
        time.sleep(1)
        browser.frame("devtools")
        since = Fixture.event_mark()
        browser.plain_key(F12)
        eventually(lambda: (value := Fixture.event("height", since)) and int(value) == start_height,
                   "F12 did not close the Web Inspector")

        browser.click(406, 29)
        time.sleep(0.6)
        browser.frame("site-info")
        browser.key("Escape")
        since = Fixture.event_mark()
        browser.plain_key(F5)
        event("cookie", since, "kept", "The page cookie was not stored")
        eventually(lambda: browser.matching_history(page, "Platform"),
                   "The page was not in the history")
        browser.key("ctrl+shift+Delete")
        time.sleep(0.6)
        browser.frame("clear-data")
        width, height = browser.size()
        # "Clear Data", the dialog's right-hand button.
        browser.click(width // 2 + 98, height // 2 + 137)
        eventually(lambda: not browser.matching_history(page, "Platform"),
                   "Clearing did not remove the history entry")
        time.sleep(0.5)
        since = Fixture.event_mark()
        browser.plain_key(F5)
        event("cookie", since, "none", "Clearing did not remove the site's cookie")
        browser.key("ctrl+k")
        browser.xdotool("type", "--clearmodifiers", "--delay", "0", "site permissions")
        time.sleep(0.3)
        browser.key("Return")
        time.sleep(0.8)
        browser.frame("site-permissions")
        browser.key("Escape")
    finally:
        browser.stop()
    return {"popup_opener": True, "notification_remembered": True, "context_menu_tab": True,
            "devtools": True, "element_hiding": True, "cleared_history_and_cookies": True}


def window_geometry(binary, directory):
    browser = Browser(binary, directory, None)
    keyfile = directory / "config" / "glib-2.0" / "settings" / "keyfile"
    try:
        browser.start()
        default = browser.size()
        browser.xdotool("windowsize", browser.window, "980", "690")
        eventually(lambda: browser.size() == (980, 690), "The window did not resize")
        time.sleep(0.3)
        browser.stop()
        saved = keyfile.read_text()
        assert "is-maximized=false" in saved, saved
        width = int(re.search(r"window-width=(\d+)", saved)[1])
        height = int(re.search(r"window-height=(\d+)", saved)[1])
        assert abs(width - 980) <= 40 and abs(height - 690) <= 40, saved
        browser.start()
        restored = browser.size()
        assert restored != default and abs(restored[0] - 980) <= 40, (default, restored)
    finally:
        browser.stop()
    return {"default_size": default, "restored_size": restored}


def check_last_tab_lifecycle(browser, uri):
    """Closing the last tab affects only its workspace and is reversible."""
    before = eventually(
        lambda: (saved := browser.current()) and saved["workspace"] == "second"
        and tab_paths(saved, 1) == [uri] and saved,
        "The session did not save the second workspace's current page",
    )
    main_tabs = tab_paths(before, 0)
    main_selected = before["workspaces"][0]["selected"]

    def second_workspace(paths, selected):
        saved = browser.current()
        if not saved or saved["workspace"] != "second":
            return False
        assert tab_paths(saved, 0) == main_tabs, "Closing a tab changed the other workspace"
        assert saved["workspaces"][0]["selected"] == main_selected
        return tab_paths(saved, 1) == paths and saved["workspaces"][1]["selected"] == selected

    home = "bitter://newtab"
    browser.key("ctrl+w")
    eventually(lambda: second_workspace([home], 0), "The last tab was not replaced with a new tab")
    path = urllib.parse.urlsplit(uri).path
    requests = Fixture.count(path)
    browser.key("ctrl+shift+t")
    eventually(lambda: second_workspace([uri, home], 0), "The closed tab was not reopened in place")
    eventually(lambda: Fixture.count(path) == requests + 1, "The reopened tab did not load")
    browser.key("ctrl+Page_Down")
    eventually(lambda: second_workspace([uri, home], 1), "The replacement tab was not selectable")
    browser.key("ctrl+w")
    eventually(lambda: second_workspace([uri], 0), "The reopened tab was not kept selected")


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
    if not shutil.which("openssl"):
        parser.error("openssl is required")
    assert os.environ.get("DISPLAY") and os.environ.get("DBUS_SESSION_BUS_ADDRESS"), "Use Xvfb and dbus-run-session"
    fixture = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    threading.Thread(target=fixture.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{fixture.server_port}"
    with tempfile.TemporaryDirectory(prefix="bitter-daily-") as root:
        directory = Path(root)
        browser = Browser(args.binary, directory / "flow", args.screenshots)
        try:
            print(json.dumps(run_flow(browser, base)), flush=True)
            print(json.dumps(window_geometry(args.binary, directory / "geometry")), flush=True)
            tls_browser = Browser(args.binary, directory / "tls", args.screenshots)
            print(json.dumps(untrusted_certificate(tls_browser, base, directory)), flush=True)
            print(json.dumps(dialogs_and_persistence(args.binary, directory / "dialogs", base,
                                                     args.screenshots)), flush=True)
            print(json.dumps(web_platform(args.binary, directory / "platform", base,
                                          args.screenshots)), flush=True)
            if args.benchmark_samples:
                print(json.dumps(benchmark(args.binary, directory, base, args.benchmark_samples), indent=2))
            assert "panicked" not in browser.log.read_text(), browser.log.read_text()
        except Exception:
            print("Last snapshot:", browser.current(), flush=True)
            print("HTTP requests:", dict(Fixture.paths), flush=True)
            print("Probe reports:", Fixture.reports, flush=True)
            if browser.log.exists():
                print(browser.log.read_text(), flush=True)
            raise
        finally:
            browser.stop()
            fixture.shutdown()
            fixture.server_close()


if __name__ == "__main__":
    main()
