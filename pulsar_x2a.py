"""Tiny, no-install config tool for the Pulsar X2A (wired, USB 3710:1404) on Windows.

No bloat: pure Python + ctypes talking to Windows' built-in hid.dll.
Settings are written to the mouse itself, so they work everywhere
(every app, every game, every PC), add zero latency, and only affect THIS mouse.
The script runs for a second and exits; nothing stays in the background.

Protocol notes come from the MIT-licensed community project
packerlschupfer/pulsar-mouse-linux (reverse-engineered from USB captures).

Usage (plug the mouse in by cable):
  python pulsar_x2a.py info                       read-only: show everything
  python pulsar_x2a.py swap | unswap              left <-> right (profile = active one)
  python pulsar_x2a.py poll 1000                  polling rate: 125 250 500 1000   (global)
  python pulsar_x2a.py debounce 3                 debounce in ms                   (global)
  python pulsar_x2a.py angle-snap on|off          (global)
  python pulsar_x2a.py ripple on|off              (global)
  python pulsar_x2a.py motion-sync on|off         (global)
  python pulsar_x2a.py dpi 400 800 1600 --active 2   DPI stages (1-6 stages, 100-26000, step 100)
  python pulsar_x2a.py stage 2                    switch to DPI stage 2
  python pulsar_x2a.py lod 1|2                    lift-off distance in mm
  python pulsar_x2a.py bind thumb1 ctrl+c         remap a button (see `bind --help`)
  python pulsar_x2a.py profile 2                  switch the mouse to profile 2

Per-profile commands (swap, dpi, stage, lod, bind) take -p N to target profile N
instead of the active one.
"""
import argparse
import ctypes
import struct
import sys
import time
from ctypes import wintypes

VID, PID, IFACE = 0x3710, 0x1404, 3
REPORT_SIZE = 64

# ---------------------------------------------------------------- protocol tables
BUTTONS = {"left": 0x01, "right": 0x02, "wheel": 0x03,
           "thumb1": 0x04, "thumb2": 0x05, "thumb3": 0x06, "thumb4": 0x07, "dpi": 0x0B}
BUTTON_LABELS = {0x01: "left", 0x02: "right", 0x03: "wheel click",
                 0x04: "thumb1 (left side, back)", 0x05: "thumb2 (left side, front)",
                 0x06: "thumb3 (right side, back)", 0x07: "thumb4 (right side, front)",
                 0x0B: "dpi button"}
BUTTON_DEFAULTS = {0x01: (0x01, 0x01, 0x00), 0x02: (0x01, 0x02, 0x00), 0x03: (0x01, 0x03, 0x00),
                   0x04: (0x01, 0x04, 0x00), 0x05: (0x01, 0x05, 0x00), 0x06: (0x01, 0x04, 0x00),
                   0x07: (0x01, 0x05, 0x00), 0x0B: (0x09, 0x03, 0x00)}

T_DISABLED, T_MOUSE, T_KEY, T_SCROLL, T_PROFILE, T_DPI, T_MEDIA, T_XCLICK = \
    0x00, 0x01, 0x02, 0x03, 0x08, 0x09, 0x0D, 0x0E

SIMPLE_FUNCS = {  # name -> (type, a1, a2)
    "left": (T_MOUSE, 0x01, 0), "right": (T_MOUSE, 0x02, 0), "middle": (T_MOUSE, 0x03, 0),
    "back": (T_MOUSE, 0x04, 0), "forward": (T_MOUSE, 0x05, 0),
    "scrollup": (T_SCROLL, 0x01, 0), "scrolldown": (T_SCROLL, 0xFF, 0),
    "dpi+": (T_DPI, 0x01, 0), "dpi-": (T_DPI, 0x02, 0), "dpiloop": (T_DPI, 0x03, 0),
    "profile+": (T_PROFILE, 0x03, 0), "profile-": (T_PROFILE, 0x04, 0),
    "doubleclick": (T_XCLICK, 0x01, 0), "disabled": (T_DISABLED, 0, 0),
}
MEDIA = {"play": 0x00CD, "next": 0x00B5, "prev": 0x00B6, "stop": 0x00B7, "mute": 0x00E2,
         "vol+": 0x00E9, "vol-": 0x00EA, "eject": 0x00B8, "calc": 0x0192,
         "homepage": 0x0223, "mediaplayer": 0x0183}
