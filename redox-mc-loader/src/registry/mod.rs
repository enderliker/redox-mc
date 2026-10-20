//! In-game registration and deterministic content hashing.

pub mod hash;
pub mod item;

pub use hash::{
    compute_registry_hash, get_registry_hash, set_registry_hash, RegisteredItemEntry,
    RegistryHasher, FLAG_HAS_FINISH_USING, FLAG_HAS_USE, FLAG_HAS_USE_ON,
};
pub use item::register_item_in_game;
