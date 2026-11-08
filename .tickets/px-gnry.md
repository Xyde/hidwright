---
id: px-gnry
status: open
deps: []
links: []
created: 2026-11-08T22:43:49Z
type: task
priority: 2
assignee: user
tags: [macos, vcs]
---
# Decide: a remote so the repo can reach the Mac

The jj repo has no remote. To work on both machines: e.g. a private GitHub repo under Xyde, then jj git push from the PC and jj git clone on the Mac. Needs the user's explicit yes (CLAUDE.md: don't push without it); repo name and visibility are the user's call.


## Notes

**2026-11-08T22:50:01Z**

User 2026-11-08: 'A private repo would be nice, but I don't mind a public one either', leaning public if it means unlimited GitHub Actions. Facts to confirm on GitHub's billing page: standard GitHub-hosted runners are free and unlimited for public repos; private repos on the Free plan get a monthly minute quota, with macOS minutes counted at a multiplier. A free macOS runner would compile and test the Mac backend on every push, without the Mac. Before a public push: the repo contains the Windows user name (paths in HANDOFF.md, rawaccel_cfg.py) and the PC name (ticket px-23l1); no email (noreply) and no serials. Scrub history first if the user wants. Needs a LICENSE (upstream Pulsar protocol: MIT; the Dumang work must not copy GPL code). Still needed from the user: explicit go, repo name, public or private.

**2026-11-08T23:49:10Z**

Name chosen by the agent (user delegated, 2026-11-08): hidwright (HID + wright, a maker). Available on github.com/Xyde. Still waiting for the user's explicit go and public/private.
