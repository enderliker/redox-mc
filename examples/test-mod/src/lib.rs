//! Example native Redox-MC mod written in 100% Rust.

use redox_mc_api::event::{
    global, CancellableEvent, Event, EventPriority, ServerChatEvent, ServerTickEvent,
};
use redox_mc_api::redox_export;
use redox_mc_api::types::{FfiResult, FfiStatus, FfiStr};
use std::sync::atomic::{AtomicU64, Ordering};

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

#[redox_export]
pub fn redox_mod_init() -> i32 {
    println!("[test-mod] Native Rust mod initialized successfully via redox_mod_init()!");

    // Subscribe to ServerTickEvent on the typed Rust EventBus
    global().subscribe(|event: &mut ServerTickEvent| {
        let count = TICK_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        if count <= 10 || count.is_multiple_of(20) {
            println!(
                "[test-mod] EventBus received ServerTickEvent: tick={} (local_count={})",
                event.tick_count(),
                count
            );
        }
    });

    // Subscribe to ServerChatEvent on the typed Rust EventBus
    global().subscribe_with_priority(EventPriority::Normal, |event: &mut ServerChatEvent| {
        if event.message().starts_with('!') {
            println!(
                "[test-mod] EventBus received ServerChatEvent -> CANCELLED chat from '{}': '{}'",
                event.player_name(),
                event.message()
            );
            event.cancel();
        } else {
            println!(
                "[test-mod] EventBus received ServerChatEvent -> ALLOWED chat from '{}': '{}'",
                event.player_name(),
                event.message()
            );
        }
    });

    0
}

#[redox_export]
pub fn redox_mod_on_bootstrap() -> FfiStatus {
    println!("[test-mod] Hook received: Bootstrap.bootStrap() entered! Game is bootstrapping vanilla registries.");
    FfiStatus::Success
}

#[redox_export]
pub fn redox_mod_on_register_content() -> FfiStatus {
    println!(
        "[test-mod] Hook received: BuiltInRegistries.freeze()! Registries open for custom content."
    );
    let ruby = redox_mc_api::item::ItemDefinition::new("testmod:ruby")
        .max_stack_size(64)
        .fire_resistant(true)
        .has_use(true);
    redox_mc_api::item::register_item(ruby);
    println!("[test-mod] Registered custom item: 'testmod:ruby'");

    let ruby_block = redox_mc_api::block::BlockDefinition::new("testmod:ruby_block")
        .strength(3.0, 6.0)
        .requires_tool(true)
        .has_use_without_item(true);
    redox_mc_api::block::register_block(ruby_block);
    println!("[test-mod] Registered custom block: 'testmod:ruby_block'");

    // Register in-memory virtual recipe for testmod:ruby
    redox_mc_api::resource::register_data_resource(
        "testmod",
        "recipe/ruby_from_crafting.json",
        br#"{
  "type": "minecraft:crafting_shapeless",
  "category": "misc",
  "ingredients": [
    "minecraft:diamond"
  ],
  "result": {
    "count": 1,
    "id": "testmod:ruby"
  }
}"#,
    );

    // Register in-memory virtual loot table for testmod:ruby
    redox_mc_api::resource::register_data_resource(
        "testmod",
        "loot_table/blocks/ruby_block.json",
        br#"{
  "type": "minecraft:block",
  "pools": [
    {
      "entries": [
        {
          "type": "minecraft:item",
          "name": "testmod:ruby"
        }
      ],
      "rolls": 1
    }
  ]
}"#,
    );
    println!("[test-mod] Registered in-memory virtual recipe and loot table for 'testmod:ruby'");

    FfiStatus::Success
}

#[redox_export]
pub fn redox_mod_on_item_use(item_id: FfiStr, hand: i32) -> i32 {
    let id = unsafe { item_id.as_str().unwrap_or("") };
    println!("[test-mod] Custom item used: '{}' (hand={})", id, hand);
    1 // SUCCESS
}

#[redox_export]
pub fn redox_mod_on_block_use_without_item(
    block_id: FfiStr,
    x: i32,
    y: i32,
    z: i32,
    direction: i32,
) -> i32 {
    let id = unsafe { block_id.as_str().unwrap_or("") };
    println!(
        "[test-mod] Custom block interacted without item: '{}' at ({}, {}, {}) direction={}",
        id, x, y, z, direction
    );
    1 // SUCCESS
}

#[redox_export]
pub fn redox_mod_on_server_tick(tick_count: i64) -> FfiStatus {
    let mut event = ServerTickEvent::new(tick_count);
    global().dispatch(&mut event);
    FfiStatus::Success
}

#[redox_export]
pub fn redox_mod_on_server_chat(player_name: FfiStr, message: FfiStr) -> FfiResult<bool> {
    let player = unsafe { player_name.as_str().unwrap_or("unknown") };
    let msg = unsafe { message.as_str().unwrap_or("") };

    let mut event = ServerChatEvent::new(player, msg);
    let res = global().dispatch(&mut event);

    if res.is_cancelled() || event.is_cancelled() {
        FfiResult::ok(false)
    } else {
        FfiResult::ok(true)
    }
}
