//! set-mouse, Rust version (port in progress, epic px-0oek).
//!
//! Reads both mice, writes every Pulsar X2A setting, sets per-mouse RawAccel rotation and loads
//! RawAccel's settings. This is what `set-mouse` on PATH runs (`bin\set-mouse.cmd`), except
//! `set-mouse portal`, which is still the Python version. Messages match the Python's word for
//! word, apart from deliberate differences (the M42 in `rotation`).

mod shared;

use std::process::ExitCode;
use std::time::Duration;

use mouse_core::dpi_restore;
use mouse_core::rawaccel::{self, Change};
use mouse_core::rawinput::{self, RawMouse};
use mouse_core::viper::{self, Viper};
use mouse_core::x2a::{self, Binding, Button, Setting, Snapshot, Toggle, X2a};
use shared::data_dir;

/// `println!` that stops quietly when the reader has gone (`set-mouse | head -1`) instead of
/// panicking. Anything already sent to a mouse has been sent by then.
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

/// Text that already ends in a newline, with the same rule as [`say!`].
fn say_raw(text: &str) {
    use std::io::Write as _;
    if std::io::stdout().write_all(text.as_bytes()).is_err() {
        std::process::exit(0);
    }
}

const USAGE: &str = "\
set-mouse (Rust, port in progress)

  set-mouse                       status of everything (Pulsar, Razer, RawAccel rotation)
  set-mouse razer                 Razer Viper settings (read-only)

  Pulsar X2A (saved in the mouse itself):
  set-mouse info                  all Pulsar settings
  set-mouse swap | unswap         left <-> right
  set-mouse dpi 400 800 1600 3200 --active 4
  set-mouse stage 2               switch DPI stage
  set-mouse poll 1000             125 250 500 1000
  set-mouse debounce 1            ms
  set-mouse ripple off            also: motion-sync, angle-snap  (on|off)
  set-mouse lod 1                 lift-off 1 or 2 mm
  set-mouse bind thumb1 ctrl+c    remap a button ('default' = factory)
  set-mouse profile 2             switch onboard profile

  info, swap, unswap, dpi, stage, lod and bind take -p 1-5 for another onboard profile.

  RawAccel rotation (per mouse):
  set-mouse rotation              show each mouse's rotation
  set-mouse rotation pulsar 0     set one mouse's rotation (pulsar, razer, m42)
  set-mouse reload                load settings.json into RawAccel now (runs at sign-in too)

Not here yet (the Python `set-mouse` on PATH does it): portal.";

/// Names `set-mouse rotation` accepts, and the mouse each one means.
const ALIASES: [(&str, &str); 6] = [
    ("pulsar", "Pulsar X2A"),
    ("x2a", "Pulsar X2A"),
    ("razer", "Razer Viper"),
    ("viper", "Razer Viper"),
    ("m42", "Xtrfy M42"),
    ("xtrfy", "Xtrfy M42"),
];

