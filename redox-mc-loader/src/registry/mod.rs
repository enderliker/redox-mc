//! In-game registration and deterministic content hashing.

pub mod block;
pub mod block_entity;
pub mod hash;
pub mod item;
pub mod menu;
pub mod resource;

pub use block::register_block_in_game;
pub use block_entity::register_block_entity_in_game;
pub use hash::{
    clear_registry_hash, compute_full_registry_hash, compute_full_registry_hash_all,
    compute_full_registry_hash_with_entities, compute_registry_content_hash,
    compute_registry_content_hash_all, compute_registry_content_hash_full, compute_registry_hash,
    get_registry_hash, set_registry_hash, RegisteredBlockEntityEntry, RegisteredBlockEntry,
    RegisteredItemEntry, RegistryHashBuilder, RegistryHasher, BLOCK_FLAG_HAS_DESTROY,
    BLOCK_FLAG_HAS_STEP_ON, BLOCK_FLAG_HAS_USE_WITHOUT_ITEM, FLAG_HAS_FINISH_USING, FLAG_HAS_USE,
    FLAG_HAS_USE_ON,
};
pub use item::register_item_in_game;
pub use menu::{register_menu_in_game, RegisteredMenuEntry};
