---
id: px-rwty
status: open
deps: []
links: []
created: 2026-11-09T19:51:56Z
type: task
priority: 4
assignee: user
tags: [rust, footprint]
---
# Decide: set-mouse.exe size, 500 KB now; opt-level z gives 410 KB

With RawAccel support (px-5xrv part b) the release exe grew from 211 KB to 500 KB. Measured with a linker map (2026-11-09): std::process + threads ~65 KB (running writer.exe), serde_json ~50 KB (settings.json), float parse/print ~25 KB (rotation degrees), new logic ~90 KB. opt-level="z" gives 410 KB (opt-level="s" 421 KB) at some speed cost, which doesn't matter for set-mouse (all I/O-bound) but may for the egui portal later (a per-package profile override can limit it to set-mouse). Cutting further: CreateProcessW by hand (~-40 KB, more unsafe) or a hand-written JSON reader (~-40 KB, more risk); agent recommends neither. Options: (a) keep as is (agent's default), (b) opt-level z for the set-mouse package only.

