---
id: px-8swt
status: closed
deps: []
links: []
created: 2026-11-09T20:23:21Z
type: task
priority: 1
assignee: user
tags: [vcs]
---
# Push main to GitHub (7 changes, 2026-11-09)

The user said yes to pushing; Claude Code's auto-mode classifier blocks pushes to the public repo from the agent, so the user runs it: in ~\Documents\pulsar-x2a, 'jj git push --bookmark main'. main = xpnmqqpt 'set-mouse on PATH runs the Rust exe (px-5xrv part c)', 7 changes above main@origin (1bec8eda). Checked before: cargo fmt/clippy/test (26)/release build, py_compile, 0 personal-data hits in the diff. After pushing: 'gh run list --repo Xyde/hidwright --limit 1' to see CI (first CI run with serde_json and the new modules on macOS). Close this ticket then.


## Notes

**2026-11-09T20:30:34Z**

2026-11-09: user pushed (main = xpnmqqpt). CI 42692951681: macOS + Python green, Windows red: 2 rawaccel fixture tests failed because the Windows runner's git checks text files out as CRLF; fixed in the next change (tests normalise line ends, checked with CRLF fixtures locally).
