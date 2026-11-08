"""Per-mouse raw movement capture (Windows Raw Input), for the rotation calibrator.

A background thread owns a hidden message-only window registered for raw mouse
input with RIDEV_INPUTSINK, so it sees movement from every mouse separately,
even while the browser has focus. It only keeps movement deltas (no buttons),
only in memory, and only while the portal runs. It doesn't change or delay
input for any other program.

Note: RawAccel's driver sits below Raw Input, so the deltas seen here are
already rotated/accelerated by whatever RawAccel profile is active.
"""
import ctypes
import math
import threading
import time
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)
cfgmgr32 = ctypes.WinDLL("cfgmgr32")
kernel32 = ctypes.WinDLL("kernel32")

WM_INPUT = 0x00FF
RID_INPUT = 0x10000003
RIDI_DEVICENAME = 0x20000007
RIM_TYPEMOUSE = 0
RIDEV_INPUTSINK = 0x00000100
MOUSE_MOVE_ABSOLUTE = 0x01
HWND_MESSAGE = wintypes.HWND(-3)

KNOWN = {"VID_3710&PID_1404": "Pulsar X2A", "VID_1532&PID_0078": "Razer Viper"}

LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)


class WNDCLASSW(ctypes.Structure):
    _fields_ = [("style", wintypes.UINT), ("lpfnWndProc", WNDPROC), ("cbClsExtra", ctypes.c_int),
                ("cbWndExtra", ctypes.c_int), ("hInstance", wintypes.HINSTANCE),
                ("hIcon", wintypes.HICON), ("hCursor", wintypes.HANDLE),
                ("hbrBackground", wintypes.HBRUSH), ("lpszMenuName", wintypes.LPCWSTR),
                ("lpszClassName", wintypes.LPCWSTR)]


class RAWINPUTDEVICE(ctypes.Structure):
    _fields_ = [("usUsagePage", wintypes.USHORT), ("usUsage", wintypes.USHORT),
                ("dwFlags", wintypes.DWORD), ("hwndTarget", wintypes.HWND)]


class RAWINPUTDEVICELIST(ctypes.Structure):
    _fields_ = [("hDevice", wintypes.HANDLE), ("dwType", wintypes.DWORD)]


class RAWINPUTHEADER(ctypes.Structure):
    _fields_ = [("dwType", wintypes.DWORD), ("dwSize", wintypes.DWORD),
                ("hDevice", wintypes.HANDLE), ("wParam", wintypes.WPARAM)]


class RAWMOUSE(ctypes.Structure):
    _fields_ = [("usFlags", wintypes.USHORT), ("ulButtons", wintypes.ULONG),
                ("ulRawButtons", wintypes.ULONG), ("lLastX", wintypes.LONG),
                ("lLastY", wintypes.LONG), ("ulExtraInformation", wintypes.ULONG)]


class RAWINPUT_MOUSE(ctypes.Structure):
    _fields_ = [("header", RAWINPUTHEADER), ("mouse", RAWMOUSE)]


class DEVPROPKEY(ctypes.Structure):
    _fields_ = [("fmtid", ctypes.c_byte * 16), ("pid", wintypes.ULONG)]


def _devpkey_instance_id():
    import uuid
    k = DEVPROPKEY()
    ctypes.memmove(k.fmtid, uuid.UUID("78c34fc8-104a-4aca-9ea4-524d52996e57").bytes_le, 16)
    k.pid = 256
    return k


DEVPKEY_Device_InstanceId = _devpkey_instance_id()

user32.DefWindowProcW.restype = LRESULT
user32.DefWindowProcW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.RegisterClassW.argtypes = [ctypes.POINTER(WNDCLASSW)]
user32.CreateWindowExW.restype = wintypes.HWND
user32.CreateWindowExW.argtypes = [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD,
                                   ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                   wintypes.HWND, wintypes.HMENU, wintypes.HINSTANCE, wintypes.LPVOID]
user32.RegisterRawInputDevices.argtypes = [ctypes.POINTER(RAWINPUTDEVICE), wintypes.UINT, wintypes.UINT]
user32.GetRawInputData.argtypes = [wintypes.HANDLE, wintypes.UINT, wintypes.LPVOID,
                                   ctypes.POINTER(wintypes.UINT), wintypes.UINT]
