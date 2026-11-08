---
id: px-36ci
status: in_progress
deps: [px-d7s0]
links: []
created: 2026-11-08T22:34:07Z
type: task
priority: 1
assignee: agent
parent: px-0oek
tags: [rust, razer]
---
# Rust 3: Razer Viper protocol (read + write)

Port razer_viper.py with the px-wfee fix (resend on reply cls/cmd mismatch). Then the px-w1a0 features: lighting, brightness, persistent DPI stages, polling. OpenRazer source checked per command.


## Notes

**2026-11-08T23:50:53Z**

Read side done 2026-11-08: viper.rs (open by 90-byte feature report + transaction-id probe, firmware/serial/polling/dpi/dpi_stages, px-wfee retry). Request bytes and stage parsing tested against vectors captured from the Python through a fake hid.dll; 'set-mouse razer' output identical to python razer_viper.py on the real mouse. Remaining: temporary DPI (NOSTORE, for the calibrator), then the writes for px-w1a0 (lighting, persistent stages, polling), each checked against OpenRazer first.
