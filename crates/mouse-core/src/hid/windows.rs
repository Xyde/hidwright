//! Windows HID backend: SetupAPI lists device paths, hid.dll exchanges feature reports.
//!
//! Listing only collects paths (nothing is opened), which keeps it fast; the one device we
//! want is opened afterwards. Same approach as `pulsar_x2a.py` (`hid_paths`, `X2A.__init__`).

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HIDP_CAPS, HIDP_STATUS_SUCCESS, HidD_FreePreparsedData, HidD_GetFeature, HidD_GetHidGuid,
    HidD_GetPreparsedData, HidD_SetFeature, HidP_GetCaps,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::core::GUID;

use super::{Interface, with_report_id};
use crate::{Error, Result};

/// Paths of all present HID interfaces, e.g.
/// `\\?\hid#vid_3710&pid_1404&mi_03#...#{4d1e55b2-f16f-11cf-88cb-001111000030}`.
fn hid_paths() -> Vec<String> {
    let mut paths = Vec::new();
    // SAFETY: plain SetupAPI enumeration. Every pointer passed points at a live local, the detail
    // buffer is sized by the first call and 8-byte aligned (Vec<u64>), and the device-info set is
    // destroyed before returning.
    unsafe {
        let mut guid: GUID = zeroed();
        HidD_GetHidGuid(&mut guid);
        let set = SetupDiGetClassDevsW(
            &guid,
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == -1 {
            return paths;
        }
        for index in 0.. {
            let mut ifd: SP_DEVICE_INTERFACE_DATA = zeroed();
            ifd.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(set, null(), &guid, index, &mut ifd) == 0 {
                break; // no more interfaces
            }
            let mut need = 0u32;
            SetupDiGetDeviceInterfaceDetailW(set, &ifd, null_mut(), 0, &mut need, null_mut());
            if need < 6 {
                continue;
            }
            let mut buf = vec![0u64; (need as usize).div_ceil(8)];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            // The struct's size (not the buffer's): 8 on 64-bit, 6 on 32-bit.
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(set, &ifd, detail, need, null_mut(), null_mut())
                == 0
            {
                continue;
            }
            // DevicePath starts after the 4-byte cbSize and is NUL-terminated UTF-16.
            let chars = (*detail).DevicePath.as_ptr();
            let max = (need as usize - 4) / 2;
            let len = (0..max).find(|&i| *chars.add(i) == 0).unwrap_or(max);
            paths.push(String::from_utf16_lossy(std::slice::from_raw_parts(
                chars, len,
            )));
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    paths
}

/// `&mi_03` in a lower-cased path → 3.
fn interface_number(path_lower: &str) -> i32 {
    path_lower
        .find("&mi_")
        .and_then(|i| path_lower.get(i + 4..i + 6))
        .and_then(|hex| i32::from_str_radix(hex, 16).ok())
        .unwrap_or(-1)
}

pub fn interfaces(vid: u16, pid: u16) -> Result<Vec<Interface>> {
    let tag = format!("vid_{vid:04x}&pid_{pid:04x}");
    Ok(hid_paths()
        .into_iter()
        .filter_map(|path| {
            let lower = path.to_lowercase();
            lower.contains(&tag).then(|| Interface {
                interface: interface_number(&lower),
                usage_page: 0,
                path,
            })
        })
        .collect())
}

fn last_error() -> u32 {
    // SAFETY: reads the calling thread's last-error value; no preconditions.
    unsafe { GetLastError() }
}

/// An open HID interface that exchanges fixed-size feature reports (ID 0).
pub struct Device {
    handle: HANDLE,
    report_len: usize,
}

impl Device {
    /// Open `iface` for feature reports of `report_len` bytes (without the report-ID byte).
    /// Fails if the interface's feature report has a different size: that's the wrong interface.
    pub fn open(iface: &Interface, report_len: usize) -> Result<Self> {
        let wide: Vec<u16> = iface.path.encode_utf16().chain(Some(0)).collect();
        // Read/write first; some interfaces only allow opening with no access rights,
        // which is still enough for feature reports.
        let mut handle = INVALID_HANDLE_VALUE;
        for access in [GENERIC_READ | GENERIC_WRITE, 0] {
            // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call.
            handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    0,
                    null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                break;
            }
        }
        if handle == INVALID_HANDLE_VALUE {
            return Err(Error::Hid(format!("Windows error {}", last_error())));
        }
        let dev = Device { handle, report_len }; // from here on, Drop closes the handle
        let have = dev.feature_report_byte_length()?;
        if have != report_len + 1 {
            return Err(Error::Hid(format!(
                "this interface's feature report is {have} bytes, expected {}",
                report_len + 1
            )));
        }
        Ok(dev)
    }

    /// Feature report size the device declares, report-ID byte included.
    fn feature_report_byte_length(&self) -> Result<usize> {
        // SAFETY: `pp` is filled by HidD_GetPreparsedData and freed exactly once below;
        // `caps` is a properly sized local.
        unsafe {
            let mut pp = 0;
            if !HidD_GetPreparsedData(self.handle, &mut pp) {
                return Err(Error::Hid(format!(
                    "HidD_GetPreparsedData failed (Windows error {})",
                    last_error()
                )));
            }
            let mut caps: HIDP_CAPS = zeroed();
            let status = HidP_GetCaps(pp, &mut caps);
            HidD_FreePreparsedData(pp);
            if status != HIDP_STATUS_SUCCESS {
                return Err(Error::Hid(format!(
                    "HidP_GetCaps failed (status 0x{status:08x})"
                )));
            }
            Ok(caps.FeatureReportByteLength as usize)
        }
    }

    /// Send one feature report. `data` shorter than the report is zero-padded.
    pub fn set_feature(&self, data: &[u8]) -> Result<()> {
        let buf = with_report_id(data, self.report_len);
        // SAFETY: `buf` is report_len + 1 bytes, the length we pass.
        let ok = unsafe {
            HidD_SetFeature(self.handle, buf.as_ptr() as *const c_void, buf.len() as u32)
        };
        if ok {
            Ok(())
        } else {
            Err(Error::Hid(format!(
                "SetFeature failed (Windows error {})",
                last_error()
            )))
        }
    }

    /// Fetch one feature report (without the report-ID byte).
    pub fn get_feature(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; self.report_len + 1];
        // SAFETY: `buf` is report_len + 1 bytes, the length we pass; byte 0 is the report ID (0).
        let ok = unsafe {
            HidD_GetFeature(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
            )
        };
        if !ok {
            return Err(Error::Hid(format!(
                "GetFeature failed (Windows error {})",
                last_error()
            )));
        }
        buf.remove(0);
        Ok(buf)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: `handle` came from a successful CreateFileW and is closed only here.
        unsafe { CloseHandle(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::interface_number;

    #[test]
    fn interface_number_from_path() {
        assert_eq!(
            interface_number(r"\\?\hid#vid_3710&pid_1404&mi_03#a&200beea6&0&0000#{4d1e}"),
            3
        );
        assert_eq!(
            interface_number(r"\\?\hid#vid_1532&pid_0078&mi_01&col03#8&d5e9#{4d1e}"),
            1
        );
        assert_eq!(
            interface_number(r"\\?\hid#vid_046d&pid_c077#7&1f4f#{4d1e}"),
            -1
        );
    }
}
