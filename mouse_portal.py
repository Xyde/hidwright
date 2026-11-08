"""Mouse Portal: a local web page showing every supported mouse side by side.

  python mouse_portal.py            # opens http://127.0.0.1:8642 in your browser
  python mouse_portal.py --no-browser

Pulsar X2A: full read + edit (writes go to the mouse's onboard memory).
Razer Viper: read-only.
Rotation calibrator: measures each mouse's swipe angle separately (Raw Input)
and can give each mouse its own RawAccel profile/rotation (backs up settings.json first).
Only listens on 127.0.0.1; close the window (Ctrl+C) when you're done.
Nothing here affects input latency; the mice do all the work themselves.
"""
import argparse
import json
import os
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pulsar_x2a as px
import rawaccel_cfg
import test_dpi
from razer_viper import RazerViper
from rawinput_monitor import RawMouseMonitor

PORT = 8642
HTML = Path(__file__).with_name("mouse_portal.html")
hid_lock = threading.Lock()  # one HID conversation at a time
monitor = None  # RawMouseMonitor, started in main()


def known_mice():
    """Mice seen by raw input that we have names for (Pulsar, Razer)."""
    return [d for d in monitor.devices_info() if not d["name"].startswith("Mouse ")]


def rawaccel_state():
    return rawaccel_cfg.summary(known_mice())


TEST_DPIS = (400, 800, 1600, 3200)


def stop_calibration():
    """End a run: freeze the recording, put any test DPI back, return the analysis."""
    monitor.stop()
    try:
        with hid_lock:
            restored = test_dpi.restore_if_pending()
    except Exception as e:  # never lose the result over this; the file stays for the next try
        restored = {"error": str(e)}
    return {**monitor.status(), "restored": restored}


def pulsar_state(m, profile=None):
    active = m.active_profile()
    prof = profile or active
    act_stage, stages = m.dpi_stages(prof)
    return {
        "present": True, "name": "Pulsar X2A", "editable": True,
        "firmware": m.firmware(), "active_profile": active, "profile": prof,
        "polling": m.polling(), "debounce": m.debounce(),
        "toggles": {n: m.toggle(n) for n in px.X2A.TOGGLES},
        "dpi_stages": [x for x, _ in stages], "dpi_active": act_stage,
        "lod": m.lod(prof),
        "buttons": [{"key": key, "label": px.BUTTON_LABELS[bid],
                     "binding": px.describe(b), "default": b == px.BUTTON_DEFAULTS[bid]}
                    for key, bid in px.BUTTONS.items()
                    for b in [m.get_button(bid, prof)]],
        "polling_options": sorted(px.POLL_HZ_TO_CODE),
    }


def read_pulsar(profile=None):
    try:
        m = px.X2A()
    except SystemExit as e:
        return {"present": False, "name": "Pulsar X2A", "error": str(e)}
    try:
        return pulsar_state(m, profile)
    finally:
        m.close()


def read_razer():
    try:
        m = RazerViper()
    except RuntimeError as e:
        return {"present": False, "name": "Razer Viper", "error": str(e)}
    try:
        s = m.settings()
        act, st = s["dpi_stages"] or (None, [])
        return {"present": True, "name": "Razer Viper", "editable": False,
                "firmware": s["firmware"], "polling": s["polling"],
                "dpi": s["dpi"][0] if s["dpi"] else None,
                "dpi_stages": [x for x, _ in st], "dpi_active": act,
                "debounce": "none (optical switches)"}
    finally:
        m.close()


