//! Typed, bidirectional network channels for Redox-MC mods.
//!
//! A mod registers one or more channels by identifier (for example
//! `"testmod:data"`) and attaches a receive callback. Messages are encoded with
//! [`NetworkMessage`], queued in a thread-safe outbox, and drained by the native
//! loader on the next server/client tick or immediately after an inbound packet.
//!
//! # Example
//!
//! ```ignore
//! use redox_mc_api::network::{NetworkChannel, NetworkContext, NetworkMessage};
//!
//! #[derive(Clone)]
//! struct Ping { nonce: u64 }
//!
//! impl NetworkMessage for Ping {
//!     fn encode(&self) -> Result<Vec<u8>, redox_mc_api::network::NetworkError> {
//!         Ok(self.nonce.to_le_bytes().to_vec())
//!     }
//!     fn decode(bytes: &[u8]) -> Result<Self, redox_mc_api::network::NetworkError> {
//!         let raw: [u8; 8] = bytes.try_into().map_err(|_| {
//!             redox_mc_api::network::NetworkError::Decode("expected 8 bytes".into())
//!         })?;
//!         Ok(Self { nonce: u64::from_le_bytes(raw) })
//!     }
//! }
//!
//! let channel = NetworkChannel::<Ping>::register("testmod:ping").unwrap();
//! channel.on_receive(|ctx: &NetworkContext, ping: Ping| {
//!     println!("ping {} from {}", ping.nonce, ctx.sender().name());
//! });
//!
//! // Export the two FFI entry points exactly once in the mod crate root:
//! // redox_mc_api::export_network_handlers!();
//! ```

use redox_mc_abi::types::{
    FfiSlice, FfiStatus, FfiStr, NetworkOutboundVisitor, NETWORK_SIDE_CLIENT, NETWORK_SIDE_SERVER,
    NETWORK_TARGET_BROADCAST, NETWORK_TARGET_PLAYER, NETWORK_TARGET_SERVER,
};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Maximum size of a single encoded mod message (1 MiB, matching clientbound limits).
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Maximum number of messages that may wait in the outbox at once.
pub const MAX_OUTBOX_MESSAGES: usize = 1024;

/// Maximum combined payload bytes that may wait in the outbox at once.
pub const MAX_OUTBOX_BYTES: usize = 8 * 1024 * 1024;

/// Side on which a network callback is executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkSide {
    /// Dedicated or integrated server.
    Server,
    /// Game client.
    Client,
}

impl NetworkSide {
    /// Converts an ABI `NETWORK_SIDE_*` constant into a [`NetworkSide`].
    #[inline]
    pub const fn from_abi(value: u32) -> Option<Self> {
        match value {
            NETWORK_SIDE_SERVER => Some(Self::Server),
            NETWORK_SIDE_CLIENT => Some(Self::Client),
            _ => None,
        }
    }

    /// Returns the ABI `NETWORK_SIDE_*` constant for this side.
    #[inline]
    pub const fn as_abi(self) -> u32 {
        match self {
            Self::Server => NETWORK_SIDE_SERVER,
            Self::Client => NETWORK_SIDE_CLIENT,
        }
    }

    /// Returns a human-readable side name.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Client => "client",
        }
    }
}

/// Identity of the remote party that sent a network message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SenderId {
    name: String,
    uuid: String,
}

impl SenderId {
    /// Creates a sender identity from a player name and UUID string.
    #[inline]
    pub fn new(name: impl Into<String>, uuid: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uuid: uuid.into(),
        }
    }

    /// Creates the synthetic identity used by the client for inbound server messages.
    #[inline]
    pub fn server() -> Self {
        Self::new("server", "")
    }

    /// Returns the sender name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the sender UUID as a canonical string (empty for the server identity).
    #[inline]
    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    /// Returns `true` when this identity represents the remote server.
    #[inline]
    pub fn is_server(&self) -> bool {
        self.name == "server" && self.uuid.is_empty()
    }

    /// Converts this identity into a targetable [`PlayerId`], if it represents a player.
    #[inline]
    pub fn player_id(&self) -> Option<PlayerId> {
        if self.is_server() || (self.name.is_empty() && self.uuid.is_empty()) {
            None
        } else {
            Some(PlayerId::new(self.name.clone(), self.uuid.clone()))
        }
    }
}