user32.GetRawInputDeviceInfoW.argtypes = [wintypes.HANDLE, wintypes.UINT, wintypes.LPVOID,
                                          ctypes.POINTER(wintypes.UINT)]
user32.GetRawInputDeviceList.argtypes = [ctypes.POINTER(RAWINPUTDEVICELIST),
                                         ctypes.POINTER(wintypes.UINT), wintypes.UINT]
user32.GetMessageW.argtypes = [ctypes.POINTER(wintypes.MSG), wintypes.HWND, wintypes.UINT, wintypes.UINT]
user32.TranslateMessage.argtypes = [ctypes.POINTER(wintypes.MSG)]
user32.DispatchMessageW.argtypes = [ctypes.POINTER(wintypes.MSG)]
kernel32.GetModuleHandleW.restype = wintypes.HMODULE
kernel32.GetModuleHandleW.argtypes = [wintypes.LPCWSTR]
cfgmgr32.CM_Get_Device_Interface_PropertyW.argtypes = [
    wintypes.LPCWSTR, ctypes.POINTER(DEVPROPKEY), ctypes.POINTER(wintypes.ULONG),
    ctypes.c_void_p, ctypes.POINTER(wintypes.ULONG), wintypes.ULONG]


def _interface_name(hdev):
    n = wintypes.UINT(0)
    user32.GetRawInputDeviceInfoW(hdev, RIDI_DEVICENAME, None, ctypes.byref(n))
    buf = ctypes.create_unicode_buffer(n.value + 1)
    if user32.GetRawInputDeviceInfoW(hdev, RIDI_DEVICENAME, buf, ctypes.byref(n)) in (0, 0xFFFFFFFF):
        return ""
    return buf.value


def rawaccel_id(interface_name):
    """Same id RawAccel uses: the device instance id with its last \\-segment cut off."""
    ptype, size = wintypes.ULONG(), wintypes.ULONG(0)
    cfgmgr32.CM_Get_Device_Interface_PropertyW(interface_name, ctypes.byref(DEVPKEY_Device_InstanceId),
                                               ctypes.byref(ptype), None, ctypes.byref(size), 0)
    if not size.value:
        return ""
    buf = ctypes.create_string_buffer(size.value)
    if cfgmgr32.CM_Get_Device_Interface_PropertyW(interface_name, ctypes.byref(DEVPKEY_Device_InstanceId),
                                                  ctypes.byref(ptype), buf, ctypes.byref(size), 0):
        return ""
    inst = ctypes.wstring_at(buf)
    return inst[: inst.rfind("\\")] if "\\" in inst else inst


def _label(dev_id):
    up = dev_id.upper()
    for key, name in KNOWN.items():
        if key in up:
            return name
    return "Mouse " + "&".join(p for p in up.split("\\")[-1].split("&") if p.startswith(("VID_", "PID_")))


class Device:
    def __init__(self, hdev):
        self.iface = _interface_name(hdev)
        self.id = rawaccel_id(self.iface) if self.iface else ""
        self.name = _label(self.id or self.iface)
        self.last_move = 0.0
        self.counts = 0  # total movement magnitude seen (for "most active" picks)


