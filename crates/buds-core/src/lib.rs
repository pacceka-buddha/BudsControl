//! Platform-independent core of Buds Control: protocol, device catalog and the controller.
//! No OS code lives here.

pub mod catalog;
pub mod cli;
pub mod consts;
pub mod controller;
pub mod encode;
#[cfg(feature = "fake")]
pub mod fake;
pub mod ipc;
pub mod packet_log;
pub mod parse;
pub mod paths;
pub mod platform;
pub mod protocol;
pub mod settings;
pub mod types;
