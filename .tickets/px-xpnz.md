---
id: px-xpnz
status: open
deps: [px-01cy]
links: []
created: 2026-11-08T22:34:08Z
type: task
priority: 2
assignee: agent
parent: px-0oek
tags: [rust, egui, calibrator]
---
# Rust 6: egui calibrator (in-process Raw Input)

Full-screen, keyboard-only, auto-finish after N swipes, test-at-DPI with crash-safe restore, lock onto first mouse >100 counts, physical-distance thresholds. Raw Input read in the same process as drawing (no SSE). Port analyze() and check it against the Python on the same synthetic swipes (angle 1.91 deg, 11 swipes, 10 completed).

