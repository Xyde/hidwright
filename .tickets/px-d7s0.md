---
id: px-d7s0
status: closed
deps: []
links: []
created: 2026-11-08T22:34:07Z
type: task
priority: 1
assignee: agent
parent: px-0oek
tags: [rust, pulsar]
---
# Rust 1: workspace + HID plumbing + X2A read-only

Cargo workspace, mouse-core with HID enumeration/open/feature reports via the windows crate, X2A packet build + checksum (tests vs Python bytes), all X2A reads. 'set-mouse' prints the Pulsar block identical to the Python status.


## Notes

**2026-11-08T22:43:48Z**

Done 2026-11-08. Workspace (Cargo.toml, crates/mouse-core, crates/set-mouse). HID: own Win32 backend (windows-sys 0.61; SetupAPI paths + hid.dll, checks FeatureReportByteLength on open), hidapi only on macOS (untested). hidapi was dropped on Windows because HidApi::new() reads every HID device's strings/descriptors (~150 ms, twice per open); new_without_enumerate doesn't help on Windows. X2A read side ported; 7 unit tests (packets + describe() vs Python vectors). target/release/set-mouse.exe info [-p N] output identical to python pulsar_x2a.py info for profiles 1, 2, 5 (only CRLF differs). Timing: Rust ~110 ms vs Python ~178 ms; of that ~100 ms is the 20 mouse round trips; find+open 2.5 ms. Release exe 169 KB.
