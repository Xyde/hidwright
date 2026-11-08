---
id: px-0oek
status: open
deps: []
links: []
created: 2026-11-08T22:34:06Z
type: epic
priority: 1
assignee: agent
tags: [rust]
---
# Port to Rust + egui (epic)

Decision px-izgb. Cargo workspace at the repo root next to the Python (Python stays in place and is the reference until each part matches; the sign-in task and bin/set-mouse.cmd keep pointing at Python until their phase switches them).
Crates: core lib 'mouse-core' (Windows HID, X2A, Viper, RawAccel cfg, Raw Input, swipe analysis), bin 'set-mouse' (CLI + sign-in loader), bin 'mouse-portal' (eframe/egui).
Dependencies kept small: 'windows' crate (Microsoft's bindings; the same hid.dll/user32 calls the ctypes code makes, no C toolchain), serde + serde_json (RawAccel settings.json), eframe with the glow (OpenGL) backend; tiny arg parsing (lexopt or hand-rolled), not clap.
Parity rule: packet builders unit-tested against bytes produced by the Python; read output compared on the real mice before any Rust write is used.


## Notes

**2026-11-08T22:50:02Z**

Scope widening (user 2026-11-08): 'something to manage my peripherals would be COOL': more mice (XTRFY M42, px-8mzp) and keyboards (Dumang DK6, px-k3jg). Keep mouse-core's device modules independent so a device is just another module; consider renaming the crate/CLI later (user decides).
