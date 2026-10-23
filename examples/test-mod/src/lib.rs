//! Example native Redox-MC mod written in 100% Rust.

use redox_mc_api::event::{
    global, CancellableEvent, Event, EventPriority, ServerChatEvent, ServerTickEvent,
};
use redox_mc_api::network::{NetworkChannel, NetworkContext, NetworkError, NetworkMessage};
use redox_mc_api::redox_export;
use redox_mc_api::types::{FfiResult, FfiStatus, FfiStr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const PING_CHANNEL: &str = "testmod:ping";
const PONG_CHANNEL: &str = "testmod:pong";
const ACK_CHANNEL: &str = "testmod:ack";

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
static PING_NONCE: AtomicU64 = AtomicU64::new(0);
static PING_SENT: AtomicBool = AtomicBool::new(false);
static PONG_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Typed example message: a ping carrying a nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PingMessage {
    nonce: u64,
}

impl NetworkMessage for PingMessage {
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.nonce.to_le_bytes().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        let raw: [u8; 8] = bytes.try_into().map_err(|_| {
            NetworkError::Decode(format!(
                "expected an 8-byte ping, got {} bytes",
                bytes.len()
            ))
        })?;
        Ok(Self {
            nonce: u64::from_le_bytes(raw),
        })
    }
}

/// Typed example message: a pong echoing the nonce and identifying the responder.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PongMessage {
    nonce: u64,
    responder: String,
}

impl NetworkMessage for PongMessage {
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        let mut data = self.nonce.to_le_bytes().to_vec();
        data.extend_from_slice(self.responder.as_bytes());
        Ok(data)
    }

    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        if bytes.len() < 8 {
            return Err(NetworkError::Decode(format!(
                "pong payload is shorter than 8 bytes ({} bytes)",
                bytes.len()
            )));
        }
        let raw: [u8; 8] = bytes[..8]
            .try_into()
            .map_err(|_| NetworkError::Decode("failed to read the pong nonce".to_string()))?;
        let responder = String::from_utf8(bytes[8..].to_vec())
            .map_err(|err| NetworkError::Decode(format!("invalid pong responder: {err}")))?;
        Ok(Self {
            nonce: u64::from_le_bytes(raw),
            responder,
        })
    }
}

/// Typed example message: an acknowledgement emitted after a valid pong.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AckMessage {
    nonce: u64,
    responder: String,
}

impl NetworkMessage for AckMessage {
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        let mut data = self.nonce.to_le_bytes().to_vec();
        data.extend_from_slice(self.responder.as_bytes());
        Ok(data)
    }

    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        if bytes.len() < 8 {
            return Err(NetworkError::Decode(format!(
                "ack payload is shorter than 8 bytes ({} bytes)",
                bytes.len()
            )));
        }
        let raw: [u8; 8] = bytes[..8]
            .try_into()
            .map_err(|_| NetworkError::Decode("failed to read the ack nonce".to_string()))?;
        let responder = String::from_utf8(bytes[8..].to_vec())
            .map_err(|err| NetworkError::Decode(format!("invalid ack responder: {err}")))?;
        Ok(Self {
            nonce: u64::from_le_bytes(raw),
            responder,
        })
    }
}

// Required exactly once per mod crate: exports the inbound dispatch and outbox
// drain symbols resolved by the native loader.
redox_mc_api::export_network_handlers!();