/// Address of a specific player, used for server-to-client messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerId {
    name: String,
    uuid: String,
}

impl PlayerId {
    /// Creates a player address from a name and UUID string.
    #[inline]
    pub fn new(name: impl Into<String>, uuid: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uuid: uuid.into(),
        }
    }

    /// Returns the player name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the player UUID as a canonical string.
    #[inline]
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
}

/// Context passed to every receive callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkContext {
    channel: String,
    sender: SenderId,
    side: NetworkSide,
}

impl NetworkContext {
    /// Creates a receive context. Handlers normally receive this from the loader.
    #[inline]
    pub fn new(channel: impl Into<String>, sender: SenderId, side: NetworkSide) -> Self {
        Self {
            channel: channel.into(),
            sender,
            side,
        }
    }

    /// Returns the full channel identifier (`namespace:path`).
    #[inline]
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Returns the identity of the remote sender.
    #[inline]
    pub fn sender(&self) -> &SenderId {
        &self.sender
    }

    /// Returns the side on which this callback is executing.
    #[inline]
    pub fn side(&self) -> NetworkSide {
        self.side
    }

    /// Returns the sender as a targetable player, if it is not the synthetic server identity.
    #[inline]
    pub fn player(&self) -> Option<PlayerId> {
        self.sender.player_id()
    }

    /// Replies on `channel`, routing automatically to the sender.
    ///
    /// On the server this targets the originating player (falling back to a
    /// broadcast for the synthetic server identity). On the client it targets
    /// the remote server.
    pub fn reply<T: NetworkMessage>(
        &self,
        channel: &NetworkChannel<T>,
        message: &T,
    ) -> Result<(), NetworkError> {
        match self.side {
            NetworkSide::Server => match self.player() {
                Some(player) => channel.send_to_player(&player, message),
                None => channel.send(message),
            },
            NetworkSide::Client => channel.send_to_server(message),
        }
    }
}

/// Errors produced by the network channel API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    /// The channel identifier is not a valid lowercase Minecraft identifier.
    InvalidChannel(String),
    /// The encoded payload exceeds [`MAX_MESSAGE_SIZE`].
    MessageTooLarge { size: usize, max: usize },
    /// A [`NetworkMessage`] implementation failed to encode a value.
    Encode(String),
    /// A [`NetworkMessage`] implementation failed to decode a payload.
    Decode(String),
    /// The outbox reached [`MAX_OUTBOX_MESSAGES`].
    OutboxFull,
    /// The outbox reached [`MAX_OUTBOX_BYTES`].
    OutboxBytesFull,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChannel(channel) => {
                write!(f, "invalid network channel identifier '{channel}'")
            }
            Self::MessageTooLarge { size, max } => {
                write!(f, "network message is {size} bytes, maximum is {max}")
            }
            Self::Encode(reason) => write!(f, "failed to encode network message: {reason}"),
            Self::Decode(reason) => write!(f, "failed to decode network message: {reason}"),
            Self::OutboxFull => write!(
                f,
                "network outbox is full ({MAX_OUTBOX_MESSAGES} pending messages)"
            ),
            Self::OutboxBytesFull => write!(
                f,
                "network outbox is full ({MAX_OUTBOX_BYTES} pending bytes)"
            ),
        }
    }
}

impl Error for NetworkError {}

/// Binary codec for a typed channel payload.
pub trait NetworkMessage: Sized + Send + 'static {
    /// Serializes this value into a byte buffer.
    fn encode(&self) -> Result<Vec<u8>, NetworkError>;

    /// Deserializes a value from a borrowed payload.
    fn decode(bytes: &[u8]) -> Result<Self, NetworkError>;
}

impl NetworkMessage for Vec<u8> {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.clone())
    }

    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        Ok(bytes.to_vec())
    }
}

impl NetworkMessage for () {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(Vec::new())
    }

    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        if bytes.is_empty() {
            Ok(())
        } else {
            Err(NetworkError::Decode(format!(
                "expected an empty payload, got {} bytes",
                bytes.len()
            )))
        }
    }
}

impl NetworkMessage for String {
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.as_bytes().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        String::from_utf8(bytes.to_vec())
            .map_err(|err| NetworkError::Decode(format!("invalid UTF-8 string: {err}")))
    }
}

