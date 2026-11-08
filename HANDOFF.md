# Handoff: pulsar-x2a (set-mouse / Mouse Portal)

_Written 2026-11-06 ~16:40 by Claude (Claude Code desktop session); updated 2026-11-08 when the folder became a jj repo and open threads became `tk` tickets. Replaces the previous HANDOFF.md (history is in jj from now on)._

## Resume here

- **What this is:** A small, bloat-free, self-built replacement for vendor mouse software on Windows. `set-mouse` (CLI) plus the "Mouse Portal" (local web page) configure a **Pulsar X2A** directly over USB HID, read a **Razer Viper**, and give each mouse its own **RawAccel** rotation. Rotation is measured with a built-in full-screen calibrator.
- **Where it stands:** Everything the user asked for is built and was tested on the real hardware this session. The user calibrated the Pulsar: it is at **-0.5°**, and the Viper keeps **-2.6°**. The last two features weren't tried with the user's real swipes: the streamed (smoother) live view and the "test at DPI" option.
- **Next action:** User, ticket `px-ha9z`: run `set-mouse portal`, open the **Rotation calibrator** and do a run with **Test at 800 DPI**. Check that it feels smooth, that it auto-finishes at the chosen swipe count, and that the result screen says `DPI back to normal: … 3200 ✓`. Agent: after the user's next reboot, do `px-p010` (check `startup.log`). No unfinished code work is pending. `tk ready` lists everything.
- **Paste into a fresh session:**
  > Read HANDOFF.md in ~\Documents\pulsar-x2a first. Run `jj st`, `tk ready` and `set-mouse` to confirm the repo, the tickets, both mice and RawAccel still match the "Current state" section, then ask me how ticket px-ha9z (calibrator v2 with real swipes) went.

## Goal and constraints (user's words)

