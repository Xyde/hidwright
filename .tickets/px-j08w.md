---
id: px-j08w
status: open
deps: []
links: []
created: 2026-11-09T23:19:26Z
type: task
priority: 3
assignee: agent
tags: [viper]
---
# Viper DPI stages: write stage indexes 0-based like Razer

USB capture 2026-11-09: Synapse writes 04/06 with stage indexes 00..04 (OpenRazer does too); the mouse reports them 1-based in 04/86. viper.rs Setting::DpiStages writes 1-based (01..05); that also worked (same-value write read back identical), but match Razer: write i (0-based), keep reading 1-based. Update the test 'dpi_stage_write_matches_what_the_mouse_reports' accordingly and re-test on the mouse.

