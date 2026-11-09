//! What `set-mouse` and `set-mouse-startup` share: where their files live, and the log.
//! Each binary includes this file as a module (`mod shared;` / `#[path] mod shared;`).

use std::path::{Path, PathBuf};

use mouse_core::local_time;

/// Where `startup.log` and `dpi_restore.json` live. While the Python version is still in use
/// (portal, shim fallback) they are in its folder (the portal writes `dpi_restore.json` there),
/// so look for its `mouse_startup.py` in the exe's folder and up to three folders above it (a
/// cargo build sits in `target\release\`, the sign-in copy in `bin\`). Otherwise: the exe's own
/// folder. Revisit when the Python is retired (ticket px-t5i3).
pub fn data_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    exe_dir
        .ancestors()
        .take(4)
        .find(|d| d.join("mouse_startup.py").is_file())
        .map_or(exe_dir.clone(), Path::to_path_buf)
}

/// Append a timestamped line to `startup.log` (kept to its last 200 lines); returns the line.
/// Same format as the Python's `mouse_startup.py`, CRLF on Windows like its `write_text`.
pub fn append_log(msg: &str) -> String {
    let (y, mo, d, h, mi, s) = local_time();
    let line = format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}  {msg}");
    let path = data_dir().join("startup.log");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = old.lines().collect();
    lines.drain(..lines.len().saturating_sub(199));
    lines.push(&line);
    let nl = if cfg!(windows) { "\r\n" } else { "\n" };
    let _ = std::fs::write(&path, lines.join(nl) + nl);
    line
}
