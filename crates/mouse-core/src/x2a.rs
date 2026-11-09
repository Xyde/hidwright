//! Pulsar X2A (wired, USB 3710:1404): the configuration protocol, reads and writes.
//!
//! Port of `pulsar_x2a.py`. The protocol comes from the MIT-licensed community project
//! packerlschupfer/pulsar-mouse-linux (`drivers/x2a.py`), reverse-engineered from USB captures.
//!
//! Packet (64 bytes, feature report 0 on USB interface 3):
//! `[1]` category, `[2]` register (bit 7 set = read), `[3]` sub-command, `[6]` profile
//! (1-5; 0 for global reads, but global *writes* need the active profile to be saved, see
//! [`Setting`]), `[7..62]` payload, `[62..64]` little-endian u16 of `sum(bytes[0..62])`.
//! Reply byte 0: 0x00 not answered (yet), 0x01 answered, 0x02 pending, 0x03 refused.

use std::fmt;
use std::thread::sleep;
use std::time::Duration;

use crate::hid::{self, Device};
use crate::{Error, Result};

pub const VID: u16 = 0x3710;
pub const PID: u16 = 0x1404;
/// The configuration interface (`MI_03`, vendor-defined usage page).
const CONFIG_INTERFACE: i32 = 3;
pub const REPORT_SIZE: usize = 64;
pub const MAX_STAGES: usize = 6;

const NOT_ANSWERED: u8 = 0x00;
const ANSWERED: u8 = 0x01;
const PENDING: u8 = 0x02;

/// Build one request packet, checksum included.
pub fn build(cat: u8, reg: u8, sub: u8, profile: u8, payload: &[u8]) -> [u8; REPORT_SIZE] {
    let mut b = [0u8; REPORT_SIZE];
    b[1] = cat;
    b[2] = reg;
    b[3] = sub;
    b[6] = profile;
    b[7..7 + payload.len()].copy_from_slice(payload);
    let sum: u32 = b[..62].iter().map(|&x| x as u32).sum();
    b[62..64].copy_from_slice(&((sum & 0xFFFF) as u16).to_le_bytes());
    b
}

// ------------------------------------------------------------------ buttons and bindings

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Wheel,
    Thumb1,
    Thumb2,
    Thumb3,
    Thumb4,
    Dpi,
}

impl Button {
    pub const ALL: [Button; 8] = [
        Button::Left,
        Button::Right,
        Button::Wheel,
        Button::Thumb1,
        Button::Thumb2,
        Button::Thumb3,
        Button::Thumb4,
        Button::Dpi,
    ];

    pub fn id(self) -> u8 {
        match self {
            Button::Left => 0x01,
            Button::Right => 0x02,
            Button::Wheel => 0x03,
            Button::Thumb1 => 0x04,
            Button::Thumb2 => 0x05,
            Button::Thumb3 => 0x06,
            Button::Thumb4 => 0x07,
            Button::Dpi => 0x0B,
        }
    }

    /// Short name used on the command line (`set-mouse bind thumb1 …`).
    pub fn name(self) -> &'static str {
        match self {
            Button::Left => "left",
            Button::Right => "right",
            Button::Wheel => "wheel",
            Button::Thumb1 => "thumb1",
            Button::Thumb2 => "thumb2",
            Button::Thumb3 => "thumb3",
            Button::Thumb4 => "thumb4",
            Button::Dpi => "dpi",
        }
    }

    pub fn from_name(name: &str) -> Option<Button> {
        let name = name.to_lowercase();
        Button::ALL.into_iter().find(|b| b.name() == name)
    }

    /// Parse what this button should do: a function name, or `default` for its factory binding.
    pub fn parse_binding(self, text: &str) -> Result<Binding> {
        if text.trim().eq_ignore_ascii_case("default") {
            Ok(self.factory())
        } else {
            Binding::parse(text)
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::Left => "left",
            Button::Right => "right",
            Button::Wheel => "wheel click",
            Button::Thumb1 => "thumb1 (left side, back)",
            Button::Thumb2 => "thumb2 (left side, front)",
            Button::Thumb3 => "thumb3 (right side, back)",
            Button::Thumb4 => "thumb4 (right side, front)",
            Button::Dpi => "dpi button",
        }
    }

    /// What the button does out of the box. Note thumb3/thumb4 mirror thumb1/thumb2 (back/forward).
    pub fn factory(self) -> Binding {
        let (kind, a1) = match self {
            Button::Left => (kind::MOUSE, 0x01),
            Button::Right => (kind::MOUSE, 0x02),
            Button::Wheel => (kind::MOUSE, 0x03),
            Button::Thumb1 | Button::Thumb3 => (kind::MOUSE, 0x04),
            Button::Thumb2 | Button::Thumb4 => (kind::MOUSE, 0x05),
            Button::Dpi => (kind::DPI, 0x03),
        };
        Binding { kind, a1, a2: 0 }
    }
}

/// Binding type codes (first byte of a button binding).
pub mod kind {
    pub const DISABLED: u8 = 0x00;
    pub const MOUSE: u8 = 0x01;
    pub const KEY: u8 = 0x02;
    pub const SCROLL: u8 = 0x03;
    pub const PROFILE: u8 = 0x08;
    pub const DPI: u8 = 0x09;
    pub const MEDIA: u8 = 0x0D;
    pub const XCLICK: u8 = 0x0E;
}

/// What a button does: a type code and two arguments, as stored in the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub kind: u8,
    pub a1: u8,
    pub a2: u8,
}

const fn b(kind: u8, a1: u8) -> Binding {
    Binding { kind, a1, a2: 0 }
}

