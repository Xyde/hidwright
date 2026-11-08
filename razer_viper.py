"""Read-only settings reader for the Razer Viper (wired, USB 1532:0078) on Windows.

Uses the Razer 90-byte feature-report protocol as documented by the open-source
OpenRazer project. Only "get" commands are sent; nothing is written to the mouse.

  python razer_viper.py
"""
import ctypes
import struct
import sys
import time

from pulsar_x2a import (hid, kernel32, hid_paths, HIDP_CAPS, GENERIC_READ, GENERIC_WRITE,
                        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, INVALID_HANDLE)

VID, PID = 0x1532, 0x0078
REPORT_LEN = 90
NOSTORE, VARSTORE = 0x00, 0x01
STATUS = {0x00: "new", 0x01: "busy", 0x02: "ok", 0x03: "failed", 0x04: "timeout", 0x05: "not supported"}
POLL_CODE_TO_HZ = {0x01: 1000, 0x02: 500, 0x04: 250, 0x08: 125}


class RazerViper:
    name = "Razer Viper"

    def __init__(self):
        tag = f"vid_{VID:04x}&pid_{PID:04x}"
        self.h = None
        self.tid = 0x1F
        for path in (p for p in hid_paths() if tag in p.lower()):
            h = None
            for access in (GENERIC_READ | GENERIC_WRITE, 0):
                h = kernel32.CreateFileW(path, access, FILE_SHARE_READ | FILE_SHARE_WRITE,
                                         None, OPEN_EXISTING, 0, None)
                if h not in (None, INVALID_HANDLE):
                    break
                h = None
            if not h:
                continue
            pp = ctypes.c_void_p()
            hid.HidD_GetPreparsedData(h, ctypes.byref(pp))
            caps = HIDP_CAPS()
            hid.HidP_GetCaps(pp, ctypes.byref(caps))
            hid.HidD_FreePreparsedData(pp)
            if caps.FeatureReportByteLength == REPORT_LEN + 1:
                self.h = h
                for tid in (0x1F, 0xFF, 0x3F):  # transaction id differs between models
                    self.tid = tid
                    try:
                        self.firmware()
                        return
                    except IOError:
                        pass
                self.h = None
            kernel32.CloseHandle(h)
        raise RuntimeError("Razer Viper not found or not answering (is Razer Synapse holding it?)")

    def close(self):
        if self.h:
            kernel32.CloseHandle(self.h)

    def _request(self, cls, cmd, size, args=()):
        r = bytearray(REPORT_LEN)
        r[1] = self.tid
        r[5], r[6], r[7] = size, cls, cmd
        r[8:8 + len(args)] = bytes(args)
        crc = 0
        for b in r[2:88]:
            crc ^= b
        r[88] = crc
        buf = (ctypes.c_ubyte * (REPORT_LEN + 1))(0, *r)
        for attempt in range(5):
            if not hid.HidD_SetFeature(self.h, buf, REPORT_LEN + 1):
                raise IOError(f"SetFeature failed (Windows error {ctypes.get_last_error()})")
            time.sleep(0.002 + attempt * 0.005)
            out = (ctypes.c_ubyte * (REPORT_LEN + 1))()
            if not hid.HidD_GetFeature(self.h, out, REPORT_LEN + 1):
                raise IOError(f"GetFeature failed (Windows error {ctypes.get_last_error()})")
            rsp = bytes(out[1:])
            if rsp[0] == 0x01:  # busy, try again
                continue
            if rsp[0] != 0x02 or rsp[6] != cls or rsp[7] != cmd:
                raise IOError(f"Razer reply {STATUS.get(rsp[0], hex(rsp[0]))} "
                              f"for {cls:02x}/{cmd:02x}")
            return rsp[8:88]
        raise IOError("Razer mouse stayed busy")

    # ---- getters (read-only)
    def firmware(self):
        a = self._request(0x00, 0x81, 0x02)
        return f"v{a[0]}.{a[1]:02d}"

    def serial(self):
        a = self._request(0x00, 0x82, 0x16)
        return bytes(a[:22]).split(b"\0")[0].decode(errors="replace")

    def polling(self):
        v = self._request(0x00, 0x85, 0x01)[0]
        return POLL_CODE_TO_HZ.get(v, f"raw 0x{v:02x}")

    def dpi(self):
        a = self._request(0x04, 0x85, 0x07, [NOSTORE])
        return struct.unpack(">HH", bytes(a[1:5]))

    def set_dpi_temporary(self, x, y=None):
        """Change the current DPI without saving it (NOSTORE): the mouse forgets it on replug,
        and its DPI stages are left alone. Used only for calibration runs."""
        y = x if y is None else y
        if not 100 <= x <= 20000 or not 100 <= y <= 20000:
            raise ValueError("DPI out of range")
        self._request(0x04, 0x05, 0x07, [NOSTORE, x >> 8, x & 0xFF, y >> 8, y & 0xFF, 0, 0])

    def dpi_stages(self):
        a = self._request(0x04, 0x86, 0x26, [VARSTORE])
        active, count = a[1], a[2]
        stages = []
        for i in range(min(count, 5)):
            idx, x, y = struct.unpack(">BHH", bytes(a[3 + 7 * i: 8 + 7 * i]))
            stages.append((x, y))
        return active, stages

    def settings(self):
        """Everything we can read, as a dict; unreadable items are reported as None."""
        out = {}
        for key, fn in (("firmware", self.firmware), ("serial", self.serial),
                        ("polling", self.polling), ("dpi", self.dpi),
                        ("dpi_stages", self.dpi_stages)):
            try:
                out[key] = fn()
            except IOError as e:
                out[key] = None
                out.setdefault("errors", {})[key] = str(e)
        return out


def main():
    try:
        m = RazerViper()
    except RuntimeError as e:
        sys.exit(str(e))
    try:
        s = m.settings()
        print(f"Razer Viper   firmware {s['firmware']}")
        print(f"  polling rate   {s['polling']} Hz")
        if s["dpi"]:
            x, y = s["dpi"]
            print(f"  current DPI    {x}" + ("" if x == y else f" x {y}"))
        if s["dpi_stages"]:
            act, st = s["dpi_stages"]
            print("  DPI stages     " + "  ".join(
                (f"[{x}]" if i == act else f"{x}") for i, (x, _) in enumerate(st, 1))
                + f"   ([] = active stage {act})")
        for k, e in s.get("errors", {}).items():
            print(f"  ({k}: {e})")
    finally:
        m.close()


if __name__ == "__main__":
    main()
