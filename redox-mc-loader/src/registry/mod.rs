//! In-game registration and deterministic content hashing.

pub mod block;
pub mod hash;
pub mod item;
pub mod resource;

pub use block::register_block_in_game;
pub use hash::{
    compute_full_registry_hash, compute_registry_hash, get_registry_hash, set_registry_hash,
    RegisteredBlockEntry, RegisteredItemEntry, RegistryHasher, BLOCK_FLAG_HAS_DESTROY,
    BLOCK_FLAG_HAS_STEP_ON, BLOCK_FLAG_HAS_USE_WITHOUT_ITEM, FLAG_HAS_FINISH_USING, FLAG_HAS_USE,
    FLAG_HAS_USE_ON,
};
pub use item::register_item_in_game;