#[redox_export]
pub fn redox_mod_init() -> i32 {
    println!("[test-mod] Native Rust mod initialized successfully via redox_mod_init()!");

    // Register the typed ping/pong demonstration channels.
    let ping_channel = NetworkChannel::<PingMessage>::register(PING_CHANNEL)
        .expect("testmod:ping must be a valid channel identifier");
    let pong_channel = NetworkChannel::<PongMessage>::register(PONG_CHANNEL)
        .expect("testmod:pong must be a valid channel identifier");
    let ack_channel = NetworkChannel::<AckMessage>::register(ACK_CHANNEL)
        .expect("testmod:ack must be a valid channel identifier");

    let pong_reply = pong_channel.clone();
    ping_channel.on_receive(move |ctx: &NetworkContext, ping: PingMessage| {
        println!(
            "[test-mod] Received ping nonce={} from '{}' ({}).",
            ping.nonce,
            ctx.sender().name(),
            ctx.sender().uuid()
        );
        let pong = PongMessage {
            nonce: ping.nonce,
            responder: ctx.sender().name().to_string(),
        };
        match pong_reply.reply(ctx, &pong) {
            Ok(()) => println!(
                "[test-mod] Queued pong nonce={} to '{}'.",
                pong.nonce,
                ctx.sender().name()
            ),
            Err(err) => eprintln!("[test-mod] Failed to queue pong: {err}"),
        }
    });

    pong_channel.on_receive(move |ctx: &NetworkContext, pong: PongMessage| {
        let expected = PING_NONCE.load(Ordering::SeqCst);
        if pong.nonce != expected {
            eprintln!(
                "[test-mod] Ignoring pong with unexpected nonce={} (expected {}).",
                pong.nonce, expected
            );
            return;
        }

        let already_validated = PONG_RECEIVED.swap(true, Ordering::SeqCst);
        println!(
            "[test-mod] Pong validated: nonce={} responder='{}' received from '{}' ({}).",
            pong.nonce,
            pong.responder,
            ctx.sender().name(),
            ctx.sender().uuid()
        );
        if already_validated {
            return;
        }

        let ack = AckMessage {
            nonce: pong.nonce,
            responder: pong.responder,
        };
        if let Err(err) = ack_channel.reply(ctx, &ack) {
            eprintln!("[test-mod] Failed to queue ack: {err}");
        } else {
            println!(
                "[test-mod] Queued ack nonce={} after pong validation.",
                ack.nonce
            );
        }
    });
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
        .strength(3.0)
        .explosion_resistance(6.0)
        .cube_model("testmod:block/ruby_block")
        .drops("testmod:ruby")
        .requires_tool(true)
        .has_use_without_item(true);
    redox_mc_api::block::register_block(ruby_block);
    println!("[test-mod] Registered custom block: 'testmod:ruby_block' with declarative cube model and loot table");

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
    println!("[test-mod] Registered in-memory virtual recipe for 'testmod:ruby'");

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
pub fn redox_mod_on_block_step_on(block_id: FfiStr, x: i32, y: i32, z: i32) {
    let id = unsafe { block_id.as_str().unwrap_or("") };
    println!(
        "[test-mod] Custom block stepped on: '{}' at ({}, {}, {})",
        id, x, y, z
    );
}

#[redox_export]
pub fn redox_mod_on_block_destroy(block_id: FfiStr, x: i32, y: i32, z: i32) {
    let id = unsafe { block_id.as_str().unwrap_or("") };
    println!(
        "[test-mod] Custom block destroyed: '{}' at ({}, {}, {})",
        id, x, y, z
    );
}

#[redox_export]
pub fn redox_mod_on_server_tick(tick_count: i64) -> FfiStatus {
    let mut event = ServerTickEvent::new(tick_count);
    global().dispatch(&mut event);

    // Send a single ping from the server side. Clients reply with a pong, and the
    // server-side handler validates it and emits an ack.
    if !PING_SENT.swap(true, Ordering::SeqCst) {
        let nonce = u64::try_from(tick_count).unwrap_or(0).max(1);
        PING_NONCE.store(nonce, Ordering::SeqCst);
        match NetworkChannel::<PingMessage>::register(PING_CHANNEL) {
            Ok(channel) => match channel.send(&PingMessage { nonce }) {
                Ok(()) => println!(
                    "[test-mod] Sent ping nonce={} (broadcast); waiting for pong.",
                    nonce
                ),
                Err(err) => {
                    PING_SENT.store(false, Ordering::SeqCst);
                    eprintln!("[test-mod] Failed to queue ping: {err}");
                }
            },
            Err(err) => {
                PING_SENT.store(false, Ordering::SeqCst);
                eprintln!("[test-mod] Failed to open ping channel: {err}");
            }
        }
    }

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
