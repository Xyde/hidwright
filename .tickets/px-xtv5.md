---
id: px-xtv5
status: open
deps: []
links: []
created: 2026-11-08T22:09:11Z
type: task
priority: 4
assignee: user
tags: [rawaccel]
---
# Decide: give the Viper its own RawAccel profile so 'default' can be 0 deg?

Viper uses profiles[0] 'default' (-2.6 deg). Every unlisted mouse (e.g. keyboard 0483:5710 mouse collection) also gets -2.6. Harmless today.


## Notes

**2026-11-08T23:48:54Z**

More relevant now (2026-11-08): the XTRFY M42 is plugged in and, being unlisted, gets 'default' = -2.6 deg. Giving the Viper its own profile and setting 'default' to 0 deg fixes this for every future unknown mouse.
