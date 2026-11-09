---
id: px-yefs
status: open
deps: [px-5xrv]
links: []
created: 2026-11-08T22:34:08Z
type: task
priority: 2
assignee: agent
parent: px-0oek
tags: [rust, startup]
---
# Rust 4: sign-in loader in Rust; repoint the scheduled task

Replace mouse_startup.py with 'set-mouse startup' (or similar), same startup.log format. Repoint task 'Mouse Portal - load RawAccel'; verify after a real reboot (like px-p010). Also the home for re-applying Pulsar globals if px-337u shows they don't persist.


## Notes

**2026-11-09T20:22:35Z**

2026-11-09: unblocked (px-5xrv closed). 'set-mouse reload' already = mouse_startup.py (6 tries 5 s apart, same startup.log lines, log in the Python folder via data_dir()). Open design point: the task runs pythonw (no window); the Rust exe is a console program, so at sign-in a console would flash. Options: a second tiny GUI-subsystem binary (e.g. set-mouse-startup.exe with #![windows_subsystem = "windows"]), or launching via conhost --headless / a VBS wrapper. Agent leans to the GUI-subsystem binary (no extra process, no script host). Repointing the scheduled task needs the user's OK; verify after a real reboot.
