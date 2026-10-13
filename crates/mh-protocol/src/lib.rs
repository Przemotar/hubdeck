//! HID protocol of the Cooler Master MasterHUB modules. Pure encoding and decoding, no I/O.
//!
//! Every module is its own USB HID device ([`module::VENDOR_ID`], product id per [`ModuleKind`]).
//! See `PROTOCOL.md` in the repository for what was verified on which hardware.

pub mod descriptor;
pub mod input;
pub mod module;
pub mod report;
pub mod safety;

pub use input::{Input, Orientation};
pub use module::{ModuleKind, VENDOR_ID};
pub use report::{LedMode, LedTarget, Query, Report, Rgb};
