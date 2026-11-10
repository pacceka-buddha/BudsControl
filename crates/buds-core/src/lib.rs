//! Platform-independent core of Buds Control: protocol, device catalog and (from Phase 2) the
//! controller. No OS code lives here; see PLAN.md for the architecture.

pub mod catalog;
pub mod encode;
pub mod parse;
pub mod protocol;
pub mod types;
