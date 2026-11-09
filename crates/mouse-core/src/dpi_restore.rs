//! Undo the temporary DPI of a calibration run that didn't finish.
//!
//! Port of `restore_if_pending` in `test_dpi.py`. Before a calibration run changes any DPI, the
//! portal writes the original values to `dpi_restore.json`:
//! `{"pulsar": {"profile": 1, "active": 1, "stages": [3200]}, "razer": {"dpi": [3200, 3200]}}`
//! (each key only if that mouse was there). If the run dies, this puts them back.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::viper::Viper;
use crate::x2a::{Setting, X2a};
use crate::{Error, Result};

/// The file name, in the folder the caller chooses.
pub const FILE_NAME: &str = "dpi_restore.json";

/// Put back the DPIs saved in `path`, then delete it. Safe to call any time.
///
/// Returns what was restored, e.g. `[("Pulsar X2A", 3200), ("Razer Viper", 3200)]`; empty if
/// there was nothing to do. If the Pulsar can't be opened (unplugged), nothing is touched and
/// the file stays for next time. An unplugged Viper is skipped: it forgets a temporary DPI on
/// replug anyway.
pub fn restore_if_pending(path: &Path) -> Result<Vec<(&'static str, u16)>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bad = |what: &str| Error::File(format!("{} is damaged ({what})", path.display()));
    let text = fs::read_to_string(path).map_err(|e| Error::File(e.to_string()))?;
    let saved: Value = serde_json::from_str(&text).map_err(|e| bad(&e.to_string()))?;
    let mut done = Vec::new();

    if let Some(p) = saved.get("pulsar") {
        let num = |k: &str| p.get(k).and_then(Value::as_u64);
        let (Some(profile), Some(active)) = (num("profile"), num("active")) else {
            return Err(bad("pulsar profile/active"));
        };
        let stages: Vec<u16> = p
            .get("stages")
            .and_then(Value::as_array)
            .ok_or_else(|| bad("pulsar stages"))?
            .iter()
            .map(|v| v.as_u64().and_then(|d| u16::try_from(d).ok()))
            .collect::<Option<_>>()
            .ok_or_else(|| bad("pulsar stages"))?;
        let Ok(mut m) = X2a::open() else {
            return Ok(done); // Pulsar unplugged: keep the file and try again next time
        };
        let setting = Setting::DpiStages {
            dpis: stages.clone(),
            active: active as u8,
        };
        // Unchecked: these values came from the mouse, so put them back as they were.
        m.write_unchecked(&setting, Some(profile as u8))?;
        if let Some(&d) = stages.get((active as usize).wrapping_sub(1)) {
            done.push(("Pulsar X2A", d));
        }
    }

    if let Some(r) = saved.get("razer") {
        let dpi: Vec<u16> = r
            .get("dpi")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_u64().and_then(|d| u16::try_from(d).ok()))
                    .collect()
            })
            .unwrap_or_default();
        let [x, y] = dpi[..] else {
            return Err(bad("razer dpi"));
        };
        if let Ok(v) = Viper::open() {
            v.set_dpi_temporary(x, y)?;
            done.push(("Razer Viper", x));
        }
    }

    fs::remove_file(path).map_err(|e| Error::File(e.to_string()))?;
    Ok(done)
}
