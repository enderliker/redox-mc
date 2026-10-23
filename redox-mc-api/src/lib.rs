//! Public API for developing native Redox-MC mods in Rust.

pub mod assets;
pub mod block;
pub mod event;
pub mod item;
pub mod macros;
pub mod network;
pub mod resource;

pub use assets::{
    generate_block_loot_table, generate_cube_block_assets, generate_cube_block_model,
    generate_cube_blockstate, generate_item_model_definition, generate_legacy_item_model,
    split_identifier,
};

pub use block::{
    clear_registered_blocks, get_registered_blocks, register_block, BlockDefinition,
    FLAG_HAS_DESTROY, FLAG_HAS_STEP_ON, FLAG_HAS_USE_WITHOUT_ITEM,
};
pub use event::{CancellableEvent, Event, EventBus, EventPriority, EventResult};
pub use item::{
    clear_registered_items, get_registered_items, register_item, ItemDefinition,
    FLAG_HAS_FINISH_USING, FLAG_HAS_USE, FLAG_HAS_USE_ON,
};
pub use network::{
    clear_outbox, current_side, dispatch_received, drain_outbox, drain_outbox_for_side, outbox_len,
    outbox_len_for_side, running_side, set_callback_side, set_running_side, NetworkChannel,
    NetworkContext, NetworkError, NetworkMessage, NetworkSide, PlayerId, SenderId,
    MAX_MESSAGE_SIZE, MAX_SERVERBOUND_MESSAGE_SIZE,
};
pub use redox_mc_abi as abi;
pub use redox_mc_abi::types;
pub use redox_mc_api_macros::redox_export;
pub use resource::{
    clear_registered_resources, get_registered_resources, register_asset_resource,
    register_data_resource, register_resource, VirtualResource,
};