/// Pulsar commands that change something.
const PULSAR_WRITES: [&str; 12] = [
    "swap",
    "unswap",
    "poll",
    "debounce",
    "angle-snap",
    "ripple",
    "motion-sync",
    "dpi",
    "stage",
    "lod",
    "bind",
    "profile",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("{msg}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(|s| s.to_lowercase());
    // A calibration run that died leaves its temporary DPI behind; any command fixes that first
    // (the portal does it itself, and help shouldn't touch the mice).
    if !matches!(cmd.as_deref(), Some("help" | "-h" | "--help" | "portal")) {
        restore_interrupted_test_dpi();
    }
    match cmd.as_deref() {
        None => {
            // Like the Python status: each block on its own, a failure printed in its place.
            let blocks: [fn() -> Result<(), String>; 3] =
                [|| pulsar_info(None), razer_info, show_rotation];
            for block in blocks {
                if let Err(msg) = block() {
                    say!("{msg}");
                }
                say!();
            }
            Ok(())
        }
        Some("info") => pulsar_info(parse_profile(&args[1..])?),
        Some(c) if PULSAR_WRITES.contains(&c) => pulsar_write(c, &args[1..]),
        Some("razer" | "viper") => razer_info(),
        Some("rotation") => match &args[1..] {
            [] => show_rotation(),
            [who, deg] => set_rotation(who, deg),
            _ => Err("usage: set-mouse rotation [pulsar|razer|m42 DEGREES]".into()),
        },
        Some("reload") => reload(),
        Some("help" | "-h" | "--help") => {
            say!("{USAGE}");
            Ok(())
        }
        Some("portal") => Err(
            "The portal isn't in the Rust version yet; the Python `set-mouse portal` has it".into(),
        ),
        Some(_) => Err(format!(
            "unknown command '{}'. Try: set-mouse help",
            args[0]
        )),
    }
}

// ------------------------------------------------------------------ files next to the tool

fn restore_interrupted_test_dpi() {
    let path = data_dir().join(dpi_restore::FILE_NAME);
    if !path.exists() {
        return;
    }
    match dpi_restore::restore_if_pending(&path) {
        Ok(fixed) if !fixed.is_empty() => {
            // Python prints the dict: {'Pulsar X2A': 3200, 'Razer Viper': 3200}
            let items: Vec<String> = fixed.iter().map(|(n, d)| format!("'{n}': {d}")).collect();
            say!(
                "Restored DPI after an interrupted calibration: {{{}}}",
                items.join(", ")
            );
        }
        Ok(_) => {}
        Err(e) => say!("Couldn't restore DPI from an interrupted calibration yet: {e}"),
    }
}

/// Append a timestamped line to `startup.log` and print it.
fn log(msg: &str) {
    let line = shared::append_log(msg);
    say!("{line}");
}

// ------------------------------------------------------------------ RawAccel

/// Mice this tool knows by name, as Raw Input lists them.
fn known_mice() -> Vec<RawMouse> {
    rawinput::mice()
        .into_iter()
        .filter(RawMouse::is_known)
        .collect()
}

fn show_rotation() -> Result<(), String> {
    let Some(s) = rawaccel::load().map_err(|e| e.to_string())? else {
        say!("RawAccel settings.json not found");
        return Ok(());
    };
    say!("RawAccel rotation");
    for m in rawaccel::summary(&s.cfg, &known_mice()) {
        let shared = if m.shared_with.is_empty() {
            String::new()
        } else {
            format!("   (shared with {})", m.shared_with.join(", "))
        };
        say!(
            "  {:<12} {:+.1} deg   profile '{}'{shared}",
            m.name,
            m.rotation,
            m.profile
        );
    }
    Ok(())
}

fn set_rotation(who: &str, deg: &str) -> Result<(), String> {
    let who = who.to_lowercase();
    let Some(&(_, name)) = ALIASES.iter().find(|(a, _)| *a == who) else {
        let names: Vec<&str> = ALIASES.iter().map(|(a, _)| *a).collect();
        return Err(format!("which mouse? use one of: {}", names.join(", ")));
    };
    let deg: f64 = deg
        .trim()
        .parse()
        .map_err(|_| "rotation must be a number of degrees, e.g. -2.6".to_string())?;
    let mouse = known_mice()
        .into_iter()
        .find(|m| m.name == name)
        .ok_or(format!("{name} isn't plugged in"))?;
    let backup = rawaccel::apply(&[Change {
        id: mouse.id,
        name: name.into(),
        rotation: deg,
    }])
    .map_err(|e| e.to_string())?;
    let file = backup.file_name().map(|f| f.to_string_lossy().into_owned());
    say!(
        "{name}: rotation {deg:+.1} deg, loaded into RawAccel (backup: {})",
        file.unwrap_or_default()
    );
    show_rotation()
}

/// What the sign-in task does: load settings.json into the driver, log the result.
fn reload() -> Result<(), String> {
    match rawaccel::reload(6, Duration::from_secs(5)) {
        Ok(line) => {
            log(&line);
            Ok(())
        }
        Err(line) => {
            log(&line);
            Err(String::new()) // already said; just exit with 1
        }
    }
}

/// `[]`, `-p N` or `--profile N`, with N in 1-5.
fn parse_profile(rest: &[String]) -> Result<Option<u8>, String> {
    match rest {
        [] => Ok(None),
        [flag, n] if flag == "-p" || flag == "--profile" => match n.parse::<u8>() {
            Ok(p @ 1..=5) => Ok(Some(p)),
            _ => Err(format!("profile must be 1-5, not '{n}'")),
        },
        _ => Err(format!("usage: set-mouse info [-p 1-5]\n\n{USAGE}")),
    }
}

/// A Pulsar write command's arguments: positionals, plus `-p/--profile N` and `--active N`
/// where the command takes them (also as `-pN`, `--profile=N`, `--active=N`, like argparse).
struct Args {
    pos: Vec<String>,
    profile: Option<u8>,
    active: Option<u8>,
}

fn parse_args(cmd: &str, rest: &[String]) -> Result<Args, String> {
    let takes_profile = matches!(cmd, "swap" | "unswap" | "dpi" | "stage" | "lod" | "bind");
    let mut a = Args {
        pos: Vec::new(),
        profile: None,
        active: None,
    };
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ if arg.starts_with("-p") && arg.len() > 2 => ("-p", Some(arg[2..].to_string())),
            _ => (arg.as_str(), None),
        };
        let mut value = |name: &str| {
            inline
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or(format!("{name} needs a number"))
        };
        match flag {
            "-p" | "--profile" if takes_profile => {
                let v = value("-p")?;
                a.profile = match v.parse::<u8>() {
                    Ok(p @ 1..=5) => Some(p),
                    _ => return Err(format!("profile must be 1-5, not '{v}'")),
                };
            }
            "--active" if cmd == "dpi" => {
                let v = value("--active")?;
                a.active = Some(
                    v.parse()
                        .map_err(|_| format!("--active: '{v}' isn't a number"))?,
                );
            }
            // "-" alone is a key (bind thumb1 -), so only treat "-x"/"--x" as flags
            f if f.len() > 1 && f.starts_with('-') && f.parse::<i64>().is_err() => {
                return Err(format!("set-mouse {cmd}: unknown option '{arg}'"));
            }
            _ => a.pos.push(arg.clone()),
        }
    }
    Ok(a)
}

