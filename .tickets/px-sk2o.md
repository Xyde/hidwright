---
id: px-sk2o
status: open
deps: []
links: [px-e6p0]
created: 2026-11-08T22:43:49Z
type: task
priority: 2
assignee: user
tags: [macos, rotation]
---
# Decide: rotation correction on macOS (no RawAccel there)

RawAccel is a Windows kernel driver; macOS has no equivalent we can feed per-mouse rotation. Firmware settings (Pulsar swap/DPI/debounce, Viper DPI/lighting) travel with the mouse and work on the Mac with no software. Rotation options on Mac: (a) none (accept the raw angle); (b) a CGEventTap that rotates pointer movement: sits in the input path (adds latency) and games that read HID directly ignore it; (c) a DriverKit HID driver: proper, but needs Apple developer signing/entitlements; big project. Question for the user: what do you use the Mac for (games? work?), and does the -0.5 / -2.6 deg matter there?


## Notes

**2026-11-08T22:50:01Z**

User 2026-11-08: the Mac IS for gaming, and they're building their own FPS game with a custom Metal + Rust engine there. So for their own game, rotation (and anything else per mouse) can live inside the engine: it already reads raw deltas, and a 2x2 rotation there costs nothing and adds no latency. Proposal: our tool keeps a per-mouse profile (VID:PID or serial → rotation, maybe sensitivity/DPI normalisation) in a small shared file plus a tiny Rust crate the engine depends on to read it (IOHIDManager gives VID/PID per device on macOS). Other Mac games: still no clean option (CGEventTap only affects the cursor; games reading HID/GCMouse bypass it). Open: the user decides whether to do the engine integration.
