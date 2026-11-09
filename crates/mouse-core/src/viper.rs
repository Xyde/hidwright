//! Razer Viper (wired, USB 1532:0078): reads, and the writes OpenRazer supports for it.
//!
//! Reads are a port of `razer_viper.py`. Writes (ticket px-w1a0) follow OpenRazer's driver
//! (`razermouse_driver.c`, `razerchromacommon.c`; facts only, no code copied): the Viper has
//! one LED (the logo) with static, breathing, spectrum, reactive and off, a brightness, saved
//! DPI stages and the polling rate. Nothing for lift-off or angle snapping.
//! Buttons are not in OpenRazer: their layout is from razerqdhid (MIT, `qdrazer/device.py`,
//! facts only) and every value here was read from or written to this Viper (ticket px-oqc3).
//! 90-byte feature report (report ID 0).
//!
//! `[0]` status, `[1]` transaction id, `[2..4]` remaining packets, `[4]` protocol type,
//! `[5]` data size, `[6]` command class, `[7]` command id, `[8..88]` arguments,
//! `[88]` XOR of bytes 2..88, `[89]` reserved.
//!
//! Other software (Razer Synapse) may talk to the mouse at the same time. Its requests and
//! ours share one reply buffer, so a reply can belong to someone else's command: we check
//! class and id and resend if they don't match (ticket px-wfee).

use std::thread::sleep;
use std::time::Duration;

use crate::hid::{self, Device};
use crate::{Error, Result};

pub const VID: u16 = 0x1532;
pub const PID: u16 = 0x0078;
pub const REPORT_LEN: usize = 90;
pub const NOSTORE: u8 = 0x00;
pub const VARSTORE: u8 = 0x01;

const STATUS_BUSY: u8 = 0x01;
const STATUS_OK: u8 = 0x02;
/// The transaction id differs between Razer models; these are tried in order.
const TRANSACTION_IDS: [u8; 3] = [0x1F, 0xFF, 0x3F];
const ATTEMPTS: u32 = 8;

fn status_name(s: u8) -> String {
    match s {
        0x00 => "new".into(),
        0x01 => "busy".into(),
        0x02 => "ok".into(),
        0x03 => "failed".into(),
        0x04 => "timeout".into(),
        0x05 => "not supported".into(),
        v => format!("0x{v:02x}"),
    }
}

/// Build one request, checksum included.
pub fn build_request(tid: u8, class: u8, cmd: u8, size: u8, args: &[u8]) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[1] = tid;
    r[5] = size;
    r[6] = class;
    r[7] = cmd;
    r[8..8 + args.len()].copy_from_slice(args);
    r[88] = r[2..88].iter().fold(0, |crc, b| crc ^ b);
    r
}

/// Transaction ids OpenRazer uses for the Viper: lighting (class 0x0F) and everything else.
/// The Viper here answers any of 0x1F/0xFF/0x3F (checked 2026-11-09); writes use OpenRazer's.
const TID_LED: u8 = 0x3F;
const TID_MISC: u8 = 0xFF;
const LOGO_LED: u8 = 0x04;
pub const MAX_STAGES: usize = 5;
/// What Synapse offers for the Viper (its device list: 100-16000, step 50).
pub const DPI_MIN: u16 = 100;
pub const DPI_MAX: u16 = 16000;
pub const DPI_STEP: u16 = 50;

/// An RGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// `ff8800` or `#ff8800`.
    pub fn parse(text: &str) -> Result<Rgb> {
        let t = text.trim().trim_start_matches('#');
        let byte = |i: usize| t.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (t.len(), byte(0), byte(2), byte(4)) {
            (6, Some(r), Some(g), Some(b)) => Ok(Rgb(r, g, b)),
            _ => Err(Error::Input(format!(
                "colour must be 6 hex digits like ff8800, not '{text}'"
            ))),
        }
    }
}

impl std::fmt::Display for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// What the logo LED does. Saved in the mouse (VARSTORE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logo {
    Off,
    Static(Rgb),
    Spectrum,
    /// Breathing in random colours.
    BreathRandom,
    BreathSingle(Rgb),
    BreathDual(Rgb, Rgb),
    /// Lights up when clicked; speed 1 (fast) to 4 (slow).
    Reactive(u8, Rgb),
}