MODS = {"ctrl": 0x01, "shift": 0x02, "alt": 0x04, "win": 0x08,
        "rctrl": 0x10, "rshift": 0x20, "ralt": 0x40, "rwin": 0x80}
KEYS = {**{chr(ord("a") + i): 0x04 + i for i in range(26)},
        **{str((i + 1) % 10): 0x1E + i for i in range(10)},
        **{f"f{i}": 0x39 + i for i in range(1, 13)},
        "enter": 0x28, "esc": 0x29, "backspace": 0x2A, "tab": 0x2B, "space": 0x2C,
        "-": 0x2D, "=": 0x2E, "[": 0x2F, "]": 0x30, "\\": 0x31, ";": 0x33, "'": 0x34,
        "`": 0x35, ",": 0x36, ".": 0x37, "/": 0x38, "capslock": 0x39,
        "printscreen": 0x46, "scrolllock": 0x47, "pause": 0x48, "insert": 0x49,
        "home": 0x4A, "pageup": 0x4B, "delete": 0x4C, "end": 0x4D, "pagedown": 0x4E,
        "arrowright": 0x4F, "arrowleft": 0x50, "arrowdown": 0x51, "arrowup": 0x52}

POLL_HZ_TO_CODE = {125: 0x01, 250: 0x02, 500: 0x04, 1000: 0x08}
POLL_CODE_TO_HZ = {v: k for k, v in POLL_HZ_TO_CODE.items()}
DPI_MIN, DPI_MAX, DPI_STEP, MAX_STAGES = 100, 26000, 100, 6

# ---------------------------------------------------------------- Windows HID plumbing
hid = ctypes.WinDLL("hid")
setupapi = ctypes.WinDLL("setupapi")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)


class GUID(ctypes.Structure):
    _fields_ = [("Data1", wintypes.DWORD), ("Data2", wintypes.WORD),
                ("Data3", wintypes.WORD), ("Data4", ctypes.c_ubyte * 8)]


class SP_DEVICE_INTERFACE_DATA(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.DWORD), ("InterfaceClassGuid", GUID),
                ("Flags", wintypes.DWORD), ("Reserved", ctypes.c_void_p)]


class HIDP_CAPS(ctypes.Structure):
    _fields_ = [("Usage", wintypes.USHORT), ("UsagePage", wintypes.USHORT),
                ("InputReportByteLength", wintypes.USHORT),
                ("OutputReportByteLength", wintypes.USHORT),
                ("FeatureReportByteLength", wintypes.USHORT),
                ("Reserved", wintypes.USHORT * 17),
                ("rest", wintypes.USHORT * 10)]


DIGCF_PRESENT, DIGCF_DEVICEINTERFACE = 0x2, 0x10
GENERIC_READ, GENERIC_WRITE = 0x80000000, 0x40000000
FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING = 1, 2, 3
INVALID_HANDLE = ctypes.c_void_p(-1).value

setupapi.SetupDiGetClassDevsW.restype = ctypes.c_void_p
setupapi.SetupDiGetClassDevsW.argtypes = [ctypes.POINTER(GUID), wintypes.LPCWSTR,
                                          wintypes.HWND, wintypes.DWORD]
setupapi.SetupDiEnumDeviceInterfaces.argtypes = [ctypes.c_void_p, ctypes.c_void_p,
                                                 ctypes.POINTER(GUID), wintypes.DWORD,
                                                 ctypes.POINTER(SP_DEVICE_INTERFACE_DATA)]
setupapi.SetupDiGetDeviceInterfaceDetailW.argtypes = [
    ctypes.c_void_p, ctypes.POINTER(SP_DEVICE_INTERFACE_DATA), ctypes.c_void_p,
    wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
setupapi.SetupDiDestroyDeviceInfoList.argtypes = [ctypes.c_void_p]
kernel32.CreateFileW.restype = ctypes.c_void_p
kernel32.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                 ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
                                 ctypes.c_void_p]
kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
for fn in ("HidD_SetFeature", "HidD_GetFeature"):
    getattr(hid, fn).argtypes = [ctypes.c_void_p, ctypes.c_void_p, wintypes.ULONG]
