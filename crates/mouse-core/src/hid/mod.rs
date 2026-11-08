//! Cross-platform HID access, reduced to what the mouse protocols need: find a device's
//! interfaces, open one, and exchange feature reports.
//!
//! Every feature report here uses report ID 0. OS HID APIs expect the report ID as the first
//! byte of the buffer, so a 64-byte report travels as 65 bytes. The backends add and strip that
//! byte, so protocol code only ever sees the report itself.
//!
//! Backends: `windows.rs` (Win32 SetupAPI + hid.dll, the same calls the Python version makes)
//! and `macos.rs` (hidapi over IOKit). Both expose the same `interfaces` and `Device`.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{Device, interfaces};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Device, interfaces};

/// One HID interface of a USB device, as the operating system lists it.
#[derive(Clone, Debug)]
pub struct Interface {
    /// OS device path; only meaningful to `Device::open`.
    pub path: String,
    /// USB interface number (`MI_03` → 3), or -1 if the OS didn't say.
    pub interface: i32,
    /// HID usage page, or 0 if the OS doesn't report it without opening the device (Windows).
    pub usage_page: u16,
}

/// `data` zero-padded to `report_len`, with the report-ID byte (0) in front.
fn with_report_id(data: &[u8], report_len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; report_len + 1];
    let n = data.len().min(report_len);
    buf[1..=n].copy_from_slice(&data[..n]);
    buf
}

#[cfg(test)]
mod tests {
    #[test]
    fn report_id_byte_is_prepended_and_padded() {
        assert_eq!(super::with_report_id(&[7, 8], 4), vec![0, 7, 8, 0, 0]);
        assert_eq!(super::with_report_id(&[1, 2, 3, 4, 5], 3), vec![0, 1, 2, 3]);
    }
}
