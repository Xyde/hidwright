---
id: px-ut0y
status: open
deps: []
links: []
created: 2026-11-09T23:19:26Z
type: task
priority: 3
assignee: user
tags: [usb, latency]
---
# Decide: keep or remove USBPcap

USBPcap 1.5.4.0 (installed 2026-11-09 for the Synapse capture) adds an upper filter (USBPcap) on the USB class, i.e. on every USB hub: all USB data passes through it. When no capture runs it only passes data on (delay probably too small to measure). Keep: ready for future captures (Xtrfy M42 px-8mzp, Dumang px-k3jg). Remove: Settings > Apps > Installed apps > USBPcap > Uninstall, then restart once; the installer stays in Downloads for a 1-minute reinstall. Agent then checks the USB class filters are empty.

