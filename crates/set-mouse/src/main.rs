//! set-mouse, Rust version (port in progress, epic px-0oek).
//!
//! So far it reads the Pulsar X2A and the Razer Viper. Everything else still lives in the Python
//! version, which stays on PATH as `set-mouse` until this one reaches parity.

use std::process::ExitCode;

use mouse_core::viper::{self, Viper};
use mouse_core::x2a::{self, Snapshot, X2a};

const USAGE: &str = "\
set-mouse (Rust, port in progress)

  set-mouse                 status of both mice (RawAccel rotation: not yet)
  set-mouse info [-p N]     all Pulsar settings; -p 1-5 shows another onboard profile
  set-mouse razer           Razer Viper settings (read-only)

Other commands are still in the Python version (the `set-mouse` on PATH).";

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
