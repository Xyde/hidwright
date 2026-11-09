---
id: px-c0lr
status: open
deps: []
links: []
created: 2026-11-09T23:19:26Z
type: chore
priority: 4
assignee: user
tags: [cleanup]
---
# Clean up capture files and installers from the Synapse work

Leftovers of 2026-11-09, outside the repo: %TEMP%/hidwright-diag/usbcap/hub1-4.pcap (about 68 MB; they contain the Viper's serial number, never commit them), %TEMP%/hidwright-diag/*.py (diagnostic scripts: Raw Input listener, button-table reader; the protocol is also in HANDOFF.md, so they can go after the button fix), and in Downloads: RazerSynapseInstaller.exe, Web_Razer_Synapse_Installer_v2.25.1028.6.exe (both unneeded now), USBPcapSetup-1.5.4.0.exe (keep if you may reinstall USBPcap). Also in the repo folder but git-ignored: uninstall_official_razer.cmd/.ps1 (another tool's uninstall scripts; contain this PC's paths; keep or delete as you like).

