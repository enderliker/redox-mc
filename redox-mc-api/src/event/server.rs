//! Server lifecycle and gameplay events.

use super::bus::{CancellableEvent, Event};

/// Event dispatched on every server tick loop (`MinecraftServer.tickChildren`).
///
/// Dispatched at ~20 Hz (every 50 ms) during normal gameplay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerTickEvent {
    /// Monotonically increasing server tick count.
    pub tick_count: i64,
}

impl ServerTickEvent {
    /// Creates a new `ServerTickEvent` with the specified tick count.
    #[inline]
    pub const fn new(tick_count: i64) -> Self {
        Self { tick_count }
    }

    /// Returns the current tick count.
    #[inline]
    pub const fn tick_count(&self) -> i64 {
        self.tick_count
    }
}

impl Event for ServerTickEvent {}

/// Event dispatched when a player sends an in-game chat message (`ServerGamePacketListenerImpl.handleChat`).
///
/// This event is cancellable: if cancelled, the chat packet is discarded and
/// not delivered or broadcast to other players or chat logs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerChatEvent {
    /// The name of the player who sent the chat message.
    pub player_name: String,
    /// The chat message content.
    pub message: String,
    cancelled: bool,
}

impl ServerChatEvent {
    /// Creates a new `ServerChatEvent`.
    pub fn new(player_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            player_name: player_name.into(),
            message: message.into(),
            cancelled: false,
        }
    }

    /// Returns the name of the player who sent the message.
    #[inline]
    pub fn player_name(&self) -> &str {
        &self.player_name
    }

    /// Returns the chat message content.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Event for ServerChatEvent {
    #[inline]
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    #[inline]
    fn set_cancelled(&mut self, cancelled: bool) {
        self.cancelled = cancelled;
    }
}

impl CancellableEvent for ServerChatEvent {}
