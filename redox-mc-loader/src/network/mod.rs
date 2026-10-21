//! Network channel dispatch and mod payload decoding.

use std::collections::HashMap;
use std::sync::RwLock;

pub type ChannelHandler = Box<dyn Fn(&[u8]) + Send + Sync + 'static>;

static CHANNEL_HANDLERS: RwLock<Option<HashMap<String, Vec<ChannelHandler>>>> = RwLock::new(None);

/// Registers a channel handler for a given channel (e.g. "testmod:data").
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

/// Dispatches a received mod payload to registered listeners.
pub fn dispatch_mod_payload(namespace: &str, path: &str, data: &[u8]) {
    let full_channel = format!("{}:{}", namespace, path);
    let guard = CHANNEL_HANDLERS.read().unwrap();
    if let Some(map) = guard.as_ref() {
        if let Some(handlers) = map.get(&full_channel) {
            for handler in handlers {
                handler(data);
            }
        }
    }
}
