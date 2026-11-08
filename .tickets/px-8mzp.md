---
id: px-8mzp
status: open
deps: []
links: []
created: 2026-11-08T22:50:01Z
type: task
priority: 3
assignee: agent
tags: [xtrfy]
---
# XTRFY M42: identify and see if it's configurable

User 2026-11-08 has an XTRFY M42 (Xtrfy was acquired by Cherry; the user never found downloadable software). User: plug it in by cable. Agent then: VID:PID, USB/HID interfaces, any vendor-defined interface with feature reports (sign of a config protocol), chip vendor from the VID; search for community protocol work (OpenRGB etc.). Many M42 settings may be onboard button combos (DPI, polling); document those too.


## Notes

**2026-11-08T23:48:54Z**

Identified 2026-11-08: Xtrfy M42, USB 2EA8:2203 (Xtrfy's own vendor ID). Interfaces: MI_00 mouse (input 8 bytes); MI_01 col01 keyboard, col02 consumer control, col03 VENDOR page 0xFF00 with 64-byte input + 64-byte output reports and no feature report, so its config protocol runs over interrupt in/out, unlike the Pulsar/Razer feature reports. No public protocol work found (GitHub repo/code search: nothing for M42; only Cherry Xtrfy M64 and Cherry keyboards have community tools). Next: find the original Xtrfy M42 software (e.g. archived copies), and if found, the user decides whether to run it once while capturing USB (USBPcap) to learn the protocol. Until then the M42 is read-only/unsupported. NOTE: RawAccel id HID\VID_2EA8&PID_2203&MI_00 isn't in settings.json devices[], so the M42 gets profiles[0] 'default' = the Viper's -2.6 deg (see px-xtv5).
