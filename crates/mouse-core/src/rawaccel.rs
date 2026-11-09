//! RawAccel's settings.json: give individual mice their own rotation, and load the file into
//! the driver.
//!
//! Port of `rawaccel_cfg.py` and the loader in `mouse_startup.py`. How RawAccel picks settings
//! for a mouse (from its driver source, `DeviceSetup`):
//! - a mouse with an entry in `devices` whose `id` matches uses that entry's `profile`;
//! - every other mouse uses the FIRST profile in `profiles`.
//!
//! So one shared profile means one shared rotation for every mouse.
//!
//! Files are written exactly as the Python writes them (`json.dumps(indent=2)`, CRLF), so a
//! rewrite that changes nothing gives the same bytes, and a change shows up as a clean diff.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, sleep};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::rawinput::RawMouse;
use crate::{Error, Result};

/// The profile key for rotation.
pub const ROT: &str = "Degrees of rotation";
pub const ROTATION_LIMIT: f64 = 45.0;

// ------------------------------------------------------------------ finding and reading

/// Where RawAccel might be, most specific first.
///
/// `RAWACCEL_DIR` (if set) wins. Otherwise look where people usually unzip it: the release zip
/// unpacks to `RawAccel_vX.Y.Z\RawAccel`, sometimes flattened to `RawAccel_vX.Y.Z` or
/// `RawAccel`, in Downloads, Desktop, Documents or Program Files. Newest version first.
pub fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(d) = std::env::var_os("RAWACCEL_DIR").filter(|d| !d.is_empty()) {
        out.push(PathBuf::from(d));
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let program_files = std::env::var_os("ProgramFiles")
        .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from);
    for base in [
        home.join("Downloads"),
        home.join("Desktop"),
        home.join("Documents"),
        program_files,
    ] {
        let Ok(entries) = fs::read_dir(&base) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.to_lowercase().starts_with("rawaccel"))
            })
            .collect();
        // Windows paths compare case-insensitively (as Python's pathlib does there).
        found.sort_by_key(|p| std::cmp::Reverse(p.to_string_lossy().to_lowercase()));
        for d in found {
            out.push(d.join("RawAccel"));
            out.push(d);
        }
    }
    out
}

/// RawAccel's folder (the one with writer.exe), if there is one.
pub fn find_dir() -> Option<PathBuf> {
    candidates()
        .into_iter()
        .find(|d| d.join("writer.exe").is_file())
}

/// RawAccel's folder and its parsed settings.json.
pub struct Settings {
    pub dir: PathBuf,
    pub cfg: Value,
}

impl Settings {
    pub fn path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }
}