/// Named functions. Order matters for [`Binding::describe`]: the first match wins.
pub const SIMPLE_FUNCS: [(&str, Binding); 14] = [
    ("left", b(kind::MOUSE, 0x01)),
    ("right", b(kind::MOUSE, 0x02)),
    ("middle", b(kind::MOUSE, 0x03)),
    ("back", b(kind::MOUSE, 0x04)),
    ("forward", b(kind::MOUSE, 0x05)),
    ("scrollup", b(kind::SCROLL, 0x01)),
    ("scrolldown", b(kind::SCROLL, 0xFF)),
    ("dpi+", b(kind::DPI, 0x01)),
    ("dpi-", b(kind::DPI, 0x02)),
    ("dpiloop", b(kind::DPI, 0x03)),
    ("profile+", b(kind::PROFILE, 0x03)),
    ("profile-", b(kind::PROFILE, 0x04)),
    ("doubleclick", b(kind::XCLICK, 0x01)),
    ("disabled", b(kind::DISABLED, 0x00)),
];

/// Media keys: HID consumer-page usage codes.
pub const MEDIA: [(&str, u16); 11] = [
    ("play", 0x00CD),
    ("next", 0x00B5),
    ("prev", 0x00B6),
    ("stop", 0x00B7),
    ("mute", 0x00E2),
    ("vol+", 0x00E9),
    ("vol-", 0x00EA),
    ("eject", 0x00B8),
    ("calc", 0x0192),
    ("homepage", 0x0223),
    ("mediaplayer", 0x0183),
];

/// Keyboard modifier bits, in the order they're printed.
pub const MODS: [(&str, u8); 8] = [
    ("ctrl", 0x01),
    ("shift", 0x02),
    ("alt", 0x04),
    ("win", 0x08),
    ("rctrl", 0x10),
    ("rshift", 0x20),
    ("ralt", 0x40),
    ("rwin", 0x80),
];

/// Keys that aren't letters, digits or F-keys (HID keyboard-page usage codes).
const SPECIAL_KEYS: [(&str, u8); 30] = [
    ("enter", 0x28),
    ("esc", 0x29),
    ("backspace", 0x2A),
    ("tab", 0x2B),
    ("space", 0x2C),
    ("-", 0x2D),
    ("=", 0x2E),
    ("[", 0x2F),
    ("]", 0x30),
    ("\\", 0x31),
    (";", 0x33),
    ("'", 0x34),
    ("`", 0x35),
    (",", 0x36),
    (".", 0x37),
    ("/", 0x38),
    ("capslock", 0x39),
    ("printscreen", 0x46),
    ("scrolllock", 0x47),
    ("pause", 0x48),
    ("insert", 0x49),
    ("home", 0x4A),
    ("pageup", 0x4B),
    ("delete", 0x4C),
    ("end", 0x4D),
    ("pagedown", 0x4E),
    ("arrowright", 0x4F),
    ("arrowleft", 0x50),
    ("arrowdown", 0x51),
    ("arrowup", 0x52),
];

/// Name of a HID keyboard usage code: a-z, 1-0, f1-f12, or one of [`SPECIAL_KEYS`].
pub fn key_name(code: u8) -> Option<String> {
    match code {
        0x04..=0x1D => Some(char::from(b'a' + (code - 0x04)).to_string()),
        0x1E..=0x27 => Some(((code - 0x1E + 1) % 10).to_string()),
        0x3A..=0x45 => Some(format!("f{}", code - 0x39)),
        _ => SPECIAL_KEYS
            .iter()
            .find(|(_, c)| *c == code)
            .map(|(n, _)| n.to_string()),
    }
}

/// HID keyboard usage code for a key name; the inverse of [`key_name`].
pub fn key_code(name: &str) -> Option<u8> {
    match name.as_bytes() {
        [c @ b'a'..=b'z'] => return Some(0x04 + (c - b'a')),
        [b'0'] => return Some(0x27),
        [c @ b'1'..=b'9'] => return Some(0x1E + (c - b'1')),
        _ => {}
    }
    if let Some(n) = (1..=12u8).find(|n| name == format!("f{n}")) {
        return Some(0x39 + n);
    }
    SPECIAL_KEYS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}

/// Parse an integer the way Python's `int(text, 0)` does for the forms people type:
/// `0x` hex, `0o` octal, `0b` binary, or plain decimal.
fn parse_int(text: &str) -> Option<u32> {
    let t = text.replace('_', "");
    let (digits, radix) = match t.get(..2) {
        Some("0x") => (&t[2..], 16),
        Some("0o") => (&t[2..], 8),
        Some("0b") => (&t[2..], 2),
        _ => (&t[..], 10),
    };
    u32::from_str_radix(digits, radix).ok()
}

impl Binding {
    /// Parse a function name as `set-mouse bind` takes it: a name from [`SIMPLE_FUNCS`]
    /// (`back`, `dpiloop`, …), a media key (`play`, `media(0x00cd)`), or a keyboard key or
    /// combo (`f5`, `ctrl+c`, `shift`). Same rules and order as the Python `parse_function`.
    pub fn parse(text: &str) -> Result<Binding> {
        let t = text.trim().to_lowercase();
        if let Some((_, f)) = SIMPLE_FUNCS.iter().find(|(n, _)| *n == t) {
            return Ok(*f);
        }
        let media = |u: u16| Binding {
            kind: kind::MEDIA,
            a1: (u & 0xFF) as u8,
            a2: (u >> 8) as u8,
        };
        if let Some((_, u)) = MEDIA.iter().find(|(n, _)| *n == t) {
            return Ok(media(*u));
        }
        if let Some(inner) = t.strip_prefix("media(").and_then(|s| s.strip_suffix(')')) {
            return match parse_int(inner.trim()) {
                Some(u) if u <= 0xFFFF => Ok(media(u as u16)),
                _ => Err(Error::Input(format!(
                    "media usage must be a number 0-0xFFFF, not '{inner}'"
                ))),
            };
        }
        // keyboard: [mod+[mod+]]key, or modifiers alone (e.g. "shift")
        let (mut mods, mut key) = (0u8, 0u8);
        for p in t.split('+') {
            if let Some((_, bit)) = MODS.iter().find(|(n, _)| *n == p) {
                mods |= bit;
            } else if let (Some(code), 0) = (key_code(p), key) {
                key = code;
            } else {
                return Err(Error::Input(format!("don't know '{p}' in '{text}'")));
            }
        }
        Ok(Binding {
            kind: kind::KEY,
            a1: mods,
            a2: key,
        })
    }

