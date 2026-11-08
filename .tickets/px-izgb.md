---
id: px-izgb
status: closed
deps: []
links: [px-w1a0]
created: 2026-11-08T22:30:51Z
type: task
priority: 1
assignee: user
tags: [rust, architecture]
---
# Decide: port set-mouse + portal to Rust? Which UI?

User asked 2026-11-08 whether the tool would be better in Rust and whether Tauri or wgpu is needed.
Agent's view: no latency gain (nothing of ours is in the input path: settings live in mouse firmware and the RawAccel driver). Real gains: one .exe instead of Python + ctypes (the sign-in task no longer depends on a Python install); start-up in ms instead of ~1 s; typed packet builders with unit tests; the calibrator can read Raw Input in the same process as the drawing (no SSE hop).
Size: ~1750 lines of Python + 713 lines of HTML/JS. Rust 1.99.1 is installed.
UI options: (a) Tauri 2: reuses mouse_portal.html almost as is, native window via the WebView2 already on Win11 (recommended); (b) egui/eframe: pure Rust, single small exe, UI rewritten; (c) keep a local web page served by a tiny Rust HTTP server (same UX as today). wgpu is a GPU drawing layer, not a UI toolkit: not needed. WASM is not needed either (Tauri's Rust side does the maths).
Suggested shape: Cargo workspace with core lib (X2A, Viper, RawAccel cfg, raw input) + set-mouse CLI + portal. Keep the Python as the reference until each part matches byte for byte.
Order question: build the Viper features (px-w1a0) in Rust directly, rather than in Python and again later.


## Notes

**2026-11-08T22:34:05Z**

Decided 2026-11-08 by user: Rust + egui ('Then egui then'), chosen for smallest footprint (single exe, no WebView2 processes). Port plan: epic and phase tickets linked below.