impl Logo {
    pub fn describe(&self) -> String {
        match self {
            Logo::Off => "off".into(),
            Logo::Static(c) => format!("static {c}"),
            Logo::Spectrum => "spectrum".into(),
            Logo::BreathRandom => "breathing, random colours".into(),
            Logo::BreathSingle(c) => format!("breathing {c}"),
            Logo::BreathDual(a, b) => format!("breathing {a} / {b}"),
            Logo::Reactive(s, c) => format!("reactive {c}, speed {s}"),
        }
    }
}

/// Button ids as the mouse lists them (02/84), with a label for each.
/// Which side button sits where is not known yet (ticket px-31s4), so they keep their ids.
pub const BUTTONS: [(u8, &str); 10] = [
    (0x01, "left"),
    (0x02, "right"),
    (0x03, "wheel click"),
    (0x04, "side04"),
    (0x05, "side05"),
    (0x06, "side06"),
    (0x07, "side07"),
    (0x0E, "dpi button"),
    (0x09, "wheel up"),
    (0x0A, "wheel down"),
];

/// Names `bind` accepts for a button (besides the labels above without spaces).
pub fn button_id(name: &str) -> Option<u8> {
    let n = name.to_lowercase().replace(['-', '_', ' '], "");
    let n = n.strip_prefix("0x").unwrap_or(&n);
    match n {
        "left" => Some(0x01),
        "right" => Some(0x02),
        "middle" | "wheelclick" => Some(0x03),
        "dpi" | "dpibutton" => Some(0x0E),
        "wheelup" => Some(0x09),
        "wheeldown" => Some(0x0A),
        _ => {
            let id = n.strip_prefix("side").unwrap_or(n);
            match u8::from_str_radix(id, 16) {
                Ok(id @ 0x04..=0x07) => Some(id),
                _ => None,
            }
        }
    }
}

pub fn button_label(id: u8) -> String {
    BUTTONS
        .iter()
        .find(|(b, _)| *b == id)
        .map_or_else(|| format!("button {id:02x}"), |(_, l)| l.to_string())
}

/// What one button does, as the mouse stores it: a class and its value bytes.
/// Kept raw so anything the mouse reports can be shown and written back unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub class: u8,
    pub value: Vec<u8>,
}

const CLASS_DISABLED: u8 = 0x00;
const CLASS_MOUSE: u8 = 0x01;
const CLASS_KEY: u8 = 0x02;
const CLASS_DPI: u8 = 0x06;
const CLASS_HYPERSHIFT: u8 = 0x0C;

/// Function names `bind` accepts: each was read from or tested on this Viper.
const FUNCTIONS: [(&str, u8, u8); 8] = [
    ("left", CLASS_MOUSE, 0x01),
    ("right", CLASS_MOUSE, 0x02),
    ("middle", CLASS_MOUSE, 0x03),
    ("back", CLASS_MOUSE, 0x04),
    ("forward", CLASS_MOUSE, 0x05),
    ("wheel-up", CLASS_MOUSE, 0x09),
    ("wheel-down", CLASS_MOUSE, 0x0A),
    // next DPI stage, after the last one back to the first (pressed through all 5 stages)
    ("dpi-cycle", CLASS_DPI, 0x06),
];

impl Binding {
    fn simple(class: u8, value: u8) -> Binding {
        Binding {
            class,
            value: vec![value],
        }
    }

    /// The factory function of a button (Razer's defaults; this Viper's Hypershift layer
    /// holds the same back/forward pattern).
    pub fn factory(id: u8) -> Option<Binding> {
        Some(match id {
            0x01..=0x03 | 0x09 | 0x0A => Binding::simple(CLASS_MOUSE, id),
            0x04 | 0x06 => Binding::simple(CLASS_MOUSE, 0x04),
            0x05 | 0x07 => Binding::simple(CLASS_MOUSE, 0x05),
            0x0E => Binding::simple(CLASS_DPI, 0x06),
            _ => return None,
        })
    }

    /// A function name for button `id`: one of [`FUNCTIONS`] or `default`.
    pub fn parse(id: u8, name: &str) -> Result<Binding> {
        let n = name.to_lowercase();
        if n == "default" {
            return Binding::factory(id)
                .ok_or_else(|| Error::Input(format!("no factory function known for {id:02x}")));
        }
        FUNCTIONS
            .iter()
            .find(|(f, _, _)| *f == n)
            .map(|&(_, class, v)| Binding::simple(class, v))
            .ok_or_else(|| {
                let names: Vec<&str> = FUNCTIONS.iter().map(|f| f.0).collect();
                Error::Input(format!(
                    "unknown function '{name}'; use {} or default",
                    names.join(", ")
                ))
            })
    }

