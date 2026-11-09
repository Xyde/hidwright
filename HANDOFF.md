# Handoff: hidwright (set-mouse, Mouse Portal, Rust port)

_Written 2026-11-09 by Claude (Claude Code desktop session). Replaces the previous HANDOFF.md (history is in jj). This file is **public** (github.com/Xyde/hidwright): paths use `~` for the user's home folder, and it contains no names, serials or PC names. Keep it that way._

## Resume here

- **What this is:** `hidwright`, a self-built, bloat-free replacement for vendor peripheral software. The local folder is still named `pulsar-x2a`. The **Python version is what's in daily use**: `set-mouse` on PATH, the Mouse Portal, the rotation calibrator and the sign-in loader. It configures a Pulsar X2A over HID, reads a Razer Viper, and gives each mouse its own RawAccel rotation. A **Rust + egui rewrite** is in progress in `crates/` (epic `px-0oek`). Goals: replace Razer Synapse completely, cover more peripherals (Xtrfy M42, Dumang DK6 keyboard), and run on the user's Mac Studio too.
- **Where it stands:** Rust reads both mice with output identical to the Python, and fixes the Python's Synapse read collisions. Rust also writes every Pulsar setting (`px-5xrv` part a, 2026-11-09): same packets and same messages as the Python, checked on the mouse. RawAccel, the portal and Viper writes are still Python-only. Public repo with CI green on Windows and macOS. The Pulsar's global settings now survive an unplug (`px-337u`, solved 2026-11-09: writes must carry the active profile byte, not 0). The mouse is at the user's chosen debounce 1 ms / ripple off.
- **Next action (agent):** Rust phase 2 (`px-5xrv`) part b: RawAccel config (`rawaccel_cfg.py`: find the folder, back up, per-mouse profiles in `devices[]`, run `writer.exe`), the `rotation` and `reload` commands, and the interrupted-test-DPI restore. Then part c: switch `bin/set-mouse.cmd` to the Rust exe once `set-mouse` with no arguments matches too.
- **Paste into a fresh session:**
  > Read HANDOFF.md in ~\Documents\pulsar-x2a first. Run `jj st`, `tk ready` and `set-mouse` to confirm the repo, the tickets, the mice and RawAccel still match "Current state", then continue with the next action (Rust phase 2, px-5xrv). Explain what you're doing as you go.

## Goal and constraints (user's words)

Earlier sessions (still in force):
- "i want it that way for everything" / "I haev a razer who I do not want to swap that setting out of": left/right swapped everywhere, but **only on the Pulsar**.
- "i hate the idea of pulsar fusion -- maybe it has BLOAT … let's see if we can make a BETTER DRIVER or SOFTWARE": our own minimal tool; no vendor software.
- "if I want things to be as latency low…": settings live in mouse firmware or the RawAccel driver; nothing of ours sits in the input path.
- "I do not want Viper's rotations on my mouse": a RawAccel rotation per mouse.
- "have OUR solution to start with the equivalent of launchd": a sign-in task instead of the RawAccel GUI.
- Calibrator: full screen, keyboard only, "auto after 10 swipes … configured right before the test", test at a lower DPI.

This session (2026-11-08/09):
- "The objective is to see if our app can totally replace what Synapse does! … I'm looking forward to delete Synapse fully": goal `px-23l1`.
- "It's best that our tool will be as thorough as possible.": cover everything the Viper (and other devices) can do, not the minimum.
- "wouldn't our tool be better if it's then made to Rust?", then "smallest footprint is which one...?", then "Then egui then": Rust + egui.
- "I would like to move around between the two machines" (Windows PC + Mac Studio); "I use my Mac for gaming"; the user is building their own FPS game with a custom Metal + Rust engine on the Mac: "it always bothered me that I didn't have a tool that could adapt".
- "something to manage my peripherals would be... COOL": the scope is peripherals, not just these two mice.
- Dumang: "some keys like "Layout Toggling" that I would love to see if it's "bindable" to my mouse(s)".
- "the M42 is its own shape and shouldn't rely on other's.": every mouse gets its own settings; none borrows another's.
- "OF COURSE we should scrub our names for anonymity! but use generic placeholders … any downloader even a random grandma … can do it easily": no personal data in the repo; things work without editing paths.
- "You choose the name." / "You choose the next path with the least blast radius." / "go with what you'd like!": the agent picks, explains, and records.
- From the user's global instructions: they direct and decide; explain what and why ("I like learning along the way, and rushing to an MVP isn't my thing"); jj only, no git commands; never push without an explicit yes; tickets in `tk`; shells for the user: Nushell, then PowerShell.

