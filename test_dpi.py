"""Temporary DPI for calibration runs, with a crash-safe restore.

Before changing anything, the original DPIs are written to dpi_restore.json.
restore() puts them back and deletes the file. If the portal dies mid-run, the next
portal start or any set-mouse command calls restore_if_pending() and fixes it.

  Pulsar X2A: its DPI stages on the active profile -> [test DPI], later restored exactly.
  Razer Viper: current DPI changed with NOSTORE (never saved), later set back.
"""
import json
from pathlib import Path

import pulsar_x2a as px
from razer_viper import RazerViper

RESTORE = Path(__file__).with_name("dpi_restore.json")
PULSAR_ID = "HID\\VID_3710&PID_1404&MI_00"
RAZER_ID = "HID\\VID_1532&PID_0078&MI_00"


def current_dpis():
    """{device id: current DPI} for the mice we can talk to."""
    out = {}
    try:
        m = px.X2A()
        try:
            prof = m.active_profile()
            act, stages = m.dpi_stages(prof)
            if stages:
                out[PULSAR_ID] = stages[max(0, min(act, len(stages)) - 1)][0]
        finally:
            m.close()
    except (SystemExit, OSError):
        pass
    try:
        r = RazerViper()
        try:
            out[RAZER_ID] = r.dpi()[0]
        finally:
            r.close()
    except (RuntimeError, OSError):
        pass
    return out


def apply(dpi):
    """Switch both mice to `dpi` for a calibration run. Returns {device id: dpi in use}."""
    restore_if_pending()
    saved = {}
    m = None
    try:
        m = px.X2A()
        prof = m.active_profile()
        act, stages = m.dpi_stages(prof)
        saved["pulsar"] = {"profile": prof, "active": act, "stages": [x for x, _ in stages]}
    except (SystemExit, OSError):
        m = None
    r = None
    try:
        r = RazerViper()
        saved["razer"] = {"dpi": list(r.dpi())}
    except (RuntimeError, OSError):
        r = None
    RESTORE.write_text(json.dumps(saved), encoding="utf-8")  # written BEFORE any change
    used = {}
    try:
        if m:
            m.set_dpi_stages(saved["pulsar"]["profile"], [dpi], 1)
            used[PULSAR_ID] = dpi
        if r:
            r.set_dpi_temporary(dpi)
            used[RAZER_ID] = dpi
    except Exception:
        restore_if_pending()
        raise
    finally:
        if m:
            m.close()
        if r:
            r.close()
    return used


def restore_if_pending():
    """Put back the DPIs saved by apply(). Safe to call any time; returns what it restored."""
    if not RESTORE.exists():
        return {}
    saved = json.loads(RESTORE.read_text(encoding="utf-8"))
    done = {}
    if "pulsar" in saved:
        p = saved["pulsar"]
        try:
            m = px.X2A()
        except SystemExit:
            return done  # Pulsar unplugged: keep the file and try again next time
        try:
            m.set_dpi_stages(p["profile"], p["stages"], p["active"])
            done["Pulsar X2A"] = p["stages"][p["active"] - 1]
        finally:
            m.close()
    if "razer" in saved:
        try:
            r = RazerViper()
            try:
                r.set_dpi_temporary(*saved["razer"]["dpi"])
                done["Razer Viper"] = saved["razer"]["dpi"][0]
            finally:
                r.close()
        except RuntimeError:
            pass  # unplugged: it forgets the temporary DPI on replug anyway
    RESTORE.unlink()
    return done
