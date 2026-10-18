//! VTables and Context structures for dynamic loader-mod communication.

use crate::types::{
    AbiVersion, FfiBuffer, FfiLogLevel, FfiModSide, FfiResult, FfiStatus, FfiStr, FfiString,
};

/// Context provided by the Redox loader to each mod during initialization.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LoaderContext {
    /// ABI specification version of the running loader.
    pub abi_version: AbiVersion,

    /// Loader semver major version.
    pub loader_version_major: u32,
    /// Loader semver minor version.
    pub loader_version_minor: u32,
    /// Loader semver patch version.
    pub loader_version_patch: u32,

    /// Current environment side (Client, Server, Both).
    pub side: FfiModSide,

    /// Reserved for alignment and future extension (7 bytes to align vtable to 8-byte boundary).
    pub _reserved: [u8; 7],

    /// Pointer to the loader's function vtable.
    pub vtable: *const LoaderVTable,

    /// Opaque pointer to loader private state (do not dereference in mod).
    pub loader_handle: *mut (),
}

/// Function table exposed by the Redox loader to mods.
#[repr(C)]
pub struct LoaderVTable {
    /// Dispatches a structured log message to the game / loader logger.
    pub log: unsafe extern "C" fn(level: FfiLogLevel, target: FfiStr, msg: FfiStr),

    /// Registers a callback for a specific event ID.
    pub register_event_listener: unsafe extern "C" fn(
        event_id: u32,
        callback: unsafe extern "C" fn(*const ()),
        user_data: *mut (),
    ) -> FfiStatus,

    /// Frees an `FfiString` that was allocated by the loader.
    pub free_string: unsafe extern "C" fn(s: FfiString),

    /// Frees an `FfiBuffer` that was allocated by the loader.
    pub free_buffer: unsafe extern "C" fn(b: FfiBuffer),
}

/// Function table returned by a mod to the Redox loader.
#[repr(C)]
pub struct ModVTable {
    /// Called when Minecraft starts bootstrapping vanilla registries (`Bootstrap.bootStrap()`).
    pub on_bootstrap: Option<unsafe extern "C" fn() -> FfiStatus>,

    /// Called right before `BuiltInRegistries.freeze()` is invoked.
    pub on_register_content: Option<unsafe extern "C" fn() -> FfiStatus>,

    /// Called every server tick loop (`MinecraftServer.tickChildren`).
    pub on_server_tick: Option<unsafe extern "C" fn(tick_count: i64) -> FfiStatus>,

    /// Called on player chat (`ServerGamePacketListenerImpl.handleChat`).
    /// Returns FfiResult::ok(true) to allow delivery, or FfiResult::ok(false) to cancel/drop.
    pub on_server_chat:
        Option<unsafe extern "C" fn(player_name: FfiStr, message: FfiStr) -> FfiResult<bool>>,

    /// Frees an `FfiString` that was allocated by this mod.
    pub free_string: unsafe extern "C" fn(s: FfiString),

    /// Frees an `FfiBuffer` that was allocated by this mod.
    pub free_buffer: unsafe extern "C" fn(b: FfiBuffer),
}

/// Mod initialization function signature exported by mod binaries.
///
/// Mod cdylib symbol: `redox_mod_init_v1` (or wrapped by `redox_mod_init`).
pub type ModInitFn =
    unsafe extern "C" fn(context: *const LoaderContext) -> FfiResult<*const ModVTable>;
