//! Packet construction system for token-efficient context management
//!
//! This module implements the packet building system that selects and organizes
//! content for Claude CLI invocations while respecting budget constraints and
//! maintaining evidence for auditability.

mod budget;
mod builder;
mod model;
mod render;
mod selectors;

pub use builder::{DEFAULT_PACKET_MAX_BYTES, DEFAULT_PACKET_MAX_LINES, PacketBuilder};
pub use model::{PriorityRules, SelectedFile};
pub use selectors::ContentSelector;
