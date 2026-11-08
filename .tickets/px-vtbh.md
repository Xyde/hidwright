---
id: px-vtbh
status: open
deps: []
links: []
created: 2026-11-08T22:09:10Z
type: task
priority: 2
assignee: user
tags: [calibrator, rawaccel]
---
# Confirm rotation direction: re-run calibrator on the Pulsar, expect about 0 deg

Pulsar was applied at -0.5 deg (recommended = current rotation - measured angle). The sign convention was derived from RawAccel source (common/math-vec2.hpp), never confirmed by a re-run reading ~0. If a re-run shows about double the first angle instead, the sign is flipped in mouse_portal.html calFinish.

