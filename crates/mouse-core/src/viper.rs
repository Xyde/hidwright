//! Razer Viper (wired, USB 1532:0078): reads, and the writes OpenRazer supports for it.
//!
//! Reads are a port of `razer_viper.py`. Writes (ticket px-w1a0) follow OpenRazer's driver
//! (`razermouse_driver.c`, `razerchromacommon.c`; facts only, no code copied): the Viper has
//! one LED (the logo) with static, breathing, spectrum, reactive and off, a brightness, saved
//! DPI stages and the polling rate. Nothing for buttons, lift-off or angle snapping.
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
}

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

    #[test]
    fn colours_parse() {
        assert_eq!(Rgb::parse("#FF8800").unwrap(), Rgb(0xff, 0x88, 0));
        assert_eq!(Rgb::parse("00ff00").unwrap().to_string(), "00ff00");
        assert!(Rgb::parse("fff").is_err() && Rgb::parse("gg0000").is_err());
    }
}