def apply_pulsar(req):
    """Apply one change to the Pulsar; returns its fresh state."""
    a = req.get("action")
    m = px.X2A()
    try:
        prof = int(req.get("profile") or m.active_profile())
        if not 1 <= prof <= 5:
            raise ValueError("profile must be 1-5")
        if a == "poll":
            hz = int(req["value"])
            if hz not in px.POLL_HZ_TO_CODE:
                raise ValueError("unsupported polling rate")
            m.set_polling(hz)
        elif a == "debounce":
            ms = int(req["value"])
            if not 0 <= ms <= 20:
                raise ValueError("debounce must be 0-20 ms")
            m.set_debounce(ms)
        elif a == "toggle":
            name = req["name"]
            if name not in px.X2A.TOGGLES:
                raise ValueError("unknown toggle")
            m.set_toggle(name, bool(req["value"]))
        elif a == "dpi":
            dpis = [int(d) for d in req["dpis"]]
            active = int(req["active"])
            if not 1 <= len(dpis) <= px.MAX_STAGES:
                raise ValueError(f"1-{px.MAX_STAGES} DPI stages")
            if any(not px.DPI_MIN <= d <= px.DPI_MAX or d % px.DPI_STEP for d in dpis):
                raise ValueError(f"DPI must be {px.DPI_MIN}-{px.DPI_MAX} in steps of {px.DPI_STEP}")
            if not 1 <= active <= len(dpis):
                raise ValueError("active stage out of range")
            m.set_dpi_stages(prof, dpis, active)
        elif a == "lod":
            mm = int(req["value"])
            if mm not in (1, 2):
                raise ValueError("lift-off must be 1 or 2 mm")
            m.set_lod(prof, mm)
        elif a == "bind":
            key = req["button"]
            if key not in px.BUTTONS:
                raise ValueError("unknown button")
            bid = px.BUTTONS[key]
            fn = str(req["function"]).strip()
            m.set_button(bid, px.BUTTON_DEFAULTS[bid] if fn.lower() == "default"
                         else px.parse_function(fn), prof)
        elif a == "profile":
            p = int(req["value"])
            if not 1 <= p <= 5:
                raise ValueError("profile must be 1-5")
            m.set_active_profile(p)
            prof = p
        else:
            raise ValueError(f"unknown action {a!r}")
        return pulsar_state(m, prof)
    finally:
        m.close()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json"):
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def _stream(self):
        """Server-sent events: new movement of the mouse being calibrated, ~120 times a second."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        idx, last_write = 0, time.monotonic()
        try:
            while True:
                d = monitor.since(idx)
                if d["deltas"] or not d["recording"]:
                    idx += len(d["deltas"])
                    self.wfile.write(f"data: {json.dumps(d, separators=(',', ':'))}\n\n".encode())
                    self.wfile.flush()
                    last_write = time.monotonic()
                elif time.monotonic() - last_write > 1:
                    self.wfile.write(b": ping\n\n")  # notices a closed page even if nothing moves
                    self.wfile.flush()
                    last_write = time.monotonic()
                if not d["recording"]:
                    return
                time.sleep(0.008)
        except (BrokenPipeError, ConnectionError, OSError):
            # page went away mid-run (closed tab, Esc, finished): stop and put the DPI back
            if monitor.recording:
                stop_calibration()
            return

    def _local_only(self):
        host = (self.headers.get("Host") or "").split(":")[0]
        return host in ("127.0.0.1", "localhost")

    def do_GET(self):
        if not self._local_only():
            return self._send(403, {"error": "local only"})
        if self.path in ("/", "/index.html"):
            return self._send(200, HTML.read_bytes(), "text/html; charset=utf-8")
        if self.path.startswith("/api/mice"):
            prof = None
            if "profile=" in self.path:
                try:
                    prof = int(self.path.split("profile=")[1].split("&")[0])
                except ValueError:
                    prof = None
            with hid_lock:
                return self._send(200, {"pulsar": read_pulsar(prof), "razer": read_razer()})
        if self.path == "/api/motion":
            return self._send(200, {"ok": monitor.ok, "error": monitor.error,
                                    "devices": monitor.devices_info()})
        if self.path == "/api/calib/status":
            return self._send(200, monitor.status())
        if self.path == "/api/calib/stream":
            return self._stream()
        if self.path == "/api/rawaccel":
            return self._send(200, rawaccel_state())
        self._send(404, {"error": "not found"})

    def do_POST(self):
        origin = self.headers.get("Origin", "")
        port = self.server.server_address[1]
        if not self._local_only() or (origin and origin not in (
                f"http://127.0.0.1:{port}", f"http://localhost:{port}")):
            return self._send(403, {"error": "local only"})
        try:
            req = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
            if self.path == "/api/pulsar":
                with hid_lock:
                    return self._send(200, apply_pulsar(req))
            if self.path == "/api/calib/start":
                dpi = int(req.get("dpi") or 0)
                if dpi and (dpi not in TEST_DPIS):
                    raise ValueError(f"test DPI must be one of {TEST_DPIS}")
                with hid_lock:
                    dpis = test_dpi.apply(dpi) if dpi else test_dpi.current_dpis()
                monitor.start(dpis)
                return self._send(200, {"ok": True, "dpis": dpis, "test_dpi": dpi})
            if self.path == "/api/calib/stop":
                return self._send(200, stop_calibration())
            if self.path == "/api/rawaccel/apply":
                known = {d["id"]: d["name"] for d in known_mice()}
                changes = []
                for ch in req["changes"]:
                    if ch["id"] not in known:
                        raise ValueError(f"unknown mouse {ch['id']!r}")
                    changes.append({"id": ch["id"], "name": known[ch["id"]],
                                    "rotation": float(ch["rotation"])})
                result = rawaccel_cfg.apply(changes)
                return self._send(200, {**result, "state": rawaccel_state()})
            return self._send(404, {"error": "not found"})
        except (ValueError, KeyError, TypeError) as e:
            return self._send(400, {"error": str(e)})
        except (IOError, OSError, SystemExit) as e:
            return self._send(500, {"error": str(e)})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-browser", action="store_true")
    ap.add_argument("--port", type=int, default=int(os.environ.get("PORT", PORT)))
    args = ap.parse_args()
    global monitor
    try:
        fixed = test_dpi.restore_if_pending()  # a previous run crashed mid-test?
        if fixed:
            print("Restored DPI after an interrupted calibration:", fixed)
    except Exception as e:
        print("Couldn't restore DPI from an interrupted calibration yet:", e)
    monitor = RawMouseMonitor()
    ThreadingHTTPServer.allow_reuse_address = False  # on Windows, reuse would let two portals share a port
    try:
        srv = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    except OSError:
        url = f"http://127.0.0.1:{args.port}"
        print(f"Port {args.port} is busy; the portal is probably already running at {url}")
        if not args.no_browser:
            webbrowser.open(url)
        return
    url = f"http://127.0.0.1:{args.port}"
    print(f"Mouse Portal running at {url}  (Ctrl+C to stop)")
    if not args.no_browser:
        webbrowser.open(url)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
