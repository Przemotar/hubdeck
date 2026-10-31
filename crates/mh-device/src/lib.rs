//! USB HID transport and hot-plug hub for the MasterHUB modules.
//!
//! ```no_run
//! use mh_device::{Event, Hub};
//! use mh_protocol::ModuleKind;
//!
//! let hub = Hub::start()?;
//! for event in hub.events().iter() {
//!     match event {
//!         Event::Connected(id) if id.kind == ModuleKind::Keys => { /* draw the keys */ }
//!         Event::KeyDown { index, .. } => println!("key {index} down"),
//!         _ => {}
//!     }
//! }
//! # Ok::<(), mh_device::Error>(())
//! ```

mod events;
mod hub;
pub mod location;
mod module;
mod notify;
#[cfg(windows)]
mod overlapped;
/// Public for `examples/pipeline_mac.rs`; HubDeck uses it through [`Module::send_all`].
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub mod pictures;
pub mod recover;

pub use events::{Event, ModuleId};
pub use hub::Hub;
pub use module::Module;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HID error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("module {0} is not connected")]
    NotConnected(String),
    #[error("refusing a forbidden report (firmware, serial number or factory command)")]
    Forbidden,
    #[error("{kind} {serial} did not answer after switching to software mode; pull it off the base and put it back")]
    NoAnswer { kind: &'static str, serial: String },
    #[error("{0} is in hardware mode; reading it now would wedge its firmware")]
    HardwareMode(&'static str),
    #[error("{0} does not support this request")]
    Unsupported(&'static str),
    #[error("{0}")]
    Write(String),
}

pub type Result<T> = std::result::Result<T, Error>;