    /// Human-readable name, e.g. "right click", "ctrl+c", "play", "media(0x1234)".
    pub fn describe(&self) -> String {
        if let Some((name, _)) = SIMPLE_FUNCS.iter().find(|(_, f)| f == self) {
            return match *name {
                "left" | "right" | "middle" => format!("{name} click"),
                _ => name.to_string(),
            };
        }
        match self.kind {
            kind::MEDIA => {
                let usage = self.a1 as u16 | (self.a2 as u16) << 8;
                MEDIA
                    .iter()
                    .find(|(_, u)| *u == usage)
                    .map(|(n, _)| n.to_string())
                    .unwrap_or_else(|| format!("media(0x{usage:04x})"))
            }
            kind::KEY => {
                let mut names: Vec<String> = MODS
                    .iter()
                    .filter(|(_, bit)| self.a1 & bit != 0)
                    .map(|(n, _)| n.to_string())
                    .collect();
                if self.a2 != 0 {
                    names.push(
                        key_name(self.a2).unwrap_or_else(|| format!("key 0x{:02x}", self.a2)),
                    );
                }
                if names.is_empty() {
                    "nothing".into()
                } else {
                    names.join("+")
                }
            }
            _ => format!(
                "type 0x{:02x} (0x{:02x}, 0x{:02x})",
                self.kind, self.a1, self.a2
            ),
        }
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

// ------------------------------------------------------------------ global on/off settings

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    AngleSnap,
    Ripple,
    MotionSync,
}

impl Toggle {
    pub const ALL: [Toggle; 3] = [Toggle::AngleSnap, Toggle::Ripple, Toggle::MotionSync];

    pub fn name(self) -> &'static str {
        match self {
            Toggle::AngleSnap => "angle-snap",
            Toggle::Ripple => "ripple",
            Toggle::MotionSync => "motion-sync",
        }
    }

    fn register(self) -> u8 {
        match self {
            Toggle::AngleSnap => 0x04,
            Toggle::Ripple => 0x03,
            Toggle::MotionSync => 0x05,
        }
    }
}

/// Lift-off distance as stored (0 = 1 mm, 1 = 2 mm). Encoding unverified (ticket px-zd1z).
pub fn describe_lod(raw: u8) -> String {
    match raw {
        0 => "1 mm".into(),
        1 => "2 mm".into(),
        v => format!("unknown (raw 0x{v:02x})"),
    }
}

// ------------------------------------------------------------------ writes

pub const DPI_MIN: u16 = 100;
pub const DPI_MAX: u16 = 26000;
pub const DPI_STEP: u16 = 100;
pub const DEBOUNCE_MAX_MS: u8 = 20;
pub const POLLING_HZ: [u32; 4] = [125, 250, 500, 1000];

/// One thing the mouse can be told to change. [`X2a::write`] sends it.
///
/// Global settings (polling, debounce, toggles) exist once in the mouse, not per profile.
/// The profile byte of a *write* still matters: 0 changes only RAM (lost when the mouse loses
/// power), 1-5 also saves to flash. So globals are sent with the active profile's number.
/// (Found 2026-11-09, ticket px-337u; upstream sends 0.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setting {
    /// 125, 250, 500 or 1000 Hz. Global.
    Polling(u32),
    /// 0-20 ms. Global. (0 lets the switch bounce; 1 tested chatter-free.)
    Debounce(u8),
    /// Angle snapping, ripple control or motion sync. Global.
    Toggle(Toggle, bool),
    /// Up to 6 DPI values (100-26000, step 100); `active` is 1-based. Per profile.
    DpiStages { dpis: Vec<u16>, active: u8 },
    /// Switch to DPI stage 1-6. Per profile.
    DpiStage(u8),
    /// Lift-off distance, 1 or 2 mm. Per profile. Encoding unverified (px-zd1z).
    Lod(u8),
    /// What a button does. Per profile.
    Button(Button, Binding),
}

impl Setting {
    /// True if this setting exists once for the whole mouse rather than per profile.
    pub fn is_global(&self) -> bool {
        matches!(
            self,
            Setting::Polling(_) | Setting::Debounce(_) | Setting::Toggle(..)
        )
    }

    /// What a person may ask for, beyond what the protocol can encode: DPI values 100-26000
    /// in steps of 100. Restoring values read from the mouse skips this (it may hold e.g. 1850
    /// set by Pulsar Fusion), see [`X2a::write_unchecked`].
    pub fn check(&self) -> Result<()> {
        if let Setting::DpiStages { dpis, .. } = self
            && let Some(d) = dpis
                .iter()
                .find(|&&d| !(DPI_MIN..=DPI_MAX).contains(&d) || d % DPI_STEP != 0)
        {
            return Err(Error::Input(format!(
                "DPI must be {DPI_MIN}-{DPI_MAX} in steps of {DPI_STEP}, not {d}"
            )));
        }
        self.request().map(|_| ())
    }

