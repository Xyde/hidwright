# hidwright (local folder: pulsar-x2a)

**Read `HANDOFF.md` first.** It has the current state, the decisions and their reasons, the things not to do, and the next step. Open work lives in `tk` tickets (`tk ready`). Version control is jj; remote `origin` = github.com/Xyde/hidwright (**public**: no personal data in tracked files; push only with the user's explicit yes).

Self-built, bloat-free peripheral tooling. In daily use: `set-mouse` (CLI on the user PATH via `bin/set-mouse.cmd`, which runs the Rust `target\release\set-mouse.exe` and falls back to the Python) and the Mouse Portal (`set-mouse portal`, still Python, http://127.0.0.1:8642). The sign-in loader is still Python too. It configures a Pulsar X2A over HID, reads a Razer Viper, and gives each mouse its own RawAccel rotation. Being ported to Rust + egui in `crates/` (Windows + macOS). The user cares about input latency and dislikes vendor bloatware.
