# hidwright

A small, home-made tool for gaming mice, so you don't need the vendor's software.

- **No bloat.** No background service, no account, no tray icon. It changes a setting and exits.
- **No added input lag.** Settings are saved *inside the mouse* (or in the RawAccel driver), so nothing of
  ours sits between your hand and the game.
- **Each mouse gets its own settings.** Swapping buttons on one mouse never touches another.

> Status: works on **Windows** today. The command-line tool also comes as a faster Rust program (see
> below); a desktop app and macOS support are being built. Not affiliated with Pulsar, Razer, Xtrfy/Cherry, BeyondQ or RawAccel.

## What it supports

| Device | What you can do |
|---|---|
| **Pulsar X2A** (with its cable) | Swap left/right, remap any button, DPI stages, onboard profiles, polling rate, debounce, ripple control, angle snapping, motion sync: all saved in the mouse. Lift-off: experimental. |
| **Razer Viper** (with its cable) | DPI stages, polling rate, logo lighting (static, breathing, spectrum, reactive, off), brightness and button functions: all saved in the mouse (Rust version). Temporary DPI for the calibrator. |
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

### Optional: the faster Rust version

If you have [Rust](https://rustup.rs) installed, run `cargo build --release` in the project folder once.
From then on `set-mouse` uses the Rust program (one small file, starts in milliseconds, no Python
needed for it). Without that build, `set-mouse` simply uses the Python version. The Mouse Portal is
Python either way, for now.

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
set-mouse razer light static ff8800   Razer: logo colour (also: razer dpi, poll, brightness)
set-mouse razer bind dpi default      Razer: button function ('default' = factory)
set-mouse help                  the full list
```

Without the PATH step, write `python set_mouse.py` instead of `set-mouse`.

## Is it safe?

- Pulsar settings are written the same way the official software does it. Factory values are one
  command away (`unswap`, `bind <button> default`, …).
- Razer: every command uses the byte layout of the open-source [OpenRazer](https://github.com/openrazer/openrazer)
  driver and was tried on a real Viper; values are checked before anything is sent. Buttons are not in
  OpenRazer: their layout comes from razerqdhid, and each write is read back from the mouse.
- RawAccel: `settings.json` is backed up (timestamped) before every change, and restored if loading fails.

## For developers

- `crates/`: the Rust rewrite (`cargo build --release`, then `target\release\set-mouse.exe`).
  `bin\set-mouse.cmd` runs it when it exists, and the Python otherwise (and always for `portal`).
- `HANDOFF.md` and `.tickets/` are the maintainer's working notes (decisions, open questions).

## Credits

- Pulsar protocol: [packerlschupfer/pulsar-mouse-linux](https://github.com/packerlschupfer/pulsar-mouse-linux) (MIT).
- Razer protocol: as documented by the [OpenRazer](https://github.com/openrazer/openrazer) project;
  button layout from razerqdhid (MIT).
- Per-mouse rotation uses [RawAccel](https://github.com/RawAccelOfficial/rawaccel)'s driver and settings format.

## License

MIT, see [LICENSE](LICENSE).