    /// (category, register, sub-command, payload). Refuses only what can't be encoded.
    fn request(&self) -> Result<(u8, u8, u8, Vec<u8>)> {
        let bad = |m: String| Err(Error::Input(m));
        Ok(match self {
            Setting::Polling(hz) => {
                let code = match hz {
                    125 => 0x01,
                    250 => 0x02,
                    500 => 0x04,
                    1000 => 0x08,
                    _ => {
                        return bad(format!(
                            "polling rate must be 125, 250, 500 or 1000, not {hz}"
                        ));
                    }
                };
                (0x01, 0x09, 0x02, vec![code])
            }
            Setting::Debounce(ms) => {
                if *ms > DEBOUNCE_MAX_MS {
                    return bad(format!("debounce must be 0-{DEBOUNCE_MAX_MS} ms, not {ms}"));
                }
                (0x04, 0x03, 0x03, vec![*ms])
            }
            Setting::Toggle(t, on) => (0x07, t.register(), 0x02, vec![u8::from(*on)]),
            Setting::DpiStages { dpis, active } => {
                if dpis.is_empty() || dpis.len() > MAX_STAGES {
                    return bad(format!("1 to {MAX_STAGES} DPI stages, not {}", dpis.len()));
                }
                if !(1..=dpis.len()).contains(&(*active as usize)) {
                    return bad(format!("--active must be between 1 and {}", dpis.len()));
                }
                // [active, count], then per stage: index, x lo, x hi, y lo, y hi (x = y here)
                let mut p = vec![*active, dpis.len() as u8];
                for (i, d) in dpis.iter().enumerate() {
                    let [lo, hi] = d.to_le_bytes();
                    p.extend([i as u8 + 1, lo, hi, lo, hi]);
                }
                (0x05, 0x04, 0x21, p)
            }
            Setting::DpiStage(n) => {
                if !(1..=MAX_STAGES as u8).contains(n) {
                    return bad(format!("DPI stage must be 1-{MAX_STAGES}, not {n}"));
                }
                (0x05, 0x01, 0x02, vec![*n])
            }
            Setting::Lod(mm) => {
                let v = match mm {
                    1 => 0,
                    2 => 1,
                    _ => return bad(format!("lift-off must be 1 or 2 mm, not {mm}")),
                };
                (0x07, 0x02, 0x03, vec![v, v])
            }
            Setting::Button(btn, bd) => (0x04, 0x01, 0x06, vec![btn.id(), bd.kind, bd.a1, bd.a2]),
        })
    }

    /// The full packet, with `profile` in byte 6 (for globals: the active profile, see above).
    /// Checked with [`Setting::check`] first.
    pub fn packet(&self, profile: u8) -> Result<[u8; REPORT_SIZE]> {
        self.check()?;
        self.packet_unchecked(profile)
    }

    fn packet_unchecked(&self, profile: u8) -> Result<[u8; REPORT_SIZE]> {
        let (cat, reg, sub, payload) = self.request()?;
        Ok(build(cat, reg, sub, profile, &payload))
    }
}

// ------------------------------------------------------------------ the device

/// Everything `set-mouse info` shows, read in one go.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub firmware: String,
    pub active_profile: u8,
    /// The profile the per-profile fields below belong to.
    pub profile: u8,
    pub polling_hz: u32,
    pub debounce_ms: u8,
    pub toggles: Vec<(Toggle, bool)>,
    /// 1-based index into `dpi_stages`.
    pub dpi_active: u8,
    /// (x, y) DPI per stage.
    pub dpi_stages: Vec<(u16, u16)>,
    pub lod_raw: u8,
    pub buttons: Vec<(Button, Binding)>,
}

pub struct X2a {
    dev: Device,
    /// Right after opening, the mouse sometimes ignores the first request (reply byte 0x00).
    /// The first read may resend a few times; after one success, no more resends.
    resends_left: u8,
}

impl X2a {
    /// Open the wired X2A's configuration interface.
    pub fn open() -> Result<Self> {
        let all = hid::interfaces(VID, PID)?;
        let mut candidates: Vec<_> = all
            .iter()
            .filter(|i| i.interface == CONFIG_INTERFACE)
            .collect();
        if candidates.is_empty() {
            // If the OS reports no interface numbers, the vendor-defined usage page marks it.
            candidates = all
                .iter()
                .filter(|i| i.interface == -1 && i.usage_page >= 0xFF00)
                .collect();
        }
        if candidates.is_empty() {
            return Err(Error::NotFound(
                "Pulsar X2A config interface (vid_3710&pid_1404&mi_03) not found. \
                 Is the mouse plugged in by cable?"
                    .into(),
            ));
        }
        let mut last = String::new();
        for iface in candidates {
            match Device::open(iface, REPORT_SIZE) {
                Ok(dev) => {
                    return Ok(X2a {
                        dev,
                        resends_left: 6,
                    });
                }
                Err(e) => last = e.to_string(),
            }
        }
        Err(Error::Hid(format!(
            "Could not open the mouse ({last}). Close Pulsar Fusion if it's running."
        )))
    }

    /// Read replies until byte 0 is one of `accept` (up to 7 extra tries, 20 ms apart).
    fn poll(&self, pkt: &[u8; REPORT_SIZE], accept: &[u8]) -> Result<Vec<u8>> {
        let mut rsp = self.dev.get_feature()?;
        for _ in 0..7 {
            if answers(&rsp, pkt, accept) {
                break;
            }
            sleep(Duration::from_millis(20));
            rsp = self.dev.get_feature()?;
        }
        Ok(rsp)
    }

    /// Send `pkt` and wait for its reply: byte 0 in `accept`, and the reply echoing the
    /// request's category and register (see [`answers`]). If the mouse didn't answer at all
    /// (0x00, happens right after opening), resend: up to 6 times per session. If it answered
    /// a different request, resend up to twice, then give up rather than return wrong data.
    fn transact(&mut self, pkt: &[u8; REPORT_SIZE], accept: &[u8]) -> Result<Vec<u8>> {
        self.dev.set_feature(pkt)?;
        let mut rsp = self.poll(pkt, accept)?;
        let mut foreign_resends = 2;
        loop {
            let foreign = accept.contains(&rsp[0]) && !answers(&rsp, pkt, accept);
            if rsp[0] == NOT_ANSWERED && self.resends_left > 0 {
                self.resends_left -= 1;
            } else if foreign && foreign_resends > 0 {
                foreign_resends -= 1;
            } else {
                break;
            }
            sleep(Duration::from_millis(100));
            self.dev.set_feature(pkt)?;
            rsp = self.poll(pkt, accept)?;
        }
        if accept.contains(&rsp[0]) && !answers(&rsp, pkt, accept) {
            return Err(Error::Protocol(format!(
                "The mouse kept answering another request (asked {:02x}/{:02x}, got {:02x}/{:02x}); \
                 is another program talking to it?",
                pkt[1], pkt[2], rsp[1], rsp[2]
            )));
        }
        if accept.contains(&rsp[0]) {
            self.resends_left = 0;
        }
        Ok(rsp)
    }