fn number<T: std::str::FromStr>(text: &str, what: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{what} must be a number, not '{text}'"))
}

fn on_off(text: &str) -> Result<bool, String> {
    match text.to_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Ok(true),
        "off" | "0" | "false" | "no" => Ok(false),
        _ => Err(format!("use on or off, not '{text}'")),
    }
}

fn on_off_word(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// What a write command will do, worked out (and checked) before the mouse is opened.
enum Plan {
    Settings(Vec<Setting>),
    ActiveProfile(u8),
}

fn plan(cmd: &str, a: &Args) -> Result<Plan, String> {
    let usage = |u: &str| Err(format!("usage: set-mouse {u}"));
    let one = |u: &str| -> Result<&String, String> {
        match a.pos.as_slice() {
            [v] => Ok(v),
            _ => Err(format!("usage: set-mouse {u}")),
        }
    };
    let s = match cmd {
        "swap" | "unswap" => {
            if !a.pos.is_empty() {
                return usage(&format!("{cmd} [-p 1-5]"));
            }
            let (l, r) = if cmd == "swap" {
                (Binding::parse("right"), Binding::parse("left"))
            } else {
                (Ok(Button::Left.factory()), Ok(Button::Right.factory()))
            };
            let (l, r) = (l.map_err(|e| e.to_string())?, r.map_err(|e| e.to_string())?);
            vec![
                Setting::Button(Button::Left, l),
                Setting::Button(Button::Right, r),
            ]
        }
        "poll" => vec![Setting::Polling(number(
            one("poll 125|250|500|1000")?,
            "polling rate",
        )?)],
        "debounce" => vec![Setting::Debounce(number(one("debounce MS")?, "debounce")?)],
        "angle-snap" | "ripple" | "motion-sync" => {
            let t = Toggle::ALL
                .into_iter()
                .find(|t| t.name() == cmd)
                .expect("listed");
            vec![Setting::Toggle(t, on_off(one(&format!("{cmd} on|off"))?)?)]
        }
        "dpi" => {
            if a.pos.is_empty() {
                return usage("dpi DPI [DPI ...] [--active N] [-p 1-5]");
            }
            let dpis = a
                .pos
                .iter()
                .map(|d| number::<u16>(d, "DPI"))
                .collect::<Result<Vec<_>, _>>()?;
            vec![Setting::DpiStages {
                dpis,
                active: a.active.unwrap_or(1),
            }]
        }
        "stage" => vec![Setting::DpiStage(number(
            one("stage 1-6 [-p 1-5]")?,
            "stage",
        )?)],
        "lod" => vec![Setting::Lod(number(one("lod 1|2 [-p 1-5]")?, "lift-off")?)],
        "bind" => {
            let [btn, func] = a.pos.as_slice() else {
                return usage("bind BUTTON FUNCTION [-p 1-5]   (see set-mouse help)");
            };
            let names: Vec<_> = Button::ALL.iter().map(|b| b.name()).collect();
            let btn = Button::from_name(btn)
                .ok_or(format!("button must be one of: {}", names.join(", ")))?;
            vec![Setting::Button(
                btn,
                btn.parse_binding(func).map_err(|e| e.to_string())?,
            )]
        }
        "profile" => {
            let v = one("profile 1-5")?;
            return match v.parse::<u8>() {
                Ok(p @ 1..=5) => Ok(Plan::ActiveProfile(p)),
                _ => Err(format!("profile must be 1-5, not '{v}'")),
            };
        }
        _ => unreachable!("only PULSAR_WRITES get here"),
    };
    // Check every value now, so a typo never half-applies a command.
    for setting in &s {
        setting.packet(1).map_err(|e| e.to_string())?;
    }
    Ok(Plan::Settings(s))
}

fn pulsar_write(cmd: &str, rest: &[String]) -> Result<(), String> {
    let a = parse_args(cmd, rest)?;
    let plan = plan(cmd, &a)?;
    let e = |e: mouse_core::Error| e.to_string();
    let mut m = X2a::open().map_err(e)?;
    let settings = match plan {
        Plan::ActiveProfile(p) => {
            m.set_active_profile(p).map_err(e)?;
            say!("active profile now {}", m.active_profile().map_err(e)?);
            return Ok(());
        }
        Plan::Settings(s) => s,
    };
    let mut prof = 0;
    for setting in &settings {
        prof = m.write(setting, a.profile).map_err(e)?;
    }
    // Read back what the mouse now holds, and say it the way the Python does.
    match &settings[0] {
        Setting::Button(Button::Left, _) if settings.len() == 2 => say!(
            "profile {prof}: left button -> {}, right button -> {}",
            m.button(Button::Left, prof).map_err(e)?,
            m.button(Button::Right, prof).map_err(e)?
        ),
        Setting::Polling(_) => say!("polling rate now {} Hz", m.polling_hz().map_err(e)?),
        Setting::Debounce(_) => say!("debounce now {} ms", m.debounce_ms().map_err(e)?),
        Setting::Toggle(t, _) => {
            say!("{} now {}", t.name(), on_off_word(m.toggle(*t).map_err(e)?))
        }
        Setting::DpiStages { .. } => {
            let (act, stages) = m.dpi_stages(prof).map_err(e)?;
            let xs: Vec<String> = stages.iter().map(|(x, _)| x.to_string()).collect();
            say!(
                "profile {prof}: DPI stages [{}], active stage {act}",
                xs.join(", ")
            );
        }
        Setting::DpiStage(_) => say!(
            "profile {prof}: active DPI stage now {}",
            m.dpi_stage(prof).map_err(e)?
        ),
        Setting::Lod(_) => say!(
            "profile {prof}: lift-off now {}",
            x2a::describe_lod(m.lod_raw(prof).map_err(e)?)
        ),
        Setting::Button(btn, _) => say!(
            "profile {prof}: {} -> {}",
            btn.label(),
            m.button(*btn, prof).map_err(e)?
        ),
    }
    Ok(())
}

fn pulsar_info(profile: Option<u8>) -> Result<(), String> {
    let mut m = X2a::open().map_err(|e| e.to_string())?;
    let snap = m.snapshot(profile).map_err(|e| e.to_string())?;
    say_raw(&format_info(&snap));
    Ok(())
}

fn razer_info() -> Result<(), String> {
    let v = Viper::open().map_err(|e| e.to_string())?;
    say_raw(&format_razer(&v.info()));
    Ok(())
}

/// Same layout as `python razer_viper.py`, line for line.
fn format_razer(s: &viper::Info) -> String {
    let none = || "None".to_string();
    let mut out = format!(
        "Razer Viper   firmware {}\n",
        s.firmware.clone().unwrap_or_else(none)
    );
    let polling = match s.polling_code {
        Some(c) => {
            viper::polling_hz(c).map_or_else(|| format!("raw 0x{c:02x}"), |hz| hz.to_string())
        }
        None => none(),
    };
    out += &format!("  polling rate   {polling} Hz\n");
    if let Some((x, y)) = s.dpi {
        let xy = if x == y {
            String::new()
        } else {
            format!(" x {y}")
        };
        out += &format!("  current DPI    {x}{xy}\n");
    }
    if let Some((active, stages)) = &s.dpi_stages {
        let list: Vec<String> = stages
            .iter()
            .enumerate()
            .map(|(i, (x, _))| {
                if i + 1 == *active as usize {
                    format!("[{x}]")
                } else {
                    x.to_string()
                }
            })
            .collect();
        out += &format!(
            "  DPI stages     {}   ([] = active stage {active})\n",
            list.join("  ")
        );
    }
    for (field, err) in &s.errors {
        out += &format!("  ({field}: {err})\n");
    }
    out
}

/// Same layout as `python pulsar_x2a.py info`, line for line.
fn format_info(s: &Snapshot) -> String {
    let mut out = String::new();
    let mut line = |l: String| {
        out.push_str(&l);
        out.push('\n');
    };
    line(format!(
        "Pulsar X2A   firmware {}   active profile {}",
        s.firmware, s.active_profile
    ));
    line(String::new());
    line("Global".into());
    line(format!("  polling rate   {} Hz", s.polling_hz));
    line(format!("  debounce       {} ms", s.debounce_ms));
    for (t, on) in &s.toggles {
        line(format!(
            "  {:<14} {}",
            t.name(),
            if *on { "on" } else { "off" }
        ));
    }
    line(String::new());
    let active = if s.profile == s.active_profile {
        "  (active)"
    } else {
        ""
    };
    line(format!("Profile {}{active}", s.profile));
    let dpis: Vec<String> = s
        .dpi_stages
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            let v = if x == y {
                x.to_string()
            } else {
                format!("{x}x{y}")
            };
            if i + 1 == s.dpi_active as usize {
                format!("[{v}]")
            } else {
                v
            }
        })
        .collect();
    line(format!(
        "  DPI stages     {}   ([] = active stage {})",
        dpis.join("  "),
        s.dpi_active
    ));
    line(format!("  lift-off       {}", x2a::describe_lod(s.lod_raw)));
    line("  buttons".into());
    for (btn, binding) in &s.buttons {
        let mark = if *binding == btn.factory() {
            ""
        } else {
            "   <- changed"
        };
        line(format!("    {:<28} -> {binding}{mark}", btn.label()));
    }
    out
}