class RawMouseMonitor:
    def __init__(self):
        self.lock = threading.Lock()
        self.devices = {}        # hDevice value -> Device
        self.recording = False
        self.samples = {}        # hDevice value -> list of (t, dx, dy)
        self.locked = None       # the mouse being calibrated (first one to move after start)
        self.pre_counts = {}
        self.dpi_by_id = {}      # DPI each mouse runs at during the recording
        self.ok = False
        self.error = None
        self._ready = threading.Event()
        threading.Thread(target=self._run, daemon=True, name="rawinput").start()
        self._ready.wait(3)

    # ---------------- device list
    def _enumerate(self):
        n = wintypes.UINT(0)
        user32.GetRawInputDeviceList(None, ctypes.byref(n), ctypes.sizeof(RAWINPUTDEVICELIST))
        arr = (RAWINPUTDEVICELIST * n.value)()
        got = user32.GetRawInputDeviceList(arr, ctypes.byref(n), ctypes.sizeof(RAWINPUTDEVICELIST))
        if got == 0xFFFFFFFF:
            return
        for d in arr[:got]:
            if d.dwType == RIM_TYPEMOUSE and d.hDevice and d.hDevice not in self.devices:
                dev = Device(d.hDevice)
                if dev.iface and "RDP_MOU" not in dev.iface.upper():
                    self.devices[d.hDevice] = dev

    # ---------------- window thread
    def _run(self):
        try:
            self._proc = WNDPROC(self._wndproc)  # keep a reference
            hinst = kernel32.GetModuleHandleW(None)
            wc = WNDCLASSW(lpfnWndProc=self._proc, hInstance=hinst, lpszClassName="MousePortalRawInput")
            user32.RegisterClassW(ctypes.byref(wc))
            hwnd = user32.CreateWindowExW(0, "MousePortalRawInput", "", 0, 0, 0, 0, 0,
                                          HWND_MESSAGE, None, hinst, None)
            rid = RAWINPUTDEVICE(0x01, 0x02, RIDEV_INPUTSINK, hwnd)
            if not hwnd or not user32.RegisterRawInputDevices(ctypes.byref(rid), 1, ctypes.sizeof(rid)):
                raise OSError(f"raw input registration failed ({ctypes.get_last_error()})")
            with self.lock:
                self._enumerate()
            self.ok = True
        except Exception as e:  # report, don't crash the portal
            self.error = str(e)
            self._ready.set()
            return
        self._ready.set()
        msg = wintypes.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))

    def _wndproc(self, hwnd, msg, wparam, lparam):
        if msg == WM_INPUT:
            try:
                self._on_input(lparam)
            except Exception:
                pass
        return user32.DefWindowProcW(hwnd, msg, wparam, lparam)

    def _on_input(self, lparam):
        size = wintypes.UINT(ctypes.sizeof(RAWINPUT_MOUSE))
        ri = RAWINPUT_MOUSE()
        if user32.GetRawInputData(lparam, RID_INPUT, ctypes.byref(ri), ctypes.byref(size),
                                  ctypes.sizeof(RAWINPUTHEADER)) in (0xFFFFFFFF,):
            return
        if ri.header.dwType != RIM_TYPEMOUSE or ri.mouse.usFlags & MOUSE_MOVE_ABSOLUTE:
            return
        dx, dy = ri.mouse.lLastX, ri.mouse.lLastY
        if not dx and not dy:
            return
        h = ri.header.hDevice
        now = time.perf_counter()
        with self.lock:
            dev = self.devices.get(h)
            if dev is None:
                dev = self.devices[h] = Device(h)
            dev.last_move = now
            dev.counts += abs(dx) + abs(dy)
            if self.recording and (self.locked is None or self.locked == h):
                self.samples.setdefault(h, []).append((now, dx, dy))
                if self.locked is None:
                    # lock onto the first mouse that really moves; ignore the others from then on
                    moved = self.pre_counts.get(h, 0) + abs(dx) + abs(dy)
                    self.pre_counts[h] = moved
                    if moved > 100:
                        self.locked = h
                        self.samples = {h: self.samples[h]}

    # ---------------- public API
    def devices_info(self):
        now = time.perf_counter()
        with self.lock:
            self._enumerate()
            return [{"key": str(h), "id": d.id, "name": d.name,
                     "moving": now - d.last_move < 0.25} for h, d in self.devices.items()]

    def start(self, dpi_by_id=None):
        with self.lock:
            self.dpi_by_id = dict(dpi_by_id or {})
            self.samples = {}
            self.locked = None
            self.pre_counts = {}
            self.recording = True
            self.started = time.perf_counter()

    def stop(self):
        with self.lock:
            self.recording = False

    def since(self, idx):
        """New movement of the locked mouse after sample #idx (for live streaming)."""
        with self.lock:
            h = self.locked
            if h is None:
                return {"device": None, "recording": self.recording, "deltas": []}
            s = self.samples.get(h, [])
            dev = self.devices.get(h)
            dpi = self.dpi_by_id.get(dev.id if dev else "", 1600)
            hyst, min_len = thresholds(dpi)
            return {"device": {"id": dev.id, "name": dev.name} if dev else None,
                    "dpi": dpi, "hyst": hyst, "min": min_len,
                    "recording": self.recording, "start": idx,
                    "deltas": [[dx, dy] for _, dx, dy in s[idx:]],
                    "idle_for": time.perf_counter() - s[-1][0] if s else 0}

    def status(self, want_path=True):
        """Analysis of the current/last recording for the most-moved device."""
        with self.lock:
            if not self.samples:
                return {"recording": self.recording, "device": None}
            h, s = max(self.samples.items(), key=lambda kv: sum(abs(dx) + abs(dy) for _, dx, dy in kv[1]))
            s = list(s)
            dev = self.devices.get(h)
            others = {self.devices[k].name: len(v) for k, v in self.samples.items()
                      if k != h and k in self.devices}
        res = analyze(s, self.dpi_by_id.get(dev.id if dev else "", 1600))
        res.update({"recording": self.recording,
                    "device": {"key": str(h), "id": dev.id if dev else "", "name": dev.name if dev else "?"},
                    "idle_for": time.perf_counter() - s[-1][0],
                    "other_devices": others})
        if not want_path:
            res.pop("path", None)
        return res


