//! mouse-core: everything that talks to the mice.
//!
//! Settings are written into each mouse's own memory, so nothing here sits in the input path:
//! a program built on this crate configures a mouse, then exits.
//!
//! - [`hid`]: thin cross-platform HID layer (feature reports): Win32 on Windows, hidapi on macOS
//! - [`x2a`]: Pulsar X2A protocol (wired, USB 3710:1404)
//! - [`viper`]: Razer Viper protocol (wired, USB 1532:0078)

pub mod hid;
pub mod viper;
pub mod x2a;

use std::fmt;

/// Every failure this crate reports. Messages are written for the person running the tool.
#[derive(Debug)]
pub enum Error {
    /// The mouse (or the interface we need) isn't connected.
    NotFound(String),
    /// The operating system refused or failed a HID operation.
    Hid(String),
    /// The mouse answered, but not in the way the protocol says it should.
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(m) | Error::Hid(m) | Error::Protocol(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
