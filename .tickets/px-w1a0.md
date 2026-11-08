---
id: px-w1a0
status: open
deps: [px-36ci]
links: [px-23l1, px-izgb]
created: 2026-11-08T22:30:36Z
type: feature
priority: 2
assignee: agent
tags: [razer]
---
# Viper full control: everything Synapse can do for it

User 2026-11-08: 'as thorough as possible'. Cover what Synapse offers for the Viper (1532:0078), using the OpenRazer protocol. Check every command against the OpenRazer source before writing to the mouse.
- Lighting: Synapse supportEffects = STATIC, BREATHING, SPECTRUM_CYCLING, REACTIVE (hardware, if OpenRazer confirms) and AUDIO_METER, TIDAL (software-driven: need a running process, so opt-in only or skip). Brightness (now 70). Off.
- Persistent DPI stages (count, values, active stage) and polling rate (currently read-only; NOSTORE temp DPI exists).
- Find out what else the Viper supports: onboard button remapping? lift-off/asymmetric cut-off? sensor calibration? List what is hardware vs Synapse-software-only.
- Idle lighting-off is a Synapse software feature (needs a background process); probably skip, user decides.
- Firmware update: Synapse-only; out of scope, note it.
- Portal: show and edit all of it.

