"""set-mouse: one command for all of it.

  set-mouse                       status of everything (Pulsar, Razer, RawAccel rotation)
  set-mouse portal                open the Mouse Portal in your browser
  set-mouse razer                 Razer Viper settings (read-only)

  Pulsar X2A (saved in the mouse itself):
  set-mouse info                  all Pulsar settings
  set-mouse swap | unswap         left <-> right
  set-mouse dpi 400 800 1600 3200 --active 4
  set-mouse stage 2               switch DPI stage
  set-mouse poll 1000             125 250 500 1000
  set-mouse debounce 1            ms
  set-mouse ripple off            also: motion-sync, angle-snap  (on|off)
  set-mouse lod 1                 lift-off 1 or 2 mm
  set-mouse bind thumb1 ctrl+c    remap a button ('default' = factory)
  set-mouse profile 2             switch onboard profile

  RawAccel rotation (per mouse):
  set-mouse rotation              show each mouse's rotation
  set-mouse rotation pulsar 0     set one mouse's rotation (gives it its own profile)
  set-mouse reload                load settings.json into RawAccel now (runs at sign-in too)
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

PULSAR_CMDS = {"info", "swap", "unswap", "poll", "debounce", "angle-snap", "ripple", "motion-sync",
               "dpi", "stage", "lod", "bind", "profile"}
ALIASES = {"pulsar": "Pulsar X2A", "x2a": "Pulsar X2A", "razer": "Razer Viper", "viper": "Razer Viper"}


def run_pulsar(args):
    import pulsar_x2a
    sys.argv = ["set-mouse"] + args
    pulsar_x2a.main()


def known_mice():
    from rawinput_monitor import RawMouseMonitor
    return [d for d in RawMouseMonitor().devices_info() if not d["name"].startswith("Mouse ")]


def show_rotation():
    import rawaccel_cfg
    s = rawaccel_cfg.summary(known_mice())
    if not s["found"]:
        print("RawAccel settings.json not found")
        return
    print("RawAccel rotation")
    for m in s["mice"]:
        shared = f"   (shared with {', '.join(m['shared_with'])})" if m["shared_with"] else ""
        print(f"  {m['name']:12} {m['rotation']:+.1f} deg   profile '{m['profile']}'{shared}")


def set_rotation(who, deg):
    import rawaccel_cfg
    name = ALIASES.get(who.lower())
    if not name:
        sys.exit(f"which mouse? use one of: {', '.join(ALIASES)}")
    try:
        deg = float(deg)
    except ValueError:
        sys.exit("rotation must be a number of degrees, e.g. -2.6")
    mouse = next((m for m in known_mice() if m["name"] == name), None)
    if not mouse:
        sys.exit(f"{name} isn't plugged in")
    res = rawaccel_cfg.apply([{"id": mouse["id"], "name": name, "rotation": deg}])
    print(f"{name}: rotation {deg:+.1f} deg, loaded into RawAccel (backup: {Path(res['backup']).name})")
    show_rotation()


def status():
    import razer_viper
    for fn in (lambda: run_pulsar(["info"]), razer_viper.main, show_rotation):
        try:
            fn()
        except SystemExit as e:
            if e.code not in (None, 0):
                print(e.code)
        print()


def _restore_interrupted_test_dpi():
    import test_dpi
    if test_dpi.RESTORE.exists():
        try:
            fixed = test_dpi.restore_if_pending()
            if fixed:
                print("Restored DPI after an interrupted calibration:", fixed)
        except Exception as e:
            print("Couldn't restore DPI from an interrupted calibration yet:", e)


def main():
    args = sys.argv[1:]
    if not args:
        _restore_interrupted_test_dpi()
        return status()
    cmd = args[0].lower()
    if cmd not in ("-h", "--help", "help", "portal"):  # the portal does this itself
        _restore_interrupted_test_dpi()
    if cmd in ("-h", "--help", "help"):
        print(__doc__.strip())
    elif cmd in PULSAR_CMDS:
        run_pulsar(args)
    elif cmd == "portal":
        import mouse_portal
        sys.argv = ["set-mouse portal"] + args[1:]
        mouse_portal.main()
    elif cmd in ("razer", "viper"):
        import razer_viper
        razer_viper.main()
    elif cmd == "rotation":
        if len(args) == 1:
            show_rotation()
        elif len(args) == 3:
            set_rotation(args[1], args[2])
        else:
            sys.exit("usage: set-mouse rotation [pulsar|razer DEGREES]")
    elif cmd == "reload":
        import mouse_startup
        sys.exit(mouse_startup.main())
    else:
        sys.exit(f"unknown command '{args[0]}'. Try: set-mouse help")


if __name__ == "__main__":
    main()
