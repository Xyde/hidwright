//! Pulsar X2A (wired, USB 3710:1404): read side of the configuration protocol.
//!
//! Port of `pulsar_x2a.py`. The protocol comes from the MIT-licensed community project
//! packerlschupfer/pulsar-mouse-linux (`drivers/x2a.py`), reverse-engineered from USB captures.
//!
//! Packet (64 bytes, feature report 0 on USB interface 3):
//! `[1]` category, `[2]` register (bit 7 set = read), `[3]` sub-command, `[6]` profile
//! (0 = global, 1-5), `[7..62]` payload, `[62..64]` little-endian u16 of `sum(bytes[0..62])`.
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

impl Binding {
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
    fn poll(&self, accept: &[u8]) -> Result<Vec<u8>> {
        let mut rsp = self.dev.get_feature()?;
        for _ in 0..7 {
            if accept.contains(&rsp[0]) {
                break;
            }
            sleep(Duration::from_millis(20));
            rsp = self.dev.get_feature()?;
        }
        Ok(rsp)
    }

    fn read(&mut self, cat: u8, reg: u8, sub: u8, profile: u8, payload: &[u8]) -> Result<Vec<u8>> {
        let pkt = build(cat, reg | 0x80, sub, profile, payload);
        self.dev.set_feature(&pkt)?;
        let mut rsp = self.poll(&[ANSWERED])?;
        while rsp[0] == NOT_ANSWERED && self.resends_left > 0 {
            self.resends_left -= 1;
            sleep(Duration::from_millis(100));
            self.dev.set_feature(&pkt)?;
            rsp = self.poll(&[ANSWERED])?;
        }
        if rsp[0] != ANSWERED {
            return Err(Error::Protocol(format!(
                "Bad reply from mouse (byte 0 = 0x{:02x})",
                rsp[0]
            )));
        }
        self.resends_left = 0;
        Ok(rsp)
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
}
