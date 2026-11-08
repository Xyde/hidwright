---
id: px-zd1z
status: open
deps: []
links: []
created: 2026-11-08T22:09:12Z
type: task
priority: 4
assignee: agent
tags: [pulsar]
---
# Verify X2A lift-off (lod) encoding before anyone uses set-mouse lod

x2a.py uses [val,val] with 0=1mm, 1=2mm, but its X3 mixin comments call that a misreading (mode byte + tenths of mm). Never written this session. Read-back currently says 1 mm.

