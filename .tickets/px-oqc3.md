---
id: px-oqc3
status: open
deps: [px-fmim]
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
