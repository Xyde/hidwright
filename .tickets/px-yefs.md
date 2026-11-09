---
id: px-yefs
status: closed
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

**2026-11-09T20:31:10Z**

2026-11-09: built and tested; waiting for the user to repoint the task (the agent is blocked from changing scheduled tasks).
- crates/set-mouse/src/bin/set-mouse-startup.rs: windows_subsystem = "windows" (PE subsystem 2 checked, set-mouse.exe stays 3 = console). Calls rawaccel::reload(6 tries, 5 s), writes one startup.log line via shared.rs (same code as `set-mouse reload`), prints nothing, exit 0/1.
- Tested: by hand from target/release and from bin/ -> exit 0, nothing printed, log line identical to mouse_startup.py's; failure path (RawAccel unfindable, run from scratch so the real log stays clean) -> exit 1, "RawAccel folder or settings.json not found; nothing loaded".
- Copy for the task: bin/set-mouse-startup.exe (gitignore bin/*.exe). Refresh after loader changes: cp target/release/set-mouse-startup.exe bin/
- Backup of the task as it was: task-backup-python.local.xml in the project folder (git-ignored via *.local.xml, re-encoded to real UTF-16 so schtasks accepts it).
- Pulsar globals re-apply (mentioned in this ticket's description) no longer needed: px-337u solved persistence.

User runs (Nushell or PowerShell, same text):
  schtasks /Change /TN "Mouse Portal - load RawAccel" /TR "C:\Users\<you>\Documents\pulsar-x2a\bin\set-mouse-startup.exe"
  schtasks /Run /TN "Mouse Portal - load RawAccel"
Then the agent checks Get-ScheduledTaskInfo (LastTaskResult 0) + the new startup.log line. Then a real reboot, and the same check (like px-p010).
Rollback: schtasks /Create /TN "Mouse Portal - load RawAccel" /XML "<project>\task-backup-python.local.xml" /F

**2026-11-09T20:33:40Z**

2026-11-09: user repointed the task (schtasks /Change; the run-as password prompt left empty: the account has none and the task is Interactive / run only when logged on, so none is needed). Verified: action = bin\set-mouse-startup.exe, principal unchanged (Interactive, Limited), manual run 15:31:48 -> LastTaskResult 0, startup.log line 15:31:49 identical to the Python's. CI 42693897148 green (Windows, macOS, Python). Left: user reboots once; agent then checks Get-ScheduledTaskInfo (LastRunTime = that sign-in, result 0) and the startup.log line, then closes this ticket.

**2026-11-09T22:44:46Z**

2026-11-09 VERIFIED across a real reboot (boot 17:34:33): task 'Mouse Portal - load RawAccel' ran bin\set-mouse-startup.exe at 17:34:52, LastTaskResult 0; startup.log '17:34:58  RawAccel settings loaded (default: -2.6 deg, Pulsar X2A: -0.5 deg, Xtrfy M42: 0.0 deg)'; set-mouse rotation shows all three. Also: Pulsar kept debounce 1 ms / ripple off through the reboot (px-337u fix holds).