    fn read(&mut self, cat: u8, reg: u8, sub: u8, profile: u8, payload: &[u8]) -> Result<Vec<u8>> {
        let rsp = self.transact(&build(cat, reg | 0x80, sub, profile, payload), &[ANSWERED])?;
        if rsp[0] != ANSWERED {
            return Err(Error::Protocol(format!(
                "Bad reply from mouse (byte 0 = 0x{:02x})",
                rsp[0]
            )));
        }
        Ok(rsp)
    }

    /// Send a write and wait for the acknowledgement (0x01 done or 0x02 pending).
    fn cmd(&mut self, pkt: &[u8; REPORT_SIZE]) -> Result<()> {
        let rsp = self.transact(pkt, &[ANSWERED, PENDING])?;
        if rsp[0] != ANSWERED && rsp[0] != PENDING {
            return Err(Error::Protocol(format!(
                "Mouse did not acknowledge write (reply byte 0x{:02x})",
                rsp[0]
            )));
        }
        // A write can be acknowledged (0x02) slightly before it's applied.
        sleep(Duration::from_millis(30));
        Ok(())
    }

    /// Write one setting into the mouse's memory (saved: survives unplugging).
    ///
    /// `profile`: which onboard profile (1-5) a per-profile setting goes to; None = the active
    /// one. Ignored for global settings, which always go out with the active profile's number
    /// so the mouse saves them (see [`Setting`]). Returns the profile byte that was sent.
    pub fn write(&mut self, setting: &Setting, profile: Option<u8>) -> Result<u8> {
        setting.check()?;
        self.write_unchecked(setting, profile)
    }

    /// [`X2a::write`] without [`Setting::check`]: for putting back values read from the mouse.
    pub(crate) fn write_unchecked(&mut self, setting: &Setting, profile: Option<u8>) -> Result<u8> {
        let pkt_profile = match profile {
            Some(p) if !setting.is_global() => p,
            _ => self.active_profile()?,
        };
        if !(1..=5).contains(&pkt_profile) {
            return Err(Error::Protocol(format!(
                "Mouse reports active profile {pkt_profile}; expected 1-5, so nothing was written"
            )));
        }
        let pkt = setting.packet_unchecked(pkt_profile)?;
        self.cmd(&pkt)?;
        Ok(pkt_profile)
    }

    /// Make onboard profile `p` (1-5) the active one.
    pub fn set_active_profile(&mut self, p: u8) -> Result<()> {
        if !(1..=5).contains(&p) {
            return Err(Error::Input(format!("profile must be 1-5, not {p}")));
        }
        self.cmd(&build(0x02, 0x06, 0x01, p, &[]))
    }

    /// The active DPI stage (1-based) of a profile.
    pub fn dpi_stage(&mut self, profile: u8) -> Result<u8> {
        Ok(self.read(0x05, 0x01, 0x02, profile, &[])?[7])
    }

    pub fn firmware(&mut self) -> Result<String> {
        let rsp = self.read(0x01, 0x07, 0x04, 0, &[])?;
        Ok(rsp[6..10]
            .iter()
            .rev()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join("."))
    }

    pub fn active_profile(&mut self) -> Result<u8> {
        Ok(self.read(0x02, 0x06, 0x01, 0, &[])?[6])
    }

    pub fn polling_hz(&mut self) -> Result<u32> {
        let v = self.read(0x01, 0x09, 0x02, 0, &[])?[7];
        Ok(match v {
            0x01 => 125,
            0x02 => 250,
            0x04 => 500,
            0x08 => 1000,
            v => v as u32 * 125,
        })
    }

    pub fn debounce_ms(&mut self) -> Result<u8> {
        Ok(self.read(0x04, 0x03, 0x03, 0, &[])?[7])
    }

    pub fn toggle(&mut self, t: Toggle) -> Result<bool> {
        Ok(self.read(0x07, t.register(), 0x02, 0, &[])?[7] != 0)
    }

    /// (active stage, 1-based; (x, y) per stage).
    pub fn dpi_stages(&mut self, profile: u8) -> Result<(u8, Vec<(u16, u16)>)> {
        let rsp = self.read(0x05, 0x04, 0x15, profile, &[])?;
        Ok(parse_dpi_stages(&rsp))
    }

    pub fn lod_raw(&mut self, profile: u8) -> Result<u8> {
        Ok(self.read(0x07, 0x02, 0x03, profile, &[])?[7])
    }

    pub fn button(&mut self, btn: Button, profile: u8) -> Result<Binding> {
        let rsp = self.read(0x04, 0x01, 0x06, profile, &[btn.id(), 0xFF])?;
        Ok(Binding {
            kind: rsp[8],
            a1: rsp[9],
            a2: rsp[10],
        })
    }

    /// Read everything; `profile` None = the active profile.
    pub fn snapshot(&mut self, profile: Option<u8>) -> Result<Snapshot> {
        let active_profile = self.active_profile()?;
        let profile = profile.unwrap_or(active_profile);
        let firmware = self.firmware()?;
        let polling_hz = self.polling_hz()?;
        let debounce_ms = self.debounce_ms()?;
        let mut toggles = Vec::new();
        for t in Toggle::ALL {
            toggles.push((t, self.toggle(t)?));
        }
        let (dpi_active, dpi_stages) = self.dpi_stages(profile)?;
        let lod_raw = self.lod_raw(profile)?;
        let mut buttons = Vec::new();
        for btn in Button::ALL {
            buttons.push((btn, self.button(btn, profile)?));
        }
        Ok(Snapshot {
            firmware,
            active_profile,
            profile,
            polling_hz,
            debounce_ms,
            toggles,
            dpi_active,
            dpi_stages,
            lod_raw,
            buttons,
        })
    }
}