hid.HidD_GetPreparsedData.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
hid.HidD_FreePreparsedData.argtypes = [ctypes.c_void_p]
hid.HidP_GetCaps.argtypes = [ctypes.c_void_p, ctypes.POINTER(HIDP_CAPS)]


def hid_paths():
    guid = GUID()
    hid.HidD_GetHidGuid(ctypes.byref(guid))
    devs = setupapi.SetupDiGetClassDevsW(ctypes.byref(guid), None, None,
                                         DIGCF_PRESENT | DIGCF_DEVICEINTERFACE)
    try:
        i = 0
        while True:
            ifd = SP_DEVICE_INTERFACE_DATA(cbSize=ctypes.sizeof(SP_DEVICE_INTERFACE_DATA))
            if not setupapi.SetupDiEnumDeviceInterfaces(devs, None, ctypes.byref(guid), i,
                                                        ctypes.byref(ifd)):
                return
            i += 1
            need = wintypes.DWORD()
            setupapi.SetupDiGetDeviceInterfaceDetailW(devs, ctypes.byref(ifd), None, 0,
                                                      ctypes.byref(need), None)
            buf = ctypes.create_string_buffer(need.value)
            # cbSize of SP_DEVICE_INTERFACE_DETAIL_DATA_W: 8 on 64-bit, 6 on 32-bit
            struct.pack_into("I", buf, 0, 8 if ctypes.sizeof(ctypes.c_void_p) == 8 else 6)
            if setupapi.SetupDiGetDeviceInterfaceDetailW(devs, ctypes.byref(ifd), buf,
                                                         need, None, None):
                yield ctypes.wstring_at(ctypes.addressof(buf) + 4)
    finally:
        setupapi.SetupDiDestroyDeviceInfoList(devs)


