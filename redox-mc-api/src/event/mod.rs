//! Event subsystem for Redox-MC mods.
//!
//! Provides a strongly-typed, priority-ordered, thread-safe pub/sub event bus
//! with cancellation support.

pub mod bus;
pub mod server;

pub use bus::{
    global, CancellableEvent, Event, EventBus, EventPriority, EventResult, IntoEventResult,
    SubscriptionId,
};
pub use server::{ServerChatEvent, ServerTickEvent};
