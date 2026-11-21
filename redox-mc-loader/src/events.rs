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

/// Dispatches a block useWithoutItem event.
/// Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
pub fn dispatch_block_use_without_item(
    block_id: &str,
    x: i32,
    y: i32,
    z: i32,
    direction: i32,
) -> i32 {
    crate::mod_loader::on_block_use_without_item(block_id, x, y, z, direction)
}

/// Dispatches a block stepOn event.
pub fn dispatch_block_step_on(block_id: &str, x: i32, y: i32, z: i32) {
    crate::mod_loader::on_block_step_on(block_id, x, y, z);
}

/// Dispatches a block destroy event.
pub fn dispatch_block_destroy(block_id: &str, x: i32, y: i32, z: i32) {
    crate::mod_loader::on_block_destroy(block_id, x, y, z);
}

/// Dispatches a block entity tick event.
pub fn dispatch_block_entity_tick(id: &str, x: i32, y: i32, z: i32) {
    crate::mod_loader::on_block_entity_tick(id, x, y, z);
}

/// Dispatches a block entity save event.
pub fn dispatch_block_entity_save(id: &str, x: i32, y: i32, z: i32) -> Option<String> {
    crate::mod_loader::on_block_entity_save(id, x, y, z)
}

/// Dispatches a block entity load event.
pub fn dispatch_block_entity_load(id: &str, x: i32, y: i32, z: i32, data: &str) {
    crate::mod_loader::on_block_entity_load(id, x, y, z, data);
}

/// Dispatches a container menu open event.
pub fn dispatch_menu_open(menu_id: &str, container_id: i32, player_name: &str) {
    crate::mod_loader::on_menu_open(menu_id, container_id, player_name);
}

/// Dispatches a container menu slot click event.
/// Returns 0 for Allow, 1 for Deny.
pub fn dispatch_menu_slot_click(
    menu_id: &str,
    container_id: i32,
    slot_index: i32,
    button: i32,
    click_type: i32,
    player_name: &str,
) -> i32 {
    crate::mod_loader::on_menu_slot_click(
        menu_id,
        container_id,
        slot_index,
        button,
        click_type,
        player_name,
    )
}

/// Dispatches a container menu close event.
pub fn dispatch_menu_close(menu_id: &str, container_id: i32, player_name: &str) {
    crate::mod_loader::on_menu_close(menu_id, container_id, player_name);
}
