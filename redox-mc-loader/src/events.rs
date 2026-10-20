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

/// Dispatches an item use event (right-click in air).
/// Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
pub fn dispatch_item_use(item_id: &str, hand: i32) -> i32 {
    crate::mod_loader::on_item_use(item_id, hand)
}

/// Dispatches an item useOn event (right-click on block).
/// Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
pub fn dispatch_item_use_on(item_id: &str, x: i32, y: i32, z: i32, face: i32, hand: i32) -> i32 {
    crate::mod_loader::on_item_use_on(item_id, x, y, z, face, hand)
}

/// Dispatches an item finish using event.
pub fn dispatch_item_finish_using(item_id: &str) {
    crate::mod_loader::on_item_finish_using(item_id);
}
