//! set-mouse, Rust version (port in progress, epic px-0oek).
//!
//! So far it reads both mice and writes every Pulsar X2A setting. RawAccel, the portal and the
//! Viper's temporary DPI still live in the Python version, which stays on PATH as `set-mouse`
//! until this one reaches parity. Messages match the Python's word for word.

use std::process::ExitCode;

use mouse_core::viper::{self, Viper};
use mouse_core::x2a::{self, Binding, Button, Setting, Snapshot, Toggle, X2a};

const USAGE: &str = "\
set-mouse (Rust, port in progress)

  set-mouse                       status of both mice (RawAccel rotation: not yet)
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

Not here yet (the Python `set-mouse` on PATH does them): portal, rotation, reload.";

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
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(|s| s.to_lowercase());
    match cmd.as_deref() {
        None => {
            // Like the Python status: each block on its own, a failure printed in its place.
            let blocks: [fn() -> Result<(), String>; 2] = [|| pulsar_info(None), razer_info];
            for block in blocks {
                if let Err(msg) = block() {
                    println!("{msg}");
                }
                println!();
            }
            Ok(())
        }
        Some("info") => pulsar_info(parse_profile(&args[1..])?),
        Some(c) if PULSAR_WRITES.contains(&c) => pulsar_write(c, &args[1..]),
        Some("razer" | "viper") => razer_info(),
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!(
            "'{other}' isn't in the Rust version yet; the Python `set-mouse {other}` still does it"
        )),
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
            println!("active profile now {}", m.active_profile().map_err(e)?);
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
        Setting::Button(Button::Left, _) if settings.len() == 2 => println!(
            "profile {prof}: left button -> {}, right button -> {}",
            m.button(Button::Left, prof).map_err(e)?,
            m.button(Button::Right, prof).map_err(e)?
        ),
        Setting::Polling(_) => println!("polling rate now {} Hz", m.polling_hz().map_err(e)?),
        Setting::Debounce(_) => println!("debounce now {} ms", m.debounce_ms().map_err(e)?),
        Setting::Toggle(t, _) => {
            println!("{} now {}", t.name(), on_off_word(m.toggle(*t).map_err(e)?))
        }
        Setting::DpiStages { .. } => {
            let (act, stages) = m.dpi_stages(prof).map_err(e)?;
            let xs: Vec<String> = stages.iter().map(|(x, _)| x.to_string()).collect();
            println!(
                "profile {prof}: DPI stages [{}], active stage {act}",
                xs.join(", ")
            );
        }
        Setting::DpiStage(_) => println!(
            "profile {prof}: active DPI stage now {}",
            m.dpi_stage(prof).map_err(e)?
        ),
        Setting::Lod(_) => println!(
            "profile {prof}: lift-off now {}",
            x2a::describe_lod(m.lod_raw(prof).map_err(e)?)
        ),
        Setting::Button(btn, _) => println!(
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
    print!("{}", format_info(&snap));
    Ok(())
}

fn razer_info() -> Result<(), String> {
    let v = Viper::open().map_err(|e| e.to_string())?;
    print!("{}", format_razer(&v.info()));
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
