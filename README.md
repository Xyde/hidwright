# hidwright

A small, home-made tool for gaming mice, so you don't need the vendor's software.

- **No bloat.** No background service, no account, no tray icon. It changes a setting and exits.
- **No added input lag.** Settings are saved *inside the mouse* (or in the RawAccel driver), so nothing of
  ours sits between your hand and the game.
- **Each mouse gets its own settings.** Swapping buttons on one mouse never touches another.

> Status: works on **Windows** today (Python version). A faster Rust version, a desktop app and macOS
> support are being built. Not affiliated with Pulsar, Razer, Xtrfy/Cherry, BeyondQ or RawAccel.

## What it supports

| Device | What you can do |
|---|---|
| **Pulsar X2A** (with its cable) | Swap left/right, remap any button, DPI stages, onboard profiles: saved in the mouse. Polling rate, debounce, ripple control, angle snapping, motion sync: they work, but **currently reset when the mouse is unplugged** (being fixed). Lift-off: experimental. |
| **Razer Viper** (with its cable) | Read every setting; temporary DPI for the calibrator. Lighting and saved DPI are coming. |
| **Any mouse + [RawAccel](https://github.com/RawAccelOfficial/rawaccel)** | Its own sensor rotation, measured with a built-in full-screen calibrator |
| Xtrfy M42, Dumang keyboards | Planned |

## Get started (Windows)

1. **Install Python 3** from [python.org](https://www.python.org/downloads/). In the installer, tick
   **"Add python.exe to PATH"**.
2. **Download this project:** the green **Code** button above → **Download ZIP**. Unzip it anywhere, for
   example into your **Documents** folder.
3. **Open a terminal in that folder:** open the folder in File Explorer, click the address bar, type
   `powershell` and press Enter.
4. **Check your mice:**

   ```
   python set_mouse.py
   ```

   Plug the mouse in **with its cable** first. Wireless dongles aren't supported.

Want to type just `set-mouse` from anywhere? Add the project's `bin` folder to your user PATH
(Windows Settings → search "environment variables" → *Path* → *New*), then open a new terminal.

### Optional: RawAccel (sensor rotation)

Install RawAccel from its official page and unzip it into your **Downloads**, **Desktop** or **Documents**
folder. The tool finds it by itself. If you put it somewhere else, set an environment variable
`RAWACCEL_DIR` to that folder.

## Everyday commands

```
set-mouse                       show everything
set-mouse portal                open the Mouse Portal (a local web page: cards, click tester, calibrator)
set-mouse swap                  Pulsar: left <-> right  (unswap = back to normal)
set-mouse dpi 800 1600 3200 --active 3
set-mouse debounce 2            milliseconds
set-mouse bind thumb1 ctrl+c    remap a button ('default' = factory)
set-mouse rotation pulsar -1.5  give one mouse its own rotation in RawAccel
set-mouse help                  the full list
```

Without the PATH step, write `python set_mouse.py` instead of `set-mouse`.

## Is it safe?

- Pulsar settings are written the same way the official software does it. Factory values are one
  command away (`unswap`, `bind <button> default`, …).
- Razer: only reads, plus a *temporary* DPI change that the mouse forgets when unplugged.
- RawAccel: `settings.json` is backed up (timestamped) before every change, and restored if loading fails.

## For developers

- `crates/`: the Rust rewrite (`cargo build --release`, then `target\release\set-mouse.exe`).
- `HANDOFF.md` and `.tickets/` are the maintainer's working notes (decisions, open questions).

## Credits

- Pulsar protocol: [packerlschupfer/pulsar-mouse-linux](https://github.com/packerlschupfer/pulsar-mouse-linux) (MIT).
- Razer protocol: as documented by the [OpenRazer](https://github.com/openrazer/openrazer) project.
- Per-mouse rotation uses [RawAccel](https://github.com/RawAccelOfficial/rawaccel)'s driver and settings format.

## License

MIT, see [LICENSE](LICENSE).
