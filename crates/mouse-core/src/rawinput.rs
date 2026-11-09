//! Which mice are plugged in, under the device ids RawAccel uses.
//!
//! Port of the device list in `rawinput_monitor.py`: Windows' Raw Input device list, each
//! mouse's interface name, then its device instance id with the last `\`-segment cut off
//! (`HID\VID_3710&PID_1404&MI_00\7&…&0000` → `HID\VID_3710&PID_1404&MI_00`). That is the id
//! RawAccel's driver matches against `devices[].id`, case-sensitively.

/// One mouse as Raw Input lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawMouse {
    /// RawAccel's device id, or "" if Windows didn't give one.
    pub id: String,
    /// "Pulsar X2A", "Razer Viper", or "Mouse VID_xxxx&PID_xxxx" for anything else.
    pub name: String,
}

impl RawMouse {
    /// True for the mice this tool knows by name (the Python lists only those).
    pub fn is_known(&self) -> bool {
        !self.name.starts_with("Mouse ")
    }
}

/// Mice we know by name, keyed by the VID/PID part of their id. The M42 isn't in the Python's
/// list, so `set-mouse rotation` there hides it; here it shows (it has its own RawAccel
/// profile, "Xtrfy M42").
pub const KNOWN: [(&str, &str); 3] = [
    ("VID_3710&PID_1404", "Pulsar X2A"),
    ("VID_1532&PID_0078", "Razer Viper"),
    ("VID_2EA8&PID_2203", "Xtrfy M42"),
];

/// Display name for a device id (or interface name, if there is no id).
pub fn label(dev_id: &str) -> String {
    let up = dev_id.to_uppercase();
    if let Some((_, name)) = KNOWN.iter().find(|(key, _)| up.contains(key)) {
        return name.to_string();
    }
    let last = up.rsplit('\\').next().unwrap_or("");
    let parts: Vec<&str> = last
        .split('&')
        .filter(|p| p.starts_with("VID_") || p.starts_with("PID_"))
        .collect();
    format!("Mouse {}", parts.join("&"))
}

/// RawAccel's id from a device instance id: everything before the last `\`.
pub fn rawaccel_id(instance_id: &str) -> &str {
    instance_id
        .rfind('\\')
        .map_or(instance_id, |i| &instance_id[..i])
}

/// Every mouse Raw Input reports, in Windows' order (remote-desktop mice left out).
#[cfg(windows)]
pub fn mice() -> Vec<RawMouse> {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_Device_Interface_PropertyW, CR_SUCCESS,
    };
    use windows_sys::Win32::Devices::Properties::{DEVPKEY_Device_InstanceId, DEVPROPTYPE};
    use windows_sys::Win32::UI::Input::{
        GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICENAME,
        RIM_TYPEMOUSE,
    };

    const ERR: u32 = u32::MAX;
    let entry = std::mem::size_of::<RAWINPUTDEVICELIST>() as u32;

    // SAFETY (whole block): standard two-call Win32 pattern. First ask for the size, then pass
    // a buffer of exactly that size; every buffer outlives the call that fills it.
    let list = unsafe {
        let mut n = 0u32;
        GetRawInputDeviceList(std::ptr::null_mut(), &mut n, entry);
        let mut list: Vec<RAWINPUTDEVICELIST> = vec![std::mem::zeroed(); n as usize];
        let got = GetRawInputDeviceList(list.as_mut_ptr(), &mut n, entry);
        if got == ERR {
            return Vec::new();
        }
        list.truncate(got as usize);
        list
    };

    let interface_name = |h| unsafe {
        let mut n = 0u32;
        GetRawInputDeviceInfoW(h, RIDI_DEVICENAME, std::ptr::null_mut(), &mut n);
        let mut buf = vec![0u16; n as usize + 1];
        let r = GetRawInputDeviceInfoW(h, RIDI_DEVICENAME, buf.as_mut_ptr().cast(), &mut n);
        if r == 0 || r == ERR {
            return Vec::new();
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        buf.truncate(len);
        buf
    };

    let instance_id = |iface: &[u16]| unsafe {
        let mut z: Vec<u16> = iface.to_vec();
        z.push(0);
        let mut ptype: DEVPROPTYPE = 0;
        let mut size = 0u32;
        CM_Get_Device_Interface_PropertyW(
            z.as_ptr(),
            &DEVPKEY_Device_InstanceId,
            &mut ptype,
            std::ptr::null_mut(),
            &mut size,
            0,
        );
        if size == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        let r = CM_Get_Device_Interface_PropertyW(
            z.as_ptr(),
            &DEVPKEY_Device_InstanceId,
            &mut ptype,
            buf.as_mut_ptr().cast(),
            &mut size,
            0,
        );
        if r != CR_SUCCESS {
            return String::new();
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
    };

    let mut out = Vec::new();
    for d in list {
        if d.dwType != RIM_TYPEMOUSE || d.hDevice.is_null() {
            continue;
        }
        let iface = interface_name(d.hDevice);
        if iface.is_empty() {
            continue;
        }
        let iface_text = String::from_utf16_lossy(&iface);
        if iface_text.to_uppercase().contains("RDP_MOU") {
            continue;
        }
        let id = rawaccel_id(&instance_id(&iface)).to_string();
        let name = label(if id.is_empty() { &iface_text } else { &id });
        out.push(RawMouse { id, name });
    }
    out
}

/// No RawAccel outside Windows, so no list of mice for it.
#[cfg(not(windows))]
pub fn mice() -> Vec<RawMouse> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_like_python() {
        assert_eq!(label(r"HID\VID_3710&PID_1404&MI_00"), "Pulsar X2A");
        assert_eq!(label(r"HID\vid_1532&pid_0078&mi_00"), "Razer Viper");
        assert_eq!(label(r"HID\VID_2EA8&PID_2203&MI_00"), "Xtrfy M42");
        assert_eq!(
            label(r"HID\VID_0483&PID_5710&MI_02&Col03"),
            "Mouse VID_0483&PID_5710"
        );
    }

    #[test]
    fn rawaccel_id_drops_the_instance_segment() {
        assert_eq!(
            rawaccel_id(r"HID\VID_3710&PID_1404&MI_00\7&1a2b3c&0&0000"),
            r"HID\VID_3710&PID_1404&MI_00"
        );
        assert_eq!(rawaccel_id("plain"), "plain");
    }
}
