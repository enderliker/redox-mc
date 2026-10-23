//! Network channel dispatch and mod payload decoding.
//!
//! This module keeps the loader-local compatibility registry used by bridge
//! tests and by the legacy `onModCustomPayload` JNI entry point. Loaded mods
//! register their own typed handlers through `redox-mc-api` and receive
//! payloads via the FFI dispatch callback.

pub mod handshake;

use crate::manifest::ModSide;
use redox_mc_abi::types::{NETWORK_SIDE_CLIENT, NETWORK_SIDE_SERVER};
use std::collections::HashMap;
use std::sync::RwLock;

pub type ChannelHandler = Box<dyn Fn(&[u8]) + Send + Sync + 'static>;

static CHANNEL_HANDLERS: RwLock<Option<HashMap<String, Vec<ChannelHandler>>>> = RwLock::new(None);

/// Registers a loader-local channel handler for a given channel (e.g. "testmod:data").
pub fn register_channel_handler<F>(channel: &str, handler: F)
where
    F: Fn(&[u8]) + Send + Sync + 'static,
{
    let mut guard = CHANNEL_HANDLERS.write().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.entry(channel.to_string())
        .or_default()
        .push(Box::new(handler));
}

/// Dispatches a received mod payload to loader-local listeners.
pub fn dispatch_local_handlers(channel: &str, data: &[u8]) {
    let guard = CHANNEL_HANDLERS.read().unwrap();
    if let Some(map) = guard.as_ref() {
        if let Some(handlers) = map.get(channel) {
            for handler in handlers {
                handler(data);
            }
        }
    }
}

/// Dispatches a received mod payload to loader-local listeners using the legacy
/// `namespace` + `path` representation.
pub fn dispatch_mod_payload(namespace: &str, path: &str, data: &[u8]) {
    let full_channel = format!("{}:{}", namespace, path);
    dispatch_local_handlers(&full_channel, data);
}

/// Maps a [`ModSide`] to its ABI `NETWORK_SIDE_*` constant.
#[inline]
pub fn side_to_abi(side: ModSide) -> u32 {
    match side {
        ModSide::Client => NETWORK_SIDE_CLIENT,
        ModSide::Both | ModSide::Server => NETWORK_SIDE_SERVER,
    }
}

/// Maps an ABI `NETWORK_SIDE_*` constant to a [`ModSide`].
#[inline]
pub fn side_from_abi(value: u32) -> Option<ModSide> {
    match value {
        NETWORK_SIDE_SERVER => Some(ModSide::Server),
        NETWORK_SIDE_CLIENT => Some(ModSide::Client),
        _ => None,
    }
}