    /// From a 02/8C reply's arguments: `[3]` class, `[4]` length, `[5..]` value.
    pub fn from_reply(a: &[u8]) -> Result<Binding> {
        let len = a[4] as usize;
        if 5 + len > a.len() || len > 5 {
            return Err(Error::Protocol(format!("button reply length {len}")));
        }
        Ok(Binding {
            class: a[3],
            value: a[5..5 + len].to_vec(),
        })
    }

    pub fn describe(&self) -> String {
        let v = &self.value;
        let raw = || {
            let hex: Vec<String> = v.iter().map(|b| format!("{b:02x}")).collect();
            format!("class {:02x} [{}]", self.class, hex.join(" "))
        };
        match (self.class, v.as_slice()) {
            (CLASS_DISABLED, _) => "disabled".into(),
            (CLASS_MOUSE, [b]) => FUNCTIONS
                .iter()
                .find(|f| f.1 == CLASS_MOUSE && f.2 == *b)
                .map_or_else(raw, |f| f.0.into()),
            (CLASS_KEY, [mods, key]) => {
                let k = match key {
                    0x28 => "Enter".into(),
                    0x4F => "Right arrow".into(),
                    0x50 => "Left arrow".into(),
                    k => format!("HID key 0x{k:02x}"),
                };
                if *mods == 0 {
                    format!("key {k}")
                } else {
                    format!("key {k} (modifiers 0x{mods:02x})")
                }
            }
            (CLASS_DPI, [0x06]) => "dpi-cycle".into(),
            (CLASS_HYPERSHIFT, _) => "Hypershift".into(),
            _ => raw(),
        }
    }
}

/// One thing the Viper can be told to change. All are saved in the mouse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setting {
    /// 1-5 DPI values (x = y); `active` is 1-based.
    DpiStages {
        dpis: Vec<u16>,
        active: u8,
    },
    /// 125, 500 or 1000 Hz (OpenRazer's set builder knows no other rates).
    Polling(u32),
    /// Logo brightness, 0-255 as the mouse stores it.
    Brightness(u8),
    Logo(Logo),
    /// One button's function, normal layer of on-board profile 1 (the Viper's only slot).
    Button {
        id: u8,
        binding: Binding,
    },
}

/// On-board profile for button reads/writes: 1 is the saved slot (0 is temporary per
/// razerqdhid). Layer 0 = normal, 1 = Hypershift.
const BUTTON_PROFILE: u8 = 0x01;
const LAYER_NORMAL: u8 = 0x00;

