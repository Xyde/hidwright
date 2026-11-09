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


## Notes

**2026-11-09T20:50:28Z**

2026-11-09: done via px-36ci: lighting (static, breathing x3, spectrum, reactive, off), brightness, saved DPI stages, polling; status shows brightness + device mode. Left:
- Persistence across a replug can't be tested while Synapse runs (it may re-apply its profile on reconnect): test during the px-23l1 trial (set brightness 60 + temporary DPI 1600, replug, expect 60% and the stage DPI back).
- Device mode is 03 (driver, set by Synapse). After uninstalling Synapse, set 00 (normal) so the DPI button is handled by the mouse; add `set-mouse razer mode normal` then (00/04 [mode, 0], OpenRazer allows only 00 and 03).
- Current effect can't be read (no getter in OpenRazer); the portal must remember what it set.
- Software effects (audio meter, tidal), idle-off, firmware updates: Synapse-only, not hardware; skipped unless the user wants a resident process.
- Portal UI for all of it: px-01cy.

**2026-11-09T23:19:27Z**

2026-11-09: replug test passed without Synapse (brightness 60% kept; restored to 70%). The logo keeps cycling colours without Synapse (on-board spectrum effect). Still unproven: NOSTORE temporary DPI forgotten on replug (the run that should test it found the temporary DPI already reset). Device mode: set to normal 00/04 [00 00] after Synapse; add a 'set-mouse razer mode' command if it ever drifts. Button table fix: px-oqc3.