class X2A:
    def __init__(self):
        tag = f"vid_{VID:04x}&pid_{PID:04x}&mi_{IFACE:02x}"
        paths = [p for p in hid_paths() if tag in p.lower()]
        if not paths:
            sys.exit(f"Pulsar X2A config interface ({tag}) not found. Is the mouse plugged in by cable?")
        self.h = None
        for path in paths:
            for access in (GENERIC_READ | GENERIC_WRITE, 0):
                h = kernel32.CreateFileW(path, access, FILE_SHARE_READ | FILE_SHARE_WRITE,
                                         None, OPEN_EXISTING, 0, None)
                if h not in (None, INVALID_HANDLE):
                    self.h = h
                    break
            if self.h:
                break
        if not self.h:
            sys.exit(f"Could not open the mouse (Windows error {ctypes.get_last_error()}). "
                     "Close Pulsar Fusion if it's running.")
        pp = ctypes.c_void_p()
        hid.HidD_GetPreparsedData(self.h, ctypes.byref(pp))
        caps = HIDP_CAPS()
        hid.HidP_GetCaps(pp, ctypes.byref(caps))
        hid.HidD_FreePreparsedData(pp)
        # Windows wants report-ID byte + payload; report ID is 0 on this device.
        self.flen = caps.FeatureReportByteLength or REPORT_SIZE + 1
        self._resends_left = 6

    def close(self):
        kernel32.CloseHandle(self.h)

    # ---- raw packets
    @staticmethod
    def build(cat, reg, sub, profile, payload=()):
        b = bytearray(REPORT_SIZE)
        b[1], b[2], b[3], b[6] = cat, reg, sub, profile
        b[7:7 + len(payload)] = bytes(payload)
        b[62:64] = struct.pack("<H", sum(b[:62]) & 0xFFFF)
        return b

    def set_report(self, data):
        buf = (ctypes.c_ubyte * self.flen)(0, *data[: self.flen - 1])
        if not hid.HidD_SetFeature(self.h, buf, self.flen):
            raise IOError(f"SetFeature failed (Windows error {ctypes.get_last_error()})")

    def get_report(self):
        buf = (ctypes.c_ubyte * self.flen)()
        if not hid.HidD_GetFeature(self.h, buf, self.flen):
            raise IOError(f"GetFeature failed (Windows error {ctypes.get_last_error()})")
        return bytes(buf[1:])

    def poll(self, accept):
        rsp = self.get_report()
        for _ in range(7):
            if rsp[0] in accept:
                break
            time.sleep(0.02)
            rsp = self.get_report()
        return rsp

    def cmd(self, cat, reg, sub, profile, payload=()):
        self.set_report(self.build(cat, reg, sub, profile, payload))
        rsp = self.poll((0x01, 0x02))
        if rsp[0] not in (0x01, 0x02):
            raise IOError(f"Mouse did not acknowledge write (reply byte 0x{rsp[0]:02x})")
        time.sleep(0.03)  # a write can be acked (0x02) slightly before it's applied
        return rsp

    def read(self, cat, reg, sub, profile, payload=()):
        pkt = self.build(cat, reg | 0x80, sub, profile, payload)
        self.set_report(pkt)
        rsp = self.poll((0x01,))
        while rsp[0] == 0x00 and self._resends_left > 0:
            self._resends_left -= 1
            time.sleep(0.1)
            self.set_report(pkt)
            rsp = self.poll((0x01,))
        if rsp[0] != 0x01:
            raise IOError(f"Bad reply from mouse (byte 0 = 0x{rsp[0]:02x})")
        self._resends_left = 0
        return rsp

    # ---- device info / profiles
    def firmware(self):
        rsp = self.read(0x01, 0x07, 0x04, 0x00)
        return ".".join(f"{b:02X}" for b in reversed(rsp[6:10]))

    def active_profile(self):
        return self.read(0x02, 0x06, 0x01, 0x00)[6]

    def set_active_profile(self, p):
        self.cmd(0x02, 0x06, 0x01, p)

    # ---- global settings
    # The mouse keeps one copy of each global, but the profile byte of a WRITE
    # decides whether it is saved: 0 = RAM only (lost on unplug), 1-5 = saved
    # to flash. So writes carry the active profile; reads may use 0.
    # (px-337u experiment 1, 2026-11-09: 1 ms / ripple off survived a replug.)
    def cmd_global(self, cat, reg, sub, payload):
        return self.cmd(cat, reg, sub, self.active_profile(), payload)

    def polling(self):
        v = self.read(0x01, 0x09, 0x02, 0)[7]
        return POLL_CODE_TO_HZ.get(v, v * 125)

    def set_polling(self, hz):
        self.cmd_global(0x01, 0x09, 0x02, [POLL_HZ_TO_CODE[hz]])

    def debounce(self):
        return self.read(0x04, 0x03, 0x03, 0)[7]

    def set_debounce(self, ms):
        self.cmd_global(0x04, 0x03, 0x03, [ms])

    TOGGLES = {"angle-snap": 0x04, "ripple": 0x03, "motion-sync": 0x05}

    def toggle(self, name):
        return bool(self.read(0x07, self.TOGGLES[name], 0x02, 0)[7])

    def set_toggle(self, name, on):
        self.cmd_global(0x07, self.TOGGLES[name], 0x02, [1 if on else 0])

    # ---- per-profile settings
    def dpi_stages(self, profile):
        rsp = self.read(0x05, 0x04, 0x15, profile)
        active, count = rsp[7], rsp[8]
        stages = []
        for i in range(min(count, MAX_STAGES)):
            base = 9 + 5 * i
            x, y = struct.unpack_from("<HH", rsp, base + 1)
            stages.append((x, y))
        return active, stages

    def set_dpi_stages(self, profile, dpis, active):
        payload = [active, len(dpis)]
        for i, d in enumerate(dpis):
            payload += [i + 1, d & 0xFF, d >> 8, d & 0xFF, d >> 8]
        self.cmd(0x05, 0x04, 0x21, profile, payload)

    def dpi_stage(self, profile):
        return self.read(0x05, 0x01, 0x02, profile)[7]

    def set_dpi_stage(self, profile, stage):
        self.cmd(0x05, 0x01, 0x02, profile, [stage])

    def lod(self, profile):
        v = self.read(0x07, 0x02, 0x03, profile)[7]
        return {0: "1 mm", 1: "2 mm"}.get(v, f"unknown (raw 0x{v:02x})")

    def set_lod(self, profile, mm):
        v = 0 if mm == 1 else 1
        self.cmd(0x07, 0x02, 0x03, profile, [v, v])

    def get_button(self, btn, profile):
        rsp = self.read(0x04, 0x01, 0x06, profile, [btn, 0xFF])
        return rsp[8], rsp[9], rsp[10]

    def set_button(self, btn, binding, profile):
        self.cmd(0x04, 0x01, 0x06, profile, [btn, *binding])