impl Setting {
    /// (transaction id, class, command, data size, arguments), checked. Byte meanings from
    /// OpenRazer's builders, named in each arm.
    pub fn request(&self) -> Result<(u8, u8, u8, u8, Vec<u8>)> {
        let bad = |m: String| Err(Error::Input(m));
        Ok(match self {
            // razer_chroma_misc_set_dpi_stages: [store, active, count, then per stage
            // index, x hi, x lo, y hi, y lo, 0, 0]. OpenRazer numbers stages from 0; this
            // Viper reports them from 1 (as Synapse saved them), so we write 1-based to match.
            Setting::DpiStages { dpis, active } => {
                if dpis.is_empty() || dpis.len() > MAX_STAGES {
                    return bad(format!("1 to {MAX_STAGES} DPI stages, not {}", dpis.len()));
                }
                if let Some(d) = dpis
                    .iter()
                    .find(|&&d| !(DPI_MIN..=DPI_MAX).contains(&d) || d % DPI_STEP != 0)
                {
                    return bad(format!(
                        "DPI must be {DPI_MIN}-{DPI_MAX} in steps of {DPI_STEP}, not {d}"
                    ));
                }
                if !(1..=dpis.len()).contains(&(*active as usize)) {
                    return bad(format!("--active must be between 1 and {}", dpis.len()));
                }
                let mut a = vec![VARSTORE, *active, dpis.len() as u8];
                for (i, d) in dpis.iter().enumerate() {
                    let [hi, lo] = d.to_be_bytes();
                    a.extend([i as u8 + 1, hi, lo, hi, lo, 0, 0]);
                }
                (TID_MISC, 0x04, 0x06, 0x26, a)
            }
            // razer_chroma_misc_set_polling_rate: 1000 → 0x01, 500 → 0x02, 125 → 0x08
            Setting::Polling(hz) => {
                let code = match hz {
                    1000 => 0x01,
                    500 => 0x02,
                    125 => 0x08,
                    _ => return bad(format!("polling rate must be 125, 500 or 1000, not {hz}")),
                };
                (TID_MISC, 0x00, 0x05, 0x01, vec![code])
            }
            // razer_chroma_extended_matrix_brightness: [store, led, brightness]
            Setting::Brightness(b) => (TID_LED, 0x0F, 0x04, 0x03, vec![VARSTORE, LOGO_LED, *b]),
            // razer_chroma_extended_matrix_effect_*: [store, led, effect id, ...]
            Setting::Logo(logo) => {
                let mut a = vec![VARSTORE, LOGO_LED];
                let size = match *logo {
                    Logo::Off => {
                        a.push(0x00);
                        0x06
                    }
                    Logo::Static(Rgb(r, g, b)) => {
                        a.extend([0x01, 0, 0, 0x01, r, g, b]);
                        0x09
                    }
                    Logo::Spectrum => {
                        a.push(0x03);
                        0x06
                    }
                    Logo::BreathRandom => {
                        a.push(0x02);
                        0x06
                    }
                    Logo::BreathSingle(Rgb(r, g, b)) => {
                        a.extend([0x02, 0x01, 0, 0x01, r, g, b]);
                        0x09
                    }
                    Logo::BreathDual(Rgb(r1, g1, b1), Rgb(r2, g2, b2)) => {
                        a.extend([0x02, 0x02, 0, 0x02, r1, g1, b1, r2, g2, b2]);
                        0x0C
                    }
                    Logo::Reactive(speed, Rgb(r, g, b)) => {
                        if !(1..=4).contains(&speed) {
                            return bad(format!("reactive speed must be 1-4, not {speed}"));
                        }
                        a.extend([0x05, 0, speed, 0x01, r, g, b]);
                        0x09
                    }
                };
                (TID_LED, 0x0F, 0x02, size, a)
            }
            // razerqdhid set_button_function: [profile, button, layer, class, length, value]
            Setting::Button { id, binding } => {
                if !BUTTONS.iter().any(|(b, _)| b == id) {
                    return bad(format!("no Viper button {id:02x}"));
                }
                if binding.value.len() > 5 {
                    return bad("button value longer than 5 bytes".into());
                }
                let mut a = vec![BUTTON_PROFILE, *id, LAYER_NORMAL, binding.class];
                a.push(binding.value.len() as u8);
                a.extend(&binding.value);
                (TID_MISC, 0x02, 0x0C, 0x0A, a)
            }
        })
    }
}

/// Brightness 0-255 as a percentage, and back (70% ↔ 178, as Synapse stores it).
pub fn brightness_percent(raw: u8) -> u8 {
    ((raw as u32 * 100 + 127) / 255) as u8
}

pub fn brightness_raw(percent: u8) -> u8 {
    (percent.min(100) as u32 * 255 / 100) as u8
}

/// Polling-rate code → Hz.
pub fn polling_hz(code: u8) -> Option<u32> {
    match code {
        0x01 => Some(1000),
        0x02 => Some(500),
        0x04 => Some(250),
        0x08 => Some(125),
        _ => None,
    }
}

/// Everything readable, as `set-mouse razer` shows it. A field is None if reading it failed;
/// the reason is in `errors` under the field's name.
#[derive(Clone, Debug, Default)]
pub struct Info {
    pub firmware: Option<String>,
    pub serial: Option<String>,
    pub polling_code: Option<u8>,
    /// Current DPI (x, y), as set right now (NOSTORE).
    pub dpi: Option<(u16, u16)>,
    /// (active stage, 1-based; (x, y) per saved stage).
    pub dpi_stages: Option<(u8, Vec<(u16, u16)>)>,
    /// Logo brightness, 0-255.
    pub brightness: Option<u8>,
    /// Device mode: 0x00 normal, 0x03 driver (set by Synapse).
    pub device_mode: Option<u8>,
    /// (button id, function) in the order the mouse lists them, normal layer.
    pub buttons: Option<Vec<(u8, Binding)>>,
    pub errors: Vec<(&'static str, String)>,
}

pub struct Viper {
    dev: Device,
    tid: u8,
}

impl Viper {
    /// Open the Viper: the interface with a 90-byte feature report that answers a firmware query.
    pub fn open() -> Result<Self> {
        for iface in hid::interfaces(VID, PID)? {
            let Ok(dev) = Device::open(&iface, REPORT_LEN) else {
                continue;
            };
            let mut v = Viper {
                dev,
                tid: TRANSACTION_IDS[0],
            };
            for tid in TRANSACTION_IDS {
                v.tid = tid;
                if v.firmware().is_ok() {
                    return Ok(v);
                }
            }
        }
        Err(Error::NotFound(
            "Razer Viper not found or not answering (is Razer Synapse holding it?)".into(),
        ))
    }

