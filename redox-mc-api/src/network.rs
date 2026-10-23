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
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Maximum size of a single encoded mod message (1 MiB, matching clientbound limits).
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Maximum size of a serverbound mod message enforced by the vanilla client.
pub const MAX_SERVERBOUND_MESSAGE_SIZE: usize = 32767;

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

// Side on which the current thread last executed a mod callback.
thread_local! {
    static CALLBACK_SIDE: Cell<Option<NetworkSide>> = const { Cell::new(None) };
}

/// Process-wide side used when no callback context has been recorded.
static RUNNING_SIDE: AtomicU8 = AtomicU8::new(NETWORK_SIDE_SERVER as u8);

/// Returns the side that owns outbound messages queued right now.
///
/// A callback-scoped side set by the loader takes precedence over the
/// process-wide running side. This is what keeps a server-side callback in an
/// integrated server from being routed through the client transport.
#[inline]
pub fn current_side() -> NetworkSide {
    CALLBACK_SIDE.with(Cell::get).unwrap_or_else(running_side)
}

/// Returns the process-wide running side (server by default).
#[inline]
pub fn running_side() -> NetworkSide {
    if RUNNING_SIDE.load(Ordering::Acquire) == NETWORK_SIDE_CLIENT as u8 {
        NetworkSide::Client
    } else {
        NetworkSide::Server
    }
}

/// Sets the process-wide running side.
#[inline]
pub fn set_running_side(side: NetworkSide) {
    RUNNING_SIDE.store(side.as_abi() as u8, Ordering::Release);
}

/// Sets (or clears) the side associated with the current callback thread.
#[inline]
pub fn set_callback_side(side: Option<NetworkSide>) {
    CALLBACK_SIDE.with(|current| current.set(side));
}

/// Identity of the remote party that sent a network message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SenderId {
    name: String,
    uuid: String,
    server: bool,
}