impl NetworkMessage for bool {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(vec![u8::from(*self)])
    }

    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        match bytes {
            [0] => Ok(false),
            [1] => Ok(true),
            other => Err(NetworkError::Decode(format!(
                "expected a 1-byte boolean, got {} bytes",
                other.len()
            ))),
        }
    }
}

macro_rules! impl_network_message_integer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NetworkMessage for $ty {
                #[inline]
                fn encode(&self) -> Result<Vec<u8>, NetworkError> {
                    Ok(self.to_le_bytes().to_vec())
                }

                #[inline]
                fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
                    let raw: [u8; std::mem::size_of::<$ty>()] =
                        bytes.try_into().map_err(|_| {
                            NetworkError::Decode(format!(
                                "expected {} bytes for {}, got {}",
                                std::mem::size_of::<$ty>(),
                                stringify!($ty),
                                bytes.len()
                            ))
                        })?;
                    Ok(<$ty>::from_le_bytes(raw))
                }
            }
        )*
    };
}

impl_network_message_integer!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

/// A channel-bound handle used to register handlers and send messages.
///
/// The type parameter `T` defaults to [`Vec<u8>`] for raw binary channels.
pub struct NetworkChannel<T = Vec<u8>> {
    id: String,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for NetworkChannel<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for NetworkChannel<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkChannel")
            .field("id", &self.id)
            .finish()
    }
}

impl<T: NetworkMessage> NetworkChannel<T> {
    /// Validates `channel` and creates a channel handle.
    ///
    /// Registration is idempotent: creating the same channel more than once is
    /// allowed, and every callback attached with [`Self::on_receive`] runs.
    pub fn register(channel: &str) -> Result<Self, NetworkError> {
        validate_channel(channel)?;
        Ok(Self {
            id: channel.to_string(),
            _marker: PhantomData,
        })
    }

    /// Returns the full channel identifier (`namespace:path`).
    #[inline]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Attaches a receive callback and returns the channel for chaining.
    ///
    /// The callback runs on the game thread (server or client) and receives the
    /// decoded payload together with the sender identity.
    pub fn on_receive<F>(self, handler: F) -> Self
    where
        F: Fn(&NetworkContext, T) + Send + Sync + 'static,
    {
        let channel_id = self.id.clone();
        let filter_id = self.id.clone();
        let boxed: Handler = Arc::new(move |ctx: &NetworkContext, data: &[u8]| {
            if ctx.channel() != filter_id {
                return;
            }
            match T::decode(data) {
                Ok(message) => handler(ctx, message),
                Err(err) => eprintln!(
                    "[redox-mc-api] Failed to decode message on channel '{}': {}",
                    ctx.channel(),
                    err
                ),
            }
        });

        let mut registry = handler_registry().write().unwrap();
        registry.entry(channel_id).or_default().push(boxed);
        drop(registry);
        self
    }

    /// Queues a message for the other side of the connection.
    ///
    /// On the server this broadcasts to every connected player; on the client it
    /// sends to the remote server.
    pub fn send(&self, message: &T) -> Result<(), NetworkError> {
        self.enqueue(message, Destination::Broadcast)
    }

    /// Queues a message addressed to the remote server. Intended for client-side mods.
    pub fn send_to_server(&self, message: &T) -> Result<(), NetworkError> {
        self.enqueue(message, Destination::Server)
    }

    /// Queues a message addressed to a specific player. Intended for server-side mods.
    pub fn send_to_player(&self, player: &PlayerId, message: &T) -> Result<(), NetworkError> {
        self.enqueue(
            message,
            Destination::Player {
                name: player.name.clone(),
                uuid: player.uuid.clone(),
            },
        )
    }

    /// Replies on this channel to the sender carried by `context`.
    pub fn reply(&self, context: &NetworkContext, message: &T) -> Result<(), NetworkError> {
        context.reply(self, message)
    }

    /// Sends raw bytes without invoking [`NetworkMessage::encode`].
    pub fn send_raw(&self, data: &[u8]) -> Result<(), NetworkError> {
        self.enqueue_encoded(data.to_vec(), Destination::Broadcast)
    }