    /// Send one request and return the reply's argument bytes (`[8..88]`).
    fn request(&self, class: u8, cmd: u8, size: u8, args: &[u8]) -> Result<Vec<u8>> {
        self.request_tid(self.tid, class, cmd, size, args)
    }

    /// Write one setting into the mouse (saved). Resending on another program's reply is
    /// safe: the same value lands twice at most.
    pub fn write(&self, setting: &Setting) -> Result<()> {
        let (tid, class, cmd, size, args) = setting.request()?;
        self.request_tid(tid, class, cmd, size, &args)?;
        Ok(())
    }

    /// Logo brightness, 0-255 (razer_chroma_extended_matrix_get_brightness: reply arg 2).
    pub fn brightness(&self) -> Result<u8> {
        Ok(self.request_tid(TID_LED, 0x0F, 0x84, 0x03, &[VARSTORE, LOGO_LED])?[2])
    }

    /// Device mode (razer_chroma_standard_get_device_mode): 0x00 normal, 0x03 driver.
    pub fn device_mode(&self) -> Result<u8> {
        Ok(self.request_tid(TID_MISC, 0x00, 0x84, 0x02, &[])?[0])
    }

    fn request_tid(&self, tid: u8, class: u8, cmd: u8, size: u8, args: &[u8]) -> Result<Vec<u8>> {
        let req = build_request(tid, class, cmd, size, args);
        let mut foreign = 0;
        for attempt in 0..ATTEMPTS {
            self.dev.set_feature(&req)?;
            sleep(Duration::from_millis(2 + 5 * attempt.min(4) as u64));
            let rsp = self.dev.get_feature()?;
            match rsp[0] {
                STATUS_BUSY => continue,
                STATUS_OK if rsp[6] == class && rsp[7] == cmd => return Ok(rsp[8..88].to_vec()),
                STATUS_OK => foreign += 1, // another program's reply: ask again
                s => {
                    return Err(Error::Protocol(format!(
                        "Razer reply {} for {class:02x}/{cmd:02x}",
                        status_name(s)
                    )));
                }
            }
        }
        Err(Error::Protocol(if foreign > 0 {
            format!(
                "replies for {class:02x}/{cmd:02x} kept belonging to another program's request \
                 ({foreign} of {ATTEMPTS}); is Razer Synapse running?"
            )
        } else {
            "Razer mouse stayed busy".into()
        }))
    }

    pub fn firmware(&self) -> Result<String> {
        let a = self.request(0x00, 0x81, 0x02, &[])?;
        Ok(format!("v{}.{:02}", a[0], a[1]))
    }

    pub fn serial(&self) -> Result<String> {
        let a = self.request(0x00, 0x82, 0x16, &[])?;
        let end = a[..22].iter().position(|&b| b == 0).unwrap_or(22);
        Ok(String::from_utf8_lossy(&a[..end]).into_owned())
    }

    pub fn polling_code(&self) -> Result<u8> {
        Ok(self.request(0x00, 0x85, 0x01, &[])?[0])
    }

    pub fn dpi(&self) -> Result<(u16, u16)> {
        let a = self.request(0x04, 0x85, 0x07, &[NOSTORE])?;
        Ok((
            u16::from_be_bytes([a[1], a[2]]),
            u16::from_be_bytes([a[3], a[4]]),
        ))
    }

    /// Change the current DPI without saving it (NOSTORE): the mouse forgets it on replug, and
    /// its DPI stages are left alone. Used only by calibration runs and their restore.
    pub fn set_dpi_temporary(&self, x: u16, y: u16) -> Result<()> {
        if !(100..=20000).contains(&x) || !(100..=20000).contains(&y) {
            return Err(Error::Input("DPI out of range".into()));
        }
        self.request(0x04, 0x05, 0x07, &temporary_dpi_args(x, y))?;
        Ok(())
    }

    pub fn dpi_stages(&self) -> Result<(u8, Vec<(u16, u16)>)> {
        let a = self.request(0x04, 0x86, 0x26, &[VARSTORE])?;
        Ok(parse_dpi_stages(&a))
    }