/// None if RawAccel or its settings.json isn't there.
pub fn load() -> Result<Option<Settings>> {
    let Some(dir) = find_dir() else {
        return Ok(None);
    };
    let path = dir.join("settings.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| Error::File(format!("Can't read {}: {e}", path.display())))?;
    let cfg = serde_json::from_str(&text)
        .map_err(|e| Error::File(format!("{} isn't valid JSON: {e}", path.display())))?;
    Ok(Some(Settings { dir, cfg }))
}

/// Text exactly as Python's `json.dumps(cfg, indent=2)` written with `Path.write_text` on
/// Windows: 2-space indent, non-ASCII escaped as `\uXXXX`, CRLF line ends, no final newline.
pub fn to_text(cfg: &Value) -> String {
    let pretty = serde_json::to_string_pretty(cfg).expect("a JSON value always serializes");
    let mut out = String::with_capacity(pretty.len() + pretty.len() / 16);
    for c in pretty.chars() {
        match c {
            // Newlines inside strings are escaped, so every raw '\n' is a line break.
            '\n' => out.push_str("\r\n"),
            c if !c.is_ascii() => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

// ------------------------------------------------------------------ which profile a mouse uses

fn name_of(p: &Value) -> Option<&str> {
    p.get("name").and_then(Value::as_str)
}

/// (profile name, rotation, has its own `devices` entry) the driver would use for `dev_id`.
pub fn resolve(cfg: &Value, dev_id: &str) -> (String, f64, bool) {
    let fallback = [json!({"name": "default"})];
    let profiles = match cfg.get("profiles").and_then(Value::as_array) {
        Some(p) if !p.is_empty() => p.as_slice(),
        _ => &fallback,
    };
    let mut prof = &profiles[0];
    let mut own = false;
    let devices = cfg.get("devices").and_then(Value::as_array);
    // The driver compares ids case-sensitively.
    if let Some(dev) = devices
        .into_iter()
        .flatten()
        .find(|d| d.get("id").and_then(Value::as_str) == Some(dev_id))
    {
        own = true;
        let want = dev.get("profile").and_then(Value::as_str);
        if let Some(p) = profiles
            .iter()
            .find(|p| want.is_some() && name_of(p) == want)
        {
            prof = p;
        }
    }
    let rot = prof.get(ROT).and_then(Value::as_f64).unwrap_or(0.0);
    (name_of(prof).unwrap_or("?").to_string(), rot, own)
}

/// One mouse's RawAccel situation.
#[derive(Clone, Debug, PartialEq)]
pub struct MouseRotation {
    pub id: String,
    pub name: String,
    pub profile: String,
    pub rotation: f64,
    pub own_profile: bool,
    /// Other listed mice on the same profile (so changing it would change them too).
    pub shared_with: Vec<String>,
}

pub fn summary(cfg: &Value, mice: &[RawMouse]) -> Vec<MouseRotation> {
    let mut out: Vec<MouseRotation> = mice
        .iter()
        .map(|m| {
            let (profile, rotation, own_profile) = resolve(cfg, &m.id);
            MouseRotation {
                id: m.id.clone(),
                name: m.name.clone(),
                profile,
                rotation,
                own_profile,
                shared_with: Vec::new(),
            }
        })
        .collect();
    let pairs: Vec<(String, String)> = out
        .iter()
        .map(|m| (m.profile.clone(), m.name.clone()))
        .collect();
    for m in &mut out {
        m.shared_with = pairs
            .iter()
            .filter(|(p, n)| *p == m.profile && *n != m.name)
            .map(|(_, n)| n.clone())
            .collect();
    }
    out
}

// ------------------------------------------------------------------ changing rotation

/// Give the mouse `id` (shown as `name`) its own profile with this rotation.
#[derive(Clone, Debug)]
pub struct Change {
    pub id: String,
    pub name: String,
    pub rotation: f64,
}

/// Round to 2 decimals the way Python's `round(x, 2)` does (ties to even on the exact value).
fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// New config in which each changed mouse has its own profile with the given rotation.
///
/// A new profile is a copy of the profile the mouse uses today (same curve and sensitivity),
/// so the only thing that changes is the rotation. The first profile, which mice without their
/// own entry use, is never changed.
pub fn plan(cfg: &Value, changes: &[Change]) -> Result<Value> {
    let broken = |what: &str| Error::File(format!("settings.json: {what}"));
    let mut new = cfg.clone();
    {
        let obj = new
            .as_object_mut()
            .ok_or_else(|| broken("not a JSON object"))?;
        obj.entry("profiles").or_insert_with(|| json!([]));
        obj.entry("devices").or_insert_with(|| json!([]));
    }
    for ch in changes {
        let rot = round2(ch.rotation);
        if !(-ROTATION_LIMIT..=ROTATION_LIMIT).contains(&rot) {
            return Err(Error::Input(
                "rotation must be between -45 and 45 degrees".into(),
            ));
        }
        let pname = ch.name.as_str();
        let (cur_name, _, _) = resolve(&new, &ch.id);
        let device_config = new
            .get("defaultDeviceConfig")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let profiles = new["profiles"]
            .as_array_mut()
            .ok_or_else(|| broken("\"profiles\" isn't a list"))?;
        let idx = match profiles.iter().position(|p| name_of(p) == Some(pname)) {
            Some(i) => i,
            None => {
                let base = profiles
                    .iter()
                    .find(|p| name_of(p) == Some(cur_name.as_str()))
                    .or(profiles.first())
                    .ok_or_else(|| broken("no profiles"))?;
                let mut prof = base.clone();
                prof.as_object_mut()
                    .ok_or_else(|| broken("a profile isn't an object"))?
                    .insert("name".into(), json!(pname));
                profiles.push(prof);
                profiles.len() - 1
            }
        };
        if idx == 0 {
            return Err(Error::Input(format!(
                "'{pname}' is the default profile; rename it in RawAccel first"
            )));
        }
        profiles[idx]
            .as_object_mut()
            .ok_or_else(|| broken("a profile isn't an object"))?
            .insert(ROT.into(), json!(rot));

        let devices = new["devices"]
            .as_array_mut()
            .ok_or_else(|| broken("\"devices\" isn't a list"))?;
        let entry = match devices
            .iter()
            .position(|d| d.get("id").and_then(Value::as_str) == Some(ch.id.as_str()))
        {
            Some(i) => &mut devices[i],
            None => {
                devices.push(json!({
                    "name": pname,
                    "profile": pname,
                    "id": ch.id,
                    "config": device_config,
                }));
                devices.last_mut().expect("just pushed")
            }
        };
        entry
            .as_object_mut()
            .ok_or_else(|| broken("a device entry isn't an object"))?
            .insert("profile".into(), json!(pname));
    }
    Ok(new)
}

/// Back up settings.json, write the planned changes, and load them into the driver.
/// Returns the backup's path. If writer.exe fails, settings.json is put back.
pub fn apply(changes: &[Change]) -> Result<PathBuf> {
    let s = load()?.ok_or_else(|| Error::NotFound("RawAccel settings.json not found".into()))?;
    let new = plan(&s.cfg, changes)?;
    let path = s.path();
    let (y, mo, d, h, mi, se) = crate::local_time();
    let backup = s.dir.join(format!(
        "settings.backup-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{se:02}.json"
    ));
    let io = |what: &str, e: std::io::Error| Error::File(format!("{what}: {e}"));
    fs::copy(&path, &backup).map_err(|e| io("Couldn't back up settings.json", e))?;
    fs::write(&path, to_text(&new)).map_err(|e| io("Couldn't write settings.json", e))?;
    let restore = || fs::copy(&backup, &path).map(|_| ());
    match run_writer(&s.dir, &path, Duration::from_secs(20)) {
        Ok(out) if out.code == 0 => Ok(backup),
        Ok(out) => {
            let restored = restore().is_ok();
            Err(Error::File(format!(
                "writer.exe failed (exit {}) {}{}; settings.json {}",
                out.code,
                out.stdout,
                out.stderr,
                if restored {
                    "restored from backup"
                } else {
                    "NOT restored, copy the backup back by hand"
                }
            )))
        }
        Err(WriterError::Timeout) => Err(Error::File(
            "writer.exe didn't finish (is it showing an error window?)".into(),
        )),
        Err(WriterError::Os(e)) => {
            let _ = restore();
            Err(Error::File(format!(
                "Couldn't run writer.exe ({e}); settings.json restored from backup"
            )))
        }
    }
}

// ------------------------------------------------------------------ loading into the driver

/// What writer.exe said.
pub struct WriterOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub enum WriterError {
    /// It ran longer than allowed (and was stopped).
    Timeout,
    /// It couldn't be started or waited for.
    Os(std::io::Error),
}

/// Run RawAccel's own `writer.exe settings.json`, which loads the file into the driver.
pub fn run_writer(
    dir: &Path,
    settings: &Path,
    timeout: Duration,
) -> std::result::Result<WriterOutput, WriterError> {
    let mut cmd = Command::new(dir.join("writer.exe"));
    cmd.arg(settings)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(WriterError::Os)?;
    // Drain both pipes on their own threads so a chatty writer.exe can't block on a full pipe.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_string(&mut s);
            }
            s
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(st) = child.try_wait().map_err(WriterError::Os)? {
            break st;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(WriterError::Timeout);
        }
        sleep(Duration::from_millis(25));
    };
    Ok(WriterOutput {
        code: status.code().unwrap_or(-1),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

/// Load settings.json into the driver, trying `tries` times `wait` apart (right after boot the
/// driver can be slow to come up). Ok and Err both carry the line for the log, worded as the
/// Python's `mouse_startup.py` words it.
pub fn reload(tries: u32, wait: Duration) -> std::result::Result<String, String> {
    let Some(dir) = find_dir().filter(|d| d.join("settings.json").exists()) else {
        return Err("RawAccel folder or settings.json not found; nothing loaded".into());
    };
    let settings = dir.join("settings.json");
    let mut last = String::new();
    for _ in 0..tries {
        match run_writer(&dir, &settings, Duration::from_secs(30)) {
            Ok(out) if out.code == 0 => {
                let rots = match load() {
                    Ok(Some(s)) => profile_rotations(&s.cfg),
                    _ => String::new(),
                };
                return Ok(format!("RawAccel settings loaded ({rots})"));
            }
            Ok(out) => {
                last = format!(
                    "writer.exe exit {} {} {}",
                    out.code,
                    out.stdout.trim(),
                    out.stderr.trim()
                )
                .trim()
                .to_string();
            }
            Err(WriterError::Timeout) => last = "writer.exe didn't finish within 30 s".into(),
            Err(WriterError::Os(e)) => last = e.to_string(),
        }
        sleep(wait);
    }
    Err(format!("FAILED to load RawAccel settings: {last}"))
}

/// "default: -2.6 deg, Pulsar X2A: -0.5 deg", each value as written in the file.
pub fn profile_rotations(cfg: &Value) -> String {
    cfg.get("profiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|p| {
            let name = match p.get("name") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => "None".into(),
            };
            let rot = p.get(ROT).map_or_else(|| "0".into(), Value::to_string);
            format!("{name}: {rot} deg")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = r"HID\VID_3710&PID_1404&MI_00";
    const R: &str = r"HID\VID_1532&PID_0078&MI_00";

    fn fixture(name: &str) -> &'static str {
        match name {
            "cfg" => include_str!("../tests/fixtures/rawaccel/cfg.json"),
            "same_value" => include_str!("../tests/fixtures/rawaccel/same_value.json"),
            "change_existing" => include_str!("../tests/fixtures/rawaccel/change_existing.json"),
            "new_profile" => include_str!("../tests/fixtures/rawaccel/new_profile.json"),
            "rounding" => include_str!("../tests/fixtures/rawaccel/rounding.json"),
            "two_at_once" => include_str!("../tests/fixtures/rawaccel/two_at_once.json"),
            _ => unreachable!(),
        }
    }

    fn cfg() -> Value {
        serde_json::from_str(fixture("cfg")).unwrap()
    }

    fn ch(id: &str, name: &str, rotation: f64) -> Change {
        Change {
            id: id.into(),
            name: name.into(),
            rotation,
        }
    }

    /// Fixtures are Python's `json.dumps(plan(...), indent=2)` (LF); ours is CRLF.
    fn lf(text: String) -> String {
        text.replace("\r\n", "\n")
    }

    #[test]
    fn rewriting_unchanged_gives_the_same_text() {
        assert_eq!(lf(to_text(&cfg())), fixture("cfg"));
        assert!(to_text(&cfg()).contains(r#""Maus \u00fc""#));
        assert!(!to_text(&cfg()).contains("\n\n") && to_text(&cfg()).contains("}\r\n"));
    }

    #[test]
    fn resolve_matches_python() {
        let c = cfg();
        assert_eq!(resolve(&c, P), ("Pulsar X2A".into(), -0.5, true));
        assert_eq!(resolve(&c, R), ("default".into(), -2.6, false));
        // the driver compares ids case-sensitively
        assert_eq!(
            resolve(&c, r"hid\vid_3710&pid_1404&mi_00"),
            ("default".into(), -2.6, false)
        );
    }

    /// Expected files generated from `rawaccel_cfg.plan` (see HANDOFF.md, test vectors).
    #[test]
    fn plan_matches_python() {
        let cases: [(&str, Vec<Change>); 5] = [
            ("same_value", vec![ch(P, "Pulsar X2A", -0.5)]),
            ("change_existing", vec![ch(P, "Pulsar X2A", 1.25)]),
            ("new_profile", vec![ch(R, "Razer Viper", 3.0)]),
            (
                "rounding",
                vec![ch(R, "Razer Viper", 0.125), ch(P, "Pulsar X2A", -0.005)],
            ),
            (
                "two_at_once",
                vec![ch(P, "Pulsar X2A", 2.0), ch(R, "Razer Viper", -45.0)],
            ),
        ];
        for (name, changes) in cases {
            let got = lf(to_text(&plan(&cfg(), &changes).unwrap()));
            assert_eq!(got, fixture(name), "{name}");
        }
    }

    #[test]
    fn plan_refuses_like_python() {
        let msg = |changes: &[Change]| plan(&cfg(), changes).unwrap_err().to_string();
        assert_eq!(
            msg(&[ch(R, "Razer Viper", 45.01)]),
            "rotation must be between -45 and 45 degrees"
        );
        assert_eq!(
            msg(&[ch(R, "Razer Viper", f64::NAN)]),
            "rotation must be between -45 and 45 degrees"
        );
        assert_eq!(
            msg(&[ch(R, "default", 1.0)]),
            "'default' is the default profile; rename it in RawAccel first"
        );
    }

    #[test]
    fn summary_marks_shared_profiles() {
        let mice = [
            RawMouse {
                id: P.into(),
                name: "Pulsar X2A".into(),
            },
            RawMouse {
                id: R.into(),
                name: "Razer Viper".into(),
            },
            RawMouse {
                id: r"HID\VID_046D&PID_C077&MI_00".into(),
                name: "Other".into(),
            },
        ];
        let s = summary(&cfg(), &mice);
        assert_eq!(s[0].profile, "Pulsar X2A");
        assert!(s[0].own_profile && s[0].shared_with.is_empty());
        assert_eq!(s[1].shared_with, vec!["Other".to_string()]);
        assert_eq!(s[2].shared_with, vec!["Razer Viper".to_string()]);
    }

    #[test]
    fn rotations_line_like_the_startup_log() {
        assert_eq!(
            profile_rotations(&cfg()),
            "default: -2.6 deg, Pulsar X2A: -0.5 deg, Maus \u{fc}: 1.0 deg"
        );
    }
}
