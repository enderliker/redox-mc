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
    FfiStatus::Success
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
