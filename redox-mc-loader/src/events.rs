//! Event dispatching layer connecting the Java bridge and loaded mods.

/// Dispatches a server tick event to all loaded native mods.
pub fn dispatch_server_tick(tick_count: i64) {
    crate::mod_loader::on_server_tick(tick_count);
}

/// Dispatches a player chat event to all loaded native mods.
///
/// Returns `true` if the message is allowed and should be delivered to recipients.
/// Returns `false` if the message was cancelled by any handler or mod, dropping the packet.
pub fn dispatch_server_chat(player_name: &str, message: &str) -> bool {
    crate::mod_loader::on_server_chat(player_name, message)
}
