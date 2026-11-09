# Handoff: hidwright (set-mouse, Mouse Portal, Rust port)

_Written 2026-11-09 ~19:00 by Claude (Claude Code desktop session); updated ~20:30 after the Viper button fix (px-fmim decided; px-oqc3 and px-31s4 closed). Replaces the previous HANDOFF.md (history is in jj). This file is **public** (github.com/Xyde/hidwright): paths use `~` for the user's home folder, and it contains no names, serials or PC names. Keep it that way._

## Resume here

- **What this is:** `hidwright`, a self-built, bloat-free replacement for vendor peripheral software (local folder still named `pulsar-x2a`). `set-mouse` on PATH runs the **Rust** exe (`bin\set-mouse.cmd` → `target\release\set-mouse.exe`, Python fallback). The sign-in loader is Rust too (`bin\set-mouse-startup.exe`). Still Python: the Mouse Portal (`set-mouse portal`) and its rotation calibrator. It configures a Pulsar X2A and a Razer Viper over HID and gives each mouse its own RawAccel rotation. Being ported to Rust + egui in `crates/` (epic `px-0oek`). Goals: no vendor software at all, more peripherals (Xtrfy M42, Dumang DK6), and the user's Mac Studio too.
- **Where it stands:** Rust does everything the Python CLI did, plus full Viper control (DPI stages, polling, logo lighting, brightness). **Razer Synapse is completely removed** (`px-23l1`, closed): no app, service, driver or ghost device entry is left, and the Viper runs on Microsoft drivers only. **Viper buttons fixed (2026-11-09 ~20:00):** its memory had 3 side buttons typing keys (Enter, Right arrow, Left arrow) and the DPI button on "Hypershift". The user chose **direct writes** (`px-fmim`, option B) and factory defaults; all buttons now read factory, the DPI button cycles the 5 stages, the side buttons send back/forward and no keys. `set-mouse razer` lists the buttons; `set-mouse razer bind BUTTON FUNCTION` changes one (Rust). Confirmed after an unplug (persists) and by pressing all four side buttons (`px-31s4`, closed).
- **Next action:**
  1. The user decides `px-ut0y` (keep or remove USBPcap) and pushes (`px-cfcv`; the local changes now include the button work, change `vxkspqpq`).
  2. Continue with `px-01cy` (egui portal) or `px-j08w`. `px-5juk` (which id 04-07 is on which side) only if the user wants different functions per side.
- **Paste into a fresh session:**
  > Read HANDOFF.md in ~\Documents\pulsar-x2a first. Run `jj st`, `tk ready` and `set-mouse` to confirm the repo, the tickets, the mice and RawAccel still match "Current state". Then continue from "Next action". Explain what you're doing as you go, in short, plain sentences.

## Goal and constraints (user's words)

Standing (earlier sessions):
- "i want it that way for everything" / "I haev a razer who I do not want to swap that setting out of": left/right swapped, **only on the Pulsar**.
- "i hate the idea of pulsar fusion -- maybe it has BLOAT … let's see if we can make a BETTER DRIVER or SOFTWARE": our own minimal tool; no vendor software.
- "if I want things to be as latency low…": settings live in mouse memory or the RawAccel driver; nothing of ours sits in the input path.
- "I do not want Viper's rotations on my mouse": one RawAccel rotation per mouse. "the M42 is its own shape and shouldn't rely on other's."
- "have OUR solution to start with the equivalent of launchd": a sign-in task, not the RawAccel GUI.
- "The objective is to see if our app can totally replace what Synapse does! … I'm looking forward to delete Synapse fully" (done 2026-11-09). "It's best that our tool will be as thorough as possible."
- "smallest footprint is which one...?" → "Then egui then": Rust + egui. "I would like to move around between the two machines" (Windows PC + Mac Studio; the user games on the Mac and builds their own Metal + Rust engine).
- "something to manage my peripherals would be... COOL"; Dumang: "some keys like "Layout Toggling" that I would love to see if it's "bindable" to my mouse(s)".
- "OF COURSE we should scrub our names for anonymity!": no personal data in the public repo; things work without editing paths.
- "You choose the name." / "You choose the next path with the least blast radius.": the agent picks, explains, records.