impl SenderId {
    /// Creates a player sender identity from a name and UUID string.
    #[inline]
    pub fn new(name: impl Into<String>, uuid: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uuid: uuid.into(),
            server: false,
        }
    }

    /// Creates the synthetic identity used by the client for inbound server messages.
    #[inline]
    pub fn server() -> Self {
        Self {
            name: "server".to_string(),
            uuid: String::new(),
            server: true,
        }
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
        self.server
    }

    /// Converts this identity into a targetable [`PlayerId`], if it represents a player.
    #[inline]
    pub fn player_id(&self) -> Option<PlayerId> {
        if self.server || (self.name.is_empty() && self.uuid.is_empty()) {
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
    /// On the server this targets the originating player. On the client it
    /// targets the remote server. A server callback whose sender is not a
    /// targetable player returns [`NetworkError::NoReplyTarget`] instead of
    /// broadcasting to every connected player.
    pub fn reply<T: NetworkMessage>(
        &self,
        channel: &NetworkChannel<T>,
        message: &T,
    ) -> Result<(), NetworkError> {
        match self.side {
            NetworkSide::Server => match self.player() {
                Some(player) => channel.send_to_player(&player, message),
                None => Err(NetworkError::NoReplyTarget),
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
    /// A reply was attempted from a context whose sender is not a player.
    NoReplyTarget,
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
            Self::NoReplyTarget => write!(
                f,
                "cannot reply to a sender that is not a targetable player"
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
    /// The message is tagged with the current callback side, falling back to the
    /// process-wide running side. On the server this broadcasts to every
    /// connected player; on the client it sends to the remote server.
    pub fn send(&self, message: &T) -> Result<(), NetworkError> {
        self.enqueue(message, Destination::Broadcast, current_side())
    }

    /// Queues a message addressed to the remote server. Intended for client-side mods.
    pub fn send_to_server(&self, message: &T) -> Result<(), NetworkError> {
        self.enqueue(message, Destination::Server, NetworkSide::Client)
    }

    /// Queues a message addressed to a specific player. Intended for server-side mods.
    pub fn send_to_player(&self, player: &PlayerId, message: &T) -> Result<(), NetworkError> {
        self.enqueue(
            message,
            Destination::Player {
                name: player.name.clone(),
                uuid: player.uuid.clone(),
            },
            NetworkSide::Server,
        )
    }

    /// Replies on this channel to the sender carried by `context`.
    pub fn reply(&self, context: &NetworkContext, message: &T) -> Result<(), NetworkError> {
        context.reply(self, message)
    }

    /// Sends raw bytes without invoking [`NetworkMessage::encode`].
    pub fn send_raw(&self, data: &[u8]) -> Result<(), NetworkError> {
        self.enqueue_encoded(data.to_vec(), Destination::Broadcast, current_side())
    }

    fn enqueue(
        &self,
        message: &T,
        destination: Destination,
        side: NetworkSide,
    ) -> Result<(), NetworkError> {
        let data = message.encode()?;
        self.enqueue_encoded(data, destination, side)
    }

    fn enqueue_encoded(
        &self,
        data: Vec<u8>,
        destination: Destination,
        side: NetworkSide,
    ) -> Result<(), NetworkError> {
        // The destination fixes the transport for addressed messages, regardless
        // of which thread happened to call the API.
        let side = match destination {
            Destination::Server => NetworkSide::Client,
            Destination::Player { .. } => NetworkSide::Server,
            Destination::Broadcast => side,
        };
        let max_size = if side == NetworkSide::Client {
            MAX_SERVERBOUND_MESSAGE_SIZE
        } else {
            MAX_MESSAGE_SIZE
        };
        if data.len() > max_size {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: max_size,
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
            side,
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
    side: NetworkSide,
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

/// Returns the number of messages queued for a specific side.
#[inline]
pub fn outbox_len_for_side(side: NetworkSide) -> usize {
    outbox()
        .lock()
        .unwrap()
        .queue
        .iter()
        .filter(|message| message.side == side)
        .count()
}

/// Drops every queued outbound message. Primarily useful in tests and on shutdown.
pub fn clear_outbox() {
    let mut state = outbox().lock().unwrap();
    state.queue.clear();
    state.bytes = 0;
}

/// Drains every queued message, regardless of side, invoking `visitor` once per
/// message. Intended for tests and diagnostics; the loader uses
/// [`drain_outbox_for_side`].
pub fn drain_outbox<F>(visitor: F)
where
    F: FnMut(&str, u32, &str, &str, &[u8]) -> bool,
{
    drain_outbox_inner(None, visitor);
}

/// Drains only the messages queued for `side`, leaving the other side's queue
/// untouched.
///
/// The visitor receives `(channel, target_kind, target_name, target_uuid, data)`.
/// Returning `false` marks the message as undeliverable; it is logged and dropped
/// so a permanently unavailable target cannot block the rest of the queue.
pub fn drain_outbox_for_side<F>(side: NetworkSide, visitor: F)
where
    F: FnMut(&str, u32, &str, &str, &[u8]) -> bool,
{
    drain_outbox_inner(Some(side), visitor);
}

fn drain_outbox_inner<F>(side: Option<NetworkSide>, mut visitor: F)
where
    F: FnMut(&str, u32, &str, &str, &[u8]) -> bool,
{
    let messages = {
        let mut state = outbox().lock().unwrap();
        if side.is_none() {
            state.bytes = 0;
            std::mem::take(&mut state.queue).into_iter().collect()
        } else {
            let mut selected = Vec::new();
            let mut remaining = VecDeque::with_capacity(state.queue.len());
            while let Some(message) = state.queue.pop_front() {
                if Some(message.side) == side {
                    state.bytes -= message.data.len();
                    selected.push(message);
                } else {
                    remaining.push_back(message);
                }
            }
            state.queue = remaining;
            selected
        }
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

    let sender = if side == NetworkSide::Client && sender_name == "server" && sender_uuid.is_empty()
    {
        SenderId::server()
    } else {
        SenderId::new(sender_name, sender_uuid)
    };

    let context = NetworkContext {
        channel: channel.to_string(),
        sender,
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

/// FFI entry point invoked by the generated `redox_mod_set_running_side` export.
///
/// # Safety
/// The loader must pass a valid `NETWORK_SIDE_*` value.
pub unsafe fn ffi_set_running_side(side: u32) -> FfiStatus {
    match NetworkSide::from_abi(side) {
        Some(side) => {
            set_running_side(side);
            FfiStatus::Success
        }
        None => FfiStatus::InvalidArgument,
    }
}

/// FFI entry point invoked by the generated `redox_mod_set_network_side` export.
///
/// # Safety
/// The loader must pass a valid `NETWORK_SIDE_*` value.
pub unsafe fn ffi_set_network_side(side: u32) -> FfiStatus {
    match NetworkSide::from_abi(side) {
        Some(side) => {
            set_callback_side(Some(side));
            FfiStatus::Success
        }
        None => FfiStatus::InvalidArgument,
    }
}

/// FFI entry point invoked by the generated `redox_mod_drain_network_outbox` export.
///
/// # Safety
/// `visitor` must be a valid function pointer and `user_data` must remain valid
/// for the duration of this call.
pub unsafe fn ffi_drain_network_outbox(
    side: u32,
    visitor: NetworkOutboundVisitor,
    user_data: *mut c_void,
) -> FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(side) = NetworkSide::from_abi(side) else {
            return FfiStatus::InvalidArgument;
        };
        drain_outbox_for_side(
            side,
            |channel, target_kind, target_name, target_uuid, data| {
                let status = visitor(
                    user_data,
                    FfiStr::from_str(channel),
                    target_kind,
                    FfiStr::from_str(target_name),
                    FfiStr::from_str(target_uuid),
                    FfiSlice::from_slice(data),
                );
                status.is_success()
            },
        );
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

        /// Drains this mod's outbound network queue for one side.
        ///
        /// # Safety
        /// `visitor` must be a valid function pointer and `user_data` must remain
        /// valid for the duration of the call.
        #[no_mangle]
        pub unsafe extern "C" fn redox_mod_drain_network_outbox(
            side: u32,
            visitor: $crate::abi::types::NetworkOutboundVisitor,
            user_data: *mut ::core::ffi::c_void,
        ) -> $crate::types::FfiStatus {
            $crate::network::ffi_drain_network_outbox(side, visitor, user_data)
        }

        /// Sets the process-wide side used for outbound messages queued outside
        /// of a callback.
        ///
        /// # Safety
        /// `side` must be one of the `NETWORK_SIDE_*` constants.
        #[no_mangle]
        pub unsafe extern "C" fn redox_mod_set_running_side(side: u32) -> $crate::types::FfiStatus {
            $crate::network::ffi_set_running_side(side)
        }

        /// Sets the side associated with the current callback thread.
        ///
        /// # Safety
        /// `side` must be one of the `NETWORK_SIDE_*` constants.
        #[no_mangle]
        pub unsafe extern "C" fn redox_mod_set_network_side(side: u32) -> $crate::types::FfiStatus {
            $crate::network::ffi_set_network_side(side)
        }
    };
}