    fn enqueue(&self, message: &T, destination: Destination) -> Result<(), NetworkError> {
        let data = message.encode()?;
        self.enqueue_encoded(data, destination)
    }

    fn enqueue_encoded(&self, data: Vec<u8>, destination: Destination) -> Result<(), NetworkError> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: MAX_MESSAGE_SIZE,
            });
        }

        let mut state = outbox().lock().unwrap();
        if state.queue.len() >= MAX_OUTBOX_MESSAGES {
            return Err(NetworkError::OutboxFull);
        }
        if state.bytes + data.len() > MAX_OUTBOX_BYTES {
            return Err(NetworkError::OutboxBytesFull);
        }

        state.bytes += data.len();
        state.queue.push_back(OutboundMessage {
            channel: self.id.clone(),
            destination,
            data,
        });
        Ok(())
    }
}

type Handler = Arc<dyn Fn(&NetworkContext, &[u8]) + Send + Sync + 'static>;

fn handler_registry() -> &'static RwLock<HashMap<String, Vec<Handler>>> {
    static REGISTRY: OnceLock<RwLock<HashMap<String, Vec<Handler>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Debug, Clone)]
enum Destination {
    Broadcast,
    Server,
    Player { name: String, uuid: String },
}

#[derive(Debug)]
struct OutboundMessage {
    channel: String,
    destination: Destination,
    data: Vec<u8>,
}

struct OutboxState {
    queue: VecDeque<OutboundMessage>,
    bytes: usize,
}

fn outbox() -> &'static Mutex<OutboxState> {
    static OUTBOX: OnceLock<Mutex<OutboxState>> = OnceLock::new();
    OUTBOX.get_or_init(|| {
        Mutex::new(OutboxState {
            queue: VecDeque::new(),
            bytes: 0,
        })
    })
}

/// Returns the number of messages currently waiting in the outbox.
#[inline]
pub fn outbox_len() -> usize {
    outbox().lock().unwrap().queue.len()
}

/// Drops every queued outbound message. Primarily useful in tests and on shutdown.
pub fn clear_outbox() {
    let mut state = outbox().lock().unwrap();
    state.queue.clear();
    state.bytes = 0;
}

/// Drains the outbox, invoking `visitor` once per queued message.
///
/// The visitor receives `(channel, target_kind, target_name, target_uuid, data)`.
/// Returning `false` marks the message as undeliverable; it is logged and dropped
/// so a permanently unavailable target cannot block the rest of the queue.
pub fn drain_outbox<F>(mut visitor: F)
where
    F: FnMut(&str, u32, &str, &str, &[u8]) -> bool,
{
    let messages = {
        let mut state = outbox().lock().unwrap();
        state.bytes = 0;
        std::mem::take(&mut state.queue)
    };

    for message in messages {
        let (target_kind, target_name, target_uuid) = match &message.destination {
            Destination::Broadcast => (NETWORK_TARGET_BROADCAST, "", ""),
            Destination::Server => (NETWORK_TARGET_SERVER, "", ""),
            Destination::Player { name, uuid } => {
                (NETWORK_TARGET_PLAYER, name.as_str(), uuid.as_str())
            }
        };

        let delivered = visitor(
            &message.channel,
            target_kind,
            target_name,
            target_uuid,
            &message.data,
        );
        if !delivered {
            eprintln!(
                "[redox-mc-api] Dropping undeliverable network message on channel '{}' (target kind {}).",
                message.channel, target_kind
            );
        }
    }
}

/// Delivers an inbound payload to every handler registered for `channel`.
///
/// Returns the number of callbacks that ran. Unknown channels are ignored so
/// that multiple mods can receive the same loader-wide dispatch safely.
pub fn dispatch_received(
    channel: &str,
    data: &[u8],
    sender_name: &str,
    sender_uuid: &str,
    side: NetworkSide,
) -> usize {
    let handlers: Vec<Handler> = {
        let registry = handler_registry().read().unwrap();
        match registry.get(channel) {
            Some(handlers) => handlers.clone(),
            None => return 0,
        }
    };

    let context = NetworkContext {
        channel: channel.to_string(),
        sender: SenderId::new(sender_name, sender_uuid),
        side,
    };

    let count = handlers.len();
    for handler in handlers {
        handler(&context, data);
    }
    count
}

