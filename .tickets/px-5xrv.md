---
id: px-5xrv
status: open
deps: [px-d7s0]
links: []
created: 2026-11-08T22:34:07Z
type: task
priority: 1
assignee: agent
parent: px-0oek
tags: [rust, pulsar, rawaccel]
---
# Rust 2: X2A writes + RawAccel cfg + CLI parity

All Pulsar write commands, rawaccel_cfg (backup, per-mouse profiles, writer.exe), rotation/reload commands, test_dpi restore. CLI matches 'set-mouse help'. Switch bin/set-mouse.cmd to the Rust exe once parity is confirmed.


## Notes

**2026-11-09T19:23:57Z**

2026-11-09 part a DONE (X2A writes). mouse-core x2a.rs: Setting enum (Polling, Debounce, Toggle, DpiStages, DpiStage, Lod, Button) with validation (Error::Input) and packet(); X2a::write picks the profile byte (globals: active profile, so they're saved, see px-337u); transact() shares the 0x00-resend with reads (Python's cmd lacks it); Binding::parse = Python parse_function (media(>0xFFFF) refused instead of crashing). CLI: swap/unswap/poll/debounce/angle-snap/ripple/motion-sync/dpi/stage/lod/bind/profile, -p and --active like argparse, messages identical to Python. Verified: 15 tests (write packets + parse vectors generated from the Python with active_profile pinned to 3); 12 same-value commands gave identical Python/Rust output on the mouse, info before == after; real round trips debounce 1->2->1 and profile-5 thumb1 back->f5->default, read back by the Python. lod NOT hardware-tested (px-zd1z). Next: part b RawAccel cfg + rotation/reload + test_dpi restore; part c PATH shim switch.