## Decisions

| Decision | Why | Rejected alternative | When |
|---|---|---|---|
| Swap L/R in the Pulsar's onboard memory (profile 1) | Works everywhere incl. raw-input games, zero latency, Razer untouched | Windows "Primary mouse button" (swaps all mice); per-game setting | early |
| Own tool, no vendor software; Python + ctypes first | User dislikes bloat; the tool runs ~1 s and exits | Pulsar Fusion; Synapse | early |
| Pulsar ripple off, debounce 1 ms, DPI 3200 single stage | Latency; 1 ms tested "0 chatter"; matches the Viper's 3200 | ripple on; 0 ms (bounce); RawAccel DPI normalisation | early |
| Per-mouse RawAccel profiles via `settings.json` `devices[]` | The driver supports it (`driver/driver.cpp` `DeviceSetup`); only the GUI lacks it | One shared profile | early |
| Sign-in Task Scheduler task runs the loader once | RawAccel forgets settings on reboot; no resident process | Startup shortcut; always-on portal | early |
| **Replace Synapse fully** (`px-23l1`) | User goal. For the Viper, Synapse only adds spectrum lighting at 70%; DPI stages/polling are in the mouse; no remaps or macros | Keep Synapse | 2026-11-08 |
| **"As thorough as possible"**: full Viper control incl. lighting (`px-w1a0`); `px-hkts` closed as yes | User wants a complete Synapse replacement | Read-only Viper | 2026-11-08 |
| **Port to Rust + egui** (`px-izgb`) | Smallest footprint: one small exe, no WebView2 processes. Rust also gives one binary with no Python install, ms start-up, typed packet code. No latency gain (nothing of ours is in the input path), and the user was told so | Tauri (WebView2 adds ~4-6 processes and ~100-200 MB while open); tiny web server + browser; staying in Python | 2026-11-08 |
| Python stays the reference and the daily tool until each Rust phase matches it byte for byte | Safe migration; tests are generated from the Python | Big-bang rewrite | 2026-11-08 |
| **Windows HID via `windows-sys` directly** (SetupAPI paths + hid.dll); **`hidapi` only on macOS** (feature `macos-shared-device`) | On Windows, `HidApi::new()` reads every HID device's strings/descriptors (~150 ms, twice per open). `new_without_enumerate` is deprecated and still enumerates on Windows. hidapi also can't report `FeatureReportByteLength`, which the Viper needs. Result: find+open 2.5 ms; Rust `info` ~110 ms vs Python ~178 ms (~100 ms is mouse round trips) | `hidapi` everywhere | 2026-11-08 |
| Phase order: Viper **reads** before any writes | User asked for the least blast radius: zero device/driver writes | Starting with Pulsar writes | 2026-11-08 |
| Viper: on a reply with status OK but another command's class/id, **resend** (up to 8) | Synapse shares the reply buffer. Measured with Synapse running: Python failed a field in 8/30 runs, Rust 0/30 (`px-wfee`, closed) | Fail on mismatch (Python behaviour) | 2026-11-08 |
| M42 gets its **own** RawAccel profile `Xtrfy M42` at 0° | It was falling into `profiles[0]` = the Viper's -2.6°; user: "shouldn't rely on other's" | Leave on default; change `default` to 0° (still open: `px-xtv5`) | 2026-11-08 |
| Repo **public** as `github.com/Xyde/hidwright`, MIT licence | Free unlimited Actions incl. macOS runners (Mac code compiles in CI without the Mac). Name chosen by the agent: HID + "wright" (maker), not tied to one brand | Private repo; name `pulsar-x2a` (too narrow) | 2026-11-08/09 |
| Scrub history before the first push; RawAccel folder **auto-detected** | User: anonymity, and anyone can run it without editing paths | Placeholders users must fill in | 2026-11-08 |
| Rust code uses rustfmt defaults; CI runs fmt, clippy `-D warnings`, tests, release build on Windows + macOS, and py_compile | Every push is checked on both OSes | No CI | 2026-11-09 |
| Dumang layer switching from a mouse: computer relays a key to a `BoardSyncPacket` (`px-k3jg`) | A mouse can't press keyboard keys; the DK6 accepts layer changes from the host | Firmware-only (impossible across devices) | 2026-11-08 |
| Dumang protocol: **reimplement from the packet format**, don't copy code | Upstream `mayanez/dumang-keyboard-ctrl` is GPL-3.0; copying would force GPL on the repo | Porting its code | 2026-11-08 |
| Mac gaming: for the user's **own engine**, apply rotation inside the engine from a per-mouse profile our tool provides (`px-sk2o`, user to decide) | No RawAccel on macOS; inside the engine it's free and adds no latency | CGEventTap (in the input path; games reading HID bypass it); DriverKit driver (Apple signing, big) | 2026-11-08 |