fn validate_channel(channel: &str) -> Result<(), NetworkError> {
    let invalid = || NetworkError::InvalidChannel(channel.to_string());

    if channel.is_empty() || channel.len() > 256 {
        return Err(invalid());
    }

    let Some((namespace, path)) = channel.split_once(':') else {
        return Err(invalid());
    };

    if namespace.is_empty() || path.is_empty() || path.contains(':') {
        return Err(invalid());
    }

    if matches!(namespace, "minecraft" | "realms" | "redox") {
        return Err(invalid());
    }

    if !namespace
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'))
    {
        return Err(invalid());
    }

    if !path
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/'))
    {
        return Err(invalid());
    }

    if path.starts_with('/') || path.ends_with('/') || path.contains("//") {
        return Err(invalid());
    }

    Ok(())
}

/// FFI entry point invoked by the generated `redox_mod_dispatch_network_payload` export.
///
/// # Safety
/// `data` must point to a readable buffer for the duration of the call.
pub unsafe fn ffi_dispatch_network_payload(
    channel: FfiStr,
    data: FfiSlice<u8>,
    sender_name: FfiStr,
    sender_uuid: FfiStr,
    side: u32,
) -> FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(channel) = channel.as_str() else {
            return FfiStatus::InvalidArgument;
        };
        if channel.is_empty() {
            return FfiStatus::InvalidArgument;
        }
        let Some(side) = NetworkSide::from_abi(side) else {
            return FfiStatus::InvalidArgument;
        };

        let data = data.as_slice();
        let sender_name = sender_name.as_str().unwrap_or("");
        let sender_uuid = sender_uuid.as_str().unwrap_or("");
        dispatch_received(channel, data, sender_name, sender_uuid, side);
        FfiStatus::Success
    }));

    result.unwrap_or(FfiStatus::Panic)
}

/// FFI entry point invoked by the generated `redox_mod_drain_network_outbox` export.
///
/// # Safety
/// `visitor` must be a valid function pointer and `user_data` must remain valid
/// for the duration of this call.
pub unsafe fn ffi_drain_network_outbox(
    visitor: NetworkOutboundVisitor,
    user_data: *mut c_void,
) -> FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drain_outbox(|channel, target_kind, target_name, target_uuid, data| {
            let status = visitor(
                user_data,
                FfiStr::from_str(channel),
                target_kind,
                FfiStr::from_str(target_name),
                FfiStr::from_str(target_uuid),
                FfiSlice::from_slice(data),
            );
            status.is_success()
        });
        FfiStatus::Success
    }));

    result.unwrap_or(FfiStatus::Panic)
}

/// Exports the two FFI entry points required by the native loader.
///
/// Invoke this macro exactly once in the mod crate root:
///
/// ```ignore
/// redox_mc_api::export_network_handlers!();
/// ```
///
/// The macro must live in the mod crate itself (rather than in `redox-mc-api`)
/// so the linker retains and exports the symbols from the final `cdylib`.
#[macro_export]
macro_rules! export_network_handlers {
    () => {
        /// Receives an inbound mod payload from the Redox-MC loader.
        ///
        /// # Safety
        /// All borrowed pointers must be valid for the duration of the call.
        #[no_mangle]
        pub unsafe extern "C" fn redox_mod_dispatch_network_payload(
            channel: $crate::types::FfiStr,
            data: $crate::types::FfiSlice<u8>,
            sender_name: $crate::types::FfiStr,
            sender_uuid: $crate::types::FfiStr,
            side: u32,
        ) -> $crate::types::FfiStatus {
            $crate::network::ffi_dispatch_network_payload(
                channel,
                data,
                sender_name,
                sender_uuid,
                side,
            )
        }

        /// Drains this mod's outbound network queue.
        ///
        /// # Safety
        /// `visitor` must be a valid function pointer and `user_data` must remain
        /// valid for the duration of the call.
        #[no_mangle]
        pub unsafe extern "C" fn redox_mod_drain_network_outbox(
            visitor: $crate::abi::types::NetworkOutboundVisitor,
            user_data: *mut ::core::ffi::c_void,
        ) -> $crate::types::FfiStatus {
            $crate::network::ffi_drain_network_outbox(visitor, user_data)
        }
    };
}
