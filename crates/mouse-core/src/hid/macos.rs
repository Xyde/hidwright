//! macOS HID backend: hidapi over IOKit, opening devices shared (see Cargo.toml), so macOS keeps
//! using the mouse while we talk to it.
//!
//! Not yet built or tested on a Mac (ticket for the macOS port). Open questions there: whether
//! macOS reports interface numbers for these mice, and whether the Razer's feature reports on
//! its mouse interface are allowed without exclusive access.

use std::ffi::CString;

use hidapi::{HidApi, HidDevice};

use super::{Interface, with_report_id};
use crate::{Error, Result};

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error::Hid(e.to_string())
    }
}

pub fn interfaces(vid: u16, pid: u16) -> Result<Vec<Interface>> {
    let api = HidApi::new()?;
    Ok(api
        .device_list()
        .filter(|d| d.vendor_id() == vid && d.product_id() == pid)
        .map(|d| Interface {
            path: d.path().to_string_lossy().into_owned(),
            interface: d.interface_number(),
            usage_page: d.usage_page(),
        })
        .collect())
}

/// An open HID interface that exchanges fixed-size feature reports (ID 0).
pub struct Device {
    dev: HidDevice,
    report_len: usize,
}

impl Device {
    /// Open `iface` for feature reports of `report_len` bytes (without the report-ID byte).
    pub fn open(iface: &Interface, report_len: usize) -> Result<Self> {
        let path = CString::new(iface.path.as_str()).map_err(|e| Error::Hid(e.to_string()))?;
        let dev = HidApi::new()?.open_path(&path)?;
        Ok(Device { dev, report_len })
    }

    pub fn set_feature(&self, data: &[u8]) -> Result<()> {
        let buf = with_report_id(data, self.report_len);
        self.dev
            .send_feature_report(&buf)
            .map_err(|e| Error::Hid(format!("SetFeature failed ({e})")))
    }

    pub fn get_feature(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; self.report_len + 1];
        self.dev
            .get_feature_report(&mut buf)
            .map_err(|e| Error::Hid(format!("GetFeature failed ({e})")))?;
        buf.remove(0);
        Ok(buf)
    }
}