# ---------------------------------------------------------------- bindings <-> text
def parse_function(text):
    t = text.lower().strip()
    if t in SIMPLE_FUNCS:
        return SIMPLE_FUNCS[t]
    if t in MEDIA:
        u = MEDIA[t]
        return (T_MEDIA, u & 0xFF, u >> 8)
    if t.startswith("media(") and t.endswith(")"):
        u = int(t[6:-1], 0)
        return (T_MEDIA, u & 0xFF, u >> 8)
    # keyboard: [mod+[mod+]]key, or modifiers alone (e.g. "shift")
    parts = t.split("+") if t != "+" else ["+"]
    mods, key = 0, 0
    for p in parts:
        if p in MODS:
            mods |= MODS[p]
        elif p in KEYS and key == 0:
            key = KEYS[p]
        else:
            raise ValueError(f"don't know '{p}' in '{text}'")
    return (T_KEY, mods, key)


def describe(binding):
    t, a1, a2 = binding
    for name, b in SIMPLE_FUNCS.items():
        if b == binding:
            return name + " click" if name in ("left", "right", "middle") else name
    if t == T_MEDIA:
        u = a1 | (a2 << 8)
        return next((n for n, v in MEDIA.items() if v == u), f"media(0x{u:04x})")
    if t == T_KEY:
        names = [n for n, bit in MODS.items() if a1 & bit]
        if a2:
            names.append(next((n for n, v in KEYS.items() if v == a2), f"key 0x{a2:02x}"))
        return "+".join(names) or "nothing"
    return f"type 0x{t:02x} (0x{a1:02x}, 0x{a2:02x})"


# ---------------------------------------------------------------- commands
def show_info(m, prof):
    active = m.active_profile()
    print(f"Pulsar X2A   firmware {m.firmware()}   active profile {active}")
    print()
    print("Global")
    print(f"  polling rate   {m.polling()} Hz")
    print(f"  debounce       {m.debounce()} ms")
    for name in X2A.TOGGLES:
        print(f"  {name:14} {'on' if m.toggle(name) else 'off'}")
    print()
    print(f"Profile {prof}" + ("  (active)" if prof == active else ""))
    act_stage, stages = m.dpi_stages(prof)
    dpis = []
    for i, (x, y) in enumerate(stages, 1):
        s = f"{x}" if x == y else f"{x}x{y}"
        dpis.append(f"[{s}]" if i == act_stage else s)
    print(f"  DPI stages     {'  '.join(dpis)}   ([] = active stage {act_stage})")
    print(f"  lift-off       {m.lod(prof)}")
    print("  buttons")
    for btn, label in BUTTON_LABELS.items():
        b = m.get_button(btn, prof)
        mark = "" if b == BUTTON_DEFAULTS[btn] else "   <- changed"
        print(f"    {label:28} -> {describe(b)}{mark}")


def on_off(s):
    s = s.lower()
    if s in ("on", "1", "true", "yes"):
        return True
    if s in ("off", "0", "false", "no"):
        return False
    raise argparse.ArgumentTypeError("use on or off")


def dpi_value(s):
    d = int(s)
    if not DPI_MIN <= d <= DPI_MAX or d % DPI_STEP:
        raise argparse.ArgumentTypeError(f"DPI must be {DPI_MIN}-{DPI_MAX} in steps of {DPI_STEP}")
    return d