def thresholds(dpi):
    """Swipe detection in real distance, so it behaves the same at any DPI:
    a turn needs 0.1 inch (2.5 mm) of movement back, a swipe at least 0.4 inch (1 cm)."""
    dpi = dpi or 1600
    return max(20, round(0.1 * dpi)), max(80, round(0.4 * dpi))


def analyze(samples, dpi=1600):
    """Principal direction of the movement deltas + swipe segmentation."""
    sxx = syy = sxy = 0.0
    for _, dx, dy in samples:
        sxx += dx * dx
        syy += dy * dy
        sxy += dx * dy
    angle = 0.5 * math.degrees(math.atan2(2 * sxy, sxx - syy)) if (sxx or syy) else 0.0

    # cumulative path (raw counts, y down)
    x = y = 0
    path = [(0, 0)]
    for _, dx, dy in samples:
        x += dx
        y += dy
        path.append((x, y))

    hyst, min_len = thresholds(dpi)
    seg_angles, completed = _segment_angles(path, hyst, min_len)
    spread = _stdev(seg_angles) if len(seg_angles) >= 3 else None
    step = max(1, len(path) // 600)
    return {"angle": round(angle, 2), "swipes": len(seg_angles), "completed": completed, "dpi": dpi,
            "spread": round(spread, 2) if spread is not None else None,
            "samples": len(samples), "path": path[::step] + [path[-1]]}


def _segment_angles(path, hysteresis=150, min_len=400):
    """Angle (deg, raw coords, y down) of each swipe: a run between two x turning points.

    A turning point is confirmed once x has come back `hysteresis` counts from the
    furthest point; swipes shorter than `min_len` counts in x are ignored. The swipe
    still in progress at the end counts too. Returns (angles, completed swipes),
    where "completed" excludes that in-progress swipe.
    """
    angles = []
    start = peak = path[0]
    direction = 0
    for p in path[1:]:
        if direction == 0:
            if abs(p[0] - start[0]) > hysteresis:
                direction = 1 if p[0] > start[0] else -1
                peak = p
            continue
        if (p[0] - peak[0]) * direction >= 0:
            peak = p  # still going the same way
        elif abs(p[0] - peak[0]) > hysteresis:
            _add_segment(angles, start, peak, min_len)
            start, peak, direction = peak, p, -direction
    completed = len(angles)
    if direction:
        _add_segment(angles, start, peak, min_len)
    return angles, completed


def _add_segment(angles, a, b, min_len):
    dx, dy = b[0] - a[0], b[1] - a[1]
    if abs(dx) >= min_len:
        if dx < 0:
            dx, dy = -dx, -dy
        angles.append(math.degrees(math.atan2(dy, dx)))


def _stdev(v):
    m = sum(v) / len(v)
    return math.sqrt(sum((a - m) ** 2 for a in v) / (len(v) - 1))


if __name__ == "__main__":
    mon = RawMouseMonitor()
    print("raw input ok:", mon.ok, mon.error or "")
    for d in mon.devices_info():
        print(f"  {d['name']:14} id={d['id']}")