This session (2026-11-09):
- "Go forward: remove Razer fully" (after the Synapse-off-but-drivers-kept state broke the Viper).
- On the button fix: "wouldn't it help? Than rely on an imperfect tool … we are relying on an imperfrection...?" → the user prefers **ground truth** (what Razer itself sends, or what the mouse itself says) over second-hand write-ups. Then: "let's go with USB capture then...!", "what matters is really to find something that writes to on-board memory, huh?", and found the Synapse 3 offline-setups repo.
- Button target: "Factory defaults" (side buttons back/forward on both sides, DPI button cycles stages).
- On USBPcap: "why remove the usbpcap … what if it's useful for the future" → keep/remove is the user's call (`px-ut0y`).
- "make sure that the tickets and etc are properly stored and nothing is forgotten".
- From the user's global instructions (updated 2026-11-09): English is their second language, so **short sentences, plain words, explain a technical word the first time with a small example**; **don't guess what they mean, ask one short question**; **say exactly what to do** (which command, which window, what they should see). They direct and decide; jj only, no git commands; never push without an explicit yes; every to-do goes into `tk`; shells for the user: Nushell first, then PowerShell.

## Decisions

| Decision | Why | Rejected alternative | When |
|---|---|---|---|
| Swap L/R in the Pulsar's own memory (profile 1) | Works everywhere incl. raw-input games, zero latency, other mice untouched | Windows "Primary mouse button" (swaps all mice) | early |
| Own tool, no vendor software | User dislikes bloat; the tool runs ~1 s and exits | Pulsar Fusion; Synapse | early |
| Pulsar ripple off, debounce 1 ms, DPI 3200 single stage | Latency; 1 ms tested chatter-free; matches the Viper's 3200 | 0 ms (bounce) | early |
| Per-mouse RawAccel profiles via `settings.json` `devices[]` | The driver supports it (`DeviceSetup`); only the GUI lacks it | One shared profile | early |
| Sign-in task runs the loader once | RawAccel forgets settings on reboot; no resident process | Always-on portal | early |
| Port to Rust + egui (`px-izgb`) | Smallest footprint; one exe, no Python install | Tauri (WebView2 ~100-200 MB); staying in Python | 2026-11-08 |
| Python stays the reference until each Rust phase matches byte for byte | Safe migration; test vectors come from the Python | Big-bang rewrite | 2026-11-08 |
| Windows HID via `windows-sys` directly; `hidapi` only on macOS | `HidApi::new()` enumerates every device (~150 ms) and can't report `FeatureReportByteLength` | `hidapi` everywhere | 2026-11-08 |
| Repo **public** `github.com/Xyde/hidwright`, MIT | Free Actions incl. macOS runners | Private repo | 2026-11-08/09 |
| X2A global settings are written with the **active profile byte** (`px-337u`) | Byte 0 changes RAM only (lost on unplug); 1-5 also saves. Verified by replug and by a full reboot | Upstream's 0 | 2026-11-09 |
| X2A replies must **echo their request** (category, register, profile byte if set, first payload byte if set), Rust and Python | The mouse keeps its last reply until it has processed the next request; early polls read leftovers (shifted button bindings, firmware `03.09.0B.01`). 936 replies + 8 write-ack kinds fit; afterwards 15/15 status runs matched | Status byte only | 2026-11-09 |
| RawAccel in Rust via `serde_json` + `preserve_order`, output = Python's `json.dumps(indent=2)` + CRLF | A rewrite is byte-identical (checked on the real file) | Hand-written JSON | 2026-11-09 |
| Rust `startup.log` / `dpi_restore.json` live in the **Python install's folder** (`data_dir()` finds `mouse_startup.py` up to 3 folders above the exe), else next to the exe | The Python portal writes `dpi_restore.json` there; one shared log. Temporary until `px-t5i3` | `%LOCALAPPDATA%` (the Python wouldn't see it) | 2026-11-09 |
| PATH shim runs `target\release\set-mouse.exe`; `portal` and a missing exe go to the Python (`px-5xrv`) | No install step, no stale copy; a fresh clone without Rust still works | Copy into `bin\` | 2026-11-09 |
| Sign-in loader = separate **windowed** exe `set-mouse-startup`, task runs a **copy in `bin\`** (`px-yefs`) | No console flashes; a task can't fall back like the shim, so a `cargo clean` must not break sign-in | `conhost --headless`, VBS, S4U task | 2026-11-09 |
| Viper writes from **OpenRazer's byte layouts** (facts only), each round-trip-tested on the mouse (`px-36ci`) | OpenRazer is the best public source; the mouse confirms | Guessing | 2026-11-09 |
| **Remove Synapse completely**, incl. its drivers (`px-23l1`) | "Synapse off, drivers kept" broke the Viper (Code 10); the Razer driver `RzDev_0078` sat as a filter in the Viper's input path | Keep Synapse; keep its drivers | 2026-11-09 |
| Find the button bytes by **USB capture of Razer's own software**, not by decompiling Synapse | Exact bytes for this Viper; decompiling = days of work and a licence problem for a public repo | Ghidra/ReVa on Synapse | 2026-11-09 |
| Button target: **factory defaults** | User's choice | Right-side buttons disabled; custom | 2026-11-09 |
| Fix the buttons by **direct writes** (`px-fmim` B); press tests in our own topmost window, no Notepad | Fewest manual steps for the user; the mouse's read-back is ground truth too; nothing installed | Synapse 3 + USB capture (A) | 2026-11-09 |
| `bind` offers only functions read from or tested on this Viper (mouse buttons, dpi-cycle, default) | Repo rule: every write checked on the real mouse | Keys, `disabled`, razerqdhid's full list | 2026-11-09 |
| Gemini's uninstall scripts kept on disk but **out of the repo** (`.gitignore`, scrubbed from unpushed history) | They contain this PC's user path; repo is public | Committing them | 2026-11-09 |
| Rust lists the Xtrfy M42 in `rotation` (`m42`, `xtrfy`) | It has its own RawAccel profile | Python parity | 2026-11-09 |

### Reversals

- **Was** (old Decisions row): "for the Viper, Synapse only adds spectrum lighting; no remaps". **Now:** Synapse ran the Viper in **driver mode** (`00/04 [03 00]`), handled the buttons on the PC, and animated the logo in software (`0F/03` frames ~25/s). The mouse's **own** memory holds key remaps and a Hypershift DPI button. **Because:** button-table reads + USB capture, 2026-11-09.
- **Was:** stage 1 of the Synapse trial = app off, drivers kept. **Now:** remove everything. **Because:** the Viper's main interface failed to start (Code 10, `0xC0000120`) without Synapse.
- **Was:** "Don't add a Viper write without checking it against OpenRazer". **Now:** button writes are **not in OpenRazer**; the accepted sources are razerqdhid's source code + read-back from this Viper (or a capture of Razer's software, `px-fmim`). Every other Viper write still follows OpenRazer.
- **Was:** "a Synapse capture will give us the button bytes". **Now:** Synapse 4 never sends `02/xx`; Synapse 3 is off Razer's site but mirrored (`px-fmim`); Synapse 2 does not support the Viper.
- **Was:** Viper stages are 1-based. **Now:** the mouse *reports* 1-based, Razer *writes* 0-based; our 1-based writes also work (`px-j08w`).
- Older: RawAccel per-mouse profiles exist; Tauri → egui; `hidapi` → `windows-sys` on Windows (stale trace: `px-0oek` description still says "'windows' crate"); X2A globals with profile byte 0 → active profile; RawAccel path auto-detected.

## Current state

Each item is **verified** (checked 2026-11-09 18:30-19:00; says how) or **reported**.

- Pulsar X2A: profile 1, **L/R swapped**, 1000 Hz, debounce 1 ms, ripple off, angle-snap off, motion-sync off, DPI `[3200]`, lift-off 1 mm; settings survived a full reboot (**verified**: `set-mouse`, boot 17:34)
- Razer Viper: firmware v1.04, 1000 Hz, current DPI 3200, stages `400 800 1800 2400 [3200]`, logo 70% brightness, spectrum effect kept without Synapse, device mode **normal** (**verified**: `set-mouse razer`; effect reported by the user)
- Viper button table (profile 1, normal layer): all **factory**: `01` left, `02` right, `03` middle, `04` back, `05` forward, `06` back, `07` forward, `0e` dpi-cycle, `09`/`0a` wheel (**verified**: `set-mouse razer`, 2026-11-09 ~20:00). Was: `04` Enter, `05` Right arrow, `07` Left arrow, `0e` Hypershift (backup: `~\AppData\Local\Temp\hidwright-diag\viper_buttons_backup.json`; `viper_buttons_write.py --restore` writes it back). Hypershift layer: `04` back, `05` forward, `06` back, `07` forward. Press test (**verified**, Raw Input): DPI button cycles all 5 stages with wrap; front = forward and back = back on **both** sides (a first 'right front = back' was a left button pressed by mistake, user confirmed). Survives an unplug of 5 s (**verified**: `set-mouse razer` after replug). Which id is on which side: unknown (`px-5juk`)
- Razer software: none. No process, service, driver, `RzCommon.sys`/`RzDev_0078.sys`, driver package or ghost Viper node (**verified**: `Get-Process`, `Win32_Service`, `pnputil /enum-drivers`, `Get-PnpDevice`). Mouse class filters `rawaccel,mouclass`
- USBPcap 1.5.4.0: installed, idle; USB class upper filter `USBPcap` (**verified**). Decision `px-ut0y`
- Xtrfy M42: plugged in (**verified**); not configurable yet
- RawAccel: profiles `default` -2.6 (Viper and unlisted), `Pulsar X2A` -0.5, `Xtrfy M42` 0.0; `devices[]` = Pulsar, M42; 3 backups; driver Running (**verified**: `settings.json`, `Win32_SystemDriver`)
- Sign-in task `Mouse Portal - load RawAccel`: runs `~\Documents\pulsar-x2a\bin\set-mouse-startup.exe`; after the real reboot it ran 17:34:52, result 0; `startup.log` "17:34:58  RawAccel settings loaded (default: -2.6 deg, Pulsar X2A: -0.5 deg, Xtrfy M42: 0.0 deg)" (**verified**)
- `set-mouse` on PATH = Rust exe; portal not running; no `dpi_restore.json`; user PATH has the project `bin` (**verified**)
- Leftovers outside the repo: capture files and scripts in `~\AppData\Local\Temp\hidwright-diag\`; Razer installers in Downloads (`px-c0lr`). Git-ignored in the repo folder: `uninstall_official_razer.cmd`/`.ps1` (another tool's scripts) (**verified**: listing)

Version control: jj, colocated. `main` = `main@origin` = `umuyrvut` (pushed by the user). Local, not pushed: `rmvxlwpw` (Synapse removed), `lyyzwpwo` (replug result), and the handoff change on top (this file, tickets, `.gitignore`). Unpushed diff scanned: 0 personal-data hits. Tests: `cargo test` → **34 passed** (after the button work, ~20:00); clippy 0 problems; fmt ok (**verified**). CI on the last push: success (`42701312644`). Exes: `set-mouse.exe` 529,408 bytes, `set-mouse-startup.exe` 331,264 bytes (copy in `bin\` identical size).

## Specs and reference values

| Thing | Value | Source |
|---|---|---|
| Pulsar X2A wired | USB `3710:1404`, config interface `MI_03`, feature report 0, 64 bytes | `pulsar_x2a.py`; `crates/mouse-core/src/x2a.rs` |
| X2A packet | `[1]` cat, `[2]` reg (bit 7 = read), `[3]` sub, `[6]` profile (global **writes** need the active profile, 0 = RAM only), `[7..62]` payload, `[62..64]` LE u16 sum | upstream `packerlschupfer/pulsar-mouse-linux` (MIT) |
| X2A reply | byte 0: `00` not answered, `01` answered, `02` pending, `03` refused. Echo: `[1]` cat, `[2]` reg, `[6]` profile (per-profile reads), `[7]` button id; write acks echo the packet; DPI read sub `15` answers as `21` | measured; `answers()` |
| X2A commands | button `04/01/06`; polling `01/09/02` (125:1 250:2 500:4 1000:8); debounce `04/03/03`; angle-snap `07/04/02`; ripple `07/03/02`; motion-sync `07/05/02`; DPI write `05/04/21`, read `05/04/15`; stage `05/01/02`; lift-off `07/02/03`; active profile `02/06/01`; firmware `01/07/04` | upstream |
| Viper frame | USB `1532:0078`, 90-byte feature report: `[0]` status, `[1]` transaction id, `[5]` size, `[6]` class, `[7]` cmd (bit 7 = get), `[8..88]` args, `[88]` XOR of bytes 2..87. Status `00` new `01` busy `02` ok `03` failed `04` timeout `05` not supported | OpenRazer `razercommon.h/.c` |
| Viper transaction id | OpenRazer: `0x3F` lighting, `0xFF` other; razerqdhid: `0x1F`; Synapse 4 counts up `0x00..`; this Viper accepts any | measured, capture |
| Viper OpenRazer commands | stages write `04/06` size `0x26` [store, active, count, (index, x hi, x lo, y hi, y lo, 0, 0)×n], read `04/86`; polling `00/05` [1000→01, 500→02, 125→08]; brightness `0F/04` [01, 04, 0-255], read `0F/84`; logo effect `0F/02` [01, 04, effect…] (off 00, static 01, breathing 02, spectrum 03, reactive 05); device mode `00/04` [00 normal / 03 driver, 00], read `00/84`; temporary DPI `04/05` [00, x, y] | OpenRazer `razerchromacommon.c` |
| **Viper button table** | list `02/84` → `0a 01 02 03 04 05 06 07 0e 09 0a`; read `02/8C` size `0x0A` args [profile 01, button, layer 00/01]; reply [profile, button, flag, **class, length, value…**]; write `02/0C` same layout. Classes: `00` disabled, `01` mouse (value `01` L, `02` R, `03` M, `04` back, `05` forward, `09`/`0a` wheel), `02` keyboard (modifier, HID key: `28` Enter, `4f` Right, `50` Left), `06` DPI (`06` = next with wrap), `0c` Hypershift. Profile 2 = empty (one slot); profile 0 = "direct" (temporary per razerqdhid) | reads from this Viper; razerqdhid `public/py/qdrazer/device.py`, `protocol.py` (MIT) |
| Planned button writes | `04` → `01 01 04`, `05` → `01 01 05`, `07` → `01 01 05`, `0e` → `06 01 06`. Undo: `04` `02 02 00 28`, `05` `02 02 00 4f`, `07` `02 02 00 50`, `0e` `0c 01 01` | `px-oqc3` |
| Synapse 4 capture | 4259 SET_REPORTs, **no `02/xx`**; `00/04 [03 00]`; `0F/02 [00 00 08]` then 4219 × `0F/03` frames; `04/06` with 0-based indexes; `07/01` and `06/8e` → status `05` | USB capture 2026-11-09 |
| Xtrfy M42 | USB `2EA8:2203`; `MI_01` col03 vendor page `0xFF00`, 64-byte in/out, no feature report | `HidP_GetCaps` |
| Dumang DK6 | USB `0483:5710`; `BoardSyncPacket` layout in `px-k3jg` | `mayanez/dumang-keyboard-ctrl` (GPL: facts only) |
| RawAccel device ids | Pulsar `HID\VID_3710&PID_1404&MI_00`, Viper `HID\VID_1532&PID_0078&MI_00`, M42 `HID\VID_2EA8&PID_2203&MI_00` (case-sensitive) | `rawinput.rs` |
| RawAccel profile choice | matching `devices[].id` → its profile; else `profiles[0]` | RawAccel driver source |
| Portal | `http://127.0.0.1:8642` | `mouse_portal.py` |
| Toolchain | rustc 1.99.1, MSVC Build Tools 2022, Python 3.15.2, `windows-sys` 0.61, `serde_json` 1 | `rustc --version` |

