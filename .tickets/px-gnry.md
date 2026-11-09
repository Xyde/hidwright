---
id: px-gnry
status: closed
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

**2026-11-09T00:08:06Z**

2026-11-08, user: 'go with what you'd like! And OF COURSE we should scrub our names', placeholders anyone can use. Done: history rewritten with jj before any push. User folder paths → ~\... in HANDOFF.md; rawaccel_cfg.py now searches ~\Downloads|Desktop|Documents and Program Files for RawAccel* (RAWACCEL_DIR still overrides); PC name → <PC-NAME>; Claude package id → Claude_*. Every revision and message scanned: 0 hits. Added README.md (beginner steps) and LICENSE (MIT). Public repo chosen (free Actions incl. macOS runners).

**2026-11-09T00:08:41Z**

Push blocked 2026-11-08 by Claude Code's auto-mode classifier ('Create Public Surface'). Ready: bookmark main at the README/licence change, history scrubbed. User runs: gh repo create Xyde/hidwright --public --description ...; jj git remote add origin https://github.com/Xyde/hidwright.git; jj git push --bookmark main. After that, the agent can add CI (GitHub Actions: Windows + macOS build/test).

**2026-11-09T18:59:07Z**

Published 2026-11-09 by the user: github.com/Xyde/hidwright (public), 4 commits, all authored Xyde <noreply>, 53 files, 0 build files. CI workflow (.github/workflows/ci.yml, Windows + macOS) is committed locally on main and waits for the user's push.
