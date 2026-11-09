---
id: px-fmim
status: closed
deps: []
links: []
created: 2026-11-09T23:21:21Z
type: task
priority: 1
assignee: user
tags: [viper, buttons, usb]
---
# Decide: fix Viper buttons via Synapse 3 (offline setups) + USB capture, or write them directly (px-oqc3)

Found by the user 2026-11-09: github.com/TicTac7x/razer-synapse-3-offline-setups (55 stars, last commit 2026-07-03, no releases, no hashes, no signature claims). README: 'All listed files are currently downloadable directly from Razer servers ... The files uploaded here are only for backup purposes'; links point to cdn.razersynapse.com. Core setups in a given install order (Synapse v3.11.0228.012810, Dependencies v3.7.0830.1, Razer Central v7.21.0.1154, GMS v2.4.160.155, String Translations, Video Player, Game Profile, Configuration Data; module Calibration Files). Viper module: ID 120 'Razer_120_0_Setup_v3.9.0331.030402.exe' (Synapse internal ID; mapping to USB 1532:0078 not confirmed). Caveats: 'Manually installed modules might not appear in the Synapse' (copy Manifest.xml to C:/ProgramData/Razer/Synapse3/Update/Manifest.xml); online installer works 'for now'. Unknown: whether Synapse 3 writes the Viper's on-board button table (it had an On-board memory tab; Synapse 4 does not write it). Option A (this): download only from cdn.razersynapse.com links, verify each file with Get-AuthenticodeSignature (must be Valid, signer Razer USA Ltd), install, start USBPcap capture (admin; add --inject-descriptors), put a default profile into on-board memory, stop, parse with the reader script, then uninstall Synapse 3 the same careful way (ghost nodes, RzDev/RzCommon, takeown). Gives Razer's exact bytes. Option B: px-oqc3 direct writes (razerqdhid code + read-back + Notepad test). Agent leaning: B is faster and already well-founded; A is the purist route the user asked for. User decides.


## Notes

**2026-11-09T23:41:09Z**

2026-11-09: user chose B (direct writes), on the condition of the fewest manual steps and no Notepad. Synapse 3 capture (A) not done.
