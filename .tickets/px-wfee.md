---
id: px-wfee
status: open
deps: []
links: [px-hkts, px-23l1]
created: 2026-11-08T22:18:53Z
type: bug
priority: 3
assignee: agent
tags: [razer]
---
# Razer reads intermittently fail: Synapse talks to the Viper at the same time

Found 2026-11-08: set-mouse printed '(dpi: Razer reply ok for 04/85)' and on the next run '(firmware: Razer reply ok for 00/81)'; a third run was clean. razer_viper._request raises when status is OK but rsp[6]/rsp[7] don't match the command, i.e. the reply belongs to someone else's request. Razer Synapse 4.0.896 (RazerAppEngine, installed 2026-04-28, updated 2026-10-28) plus drivers RzCommon/RzDev_0078 are running and poll the mouse. Fix: on a cls/cmd mismatch, resend (a few tries) instead of failing. Separate question for the user: keep Synapse at all? (not installed by us; RzDev_0078 driver may be needed for nothing we use).