    /// One button's function (02/8C), normal layer of profile 1.
    pub fn button(&self, id: u8) -> Result<Binding> {
        let a = self.request_tid(
            TID_MISC,
            0x02,
            0x8C,
            0x0A,
            &[BUTTON_PROFILE, id, LAYER_NORMAL],
        )?;
        Binding::from_reply(&a)
    }

    /// Every button the mouse lists (02/84: count, then ids) with its function.
    pub fn buttons(&self) -> Result<Vec<(u8, Binding)>> {
        let list = self.request_tid(TID_MISC, 0x02, 0x84, 0x00, &[0x00])?;
        let n = (list[0] as usize).min(list.len() - 1);
        list[1..1 + n]
            .iter()
            .map(|&id| Ok((id, self.button(id)?)))
            .collect()
    }

    /// Read everything; failures are collected instead of stopping.
    pub fn info(&self) -> Info {
        fn take<T>(
            name: &'static str,
            r: Result<T>,
            errors: &mut Vec<(&'static str, String)>,
        ) -> Option<T> {
            r.map_err(|e| errors.push((name, e.to_string()))).ok()
        }
        let mut errors = Vec::new();
        Info {
            firmware: take("firmware", self.firmware(), &mut errors),
            serial: take("serial", self.serial(), &mut errors),
            polling_code: take("polling", self.polling_code(), &mut errors),
            dpi: take("dpi", self.dpi(), &mut errors),
            dpi_stages: take("dpi_stages", self.dpi_stages(), &mut errors),
            brightness: take("brightness", self.brightness(), &mut errors),
            device_mode: take("device_mode", self.device_mode(), &mut errors),
            buttons: take("buttons", self.buttons(), &mut errors),
            errors,
        }
    }
}

/// Arguments of a DPI set (04/05): NOSTORE, x and y big-endian, 2 reserved.
fn temporary_dpi_args(x: u16, y: u16) -> [u8; 7] {
    let ([xh, xl], [yh, yl]) = (x.to_be_bytes(), y.to_be_bytes());
    [NOSTORE, xh, xl, yh, yl, 0, 0]
}

/// Stage reply arguments: `[1]` active stage, `[2]` count, then 7 bytes per stage from `[3]`:
/// index, x (big-endian u16), y (big-endian u16), 2 reserved.
fn parse_dpi_stages(a: &[u8]) -> (u8, Vec<(u16, u16)>) {
    let (active, count) = (a[1], a[2] as usize);
    let stages = (0..count.min(5))
        .map(|i| {
            let s = &a[3 + 7 * i..];
            (
                u16::from_be_bytes([s[1], s[2]]),
                u16::from_be_bytes([s[3], s[4]]),
            )
        })
        .collect();
    (active, stages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// Requests the Python reference sends (captured through a fake hid.dll), transaction id 0x1F.
    #[test]
    fn requests_match_python() {
        let pad = |s: &str| format!("{s:0<176}");
        let cases: [(u8, u8, u8, &[u8], String); 5] = [
            (0x00, 0x81, 0x02, &[], pad("001f000000020081") + "8300"),
            (0x00, 0x82, 0x16, &[], pad("001f000000160082") + "9400"),
            (0x00, 0x85, 0x01, &[], pad("001f000000010085") + "8400"),
            (
                0x04,
                0x85,
                0x07,
                &[NOSTORE],
                pad("001f000000070485") + "8600",
            ),
            (
                0x04,
                0x86,
                0x26,
                &[VARSTORE],
                pad("001f00000026048601") + "a500",
            ),
        ];
        for (class, cmd, size, args, want) in cases {
            assert_eq!(
                hex(&build_request(0x1F, class, cmd, size, args)),
                want,
                "{class:02x}/{cmd:02x}"
            );
        }
    }

    /// `set_dpi_temporary(3200)` and `(400, 1600)` as the Python sends them (fake hid.dll).
    #[test]
    fn temporary_dpi_matches_python() {
        let cases: [((u16, u16), &str); 2] = [
            (
                (3200, 3200),
                "001f000000070405000c800c800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600",
            ),
            (
                (400, 1600),
                "001f0000000704050001900640000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000d100",
            ),
        ];
        for ((x, y), want) in cases {
            let req = build_request(0x1F, 0x04, 0x05, 0x07, &temporary_dpi_args(x, y));
            assert_eq!(hex(&req), want, "{x}x{y}");
        }
    }

    /// Same canned reply the Python parsed to (5, [(400,400), (800,800), (1800,1800), (2400,2400), (3200,3200)]).
    #[test]
    fn dpi_stages_parse_like_python() {
        let a = [
            0x01, 5, 5, 1, 0x01, 0x90, 0x01, 0x90, 0, 0, 2, 0x03, 0x20, 0x03, 0x20, 0, 0, 3, 0x07,
            0x08, 0x07, 0x08, 0, 0, 4, 0x09, 0x60, 0x09, 0x60, 0, 0, 5, 0x0C, 0x80, 0x0C, 0x80, 0,
            0,
        ];
        assert_eq!(
            parse_dpi_stages(&a),
            (
                5,
                vec![
                    (400, 400),
                    (800, 800),
                    (1800, 1800),
                    (2400, 2400),
                    (3200, 3200)
                ]
            )
        );
    }

    #[test]
    fn polling_codes() {
        assert_eq!(polling_hz(0x01), Some(1000));
        assert_eq!(polling_hz(0x08), Some(125));
        assert_eq!(polling_hz(0x03), None);
    }

    fn args_hex(s: &Setting) -> (u8, u8, u8, u8, String) {
        let (tid, class, cmd, size, args) = s.request().unwrap();
        (tid, class, cmd, size, hex(&args))
    }

    /// The bytes this Viper reported for its stages (2026-11-09) and accepted back unchanged.
    #[test]
    fn dpi_stage_write_matches_what_the_mouse_reports() {
        let s = Setting::DpiStages {
            dpis: vec![400, 800, 1800, 2400, 3200],
            active: 5,
        };
        assert_eq!(
            args_hex(&s),
            (
                0xFF,
                0x04,
                0x06,
                0x26,
                "010505010190019000000203200320000003070807080000040960096000000\
                 50c800c800000"
                    .replace(' ', "")
            )
        );
    }

    /// Layouts from OpenRazer's builders (razerchromacommon.c), logo LED 0x04, VARSTORE 0x01.
    #[test]
    fn writes_follow_openrazer() {
        let red = Rgb(0xff, 0, 0);
        let blue = Rgb(0, 0, 0xff);
        /// (transaction id, class, command, data size, arguments as hex)
        type Expected<'a> = (u8, u8, u8, u8, &'a str);
        let cases: [(Setting, Expected); 10] = [
            (Setting::Polling(1000), (0xFF, 0x00, 0x05, 0x01, "01")),
            (Setting::Polling(500), (0xFF, 0x00, 0x05, 0x01, "02")),
            (Setting::Polling(125), (0xFF, 0x00, 0x05, 0x01, "08")),
            (Setting::Brightness(178), (0x3F, 0x0F, 0x04, 0x03, "0104b2")),
            (Setting::Logo(Logo::Off), (0x3F, 0x0F, 0x02, 0x06, "010400")),
            (
                Setting::Logo(Logo::Spectrum),
                (0x3F, 0x0F, 0x02, 0x06, "010403"),
            ),
            (
                Setting::Logo(Logo::Static(red)),
                (0x3F, 0x0F, 0x02, 0x09, "010401000001ff0000"),
            ),
            (
                Setting::Logo(Logo::BreathSingle(red)),
                (0x3F, 0x0F, 0x02, 0x09, "010402010001ff0000"),
            ),
            (
                Setting::Logo(Logo::BreathDual(red, blue)),
                (0x3F, 0x0F, 0x02, 0x0C, "010402020002ff00000000ff"),
            ),
            (
                Setting::Logo(Logo::Reactive(2, red)),
                (0x3F, 0x0F, 0x02, 0x09, "010405000201ff0000"),
            ),
        ];
        for (s, (tid, class, cmd, size, args)) in cases {
            assert_eq!(
                args_hex(&s),
                (tid, class, cmd, size, args.to_string()),
                "{s:?}"
            );
        }
        assert_eq!(
            args_hex(&Setting::Logo(Logo::BreathRandom)).4,
            "010402",
            "breathing random"
        );
    }

    #[test]
    fn bad_viper_values_are_refused() {
        for s in [
            Setting::Polling(250), // OpenRazer's set builder has no 250
            Setting::DpiStages {
                dpis: vec![],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![800; 6],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![125],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![16050],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![800],
                active: 2,
            },
            Setting::Logo(Logo::Reactive(5, Rgb(1, 2, 3))),
        ] {
            assert!(matches!(s.request(), Err(Error::Input(_))), "{s:?}");
        }
        assert!(
            Setting::DpiStages {
                dpis: vec![850],
                active: 1
            }
            .request()
            .is_ok()
        ); // step 50
    }

    #[test]
    fn brightness_percent_round_trips() {
        assert_eq!((brightness_raw(70), brightness_percent(178)), (178, 70));
        for p in 0..=100 {
            assert_eq!(brightness_percent(brightness_raw(p)), p);
        }
    }

    /// The four 02/0C writes this Viper accepted and read back on 2026-11-09 (sent padded to
    /// 10 argument bytes by the Python diagnostic; the frame is the same).
    #[test]
    fn button_writes_match_what_the_mouse_accepted() {
        let cases = [
            (0x04, "back", "01040001010400000000"),
            (0x05, "forward", "01050001010500000000"),
            (0x07, "forward", "01070001010500000000"),
            (0x0E, "dpi-cycle", "010e0006010600000000"),
            (0x06, "default", "01060001010400000000"), // the same-value test write
        ];
        for (id, func, sent) in cases {
            let s = Setting::Button {
                id,
                binding: Binding::parse(id, func).unwrap(),
            };
            let (tid, class, cmd, size, args) = s.request().unwrap();
            assert_eq!((class, cmd, size), (0x02, 0x0C, 0x0A));
            let sent: Vec<u8> = (0..sent.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&sent[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(
                build_request(tid, class, cmd, size, &args),
                build_request(tid, class, cmd, size, &sent),
                "{id:02x} {func}"
            );
        }
    }

    /// 02/8C replies from this Viper (first 10 argument bytes), before and after the fix.
    #[test]
    fn button_replies_decode() {
        let cases = [
            (
                [0x01, 0x04, 0x01, 0x02, 0x02, 0x00, 0x28, 0, 0, 0],
                "key Enter",
            ),
            (
                [0x01, 0x05, 0x01, 0x02, 0x02, 0x00, 0x4F, 0, 0, 0],
                "key Right arrow",
            ),
            (
                [0x01, 0x07, 0x01, 0x02, 0x02, 0x00, 0x50, 0, 0, 0],
                "key Left arrow",
            ),
            (
                [0x01, 0x0E, 0x01, 0x0C, 0x01, 0x01, 0, 0, 0, 0],
                "Hypershift",
            ),
            ([0x01, 0x06, 0x01, 0x01, 0x01, 0x04, 0, 0, 0, 0], "back"),
            ([0x01, 0x09, 0x00, 0x01, 0x01, 0x09, 0, 0, 0, 0], "wheel-up"),
            (
                [0x01, 0x0E, 0x01, 0x06, 0x01, 0x06, 0, 0, 0, 0],
                "dpi-cycle",
            ),
        ];
        for (reply, want) in cases {
            let b = Binding::from_reply(&reply).unwrap();
            assert_eq!(b.describe(), want);
            // what is read can be written back unchanged
            let (_, _, _, _, args) = Setting::Button {
                id: reply[1],
                binding: b,
            }
            .request()
            .unwrap();
            assert_eq!(&args[3..], &reply[3..3 + args.len() - 3]);
        }
        assert_eq!(
            Binding::from_reply(&[1, 0x04, 1, 0x02, 0x02, 0x00, 0x28, 0, 0, 0]).unwrap(),
            Binding {
                class: 0x02,
                value: vec![0x00, 0x28]
            }
        );
    }

    #[test]
    fn button_names_and_factory() {
        assert_eq!(button_id("dpi"), Some(0x0E));
        assert_eq!(button_id("side04"), Some(0x04));
        assert_eq!(button_id("side-07"), Some(0x07));
        assert_eq!(button_id("05"), Some(0x05));
        assert_eq!(button_id("Wheel-Click"), Some(0x03));
        assert_eq!(button_id("side08"), None);
        assert_eq!(button_id("thumb1"), None);
        for (id, _) in BUTTONS {
            assert!(Binding::factory(id).is_some(), "{id:02x}");
        }
        assert_eq!(Binding::factory(0x07).unwrap().describe(), "forward");
        assert!(matches!(
            Binding::parse(0x04, "enter"),
            Err(Error::Input(_))
        ));
        assert!(matches!(
            Setting::Button {
                id: 0x08,
                binding: Binding::factory(0x04).unwrap()
            }
            .request(),
            Err(Error::Input(_))
        ));
    }

    #[test]
    fn colours_parse() {
        assert_eq!(Rgb::parse("#FF8800").unwrap(), Rgb(0xff, 0x88, 0));
        assert_eq!(Rgb::parse("00ff00").unwrap().to_string(), "00ff00");
        assert!(Rgb::parse("fff").is_err() && Rgb::parse("gg0000").is_err());
    }
}
