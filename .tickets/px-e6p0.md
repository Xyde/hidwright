---
id: px-e6p0
status: open
deps: [px-gnry]
links: [px-sk2o]
created: 2026-11-08T22:43:49Z
type: task
priority: 2
assignee: agent
parent: px-0oek
tags: [macos]
---
# macOS: build and test set-mouse on the Mac Studio

User 2026-11-08 wants to move between the Windows PC and a Mac Studio. mouse-core's HID layer has a macOS backend (hidapi, shared open) that has never been compiled. On the Mac: install Rust, cargo build, run set-mouse info with the X2A plugged in. Check: interface numbers reported? X2A config interface opens? Then the Viper (Razer feature reports on the mouse interface; razer-macos uses USB control transfers instead, so this may need a different route). Raw-input calibrator on macOS = IOHIDManager (needs Input Monitoring permission). Blocked on: code reaching the Mac (see the remote ticket).

