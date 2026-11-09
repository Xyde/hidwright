//! set-mouse-startup: the sign-in loader. Puts the RawAccel settings back into the driver after
//! a reboot (the driver forgets them), writes one line to `startup.log`, and exits.
//!
//! The same work as `set-mouse reload` and the Python's `mouse_startup.py`, built as a windowed
//! program: Windows creates no console for it, so nothing flashes at sign-in (the Python task
//! used `pythonw` for the same reason). It prints nothing; the log line is the output.
//! Run by the scheduled task "Mouse Portal - load RawAccel" (ticket px-yefs).

#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "../shared.rs"]
mod shared;

use std::process::ExitCode;
use std::time::Duration;

use mouse_core::rawaccel;

fn main() -> ExitCode {
    // Right after boot the driver can be slow to come up: 6 tries, 5 s apart.
    let (line, code) = match rawaccel::reload(6, Duration::from_secs(5)) {
        Ok(line) => (line, ExitCode::SUCCESS),
        Err(line) => (line, ExitCode::FAILURE),
    };
    shared::append_log(&line);
    code
}