/// True if `rsp` is the reply to `pkt`, not a leftover.
///
/// The mouse has one reply buffer that keeps its last answer, status 0x01 included, until it
/// has processed the next request; a poll that comes too early gets the previous answer. Seen
/// 2026-11-09 in both the Python and Rust readers: button reads shifted by one, globals read
/// from other replies. So a reply must echo what identifies the request:
/// - status byte in `accept`;
/// - bytes 1-2, category and register (read bit included);
/// - byte 6, the profile, when the request names one (for profile-0 reads like firmware and
///   active profile, byte 6 carries data);
/// - byte 7, the first payload byte, when the request has one (button id for button reads;
///   write acknowledgements echo the whole packet).
///
/// Byte 3 isn't compared: a DPI-stage read (sub 0x15) is answered with sub 0x21.
/// Measured 2026-11-09: 936 read replies and 8 kinds of write acknowledgement all fit this.
fn answers(rsp: &[u8], pkt: &[u8; REPORT_SIZE], accept: &[u8]) -> bool {
    accept.contains(&rsp[0])
        && rsp[1] == pkt[1]
        && rsp[2] == pkt[2]
        && (pkt[6] == 0 || rsp[6] == pkt[6])
        && (pkt[7] == 0 || rsp[7] == pkt[7])
}