def main():
    ap = argparse.ArgumentParser(description="Pulsar X2A mini config tool (writes to the mouse's onboard memory)")
    sub = ap.add_subparsers(dest="action", required=True)

    def per_profile(p):
        p.add_argument("-p", "--profile", type=int, choices=range(1, 6),
                       help="profile 1-5 (default: the active one)")
        return p

    per_profile(sub.add_parser("info", help="show all settings (read-only)"))
    per_profile(sub.add_parser("swap", help="left button does right-click and vice versa"))
    per_profile(sub.add_parser("unswap", help="factory left/right"))
    sub.add_parser("poll", help="polling rate (global)").add_argument(
        "hz", type=int, choices=sorted(POLL_HZ_TO_CODE))
    sub.add_parser("debounce", help="debounce in ms (global; factory default 3)").add_argument(
        "ms", type=int, choices=range(0, 21), metavar="MS")
    for name in X2A.TOGGLES:
        sub.add_parser(name, help=f"{name} on/off (global)").add_argument("state", type=on_off)
    p = per_profile(sub.add_parser("dpi", help="set DPI stages, e.g. dpi 400 800 1600 --active 2"))
    p.add_argument("dpis", type=dpi_value, nargs="+")
    p.add_argument("--active", type=int, default=1, help="which stage is active (1-based)")
    per_profile(sub.add_parser("stage", help="switch active DPI stage")).add_argument(
        "n", type=int, choices=range(1, MAX_STAGES + 1))
    per_profile(sub.add_parser("lod", help="lift-off distance (mm)")).add_argument(
        "mm", type=int, choices=[1, 2])
    p = per_profile(sub.add_parser("bind", help="remap a button", description=(
        "Buttons: " + ", ".join(BUTTONS) + ".  Functions: " + ", ".join(SIMPLE_FUNCS) +
        ", media keys (" + ", ".join(MEDIA) + ", media(0xNNNN)), keyboard keys/combos like "
        "ctrl+c, shift, f5, alt+tab, or 'default' for factory.")))
    p.add_argument("button", choices=list(BUTTONS))
    p.add_argument("function")
    sub.add_parser("profile", help="switch the mouse's active profile").add_argument(
        "n", type=int, choices=range(1, 6))
    args = ap.parse_args()

    # validate pure-text inputs before touching the mouse
    binding = None
    if args.action == "bind":
        btn = BUTTONS[args.button]
        try:
            binding = BUTTON_DEFAULTS[btn] if args.function.lower() == "default" \
                else parse_function(args.function)
        except ValueError as e:
            ap.error(str(e))
    if args.action == "dpi":
        if len(args.dpis) > MAX_STAGES:
            ap.error(f"at most {MAX_STAGES} DPI stages")
        if not 1 <= args.active <= len(args.dpis):
            ap.error(f"--active must be between 1 and {len(args.dpis)}")

    m = X2A()
    try:
        prof = getattr(args, "profile", None) or m.active_profile()
        a = args.action
        if a == "info":
            show_info(m, prof)
            return
        if a in ("swap", "unswap"):
            l, r = ((T_MOUSE, 0x02, 0), (T_MOUSE, 0x01, 0)) if a == "swap" \
                else (BUTTON_DEFAULTS[0x01], BUTTON_DEFAULTS[0x02])
            m.set_button(0x01, l, prof)
            m.set_button(0x02, r, prof)
            print(f"profile {prof}: left button -> {describe(m.get_button(0x01, prof))}, "
                  f"right button -> {describe(m.get_button(0x02, prof))}")
        elif a == "poll":
            m.set_polling(args.hz)
            print(f"polling rate now {m.polling()} Hz")
        elif a == "debounce":
            m.set_debounce(args.ms)
            print(f"debounce now {m.debounce()} ms")
        elif a in X2A.TOGGLES:
            m.set_toggle(a, args.state)
            print(f"{a} now {'on' if m.toggle(a) else 'off'}")
        elif a == "dpi":
            m.set_dpi_stages(prof, args.dpis, args.active)
            act, stages = m.dpi_stages(prof)
            print(f"profile {prof}: DPI stages {[x for x, _ in stages]}, active stage {act}")
        elif a == "stage":
            m.set_dpi_stage(prof, args.n)
            print(f"profile {prof}: active DPI stage now {m.dpi_stage(prof)}")
        elif a == "lod":
            m.set_lod(prof, args.mm)
            print(f"profile {prof}: lift-off now {m.lod(prof)}")
        elif a == "bind":
            m.set_button(btn, binding, prof)
            print(f"profile {prof}: {BUTTON_LABELS[btn]} -> {describe(m.get_button(btn, prof))}")
        elif a == "profile":
            m.set_active_profile(args.n)
            print(f"active profile now {m.active_profile()}")
    finally:
        m.close()


if __name__ == "__main__":
    main()
