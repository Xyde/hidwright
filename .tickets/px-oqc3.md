---
id: px-oqc3
status: closed
deps: [px-fmim, px-31s4]
links: []
created: 2026-11-09T23:19:26Z
type: bug
priority: 1
assignee: agent
parent: px-w1a0
tags: [viper, buttons]
---
# Viper on-board buttons: restore factory functions (3 side buttons send keys, DPI button = Hypershift)

Found 2026-11-09 (px-23l1 notes have the full story). The Viper's own memory (profile 1, the only slot) holds: button 04 = keyboard Enter, 05 = Right arrow, 07 = Left arrow, 0e (DPI button, underside) = Hypershift (class 0x0C), so the DPI button never changes DPI and side presses type keys into the focused window. Synapse hid this (driver mode, host-side buttons); Synapse 4 never writes the on-board table (USB capture: no 02/xx command), Synapse 3 is no longer downloadable, Synapse 2 does not support the Viper. User target: factory defaults. Plan: write with 02/0C (data size 0x0A, args [profile 01, button, hypershift 00, class, length, value...]): 04 -> 01 01 04 (back), 05 -> 01 01 05 (forward), 07 -> 01 01 05 (forward), 0e -> 06 01 06 (DPI next with wrap). Order: first rewrite 06 with its own value (01 01 04) to prove the command; then one button at a time: write, read back with 02/8C, user presses it once in Notepad (focus Notepad with the Pulsar first). Originals for rollback: 04 '02 02 00 28', 05 '02 02 00 4f', 07 '02 02 00 50', 0e '0c 01 01' (class, length, value). Layout confirmed by razerqdhid source code (MIT, public/py/qdrazer/device.py set_button_function/get_button_function) and by every read from this Viper. Hypershift layer (byte 3 = 01) already holds back/forward. Then add 'set-mouse razer bind BUTTON FUNCTION' (and a read-out of the table in 'set-mouse razer'), with tests from the bytes above.


## Notes

**2026-11-09T23:21:21Z**

2026-11-09: approach not yet approved by the user (target 'factory defaults' was chosen). Alternative being considered: px-fmim (Synapse 3 offline setups + USB capture).

**2026-11-09T23:41:09Z**

2026-11-09: writes done with ~/AppData/Local/Temp/hidwright-diag/viper_buttons_write.py (backup + --restore + --dry-run). Same-value write of 06 left the full table (10 buttons x 2 layers) unchanged; then 04 -> 01 01 04, 05 -> 01 01 05, 07 -> 01 01 05, 0e -> 06 01 06, each read back exact and nothing else changed. Backup of originals: viper_buttons_backup.json in the same folder. Press test without Notepad: viper_press_test.py (topmost tk window + Raw Input; mouse buttons go to the window under the pointer). Pending: press test result, persistence after replug, then 'set-mouse razer bind' in Rust.

**2026-11-09T23:50:15Z**

2026-11-09: press test results. Bug found in my own test scripts (RAWMOUSE missing the 2 padding bytes after usFlags, so button flags read 0; fixed in viper_press_test.py and viper_listen.py; repo code was fine). DPI button: cycles all 5 stages with wrap (seen twice), then set back to stage 5. Side buttons no longer type keys. Guided test: left front = forward, left back = back, right back = back, right front = back (probably a mispress, see px-31s4). Remaining: px-31s4 (user), then the Rust bind command.

**2026-11-09T23:53:15Z**

2026-11-09: Rust done. viper.rs: Binding (raw class + value, describe, parse, factory), BUTTONS ids/labels, Setting::Button (02/0C, TID 0xFF), Viper::button/buttons (02/8C, 02/84), Info.buttons. CLI: 'set-mouse razer' lists buttons with '<- changed' marks; 'set-mouse razer bind BUTTON FUNCTION' (buttons left right middle side04-side07 dpi wheel-up wheel-down; functions left right middle back forward wheel-up wheel-down dpi-cycle default) writes and reads back. Only functions read from or tested on this Viper are offered (no keys, no 'disabled' yet). Tests: 34 pass (3 new from today's real frames and replies); clippy 0; fmt ok. On the mouse: same-value write side06 default ok; real change side07 back -> read back changed -> side07 default ok. All buttons read factory. Close this when px-31s4 is done (side names get physical names then).

**2026-11-10T00:02:52Z**

2026-11-09: closed. Persistence and all four side buttons confirmed (px-31s4). Which id (04-07) sits on which side stays unknown; not needed for use (both sides are back/forward), moved to a P4 ticket.