/// DPI-stage reply: `[7]` active stage, `[8]` count, then 5 bytes per stage
/// (index, x lo, x hi, y lo, y hi) from byte 9.
fn parse_dpi_stages(rsp: &[u8]) -> (u8, Vec<(u16, u16)>) {
    let (active, count) = (rsp[7], rsp[8] as usize);
    let stages = (0..count.min(MAX_STAGES))
        .map(|i| {
            let base = 9 + 5 * i;
            let x = u16::from_le_bytes([rsp[base + 1], rsp[base + 2]]);
            let y = u16::from_le_bytes([rsp[base + 3], rsp[base + 4]]);
            (x, y)
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

    /// (cat, reg, sub, profile, payload) → expected packet as hex.
    type PacketVector<'a> = ((u8, u8, u8, u8, &'a [u8]), &'a str);

    /// Generated from the Python reference (`pulsar_x2a.X2A.build`).
    #[test]
    fn packets_match_python() {
        let vectors: &[PacketVector] = &[
            (
                (0x01, 0x87, 0x04, 0, &[]),
                "00018704000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008c00",
            ),
            (
                (0x02, 0x86, 0x01, 0, &[]),
                "00028601000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008900",
            ),
            (
                (0x02, 0x06, 0x01, 3, &[]),
                "00020601000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00",
            ),
            (
                (0x04, 0x03, 0x03, 0, &[1]),
                "00040303000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b00",
            ),
            (
                (0x01, 0x09, 0x02, 0, &[8]),
                "00010902000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001400",
            ),
            (
                (0x04, 0x81, 0x06, 1, &[1, 255]),
                "0004810600000101ff00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008c01",
            ),
            (
                (0x04, 0x01, 0x06, 1, &[1, 1, 2, 0]),
                "00040106000001010102000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000",
            ),
            (
                (
                    0x05,
                    0x04,
                    0x21,
                    2,
                    &[
                        3, 3, 1, 144, 1, 144, 1, 2, 32, 3, 32, 3, 3, 128, 12, 128, 12,
                    ],
                ),
                "0005042100000203030190019001022003200303800c800c0000000000000000000000000000000000000000000000000000000000000000000000000000b802",
            ),
        ];
        for ((cat, reg, sub, prof, payload), want) in vectors {
            assert_eq!(
                hex(&build(*cat, *reg, *sub, *prof, payload)),
                *want,
                "{cat:02x}/{reg:02x}/{sub:02x}"
            );
        }
    }

    /// Generated from the Python reference (`pulsar_x2a.describe`).
    #[test]
    fn describe_matches_python() {
        let vectors: &[((u8, u8, u8), &str)] = &[
            ((0x01, 0x01, 0x00), "left click"),
            ((0x01, 0x02, 0x00), "right click"),
            ((0x01, 0x03, 0x00), "middle click"),
            ((0x01, 0x04, 0x00), "back"),
            ((0x09, 0x03, 0x00), "dpiloop"),
            ((0x02, 0x01, 0x06), "ctrl+c"),
            ((0x02, 0x00, 0x2d), "-"),
            ((0x02, 0x01, 0x00), "ctrl"),
            ((0x02, 0x00, 0x00), "nothing"),
            ((0x0d, 0xcd, 0x00), "play"),
            ((0x0d, 0x34, 0x12), "media(0x1234)"),
            ((0x03, 0xff, 0x00), "scrolldown"),
            ((0x00, 0x00, 0x00), "disabled"),
            ((0x0e, 0x01, 0x00), "doubleclick"),
            ((0x02, 0x05, 0x3a), "ctrl+alt+f1"),
            ((0x02, 0x00, 0xe0), "key 0xe0"),
            ((0x42, 0x01, 0x02), "type 0x42 (0x01, 0x02)"),
        ];
        for ((kind, a1, a2), want) in vectors {
            assert_eq!(
                Binding {
                    kind: *kind,
                    a1: *a1,
                    a2: *a2
                }
                .describe(),
                *want
            );
        }
    }

    #[test]
    fn key_names_cover_the_python_table() {
        assert_eq!(key_name(0x04).as_deref(), Some("a"));
        assert_eq!(key_name(0x1D).as_deref(), Some("z"));
        assert_eq!(key_name(0x1E).as_deref(), Some("1"));
        assert_eq!(key_name(0x27).as_deref(), Some("0"));
        assert_eq!(key_name(0x39).as_deref(), Some("capslock"));
        assert_eq!(key_name(0x3A).as_deref(), Some("f1"));
        assert_eq!(key_name(0x45).as_deref(), Some("f12"));
        assert_eq!(key_name(0x32), None);
    }

    /// Real replies seen on the mouse (2026-11-09).
    #[test]
    fn replies_must_echo_their_request() {
        let rsp = |hex: &str| -> Vec<u8> {
            let mut v: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            v.resize(REPORT_SIZE, 0);
            v
        };
        let debounce_read = build(0x04, 0x83, 0x03, 0, &[]);
        assert!(answers(
            &rsp("0104830300000001"),
            &debounce_read,
            &[ANSWERED]
        ));
        // a reply for the DPI-stage read is not one for debounce
        assert!(!answers(
            &rsp("0105842100000101"),
            &debounce_read,
            &[ANSWERED]
        ));
        // the DPI-stage read is answered with sub 0x21; still its reply
        let dpi_read = build(0x05, 0x84, 0x15, 1, &[]);
        assert!(answers(&rsp("0105842100000101"), &dpi_read, &[ANSWERED]));
        // write acknowledgements echo too (debounce = 1, profile 1)
        let debounce_write = Setting::Debounce(1).packet(1).unwrap();
        assert!(answers(
            &rsp("0204030300000101"),
            &debounce_write,
            &[ANSWERED, PENDING]
        ));
        // right reply, wrong status
        assert!(!answers(
            &rsp("0304830300000001"),
            &debounce_read,
            &[ANSWERED]
        ));

        // The failure seen: reading the wheel's binding, the buffer still holds the reply for
        // the right button (id 02). Same category and register; only byte 7 tells them apart.
        let wheel_read = build(0x04, 0x81, 0x06, 1, &[Button::Wheel.id(), 0xFF]);
        assert!(!answers(
            &rsp("010481060000010201010000"),
            &wheel_read,
            &[ANSWERED]
        ));
        assert!(answers(
            &rsp("010481060000010301030000"),
            &wheel_read,
            &[ANSWERED]
        ));
        // ...and the same button on another profile is another request too
        assert!(!answers(
            &rsp("010481060000020301030000"),
            &wheel_read,
            &[ANSWERED]
        ));
        // firmware read: byte 6 is data there, so it isn't compared
        let firmware_read = build(0x01, 0x87, 0x04, 0, &[]);
        assert!(answers(
            &rsp("0101870400000001000000"),
            &firmware_read,
            &[ANSWERED]
        ));
        // write acknowledgements echo their whole packet (8 kinds measured)
        let dpi_write = Setting::DpiStages {
            dpis: vec![3200],
            active: 1,
        }
        .packet(1)
        .unwrap();
        assert!(answers(
            &rsp("02050421000001010101800c800c"),
            &dpi_write,
            &[ANSWERED, PENDING]
        ));
    }

    #[test]
    fn dpi_reply_parses() {
        // reply to a DPI-stage read: active 2 of 2 stages, 800 and 3200x1600
        let mut rsp = [0u8; REPORT_SIZE];
        rsp[0] = ANSWERED;
        rsp[7..19].copy_from_slice(&[2, 2, 1, 0x20, 0x03, 0x20, 0x03, 2, 0x80, 0x0C, 0x40, 0x06]);
        assert_eq!(parse_dpi_stages(&rsp), (2, vec![(800, 800), (3200, 1600)]));
    }

    #[test]
    fn factory_bindings_match_python() {
        // BUTTON_DEFAULTS in pulsar_x2a.py
        let want = [
            (1, 1),
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 4),
            (1, 5),
            (9, 3),
        ];
        for (btn, (k, a1)) in Button::ALL.iter().zip(want) {
            assert_eq!(btn.factory(), Binding { kind: k, a1, a2: 0 }, "{btn:?}");
        }
    }

    const fn bd(kind: u8, a1: u8, a2: u8) -> Binding {
        Binding { kind, a1, a2 }
    }

    /// Generated from the Python reference: its setters, with `set_report` recording the
    /// packet and `active_profile()` pinned to 3. So globals must carry 3 (saved to flash),
    /// per-profile writes the profile asked for.
    #[test]
    fn write_packets_match_python() {
        const ACTIVE: u8 = 3;
        let vectors: Vec<(Setting, Option<u8>, &str)> = vec![
            (
                Setting::Polling(1000),
                None,
                "00010902000003080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001700",
            ),
            (
                Setting::Polling(125),
                Some(5),
                "00010902000003010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000",
            ),
            (
                Setting::Debounce(1),
                None,
                "00040303000003010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e00",
            ),
            (
                Setting::Toggle(Toggle::Ripple, false),
                None,
                "00070302000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f00",
            ),
            (
                Setting::Toggle(Toggle::AngleSnap, true),
                None,
                "00070402000003010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001100",
            ),
            (
                Setting::Toggle(Toggle::MotionSync, true),
                None,
                "00070502000003010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001200",
            ),
            (
                Setting::DpiStages {
                    dpis: vec![400, 800, 1600, 3200, 6400, 26000],
                    active: 4,
                },
                Some(2),
                "00050421000002040601900190010220032003034006400604800c800c0500190019069065906500000000000000000000000000000000000000000000007305",
            ),
            (
                Setting::DpiStages {
                    dpis: vec![3200],
                    active: 1,
                },
                Some(1),
                "00050421000001010101800c800c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004601",
            ),
            (
                Setting::DpiStage(6),
                Some(5),
                "00050102000005060000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001300",
            ),
            (
                Setting::Lod(1),
                Some(1),
                "00070203000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000d00",
            ),
            (
                Setting::Lod(2),
                Some(4),
                "00070203000004010100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001200",
            ),
            (
                Setting::Button(Button::Left, bd(1, 2, 0)),
                Some(1),
                "00040106000001010102000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000",
            ),
            (
                Setting::Button(Button::Right, bd(1, 1, 0)),
                Some(1),
                "00040106000001020101000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000",
            ),
            (
                Setting::Button(Button::Thumb4, bd(2, 3, 69)),
                Some(2),
                "00040106000002070203450000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005e00",
            ),
            (
                Setting::Button(Button::Wheel, bd(13, 35, 2)),
                Some(3),
                "00040106000003030d23020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004300",
            ),
            (
                Setting::Button(Button::Thumb1, bd(13, 233, 0)),
                Some(1),
                "00040106000001040de9000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000601",
            ),
            (
                Setting::Button(Button::Dpi, bd(0, 0, 0)),
                Some(5),
                "000401060000050b0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001b00",
            ),
            (
                Setting::Button(Button::Thumb2, bd(3, 255, 0)),
                Some(1),
                "000401060000010503ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001301",
            ),
        ];
        for (setting, requested, want) in vectors {
            // the same choice X2a::write makes
            let prof = match requested {
                Some(p) if !setting.is_global() => p,
                _ => ACTIVE,
            };
            assert_eq!(hex(&setting.packet(prof).unwrap()), want, "{setting:?}");
        }
        // set_active_profile(4)
        assert_eq!(
            hex(&build(0x02, 0x06, 0x01, 4, &[])),
            "00020601000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000d00"
        );
    }

    /// Generated from the Python reference (`pulsar_x2a.parse_function`). One deliberate
    /// difference: `media(0x10000)` crashes the Python later; here it's refused up front.
    #[test]
    fn parse_matches_python() {
        /// (text, Some((kind, a1, a2)) or None if it must be refused)
        type ParseVector<'a> = (&'a str, Option<(u8, u8, u8)>);
        let vectors: &[ParseVector] = &[
            ("left", Some((1, 1, 0))),
            ("Right", Some((1, 2, 0))),
            (" back ", Some((1, 4, 0))),
            ("dpiloop", Some((9, 3, 0))),
            ("profile+", Some((8, 3, 0))),
            ("doubleclick", Some((14, 1, 0))),
            ("disabled", Some((0, 0, 0))),
            ("play", Some((13, 205, 0))),
            ("VOL-", Some((13, 234, 0))),
            ("mediaplayer", Some((13, 131, 1))),
            ("media(0x00cd)", Some((13, 205, 0))),
            ("media(205)", Some((13, 205, 0))),
            ("media( 0x1234 )", Some((13, 52, 18))),
            ("ctrl+c", Some((2, 1, 6))),
            ("shift", Some((2, 2, 0))),
            ("f5", Some((2, 0, 62))),
            ("f12", Some((2, 0, 69))),
            ("alt+tab", Some((2, 4, 43))),
            ("ctrl+alt+delete", Some((2, 5, 76))),
            ("rwin+rshift+0", Some((2, 160, 39))),
            ("-", Some((2, 0, 45))),
            ("=", Some((2, 0, 46))),
            ("\\", Some((2, 0, 49))),
            ("`", Some((2, 0, 53))),
            ("capslock", Some((2, 0, 57))),
            ("arrowup", Some((2, 0, 82))),
            (
                "ctrl+shift+alt+win+rctrl+rshift+ralt+rwin+a",
                Some((2, 255, 4)),
            ),
            ("a+b", None),
            ("f13", None),
            ("f0", None),
            ("f+5", None),
            ("+", None),
            ("", None),
            ("ctrl+", None),
            ("media(0x10000)", None),
            ("media(xyz)", None),
            ("hello", None),
            ("default", None),
        ];
        for (text, want) in vectors {
            let got = Binding::parse(text).ok().map(|b| (b.kind, b.a1, b.a2));
            assert_eq!(got, *want, "{text:?}");
        }
    }

    #[test]
    fn binding_round_trips_through_describe() {
        for text in [
            "back",
            "ctrl+c",
            "play",
            "f5",
            "media(0x1234)",
            "alt+tab",
            "disabled",
        ] {
            assert_eq!(Binding::parse(text).unwrap().describe(), text);
        }
    }

    #[test]
    fn default_binding_is_the_factory_one() {
        assert_eq!(
            Button::Thumb3.parse_binding("Default").unwrap(),
            Button::Thumb3.factory()
        );
        assert_eq!(Button::from_name("THUMB3"), Some(Button::Thumb3));
        assert_eq!(Button::from_name("wheel click"), None);
    }

    #[test]
    fn bad_values_are_refused_before_sending() {
        let bad = [
            Setting::Polling(300),
            Setting::Debounce(21),
            Setting::DpiStages {
                dpis: vec![],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![150],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![26100],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![800; 7],
                active: 1,
            },
            Setting::DpiStages {
                dpis: vec![800, 1600],
                active: 3,
            },
            Setting::DpiStages {
                dpis: vec![800, 1600],
                active: 0,
            },
            Setting::DpiStage(0),
            Setting::DpiStage(7),
            Setting::Lod(3),
        ];
        for s in bad {
            assert!(
                matches!(s.packet(1), Err(Error::Input(_))),
                "{s:?} should be refused"
            );
        }
        assert!(Setting::Debounce(0).packet(1).is_ok()); // allowed, like the Python (not advised)

        // A value the CLI refuses can still be restored as read from the mouse...
        let odd = Setting::DpiStages {
            dpis: vec![1850],
            active: 1,
        };
        assert!(matches!(odd.packet(1), Err(Error::Input(_))));
        assert!(odd.packet_unchecked(1).is_ok());
        // ...but nothing the protocol can't encode.
        let none = Setting::DpiStages {
            dpis: vec![],
            active: 1,
        };
        assert!(none.packet_unchecked(1).is_err());
    }
}