### Reversals

- **Was:** "RawAccel has no per-mouse profiles". **Now:** per-mouse profiles via `devices[]`. **Because:** the driver source supports it.
- **Was:** I recommended **Tauri** for the UI. **Now:** **egui**. **Because:** the user chose the smallest footprint. Traces: none in code.
- **Was:** HID through the `hidapi` crate on both OSes (and before that, the `windows` crate). **Now:** `windows-sys` on Windows, `hidapi` on macOS. **Because:** start-up cost and missing report-size info (see Decisions). **Stale trace:** the description of ticket `px-0oek` still says "'windows' crate (Microsoft's bindings …)". The notes are correct; trust the code.
- **Was:** global writes (polling, debounce, toggles) used profile byte 0, as upstream does, and were lost on every power loss (the mouse reloaded an older saved 3 ms / ripple on). **Now:** writes send the active profile byte and are saved. **Because:** `px-337u` experiment 1 (2026-11-09): profile byte 0 = RAM only, 1-5 = saved to flash; reads return the same single copy for any byte.
- **Was:** first guess that the 2026-11-07 reboot "reset globals to factory". **Now:** see above (not factory; an older saved state).
- **Was:** the RawAccel path was hard-coded in `rawaccel_cfg.py`. **Now:** `find_dir()` searches `~\Downloads`, `~\Desktop`, `~\Documents` and Program Files for `RawAccel*` (newest first); env `RAWACCEL_DIR` overrides.
- **Was:** Synapse wasn't mentioned in the old handoff (assumed absent). **Now:** Synapse 4.0.896 is installed (since 2026-04) and running (9 `RazerAppEngine` processes); goal: remove it after a trial.

## Current state

Each item is **verified** (checked 2026-11-09 while writing this; says how) or **reported** (from the session, not re-checked).

