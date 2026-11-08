# pulsar-x2a

**Read `HANDOFF.md` first.** It has the current state, the decisions and their reasons, the things not to do, and the next step. Open work lives in `tk` tickets (`tk ready`). Version control is jj (no remote).

Self-built, bloat-free mouse tooling for Windows: `set-mouse` (CLI on the user PATH via `bin/set-mouse.cmd`) and the Mouse Portal (`set-mouse portal`, http://127.0.0.1:8642). It configures a Pulsar X2A over HID, reads a Razer Viper, and gives each mouse its own RawAccel rotation. Pure Python + ctypes, no pip dependencies. The user cares about input latency and dislikes vendor bloatware.
