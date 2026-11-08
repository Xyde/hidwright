"""Read RawAccel's settings.json and give individual mice their own rotation.

How RawAccel picks settings for a mouse (from its driver source):
  * a mouse with an entry in "devices" whose "id" matches uses that entry's "profile"
  * every other mouse uses the FIRST profile in "profiles"
So one shared profile means one shared rotation for every mouse.

apply() makes a timestamped backup of settings.json, writes the new file, and
runs RawAccel's own writer.exe to load it into the driver.
"""
import copy
import json
import os
import subprocess
import time
from pathlib import Path

ROT = "Degrees of rotation"


def candidates():
    """Where RawAccel might be, most specific first.

    RAWACCEL_DIR (if set) wins. Otherwise look where people usually unzip it: the release zip
    unpacks to RawAccel_vX.Y.Z\\RawAccel, sometimes flattened to RawAccel_vX.Y.Z or RawAccel.
    Newest version first.
    """
    env = os.environ.get("RAWACCEL_DIR")
    if env:
        yield Path(env)
    home = Path.home()
    bases = [home / "Downloads", home / "Desktop", home / "Documents",
             Path(os.environ.get("ProgramFiles", r"C:\Program Files"))]
    for base in bases:
        if base.is_dir():
            for d in sorted(base.glob("RawAccel*"), reverse=True):
                yield d / "RawAccel"
                yield d


def find_dir():
    """RawAccel's folder (the one with writer.exe), or None."""
    return next((d for d in candidates() if (d / "writer.exe").is_file()), None)


def load():
    d = find_dir()
    if not d or not (d / "settings.json").exists():
        return None, None
    return d, json.loads((d / "settings.json").read_text(encoding="utf-8"))


def resolve(cfg, dev_id):
    """(profile name, rotation, has own entry) the driver would use for this device id."""
    profiles = cfg.get("profiles") or [{"name": "default"}]
    prof = profiles[0]
    own = False
    for dev in cfg.get("devices", []):
        if dev.get("id") == dev_id:  # driver compares case-sensitively
            own = True
            match = next((p for p in profiles if p.get("name") == dev.get("profile")), None)
            if match:
                prof = match
            break
    return prof.get("name", "?"), float(prof.get(ROT, 0.0)), own


def summary(devices):
    """RawAccel status for the given [{id, name}] mice."""
    d, cfg = load()
    if cfg is None:
        return {"found": False}
    out = []
    for dev in devices:
        name, rot, own = resolve(cfg, dev["id"])
        out.append({"id": dev["id"], "name": dev["name"], "profile": name, "rotation": rot,
                    "own_profile": own})
    shared = {}
    for o in out:
        shared.setdefault(o["profile"], []).append(o["name"])
    for o in out:
        o["shared_with"] = [n for n in shared[o["profile"]] if n != o["name"]]
    return {"found": True, "dir": str(d), "version": cfg.get("version"), "mice": out,
            "default_profile": (cfg.get("profiles") or [{}])[0].get("name")}


def plan(cfg, changes):
    """New config where each changed mouse has its own profile with the given rotation.

    changes: [{"id": ..., "name": ..., "rotation": float}]
    A new profile is a copy of the profile the mouse uses today (same curve and
    sensitivity), so the only thing that changes is the rotation. The first
    profile, which mice without their own entry use, is left untouched.
    """
    new = copy.deepcopy(cfg)
    profiles = new.setdefault("profiles", [])
    devices = new.setdefault("devices", [])
    for ch in changes:
        rot = round(float(ch["rotation"]), 2)
        if not -45 <= rot <= 45:
            raise ValueError("rotation must be between -45 and 45 degrees")
        pname = ch["name"]
        cur_name, _, _ = resolve(new, ch["id"])
        prof = next((p for p in profiles if p.get("name") == pname), None)
        if prof is None:
            base = next((p for p in profiles if p.get("name") == cur_name), profiles[0])
            prof = copy.deepcopy(base)
            prof["name"] = pname
            profiles.append(prof)
        if prof is profiles[0]:
            raise ValueError(f"'{pname}' is the default profile; rename it in RawAccel first")
        prof[ROT] = rot
        entry = next((e for e in devices if e.get("id") == ch["id"]), None)
        if entry is None:
            entry = {"name": pname, "profile": pname, "id": ch["id"],
                     "config": copy.deepcopy(new.get("defaultDeviceConfig", {}))}
            devices.append(entry)
        entry["profile"] = pname
    return new


def apply(changes):
    d, cfg = load()
    if cfg is None:
        raise FileNotFoundError("RawAccel settings.json not found")
    new = plan(cfg, changes)
    path = d / "settings.json"
    backup = d / f"settings.backup-{time.strftime('%Y%m%d-%H%M%S')}.json"
    backup.write_text(path.read_text(encoding="utf-8"), encoding="utf-8")
    path.write_text(json.dumps(new, indent=2), encoding="utf-8")
    try:
        r = subprocess.run([str(d / "writer.exe"), str(path)], cwd=str(d), timeout=20,
                           capture_output=True, text=True)
    except subprocess.TimeoutExpired:
        raise IOError("writer.exe didn't finish (is it showing an error window?)")
    if r.returncode != 0:
        path.write_text(backup.read_text(encoding="utf-8"), encoding="utf-8")
        raise IOError(f"writer.exe failed (exit {r.returncode}) {r.stdout}{r.stderr}; "
                      "settings.json restored from backup")
    return {"backup": str(backup)}