- "how do I change left and right button … i want it that way for everything": swap L/R system-wide (all apps and games) …
- "I haev a razer who I do not want to swap that setting out of": … but **only on the Pulsar**. So no Windows-wide swap; everything is per mouse.
- "i hate the idea of pulsar fusion -- maybe it has BLOAT … let's see if we can make a BETTER DRIVER or SOFTWARE": write our own minimal tool. Don't install vendor software.
- "if I want things to be as latency low…": the user cares a lot about input latency. Settings live in mouse firmware or the RawAccel driver. Nothing of ours sits in the input path.
- "maybe having a cool portal to see all mouse is fun": the Mouse Portal (`set-mouse portal`).
- "I do not want Viper's rotations on my mouse": each mouse has its own RawAccel rotation. The Viper's -2.6° must never apply to the Pulsar.
- "I want this calibration tool to our tool" (pointing at Razer's mouse-rotation calibration page): a built-in rotation calibrator. It's our own implementation, not a copy of Razer's.
- "I do not want "rawaccel.exe" anymore if our solution is just as good! … have OUR solution to start with the equivalent of launchd": retire the RawAccel GUI and load settings at sign-in with our own task.
- "how about smth like set-mouse": one command on PATH.
- Calibrator: "should be full screen", "by clicking at the button, it changes the rotation" (mouse moves while clicking Start/Stop pollute the measurement), "auto after 10 swipes … configured right before the test, like some might enjoy 20 swipes".
- "feels a bit laggy": the calibrator's live view (fixed with streaming).
- "high dpi means wrist movement … instead of 3200, how about 1600 or 800 dpi or 400 dpi": a test-at-DPI option.

## Decisions

| Decision | Why | Rejected alternative | When |
|---|---|---|---|
| Swap L/R inside the Pulsar's onboard memory (profile 1) | Applies everywhere, including games that use raw input, and adds zero latency. The Razer is untouched. | Windows "Primary mouse button" (swaps **all** mice, Razer included); the in-game setting (per game only) | early |
| Own tool in pure Python + `ctypes` over Windows `hid.dll`, no pip deps | The user dislikes bloat. The script runs about 1 s and exits, so language doesn't matter for latency. | Pulsar Fusion (bloat); Rust (no latency benefit, because the remap runs in mouse firmware) | early |
| X2A protocol taken from MIT project `packerlschupfer/pulsar-mouse-linux` (`drivers/x2a.py`) | Already reverse-engineered from USB captures; supports 3710:1404 | Capturing USB ourselves | early |
| Ripple control **off** | Pure latency win at this DPI (ripple smooths jitter by adding delay) | Leave on | mid |
| Debounce 3 → 2 → **1 ms** | Faster click registration. The user tested 1 ms in the portal's click tester and reported "0 chatter". | 0 ms (mechanical contacts bounce, so double clicks are likely) | mid |
| thumb3/thumb4 reset to factory back/forward | They were bound to the `-` key and `Ctrl`, origin unknown (not set this session). The user agreed to reset. | Keep | mid |
| Pulsar DPI 1600 → **3200** (single stage) | Matches the Razer's 3200, so one RawAccel curve feels the same on both | RawAccel per-device DPI normalization ("experimental" per its own guide) | mid |
| Razer is **read-only**, except a temporary NOSTORE DPI change during calibration | We only needed to compare. NOSTORE never touches the saved stages. | Writing persistent Razer settings (not requested) | late |
| Per-mouse RawAccel rotation via `settings.json` → `devices[]` entries pointing at per-mouse `profiles[]` | The RawAccel GUI can't assign profiles per device, but the driver supports it (source: `driver/driver.cpp` `DeviceSetup`) | One shared profile (that's what put the Viper's -2.6° on the Pulsar) | late |
| Retire `rawaccel.exe` by **moving** it (plus `.config` and 2 shortcuts) to `RawAccel\_retired-gui` | The user asked ("byebye"), and moving can be undone. The user uses noaccel and only rotation, which the portal covers. | Deleting it; uninstalling RawAccel (the **driver must stay**: it applies the rotation) | late |
| Sign-in loader as a **Task Scheduler** task running `pythonw mouse_startup.py` | Windows' equivalent of launchd. The RawAccel driver forgets its settings on reboot. The task runs once and exits. | Startup-folder shortcut; keeping the portal running all the time (a needless background process) | late |
| `set-mouse` = `bin\set-mouse.cmd` → `set_mouse.py`, with `bin` on the **user** PATH | Works in PowerShell, cmd and Windows Terminal; no admin needed | `.ps1` (execution policy risk) | late |
| Calibrator v2: full-screen overlay, keyboard only (Space/Enter/Esc), auto-finish after N swipes (default 10, user-set 4-50), optional auto-apply | The user's feedback: clicking pollutes the measurement; wants it automatic and configurable | v1 inline card with Start/Stop buttons | late |
| Live view streamed with SSE (`/api/calib/stream`, ~120 Hz), analysed in the browser, drawn every frame; locks onto the first mouse that moves >100 counts | The user found v1 laggy (it polled the full analysis every 80 ms) | Faster polling | late |
| Swipe detection in **physical distance**: turn = 0.1 in, swipe ≥ 0.4 in (`thresholds(dpi)`) | Fixed counts (150/400) meant tiny wiggles counted as swipes at 3200 DPI and long swipes were needed at 400 | Fixed counts | late |
| Test-at-DPI (400/800/1600/3200) switches **both** mice for the run, then restores them; originals go to `dpi_restore.json` **before** any change | Wrist vs arm movement; crash-safe | Switching only the calibrated mouse (we don't know which mouse until it moves) | late |

### Reversals

- **Was:** I first advised the Windows Settings "Primary mouse button → Right" swap. **Now:** firmware swap on the Pulsar only. **Because:** the user also uses a Razer that must stay normal. Nothing was changed in Windows.
- **Was:** I told the user "RawAccel has no per-mouse profiles" (based on its README/Guide). **Now:** per-mouse profiles are used through `settings.json` `devices[]`. **Because:** RawAccel's source (`common/rawaccel.hpp` `device_settings{name, profile, id, config}`, `wrapper/wrapper.cpp` `DriverConfig{version, defaultDeviceConfig, profiles, devices}`) supports it; only the GUI lacks it.
- **Was:** Pulsar DPI 1600. **Now:** 3200. **Because:** to match the Razer.
- **Was:** calibrator v1 (Start/Stop buttons, polling, fixed count thresholds, picks the "most moved" mouse). **Now:** v2 (see Decisions). No traces of v1 remain in `mouse_portal.html`, but `/api/calib/status` still exists, and `monitor.status()` falls back to "most moved" before a lock.

## Current state

Each item is **verified** (checked while writing this; says how) or **reported** (from the session, not re-checked).

- Pulsar X2A: profile 1 active; **L/R swapped**; 1000 Hz; debounce **1 ms**; angle-snap off; ripple **off**; motion-sync off; DPI stages `[3200]`; lift-off 1 mm; wheel/thumb1-4/dpi buttons at factory (**verified**: `python set_mouse.py`)
- Pulsar **profiles 2-5 were never changed**: they still have factory left/right and their own DPI. Switching onboard profile (button combo or `set-mouse profile N`) would undo the swap until they're set too (`set-mouse swap -p N`). With a single DPI stage, the DPI button cycles nothing. (**reported**: the info scan earlier this session showed profiles 1-5 all factory before the swap; only profile 1 was written since)
- Razer Viper: firmware v1.04; 1000 Hz; current DPI 3200; saved stages `400 800 1800 2400 [3200]` (**verified**: same command)
- RawAccel rotation: Pulsar **-0.5°** (own profile `Pulsar X2A`), Viper **-2.6°** (profile `default`) (**verified**: same command). `settings.json` written 16:25:26 by the portal; backups `settings.backup-20261106-162517.json` and `…-162526.json` exist (**verified**: directory listing)
- RawAccel driver service `rawaccel`: Running (**verified**: `Win32_SystemDriver`)
- Sign-in task `Mouse Portal - load RawAccel`: Ready, logon trigger, runs `pythonw.exe "~\Documents\pulsar-x2a\mouse_startup.py"`, last run 16:08:19 result 0 (**verified**: `Get-ScheduledTask`/`Get-ScheduledTaskInfo`). It hasn't run since the per-mouse split; that first happens at the next sign-in (**reported**)
- User PATH contains `~\Documents\pulsar-x2a\bin` (**verified**). Already-open terminals don't see it.
- `rawaccel.exe` GUI retired to `~\Downloads\RawAccel_v1.7.1\RawAccel\_retired-gui` along with `.config`, `rawaccel.lnk` (from the Start menu) and `rawaccel - Shortcut.lnk` (from the Desktop) (**verified**).
- **The Claude app is packaged (`Claude_*`), but none of the changes above were redirected** (**verified** 2026-11-08 from *outside* the package with a temporary one-shot scheduled task, since deleted): the real Start menu and Desktop no longer have the RawAccel shortcuts, the real HKCU `Path` has the `bin` folder, and a fresh process resolves `set-mouse` to `bin\set-mouse.cmd`. At **16:33:58 someone launched it from inside `_retired-gui`, and it crashed** with `Could not load file or assembly 'wrapper'` (**verified**: `_retired-gui\error.log`). It can't run from there; see Gotchas.
- No interrupted test-DPI run pending: `dpi_restore.json` absent (**verified**)
- Mouse Portal running on `127.0.0.1:8642` (pid 34496 at writing time), started by Claude in a Terminal-panel tab (**verified**: `Get-NetTCPConnection`)
- User calibrated the Pulsar and applied it (two applies, 16:25:17 and 16:25:26) and said "Got it done" (**reported**)
- User tested debounce 1 ms: "0 chatter" (**reported**)
- After the first swap, the user said "It's on!", taken to mean it survives a replug (**reported**, a bit ambiguous)

Version control: **jj** (colocated, `jj git init` on 2026-11-08). One change holds the whole project and this handoff. **No remote, nothing pushed.** `.gitignore` excludes `__pycache__/`, `startup.log` and `dpi_restore.json`. Tests: no test suite. `python -m py_compile` on all 8 `.py` files → exit 0 (**verified**). JS live maths matched the Python `analyze()` on identical synthetic swipes (angle 1.91°, swipes 11, completed 10) (**verified** earlier this session, not re-run).

## Specs and reference values

| Thing | Value | Source |
|---|---|---|
| Pulsar X2A (wired) USB ID | `3710:1404`, config on interface `MI_03` (vendor-defined), Feature report, ID 0, 64 bytes (65 with report-ID byte on Windows) | PnP list; pulsar-mouse-linux `x2a.py` |
| X2A packet | `[0]`=0x00 cmd, `[1]` cat, `[2]` reg (bit7 = read), `[3]` sub, `[6]` profile (0 = global, 1-5), `[7..61]` payload, `[62..63]` = LE uint16 of `sum(bytes[0:62])` | `x2a.py`, implemented in `pulsar_x2a.py` `X2A.build` |
| X2A reply byte 0 | `0x00` not answered, `0x01` answered, `0x02` pending (OK for writes), `0x03` refused | `x2a.py` comments |
| X2A commands (cat/reg/sub) | button `04/01/06` payload `[btn,type,a1,a2]`; polling `01/09/02` (125:1, 250:2, 500:4, 1000:8); debounce `04/03/03`; angle-snap `07/04/02`; ripple `07/03/02`; motion-sync `07/05/02`; DPI write `05/04/21` `[active,count,(i+1,lo,hi,lo,hi)…]`; DPI read `05/04/15`; LOD `07/02/03`; active profile `02/06/01` (profile in byte 6, NOT the payload); firmware `01/07/04` | `x2a.py`, see `pulsar_x2a.py` |
| X2A button ids | left 0x01, right 0x02, wheel 0x03, thumb1 0x04, thumb2 0x05, thumb3 0x06, thumb4 0x07, dpi 0x0B | `x2a.py` |
| Razer Viper (wired) USB ID | `1532:0078`; 90-byte feature report (91 on Windows), transaction id 0x1F worked | OpenRazer protocol; `razer_viper.py` |
| Razer commands | firmware `00/81`, serial `00/82`, polling `00/85`, DPI get `04/85` (arg NOSTORE), DPI stages `04/86` (VARSTORE), DPI set `04/05` size 7 `[NOSTORE, xhi, xlo, yhi, ylo, 0, 0]`; crc = XOR of bytes 2..87 | OpenRazer; `razer_viper.py` |
| RawAccel device ids | Pulsar `HID\VID_3710&PID_1404&MI_00`, Razer `HID\VID_1532&PID_0078&MI_00` (instance id minus last `\` segment; driver compares **case-sensitively**) | `wrapper/input.h`; `rawinput_monitor.rawaccel_id` |
| RawAccel profile choice | device with a matching `devices[].id` → its `profile` by name; **everything else → `profiles[0]`** | `driver/driver.cpp` `DeviceSetup` |
| RawAccel JSON keys | top: `version`, `defaultDeviceConfig`, `profiles`, `devices`; profile: `name`, `"Degrees of rotation"`, `"Output DPI"`, …; device: `name`, `profile`, `id`, `config` | `wrapper/wrapper.cpp`; the live `settings.json` |
| Rotation maths | RawAccel: `x' = x cos - y sin`, `y' = x sin + y cos` (raw coords, y down). Calibrator: measured θ = principal axis of deltas; **recommended = current rotation − θ** | `common/math-vec2.hpp`; `mouse_portal.html` `calFinish` |
| Swipe thresholds | turn `max(20, 0.1·dpi)` counts, min swipe `max(80, 0.4·dpi)` counts → at 3200 DPI: 320 / 1280 | `rawinput_monitor.thresholds` |
| Test DPIs | 400, 800, 1600, 3200 (`TEST_DPIS`) | `mouse_portal.py` |
| Portal | `http://127.0.0.1:8642` (default `PORT`, `--port`/`PORT` env override); local-only Host check and Origin check on POST | `mouse_portal.py` |
| RawAccel folder | `~\Downloads\RawAccel_v1.7.1\RawAccel` (v1.7.1, settings `version` "1.7.0"); found automatically in `~\Downloads`, `~\Desktop`, `~\Documents` or Program Files; override with env `RAWACCEL_DIR` | `rawaccel_cfg.find_dir` |
| Python | `~\AppData\Local\Programs\Python\Python315\python.exe` (3.15.2), `pythonw.exe` alongside | `Get-Command` |

## How to run and verify

These are the same in Nushell, PowerShell and cmd (`set-mouse` is a `.cmd` on the user PATH). Open a **new** terminal after the PATH change.

```
set-mouse
set-mouse help
set-mouse portal
set-mouse rotation
set-mouse rotation pulsar 0
set-mouse reload
set-mouse debounce 2
set-mouse dpi 800 1600 3200 --active 3
```

Without PATH (from the project folder):

```
python set_mouse.py
python mouse_portal.py --no-browser --port 8650
python -m py_compile pulsar_x2a.py razer_viper.py rawinput_monitor.py rawaccel_cfg.py test_dpi.py mouse_portal.py mouse_startup.py set_mouse.py
```

Repo and tickets:

```
jj st
jj log
tk ready
```

Check the sign-in task. Nushell:

```
open ~\Documents\pulsar-x2a\startup.log
powershell -NoProfile -c "Get-ScheduledTaskInfo -TaskName 'Mouse Portal - load RawAccel'"
```

PowerShell:

```
Get-Content ~\Documents\pulsar-x2a\startup.log
Get-ScheduledTaskInfo -TaskName "Mouse Portal - load RawAccel"
```

## Environment and gotchas

- **RawAccel forgets everything on reboot**: the driver starts empty → our task `Mouse Portal - load RawAccel` runs `writer.exe settings.json` at sign-in. `writer.exe` needs **no admin** (tested).
- **Raw Input sees movement *after* RawAccel**: the calibrator measures the rotation already applied and compensates (recommended = current − measured). If RawAccel's live state differs from `settings.json`, the recommendation is off.
- **The retired GUI can't run from `_retired-gui`**: it needs `wrapper.dll` and `Newtonsoft.Json.dll` from the RawAccel folder. To bring it back, move `rawaccel.exe` (and `.config`) back to `…\RawAccel\`. Someone tried to run it at 16:33:58, see Current state.
- **`set-mouse.cmd` + Ctrl+C** → `Terminate batch job (Y/N)?`. That's a cmd batch-file quirk; answer `Y` or close the tab.
- **Windows `SO_REUSEADDR`** let two portals bind 8642 at once (an old instance kept serving old code). Fixed: `ThreadingHTTPServer.allow_reuse_address = False`, and a second start now just opens the browser to the running one. Only one portal should talk to the mice.
- **The browser pane's `requestFullscreen()` promise never settles**, so the overlay hung empty while awaiting it. Fixed by not awaiting. Real browsers do go full screen.
- **`.gitignore` has no trailing comments**: `startup.log   # comment` is read as one literal pattern, so the file was briefly tracked. Comments go on their own line.
- **The Claude app is packaged**, so writes under `%APPDATA%` from it *can* be redirected to `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming`. Check persistent changes from outside the package (a one-shot scheduled task works; see Current state).
- **`python -I`** (isolated mode) breaks scripts that import sibling modules (`razer_viper.py` imports `pulsar_x2a`). Run them from the folder without `-I`, or via `set-mouse`.
- **Old `startup.log` lines show `Â°`** (a UTF-8 degree sign read as ANSI). New lines print `deg`.
- **Keyboard 0483:5710 exposes a mouse collection** (`HID\VID_0483&PID_5710&MI_02&Col03`). As an unlisted device it gets RawAccel `profiles[0]` (`default`, -2.6°). Harmless.
- **The `HANDOFF.md` checker flags `/api/calib/stream` and `/api/calib/status`**: they're HTTP routes of the portal, not files. Its warnings about `driver/driver.cpp`, `wrapper/…`, `common/…` and `drivers/x2a.py` refer to the upstream GitHub repos (RawAccelOfficial/rawaccel, packerlschupfer/pulsar-mouse-linux), not this folder.
- **HTTP 400 lines in the browser-pane console** came from deliberately invalid test requests (bad button function, DPI 450). They're not bugs.

## Open threads

Tracked with wedow/ticket (`tk`, files in `.tickets/`). Full details: `tk show <id>`.

| Thread | Ticket | Owner | Blocked on |
|---|---|---|---|
| Try the streamed live view and test-at-DPI with real swipes; confirm smoothness, auto-finish, DPI restore | `px-ha9z` | user | nothing |
| Rotation sign never explicitly confirmed with a "≈0° after apply" re-run (the user applied twice and was satisfied) | `px-vtbh` | user | nothing; re-run the calibrator once |
| Swap left/right on Pulsar profiles 2-5 too? | `px-tkar` | user | user decision |
| Make Razer settings editable (persistent DPI/polling)? Offered, not requested | `px-hkts` | user decides, agent builds | user decision |
| Acceleration curve / sensitivity editing in the portal? Only if accel is ever wanted; currently `noaccel` | `px-xykv` | user decides, agent builds | user decision |
| Give the Viper its own RawAccel profile so `default` (unlisted devices) could be 0°? | `px-xtv5` | user decides | not requested |
| Check the sign-in task after a real reboot (`startup.log`) | `px-p010` | agent | the next reboot |
| Verify the X2A lift-off encoding before anyone uses `set-mouse lod` | `px-zd1z` | agent | nothing; low priority |
| ~~Put this folder under version control~~ | done 2026-11-08 (jj) | | |

## Unknowns and unverified

- **Does saving from the RawAccel GUI keep `devices[]` and the extra profiles?** Not verified: its main window probably edits only `profiles[0]`. Check by moving the GUI back, saving, and diffing `settings.json`. The GUI is retired anyway.
- **X2A lift-off (`lod`) encoding** (`px-zd1z`): the two-value scheme in `x2a.py` is contradicted by comments for the X3 (tenths of a mm). Never written this session; treat `set-mouse lod` as experimental.
- **Persistence of the Pulsar writes across a full power cycle**: the swap was reported to survive a replug. DPI, debounce and ripple weren't separately replug-tested, but they're read back from the mouse each time.
- **Wireless X2A / dongle**: different protocol and IDs (`3554:f507`/`f508`, or Areson `25a7:fa7c`). Not supported; the tool requires the cable.
- **Why thumb3/thumb4 had `-`/Ctrl bindings** before this session: unknown (maybe set earlier in Pulsar Fusion).
- **Behaviour of the sign-in task on a real reboot** (`px-p010`): tested only with `Start-ScheduledTask` while signed in.

## Don't

- Don't use Windows' "Primary mouse button" swap: it would swap the Razer too.
- Don't install or recommend Pulsar Fusion, and never use its "reset to defaults": it would wipe the swap, debounce and ripple settings.
- Don't uninstall the RawAccel driver or delete `writer.exe`, `wrapper.dll` or `Newtonsoft.Json.dll`: the rotation and the sign-in loader depend on them.
- Don't delete `_retired-gui`: it's the undo path for the user's "byebye".
- Don't hand-edit `settings.json` rotation in `profiles[0]` thinking it's the Pulsar's: the Pulsar has its **own** profile `Pulsar X2A`. Use `set-mouse rotation …` or the portal (they back up first).
- Don't set debounce to 0 ms (contact bounce causes double clicks).
- Don't promise per-DPI rotation: RawAccel stores **one rotation per mouse** (per profile), and the last apply wins.
- Don't start a second portal while one runs; don't run the Razer DPI write with STORE/VARSTORE (we only ever use NOSTORE).
- Don't use `python -I` on modules that import siblings.
- Don't run git commands (jj only; the `.git` folder exists only because jj colocates). Don't push: there's no remote, and the user hasn't asked for one.

## Where things live

- `set_mouse.py`: the `set-mouse` dispatcher (status, portal, Pulsar commands, rotation, reload); restores an interrupted test DPI first
- `bin/set-mouse.cmd`: PATH shim → `python "%~dp0..\set_mouse.py" %*`
- `pulsar_x2a.py`: X2A HID driver + CLI (all Pulsar settings), plus the Windows HID plumbing other modules import
- `razer_viper.py`: Razer reader; `set_dpi_temporary` (NOSTORE) is its only write
- `rawinput_monitor.py`: per-mouse Raw Input capture (message-only window, `RIDEV_INPUTSINK`), RawAccel id derivation, swipe analysis, `thresholds(dpi)`
- `rawaccel_cfg.py`: read/plan/apply per-mouse RawAccel profiles; backs up `settings.json`, runs `writer.exe`
- `test_dpi.py`: temporary DPI for calibration runs + crash-safe `dpi_restore.json`
- `mouse_portal.py` / `mouse_portal.html`: the local web portal (cards, side-by-side table, click tester, full-screen calibrator, SSE stream)
- `mouse_startup.py`: sign-in loader (run by the scheduled task), logs to `startup.log`
- `.claude/launch.json`: browser-pane preview config (portal on port 8650 with `--no-browser`)
- `.tickets/`: `tk` tickets (open threads)
- `.gitignore`: excludes `__pycache__/`, `startup.log`, `dpi_restore.json`
- Claude memory for this project: `~\.claude\projects\<this project's folder>\memory\` (`pulsar-x2a-button-swap.md`, `user-prefers-minimal-software.md`)
- RawAccel: `~\Downloads\RawAccel_v1.7.1\RawAccel\` (`settings.json`, backups, `writer.exe`, `_retired-gui\`)