## How to run and verify

Nushell or PowerShell, from `~\Documents\pulsar-x2a`:

```
set-mouse
set-mouse razer
set-mouse rotation
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
jj st
tk ready
gh run list --repo Xyde/hidwright --limit 3
```

Read the Viper's button table (read-only; the script lives outside the repo and may be gone, rebuild it from the "Viper button table" spec if so):

```
python ~\AppData\Local\Temp\hidwright-diag\viper_buttons_all.py
```

Sign-in task (PowerShell): `Get-ScheduledTaskInfo -TaskName "Mouse Portal - load RawAccel"`.

Test vectors: a throwaway Python script calls the Python packet builders and prints hex (scratchpad, never the repo). Device-side tests: same-value write first, then a real change, read back, restore.

## Environment and gotchas

- **Public repo.** No user name, PC name, email or serials in tracked files or commit messages. Other tools (Gemini) and other Claude sessions also write into this folder; jj snapshots **every new file**, so check `jj st` and scan `jj diff --from main@origin --to @ --git` before describing or pushing.
- **Viper press tests:** the side buttons no longer type keys. Don't send the user to Notepad (they asked to avoid it): run `python ~\AppData\Local\Temp\hidwright-diag\viper_press_guided.py ["RIGHT side, FRONT button" ...]`, a topmost window that names each button and shows what it sent. Mouse button messages go to the window **under the pointer**. In ctypes, `RAWMOUSE` needs 2 padding bytes after `usFlags` (button flags at offset 4); without them every press reads as movement (cost one round on 2026-11-09).
- **Claude Code's classifier blocks** `jj git push` to this public repo, changing scheduled tasks, and stopping processes by command line. The user runs those; give exact commands.
- **USBPcap:** opening `\\.\USBPcapN` needs admin, so the user starts captures (admin PowerShell `Start-Process … USBPcapCMD.exe -d \\.\USBPcapN -o … -A`); add `--inject-descriptors` to get VID/PID in the file; files are locked while capturing; stop with `Stop-Process -Name USBPcapCMD`. Without descriptors, find the Viper by its 90-byte control transfers (`wLength = 0x5a`). Capture files contain the mouse serial.
- **Razer leftovers:** driver files owned by TrustedInstaller need `takeown /f <file> /a` + `icacls <file> /grant Administrators:F` before deleting. Ghost device entries (devices not connected now) are missed by "present only" checks; list with `Get-PnpDevice` and `-not $_.Present`, remove with `pnputil /remove-device`.
- **The task has no run-as password, on purpose** (the account has none; "run only when logged on"). `schtasks /Change` asks anyway: leave it empty.
- **This Bash tool eats backslashes** in inline commands (`\r` → `r`, `\\b` → backspace). Write Python scripts to a file for anything with backslashes.
- **Windows PowerShell 5.1 strips inner double quotes** passed to native programs (`nu -c '…"…"…'` broke); use a script file.
- **The scratchpad folder changes when a session restarts**; long-lived diagnostics are in `~\AppData\Local\Temp\hidwright-diag\`.
- **Testing RawAccel writes without touching the driver:** copy `settings.json` into a scratch folder with a fake `writer.exe` (a tiny Rust program that logs its arguments and exits with `FAKE_WRITER_EXIT`) and point `RAWACCEL_DIR` at it. Both versions honour `RAWACCEL_DIR`.
- **CI's Windows runner checks text files out with CRLF.** Tests that compare files must normalise line ends (the RawAccel fixture tests do; CI run `42692951681` failed before that).
- jj: pushed commits are immutable; an empty undescribed change is abandoned on `jj edit`; old revisions don't ignore `target/` (park it before `jj edit`).
- `set-mouse` isn't found in Git Bash (`.cmd` shim); use `target/release/set-mouse.exe`.
- Python prints CRLF, Rust LF (`diff --strip-trailing-cr`).
- `tk` has no assign command (edit `assignee:`); for long text use `bash ~/bin/ticket/ticket`.
- Still true from before: RawAccel forgets settings on reboot; Raw Input sees movement after RawAccel; `set-mouse.cmd` + Ctrl+C asks "Terminate batch job"; one portal at a time; `python -I` breaks sibling imports.
- The handoff checker flags upstream paths and the `\\.\USBPcapN` device names; those aren't repo files.

## Open threads

| Thread | Ticket | Owner | Blocked on |
|---|---|---|---|
| Viper: which side button id (04-07) is where (only for per-side functions) | `px-5juk` | agent | user wish; needs 2 presses |
| Viper stage indexes 0-based like Razer | `px-j08w` | agent | nothing |
| Keep or remove USBPcap | `px-ut0y` | user | user decision |
| Clean up capture files (serial) and installers | `px-c0lr` | user | nothing |
| Push the local changes | `px-cfcv` | user | user's go |
| Viper full control: replug proof of temporary DPI, device-mode command, portal UI | `px-w1a0` | agent | `px-oqc3`, `px-01cy` |
| Rust port (epic) | `px-0oek` | agent | — |
| Rust 5: egui portal | `px-01cy` | agent | nothing (deps closed) |
| Rust 6: egui calibrator | `px-xpnz` | agent | `px-01cy` |
| Rust 7: retire the Python | `px-t5i3` | agent | `px-xpnz` |
| macOS build + test on the Mac Studio | `px-e6p0` | agent, on the Mac | a Mac session |
| Rotation on macOS / own game engine | `px-sk2o` | user | user decision |
| Dumang DK6 layer toggle from a mouse button | `px-k3jg` | agent | test on the real DK6 |
| Xtrfy M42: software / protocol | `px-8mzp` | agent | finding the software |
| Viper own RawAccel profile, `default` → 0° | `px-xtv5` | user | user decision |
| Swap L/R on Pulsar profiles 2-5? | `px-tkar` | user | user decision |
| Re-run calibrator on the Pulsar (expect ~0°) | `px-vtbh` | user | nothing |
| Acceleration/sensitivity editing? | `px-xykv` | user | user decision |
| Exe size 529 KB vs `opt-level = "z"` | `px-rwty` | user | user decision |
| X2A lift-off encoding | `px-zd1z` | agent | low priority |

## Unknowns and unverified

- **Which physical side button is `04`/`05`/`06`/`07`.** By function: `{04,06}` are the back buttons, `{05,07}` the front ones; the side of each id is unknown (`px-5juk`).
- **Viper temporary DPI (NOSTORE) forgotten on replug?** Not proven (`px-w1a0`).
- Synapse commands seen but not understood: `0b/03 [00 04 00]`, `00/b3`, `00/87 [01 04 …]`, `0F/80 [04 19 01 01 01]`.
- When the X2A answers late (leftover replies): bursts seen, cause untested; the echo check handles it.
- macOS with real mice (`px-e6p0`); Dumang ID/layer meaning (`px-k3jg`); M42 protocol (`px-8mzp`); X2A lift-off encoding (`px-zd1z`).

## Don't

- Don't put personal data in the repo; don't push without the user's explicit yes; don't rewrite pushed history. Never commit the capture files or `uninstall_official_razer.*`.
- Don't write a Viper button without its original bytes at hand (`set-mouse razer` shows them), and don't use profile 0 for lasting changes. Don't run razerqdhid's flash reset (`06/0A`): effects on the Viper are unknown.
- Don't reinstall Synapse 4 hoping it fixes the buttons: it never writes them, and it brings its drivers and filters back. Any temporary Razer install must be removed the careful way afterwards (ghost nodes, services, `.sys`).
- Don't leave Razer drivers installed without Synapse (Viper fails with Code 10). Don't delete a filter driver's `.sys` while a device still lists it (Code 39).
- Don't use Windows' "Primary mouse button" swap. Don't install Pulsar Fusion except, with the user's OK, briefly for a capture.
- Don't copy code from `dumang-keyboard-ctrl` (GPL-3.0). Don't set Viper device mode `02`.
- Don't send `portal` to the Rust exe before `px-01cy`; don't delete the Python before `px-t5i3` (also the shim's fallback). Don't uninstall the RawAccel driver or delete `writer.exe`, `wrapper.dll`, `Newtonsoft.Json.dll`, `_retired-gui`.
- Don't hand-edit `settings.json` `profiles[0]` thinking it's the Pulsar's (it's `default`); use `set-mouse rotation`.
- Don't set debounce 0 ms; don't promise per-DPI rotation; don't start a second portal. Don't run git commands; don't use `HidApi::new()` on Windows.

## Where things live

- `README.md` (public usage), `LICENSE` (MIT), `CLAUDE.md` (orientation; points here), `Cargo.toml` (workspace, release profile).
- `crates/mouse-core/src/`: `hid/` (Windows + macOS HID), `x2a.rs` (Pulsar reads/writes, echo check), `viper.rs` (Viper reads/writes, `Setting`, `Logo`, `Rgb`), `rawaccel.rs`, `rawinput.rs`, `dpi_restore.rs`, `lib.rs`. Fixtures: `crates/mouse-core/tests/fixtures/rawaccel/`.
- `crates/set-mouse/src/main.rs` (CLI; checks input before opening a mouse; prints with `say!`, which exits quietly when piped into `head` instead of panicking), `crates/set-mouse/src/bin/set-mouse-startup.rs` (sign-in loader), `crates/set-mouse/src/shared.rs` (`data_dir()`, log writer).
- `bin/set-mouse.cmd` (PATH shim); `bin/set-mouse-startup.exe` (git-ignored copy for the task; refresh: `cp target/release/set-mouse-startup.exe bin/`).
- `task-backup-python.local.xml` (git-ignored: the task as it was, for rollback); `uninstall_official_razer.cmd`/`.ps1` (git-ignored, another tool's scripts).
- Python (portal, fallback): `set_mouse.py`, `pulsar_x2a.py`, `razer_viper.py`, `rawinput_monitor.py`, `rawaccel_cfg.py`, `test_dpi.py`, `mouse_portal.py` + `mouse_portal.html`, `mouse_startup.py` (old loader, kept for rollback).
- `.tickets/` (`tk`), `.github/workflows/ci.yml` (CI).
- Outside the repo: RawAccel in `~\Downloads\RawAccel_v1.7.1\RawAccel\`; diagnostics and captures in `~\AppData\Local\Temp\hidwright-diag\` (`viper_listen.py` Raw Input listener, `viper_buttons_all.py` button reader, `usbpcap_read.py` USBPcap file reader without Wireshark, `viper_cmds.py` decodes the Viper's commands from `usbcap\hub2.pcap`; Windows may clean `Temp`, so rebuild from the specs if gone); Claude memory in `~\.claude\projects\<this project's folder>\memory\`.
