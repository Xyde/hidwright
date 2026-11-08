//! Razer Viper (wired, USB 1532:0078): read side.
//!
//! Port of `razer_viper.py`. Protocol as documented by the open-source OpenRazer project:
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
            let Ok(dev) = Device::open(&iface, REPORT_LEN) else { continue };
            let mut v = Viper { dev, tid: TRANSACTION_IDS[0] };
            for tid in TRANSACTION_IDS {
                v.tid = tid;
                if v.firmware().is_ok() {
                    return Ok(v);
                }
            }
        }
        Err(Error::NotFound("Razer Viper not found or not answering (is Razer Synapse holding it?)".into()))
    }

    /// Send one request and return the reply's argument bytes (`[8..88]`).
    fn request(&self, class: u8, cmd: u8, size: u8, args: &[u8]) -> Result<Vec<u8>> {
        let req = build_request(self.tid, class, cmd, size, args);
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
        Ok((u16::from_be_bytes([a[1], a[2]]), u16::from_be_bytes([a[3], a[4]])))
    }

    pub fn dpi_stages(&self) -> Result<(u8, Vec<(u16, u16)>)> {
        let a = self.request(0x04, 0x86, 0x26, &[VARSTORE])?;
        Ok(parse_dpi_stages(&a))
    }

    /// Read everything; failures are collected instead of stopping.
    pub fn info(&self) -> Info {
        fn take<T>(name: &'static str, r: Result<T>, errors: &mut Vec<(&'static str, String)>) -> Option<T> {
            r.map_err(|e| errors.push((name, e.to_string()))).ok()
        }
        let mut errors = Vec::new();
        Info {
            firmware: take("firmware", self.firmware(), &mut errors),
            serial: take("serial", self.serial(), &mut errors),
            polling_code: take("polling", self.polling_code(), &mut errors),
            dpi: take("dpi", self.dpi(), &mut errors),
            dpi_stages: take("dpi_stages", self.dpi_stages(), &mut errors),
            errors,
        }
    }
}

/// Stage reply arguments: `[1]` active stage, `[2]` count, then 7 bytes per stage from `[3]`:
/// index, x (big-endian u16), y (big-endian u16), 2 reserved.
fn parse_dpi_stages(a: &[u8]) -> (u8, Vec<(u16, u16)>) {
    let (active, count) = (a[1], a[2] as usize);
    let stages = (0..count.min(5))
        .map(|i| {
            let s = &a[3 + 7 * i..];
            (u16::from_be_bytes([s[1], s[2]]), u16::from_be_bytes([s[3], s[4]]))
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
            (0x04, 0x85, 0x07, &[NOSTORE], pad("001f000000070485") + "8600"),
            (0x04, 0x86, 0x26, &[VARSTORE], pad("001f00000026048601") + "a500"),
        ];
        for (class, cmd, size, args, want) in cases {
            assert_eq!(hex(&build_request(0x1F, class, cmd, size, args)), want, "{class:02x}/{cmd:02x}");
        }
    }

    /// Same canned reply the Python parsed to (5, [(400,400), (800,800), (1800,1800), (2400,2400), (3200,3200)]).
    #[test]
    fn dpi_stages_parse_like_python() {
        let a = [
            0x01, 5, 5, 1, 0x01, 0x90, 0x01, 0x90, 0, 0, 2, 0x03, 0x20, 0x03, 0x20, 0, 0, 3, 0x07,
            0x08, 0x07, 0x08, 0, 0, 4, 0x09, 0x60, 0x09, 0x60, 0, 0, 5, 0x0C, 0x80, 0x0C, 0x80, 0, 0,
        ];
        assert_eq!(
            parse_dpi_stages(&a),
            (5, vec![(400, 400), (800, 800), (1800, 1800), (2400, 2400), (3200, 3200)])
        );
    }

    #[test]
    fn polling_codes() {
        assert_eq!(polling_hz(0x01), Some(1000));
        assert_eq!(polling_hz(0x08), Some(125));
        assert_eq!(polling_hz(0x03), None);
    }
}
