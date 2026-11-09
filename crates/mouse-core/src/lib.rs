//! mouse-core: everything that talks to the mice.
//!
//! Settings are written into each mouse's own memory, so nothing here sits in the input path:
//! a program built on this crate configures a mouse, then exits.
//!
//! - [`hid`]: thin cross-platform HID layer (feature reports): Win32 on Windows, hidapi on macOS
//! - [`x2a`]: Pulsar X2A protocol (wired, USB 3710:1404)
//! - [`viper`]: Razer Viper protocol (wired, USB 1532:0078)
//! - [`rawaccel`]: RawAccel's settings.json: per-mouse rotation, loading it into the driver
//! - [`rawinput`]: which mice are plugged in, under the ids RawAccel uses
//! - [`dpi_restore`]: undo the temporary DPI of an interrupted calibration run

pub mod dpi_restore;
pub mod hid;
pub mod rawaccel;
pub mod rawinput;
pub mod viper;
pub mod x2a;

use std::fmt;

/// Local date and time as (year, month, day, hour, minute, second).
pub fn local_time() -> (u16, u16, u16, u16, u16, u16) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: GetLocalTime only fills the struct we pass.
        let mut t: SYSTEMTIME = unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut t) };
        (t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
    }
    #[cfg(not(windows))]
    {
        // Only RawAccel (Windows) needs timestamps today; UTC is good enough elsewhere.
        let s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        utc_from_unix(s)
    }
}

/// Unix seconds → UTC (year, month, day, hour, minute, second). Howard Hinnant's
/// civil-from-days algorithm.
#[cfg_attr(windows, allow(dead_code))]
fn utc_from_unix(s: u64) -> (u16, u16, u16, u16, u16, u16) {
    let (days, rem) = (s / 86400, s % 86400);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let (h, mi, se) = (rem / 3600, rem % 3600 / 60, rem % 60);
    (y as u16, m as u16, d as u16, h as u16, mi as u16, se as u16)
}

#[cfg(test)]
mod tests {
    #[test]
    fn utc_from_unix_known_dates() {
        assert_eq!(super::utc_from_unix(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(super::utc_from_unix(951_782_400), (2000, 2, 29, 0, 0, 0)); // leap day
        assert_eq!(
            super::utc_from_unix(1_793_965_455),
            (2026, 11, 6, 11, 44, 15) // checked with Python's datetime
        );
    }
}

/// Every failure this crate reports. Messages are written for the person running the tool.
#[derive(Debug)]
pub enum Error {
    /// The mouse (or the interface we need) isn't connected.
    NotFound(String),
    /// The operating system refused or failed a HID operation.
    Hid(String),
    /// The mouse answered, but not in the way the protocol says it should.
    Protocol(String),
    /// A value the user asked for is out of range or not understood; nothing was sent.
    Input(String),
    /// Reading or writing a file, or running a helper program (RawAccel's writer.exe), failed.
    File(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(m)
            | Error::Hid(m)
            | Error::Protocol(m)
            | Error::Input(m)
            | Error::File(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
