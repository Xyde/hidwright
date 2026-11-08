"""Login-time loader: puts your RawAccel settings back into the driver after a reboot.

RawAccel's driver forgets its settings at every restart. This runs once at sign-in
(via the "Mouse Portal - load RawAccel" scheduled task), loads settings.json with
RawAccel's own writer.exe, writes one line to startup.log, and exits. Nothing stays
running. Pulsar settings don't need this: they live in the mouse itself.

  pythonw mouse_startup.py        (what the task runs: no window)
  python  mouse_startup.py        (same, by hand)
"""
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rawaccel_cfg  # noqa: E402

LOG = Path(__file__).with_name("startup.log")


def log(msg):
    line = f"{time.strftime('%Y-%m-%d %H:%M:%S')}  {msg}"
    try:
        lines = LOG.read_text(encoding="utf-8").splitlines()[-199:] if LOG.exists() else []
        LOG.write_text("\n".join(lines + [line]) + "\n", encoding="utf-8")
    except OSError:
        pass
    print(line)


def main():
    d = rawaccel_cfg.find_dir()
    if not d or not (d / "settings.json").exists():
        log("RawAccel folder or settings.json not found; nothing loaded")
        return 1
    settings = d / "settings.json"
    last = ""
    for attempt in range(6):  # the driver can be slow to come up right after boot
        try:
            r = subprocess.run([str(d / "writer.exe"), str(settings)], cwd=str(d), timeout=30,
                               capture_output=True, text=True,
                               creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
            if r.returncode == 0:
                d_, cfg = rawaccel_cfg.load()
                rots = ", ".join(f"{p.get('name')}: {p.get(rawaccel_cfg.ROT, 0)} deg"
                                 for p in cfg.get("profiles", []))
                log(f"RawAccel settings loaded ({rots})")
                return 0
            last = f"writer.exe exit {r.returncode} {r.stdout.strip()} {r.stderr.strip()}".strip()
        except (OSError, subprocess.TimeoutExpired) as e:
            last = str(e)
        time.sleep(5)
    log(f"FAILED to load RawAccel settings: {last}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
