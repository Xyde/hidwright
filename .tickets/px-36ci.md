---
id: px-36ci
status: closed
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

**2026-11-09T20:22:35Z**

2026-11-09: temporary DPI done (Viper::set_dpi_temporary, NOSTORE, vectors from the Python; used by dpi_restore.rs and checked on the real mouse with a same-value restore). Remaining: px-w1a0 writes (lighting, persistent stages, polling), each checked against OpenRazer first.

**2026-11-09T20:50:27Z**

2026-11-09 DONE: Viper writes in Rust. Source: OpenRazer master (razermouse_driver.c, razerchromacommon.c, razercommon.h; facts only). The Viper supports: logo LED (0x04) static/breathing(random, 1, 2 colours)/spectrum/reactive/off + brightness (0F/04, get 0F/84), saved DPI stages (04/06, size 0x26), polling (00/05: 1000=01, 500=02, 125=08; no 250 in the set builder), device mode (00/04, get 00/84). No button remapping, lift-off, angle snapping or calibration in OpenRazer. Transaction ids: OpenRazer uses 0x3F (lighting) / 0xFF (rest); this Viper answers 0x1F/0xFF/0x3F alike; writes use OpenRazer's.
Differences from OpenRazer: stage indexes written 1-based (the mouse reports 01..05, as Synapse saved them; OpenRazer writes 0-based). OpenRazer's set_dpi_xy always sends VARSTORE; our set_dpi_temporary sends NOSTORE (unverified that it is forgotten on replug: px-23l1 trial).
Verified on the mouse: read-only probe (brightness 178 = 70%, device mode 03 = driver, stage reply 1-based); same-value writes of all four kinds acked + read back identical; real round trips brightness 70->40->70, polling 1000->500->1000, stage 5->4->5 (live DPI follows), logo static red -> spectrum. 31 tests (bytes = OpenRazer layouts + the stage bytes the mouse reported).
CLI: set-mouse razer [dpi ... --active N | stage N | poll HZ | brightness % | light off|spectrum|static C|breath [C [C]]|reactive C [1-4]]. Status shows brightness and device mode.