- Pulsar X2A: profile 1 active, **L/R swapped**, 1000 Hz, **debounce 1 ms**, **ripple off**, angle-snap off, motion-sync off, DPI `[3200]`, lift-off 1 mm (**verified**: `set-mouse` 2026-11-09 after the replug)
- Pulsar persistence: swap, DPI, debounce 1 ms and ripple off all survived a replug (**verified** 2026-11-09: Windows device arrival 14:15:47, after the write at 14:14:15; then `set-mouse`). Polling, angle-snap and motion-sync use the same write path but weren't replug-tested one by one
- Pulsar profiles 2-5: never written; factory L/R; profile 2 DPI `400 [800] 1600 3200 6400 12800` (**verified**: Rust vs Python diff for `-p 2` and `-p 5`, 2026-11-08)
- Razer Viper: firmware v1.04, 1000 Hz, current DPI 3200, stages `400 800 1800 2400 [3200]` (**verified**: Rust `set-mouse`. The Python read failed `polling` this time with "Razer reply ok for 00/85", the known Synapse collision)
- Synapse: running, 9 `RazerAppEngine` processes (**verified**: `Get-Process`); its Viper profile: DPI stages as above, `mappings: []`, lighting `spectrumCycling` brightness 70 (**reported**: from Synapse's logs in `%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs`, 2026-11-08)
- Xtrfy M42: plugged in (**verified**: `Get-PnpDevice`); not configurable yet
- RawAccel: profiles `default` -2.6 (Viper and any unlisted device), `Pulsar X2A` -0.5, `Xtrfy M42` 0.0; `devices[]` has the Pulsar and the M42; 3 backups exist (**verified**: read `settings.json`). Driver service `rawaccel` Running (**verified**: `Win32_SystemDriver`)
- Sign-in task `Mouse Portal - load RawAccel`: Ready; runs `pythonw.exe "~\Documents\pulsar-x2a\mouse_startup.py"`; last run 2026-11-09 11:08:55, result 0. `startup.log` shows `default: -2.6 deg, Pulsar X2A: -0.5 deg, Xtrfy M42: 0.0 deg` (**verified**). It also worked after the real reboot of 2026-11-07 14:56 (`px-p010`, closed)
- Portal: not running (**verified**: nothing listens on 8642). User PATH contains the project's `bin` (**verified**). No interrupted test-DPI run (`dpi_restore.json` absent, **verified**)
- `rawaccel.exe` GUI retired to `…\RawAccel\_retired-gui` (**reported**, 2026-11-06/08)
- Calibrator v2 (streamed live view, auto-finish, test-at-DPI and restore) tried with real swipes: "Everything went fine" (**reported**, `px-ha9z` closed)

Version control: jj, colocated. `main` = `1bec8eda` "CI: Windows + macOS build/test; rustfmt" = `main@origin` (pushed by the user). Working copy `rqqmnkxv` on top holds ticket notes and this handoff, not pushed. History before the first push was rewritten to remove personal data; every revision was scanned, 0 hits. Tests: `cargo test` → 15 passed (**verified** 2026-11-09). CI run `42679662164`: Rust Windows ✅, Rust macOS ✅, Python ✅ (**verified**: `gh run view`).

## Specs and reference values

| Thing | Value | Source |
|---|---|---|
| Pulsar X2A wired | USB `3710:1404`, config on interface `MI_03` (vendor page), feature report ID 0, 64 bytes (65 with ID) | `pulsar_x2a.py`; `crates/mouse-core/src/x2a.rs` |
| X2A packet | `[1]` cat, `[2]` reg (bit7 = read), `[3]` sub, `[6]` profile (0 = global, 1-5; **global writes need the active profile here to be saved**, 0 = RAM only), `[7..62]` payload, `[62..64]` LE u16 of `sum(bytes[0..62])` | upstream `packerlschupfer/pulsar-mouse-linux` `src/pulsar_mouse/drivers/x2a.py` (MIT) |
| X2A reply byte 0 | `0x00` not answered, `0x01` answered, `0x02` pending (OK for writes), `0x03` refused | same |
| X2A commands (cat/reg/sub) | button `04/01/06` `[btn,type,a1,a2]`; polling `01/09/02` (125:1 250:2 500:4 1000:8); debounce `04/03/03`; angle-snap `07/04/02`; ripple `07/03/02`; motion-sync `07/05/02`; DPI write `05/04/21`; DPI read `05/04/15`; active stage `05/01/02`; LOD `07/02/03`; active profile `02/06/01` (profile in byte 6); firmware `01/07/04`; factory reset `02/05/01` (resets that profile **and all globals**) | same |
| X2A factory globals | 1000 Hz, debounce 3 ms, angle-snap/ripple/motion-sync **off** | upstream comment "Measured on a cabled X2A (2026-10-13)" |
| Razer Viper wired | USB `1532:0078`; 90-byte feature report (91 with ID); transaction id `0x1F`; crc = XOR of bytes 2..88 | OpenRazer; `crates/mouse-core/src/viper.rs` |
| Razer commands | firmware `00/81`, serial `00/82`, polling `00/85`, DPI get `04/85` arg NOSTORE, stages `04/86` arg VARSTORE, DPI set `04/05` size 7 `[NOSTORE, xhi, xlo, yhi, ylo, 0, 0]` | OpenRazer; `razer_viper.py` |
| Viper status byte | `0x00` new, `0x01` busy, `0x02` ok, `0x03` failed, `0x04` timeout, `0x05` not supported | `viper.rs` |
| Viper lighting effects Synapse offers | STATIC, BREATHING, SPECTRUM_CYCLING, REACTIVE, AUDIO_METER, TIDAL; DPI 100-16000 step 50 | Synapse log `connectedDevices` JSON |
| Xtrfy M42 | USB `2EA8:2203`; `MI_00` mouse (input 8); `MI_01` col01 keyboard, col02 consumer, **col03 vendor page `0xFF00`, 64-byte input + output reports, no feature report** | `HidP_GetCaps` read, 2026-11-08 |
| Dumang DK6 | USB `0483:5710` ("DuMang KeyBoard DK6"); also exposes a mouse collection `MI_02&Col03` | PnP `BusReportedDeviceDesc` |
| Dumang layer packet | `BoardSyncPacket`: `[cmd, 0x01, 0x03 active / 0x02 inactive, 0x00, ID, layer_info, layer_info & 0x03, 0x00]`; layer keycodes `0xD0-D3` (switch), `0xD4-D7` (momentary), `0xDC-DF` (long-press) | `mayanez/dumang-keyboard-ctrl` `dumang_ctrl/dumang/common.py` (GPL-3.0: facts only, no code) |
| RawAccel device ids | Pulsar `HID\VID_3710&PID_1404&MI_00`, Viper `HID\VID_1532&PID_0078&MI_00`, M42 `HID\VID_2EA8&PID_2203&MI_00`, DK6 mouse `HID\VID_0483&PID_5710&MI_02&Col03` (case-sensitive) | `rawinput_monitor` `devices_info()` |
| RawAccel profile choice | matching `devices[].id` → that `profile`; everything else → `profiles[0]` | `driver/driver.cpp` `DeviceSetup` (RawAccel repo) |
| Rotation maths | recommended = current rotation − measured θ (Raw Input sees movement *after* RawAccel) | `mouse_portal.html` `calFinish` |
| Swipe thresholds | turn `max(20, 0.1·dpi)`, min swipe `max(80, 0.4·dpi)` counts | `rawinput_monitor.thresholds` |
| Portal | `http://127.0.0.1:8642` | `mouse_portal.py` |
| Rust release exe | `target\release\set-mouse.exe`, ~211 KB (2026-11-09, with the X2A writes; was 169 KB); profile `lto`, `codegen-units = 1`, `strip`, `panic = "abort"` | `Cargo.toml` |
| Toolchain | rustc/cargo 1.99.1, MSVC Build Tools 2022; Python 3.15.2 | `rustc --version`; `vswhere` |
| `windows-sys` | 0.61 (same version hidapi uses) | `crates/mouse-core/Cargo.toml` |

## How to run and verify

Nushell and PowerShell take these as written, from the project folder (`~\Documents\pulsar-x2a`):

```
set-mouse
set-mouse help
set-mouse portal
set-mouse rotation
python set_mouse.py razer
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
target\release\set-mouse.exe
target\release\set-mouse.exe info -p 2
target\release\set-mouse.exe razer
jj st
jj log
tk ready
tk blocked
gh run list --repo Xyde/hidwright --limit 3
```

Parity check, Rust against Python (Git Bash; only line endings differ):

```
python pulsar_x2a.py info > py.txt; target/release/set-mouse.exe info > rs.txt; diff --strip-trailing-cr py.txt rs.txt && echo identical
```

**How the Rust tests' reference vectors were made** (repeat this for the phase 2 write commands): a throwaway Python script imports the module, calls the packet builder (`pulsar_x2a.X2A.build`, `describe`) and prints hex. For the Viper, it replaces `razer_viper.hid` with a fake object whose `HidD_SetFeature` records the sent bytes and whose `HidD_GetFeature` returns a canned reply. That captures request bytes *and* parsing without touching a device. For the X2A writes (2026-11-09), the script made an `X2A` with `object.__new__` (no device), set `set_report` to append to a list, `get_report` to return `0x02` + zeros, and pinned `active_profile` to **3** so a global write sent with 0 or 1 fails the test. Paste the output into a `#[test]`. Keep such scripts in the scratchpad, not in the repo.

Sign-in task (PowerShell): `Get-ScheduledTaskInfo -TaskName "Mouse Portal - load RawAccel"`; log: `Get-Content ~\Documents\pulsar-x2a\startup.log`.

## Environment and gotchas

- **This repo is public.** Never write the Windows user name, PC name, email, or device serials into tracked files or commit messages. Use `~\…`. Synapse's logs and the scratchpad contain such data; don't paste them in.
- **Claude Code's auto-mode classifier blocks creating public surfaces** (`gh repo create --public`). The user ran that and the pushes. Pushing always needs the user's explicit yes anyway.
- **jj: pushed commits are immutable.** After a push, jj starts a new empty change on top. Don't rewrite `main@origin` history.
- **jj abandons an empty, undescribed working-copy change when you `jj edit` another revision.** Its change id then no longer exists. Use `jj new <tip>` to come back.
- **Old revisions don't ignore `target/`.** `.gitignore` gained `target/` only in the Rust commit. Checking out an older revision makes jj snapshot build files into it. Before `jj edit` on an old revision, move `target` out of the repo (`mv target ../_hidwright_target_parked`), and move it back only after `jj new <tip>`.
- **Synapse collisions:** the Python Viper reader fails a field in ~27% of runs while Synapse runs (seen again today). Rust resends. Don't "fix" this by stopping Synapse silently; that's the user's trial (`px-23l1`).
- **hidapi on Windows enumerates every device on `HidApi::new()`** (~150 ms); `new_without_enumerate()` doesn't help there and is deprecated. That's why Windows uses `crates/mouse-core/src/hid/windows.rs`.
- **`link.exe` on PATH is Git's coreutils `link`**, not MSVC's. Harmless: rustc finds MSVC through the Visual Studio installation.
- **Shells:** commands for the user go in Nushell first, then PowerShell (not bash). On the Mac the user uses **fish**.
- **Python prints CRLF on Windows, Rust prints LF.** Use `diff --strip-trailing-cr` for parity checks.
- **Shell quoting:** inline `python -c` with Windows paths or backslashes broke twice. Put scripts in a file (scratchpad) instead.
- **`tk` has no assign command:** change `assignee:` in `.tickets/<id>.md` directly. `tk.cmd` goes through cmd.exe; for long text use `bash ~/bin/ticket/ticket` from Git Bash.
- From the old handoff, still true: RawAccel forgets settings on reboot (the sign-in task reloads them); Raw Input sees movement after RawAccel; the retired GUI can't run from `_retired-gui`; `set-mouse.cmd` + Ctrl+C asks "Terminate batch job"; only one portal at a time; `python -I` breaks sibling imports; `.gitignore` comments must be on their own line; the Claude app is packaged, so `%APPDATA%` writes from it can be redirected (check from outside).
- The handoff checker flags `/api/calib/stream` (an HTTP route) and upstream repo paths (`driver/driver.cpp`, `src/pulsar_mouse/drivers/x2a.py`, `dumang_ctrl/dumang/common.py`); those aren't files in this repo.

## Open threads

| Thread | Ticket | Owner | Blocked on |
|---|---|---|---|
| Rust port (epic) | `px-0oek` | agent | — |
| Rust 2: X2A writes + RawAccel cfg + CLI parity, then switch the PATH shim | `px-5xrv` | agent | nothing |
| Rust 3: Viper (reads done; temporary DPI and writes left) | `px-36ci` | agent | nothing |
| Viper full control (lighting, saved DPI, polling, anything else) | `px-w1a0` | agent | `px-36ci` |
| Rust 4: sign-in loader in Rust | `px-yefs` | agent | `px-5xrv` |
| Rust 5: egui portal | `px-01cy` | agent | `px-5xrv`, `px-36ci` |
| Rust 6: egui calibrator | `px-xpnz` | agent | `px-01cy` |
| Rust 7: retire the Python | `px-t5i3` | agent | `px-yefs`, `px-xpnz` |
| Replace Synapse: trial without it, then uninstall (incl. `PID0078Drv`, `RzComDriver`) | `px-23l1` | user (agent gives commands) | user's go |
| macOS: build + test on the Mac Studio (CI compile already green) | `px-e6p0` | agent, on the Mac | a session on the Mac |
| Rotation on macOS / engine integration for the user's own game | `px-sk2o` | user decides | user decision |
| Dumang DK6: layer toggle from a mouse button | `px-k3jg` | agent | test on the real DK6 |
| Xtrfy M42: find the original software, maybe capture its protocol | `px-8mzp` | agent | finding the software; user OK to run it |
| Give the Viper its own RawAccel profile, `default` → 0° | `px-xtv5` | user decides | user decision |
| Swap L/R on Pulsar profiles 2-5 too? | `px-tkar` | user decides | user decision |
| Re-run the calibrator once on the Pulsar, expect about 0° | `px-vtbh` | user | nothing |
| Acceleration/sensitivity editing? | `px-xykv` | user decides | user decision |
| Verify X2A lift-off encoding before using `lod` | `px-zd1z` | agent | nothing; low priority |
| Calibrate the M42's own rotation | no ticket (mentioned in `px-8mzp`) | user | nothing |
| Python `set-mouse rotation` doesn't list the M42 (unnamed mice are filtered out) | no ticket (noted in `px-8mzp`); fix in the Rust port | agent | — |

## Unknowns and unverified

- **Viper without Synapse:** does the logo keep spectrum cycling (Razer's factory default) and at what brightness? Check with the `px-23l1` trial.
- **What else the Viper supports** (onboard remapping? lift-off?): check OpenRazer before `px-w1a0`.
- **macOS with real mice:** does macOS report interface numbers? Can we send feature reports to the Viper's mouse interface without exclusive access? (`px-e6p0`)
- **Dumang:** one USB device shows up; what the board `ID` and `layer_info` mean on a DK6; momentary vs latched behaviour (`px-k3jg`).
- **M42 protocol:** unknown; no public work found.
- **X2A lift-off encoding** (`px-zd1z`); whether the RawAccel GUI would keep `devices[]` on save (GUI retired, not tested).

## Don't

- Don't put personal data in the repo (public). Don't push without the user's explicit yes, and don't rewrite pushed history.
- Don't use Windows' "Primary mouse button" swap (it swaps every mouse).
- Don't install or recommend Pulsar Fusion or Synapse as the solution. The *only* considered exception is a temporary Fusion install for a USB capture, and only with the user's OK. Never use Fusion's "reset to defaults".
- Don't uninstall Synapse or its drivers before the user's trial (`px-23l1`). Don't uninstall the RawAccel driver or delete `writer.exe`, `wrapper.dll`, `Newtonsoft.Json.dll`. Don't delete `_retired-gui`.
- Don't write to the Viper with STORE/VARSTORE until each command is checked against OpenRazer (`px-w1a0`).
- Don't copy code from `dumang-keyboard-ctrl` (GPL-3.0); reimplement from the packet format.
- Don't switch `bin/set-mouse.cmd` or the sign-in task to Rust before parity; don't delete the Python before `px-t5i3`.
- Don't hand-edit `settings.json` `profiles[0]` thinking it's the Pulsar's (it's `default`). Use `set-mouse rotation` or `rawaccel_cfg.apply` (they back up first).
- Don't set debounce 0 ms; don't promise per-DPI rotation (one rotation per profile); don't start a second portal; don't use `python -I` on modules that import siblings.
- Don't run git commands (jj only). Don't use `HidApi::new()` in the Windows code path.

## Where things live

- `README.md`: public, beginner-friendly usage. `LICENSE`: MIT.
- `CLAUDE.md`: short orientation; points here.
- `Cargo.toml`: workspace (`crates/mouse-core`, `crates/set-mouse`), release profile.
- `crates/mouse-core/src/hid/`: `mod.rs` (shared `Interface`, report-ID helper), `windows.rs` (SetupAPI + hid.dll), `macos.rs` (hidapi).
- `crates/mouse-core/src/x2a.rs`: Pulsar reads (`Snapshot`) and writes (`Setting` enum + `X2a::write`, which picks the profile byte: active profile for globals), `Binding::parse`, tests vs Python vectors.
- `crates/mouse-core/src/viper.rs`: Viper read side, collision retry, tests vs Python vectors.
- `crates/set-mouse/src/main.rs`: Rust CLI (`set-mouse`, `info`, all Pulsar write commands with the Python's messages, `razer`, `help`). Input is checked before the mouse is opened.
- `.github/workflows/ci.yml`: CI (Windows + macOS Rust, Python compile).
- Python (daily use): `set_mouse.py` (dispatcher), `bin/set-mouse.cmd` (PATH shim), `pulsar_x2a.py`, `razer_viper.py`, `rawinput_monitor.py`, `rawaccel_cfg.py`, `test_dpi.py`, `mouse_portal.py` + `mouse_portal.html`, `mouse_startup.py` (sign-in loader → `startup.log`).
- `.tickets/`: `tk` tickets. `.claude/launch.json`: browser-pane preview config.
- Outside the repo: RawAccel in `~\Downloads\RawAccel_v1.7.1\RawAccel\` (`settings.json`, backups, `writer.exe`, `_retired-gui\`); Claude memory in `~\.claude\projects\<this project's folder>\memory\`.
