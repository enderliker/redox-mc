//! JNI entry points for `net.redoxmc.bridge.RedoxBridge`.
//!
//! The module is split by responsibility:
//!
//! - [`register`]: `RegisterNatives` binding table.
//! - [`events`]: lifecycle, tick, chat, item, and block callbacks.
//! - [`network`]: mod payload dispatch and outbound outbox draining.
//! - [`registry`]: virtual resources and the registry content hash.
//! - [`handshake`]: handshake manifest and client validation.
//! - [`strings`]: JNI conversion helpers.

mod events;
mod handshake;
mod network;
mod register;
mod registry;
mod strings;

pub use register::register_bridge_natives;
