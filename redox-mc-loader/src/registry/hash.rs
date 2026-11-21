//! Deterministic content hasher for registered mod items and blocks.
//!
//! Computes a canonical SHA-256 digest over all declared game objects in
//! topological mod order, guaranteeing identical hashes across client and server
//! during network handshake negotiation.

use super::menu::RegisteredMenuEntry;
use sha2::{Digest, Sha256};

pub use redox_mc_abi::types::{
    BLOCK_FLAG_HAS_DESTROY, BLOCK_FLAG_HAS_STEP_ON, BLOCK_FLAG_HAS_USE_WITHOUT_ITEM,
    ITEM_FLAG_HAS_FINISH_USING as FLAG_HAS_FINISH_USING, ITEM_FLAG_HAS_USE as FLAG_HAS_USE,
    ITEM_FLAG_HAS_USE_ON as FLAG_HAS_USE_ON,
};

/// Canonical representation of a registered item for content hashing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredItemEntry {
    /// Namespaced identifier of the item (e.g. "testmod:ruby").
    pub id: String,
    /// Maximum stack size.
    pub max_stack_size: u32,
    /// Maximum durability, or `None` if non-damageable.
    pub durability: Option<u32>,
    /// Whether the item is fire/lava resistant.
    pub fire_resistant: bool,
    /// Bitmask of active behavior callbacks.
    pub behavior_flags: u32,
}

impl RegisteredItemEntry {
    /// Creates a new `RegisteredItemEntry`.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_stack_size: 64,
            durability: None,
            fire_resistant: false,
            behavior_flags: 0,
        }
    }
}

/// Canonical representation of a registered block entity for in-game registration and content hashing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredBlockEntityEntry {
    /// Namespaced identifier of the block entity (e.g. "testmod:counter_be").
    pub id: String,
    /// Valid block identifiers that can hold this block entity.
    pub valid_blocks: Vec<String>,
    /// Bitmask of active behavior callbacks.
    pub behavior_flags: u32,
}

impl RegisteredBlockEntityEntry {
    /// Creates a new `RegisteredBlockEntityEntry`.
    pub fn new(id: impl Into<String>, valid_blocks: Vec<String>, behavior_flags: u32) -> Self {
        Self {
            id: id.into(),
            valid_blocks,
            behavior_flags,
        }
    }
}

/// Canonical representation of a registered block for in-game registration and content hashing.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredBlockEntry {
    /// Namespaced identifier of the block (e.g. "testmod:ruby_block").
    pub id: String,
    /// Destroy time / hardness of the block.
    pub destroy_time: f32,
    /// Explosion resistance of the block.
    pub explosion_resistance: f32,
    /// Whether the block requires an appropriate tool.
    pub requires_tool: bool,
    /// Whether the block has no collision box.
    pub no_collision: bool,
    /// Bitmask of active behavior callbacks.
    pub behavior_flags: u32,
    /// Whether to register an associated BlockItem.
    pub has_item: bool,
    /// Associated block entity, if declared.
    pub block_entity: Option<RegisteredBlockEntityEntry>,
}

impl RegisteredBlockEntry {
    /// Creates a new `RegisteredBlockEntry` with default properties.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            destroy_time: 1.5,
            explosion_resistance: 6.0,
            requires_tool: false,
            no_collision: false,
            behavior_flags: 0,
            has_item: true,
            block_entity: None,
        }
    }
}

/// Accumulative SHA-256 hasher for registry content.
#[derive(Default)]
pub struct RegistryHasher {
    hasher: Sha256,
}

impl RegistryHasher {
    /// Creates a new `RegistryHasher`.
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }

    /// Feeds an item's identity and properties into the hash digest.
    pub fn update_item(&mut self, item: &RegisteredItemEntry) {
        // Feed in deterministic binary representation:
        // 1. ID length + ID bytes
        self.hasher.update((item.id.len() as u32).to_le_bytes());
        self.hasher.update(item.id.as_bytes());
        // 2. Max stack size
        self.hasher.update(item.max_stack_size.to_le_bytes());
        // 3. Durability (0 if None)
        self.hasher
            .update(item.durability.unwrap_or(0).to_le_bytes());
        // 4. Fire resistance boolean
        self.hasher.update([item.fire_resistant as u8]);
        // 5. Active behavior flags
        self.hasher.update(item.behavior_flags.to_le_bytes());
    }

    /// Feeds a block's identity and properties into the hash digest.
    pub fn update_block(&mut self, block: &RegisteredBlockEntry) {
        // Feed in deterministic binary representation:
        // 1. ID length + ID bytes
        self.hasher.update((block.id.len() as u32).to_le_bytes());
        self.hasher.update(block.id.as_bytes());
        // 2. Destroy time / hardness
        self.hasher.update(block.destroy_time.to_le_bytes());
        // 3. Explosion resistance
        self.hasher.update(block.explosion_resistance.to_le_bytes());
        // 4. Requires tool boolean
        self.hasher.update([block.requires_tool as u8]);
        // 5. No collision boolean
        self.hasher.update([block.no_collision as u8]);
        // 6. Active behavior flags
        self.hasher.update(block.behavior_flags.to_le_bytes());
        // 7. Has item boolean
        self.hasher.update([block.has_item as u8]);
        // 8. Block entity (if attached)
        if let Some(be) = &block.block_entity {
            self.hasher.update([1u8]);
            self.update_block_entity(be);
        } else {
            self.hasher.update([0u8]);
        }
    }

    /// Feeds a block entity's identity and properties into the hash digest.
    pub fn update_block_entity(&mut self, be: &RegisteredBlockEntityEntry) {
        // Feed in deterministic binary representation:
        // 1. ID length + ID bytes
        self.hasher.update((be.id.len() as u32).to_le_bytes());
        self.hasher.update(be.id.as_bytes());
        // 2. Sorted valid block count + IDs
        let mut sorted_valid_blocks = be.valid_blocks.clone();
        sorted_valid_blocks.sort();
        self.hasher
            .update((sorted_valid_blocks.len() as u32).to_le_bytes());
        for b in &sorted_valid_blocks {
            self.hasher.update((b.len() as u32).to_le_bytes());
            self.hasher.update(b.as_bytes());
        }
        // 3. Active behavior flags
        self.hasher.update(be.behavior_flags.to_le_bytes());
    }

    /// Feeds a menu's identity and properties into the hash digest.
    pub fn update_menu(&mut self, menu: &RegisteredMenuEntry) {
        // Feed in deterministic binary representation:
        // 1. Tag "menu"
        self.hasher.update(b"menu");
        // 2. ID length + ID bytes
        self.hasher.update((menu.id.len() as u32).to_le_bytes());
        self.hasher.update(menu.id.as_bytes());
        // 3. Slot count
        self.hasher.update((menu.slots as u32).to_le_bytes());
    }

    /// Finalizes the hash and returns the raw 32-byte SHA-256 digest.
    pub fn finalize(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }

    /// Finalizes the hash and returns a 64-character lowercase hex string.
    pub fn finalize_hex(self) -> String {
        let hash = self.finalize();
        let mut hex = String::with_capacity(64);
        for byte in hash {
            use std::fmt::Write;
            write!(&mut hex, "{:02x}", byte).unwrap();
        }
        hex
    }
}

/// Fluent builder for constructing and computing registry content hashes.
#[derive(Default, Clone)]
pub struct RegistryHashBuilder {
    items: Vec<RegisteredItemEntry>,
    blocks: Vec<RegisteredBlockEntry>,
    block_entities: Vec<RegisteredBlockEntityEntry>,
    menus: Vec<RegisteredMenuEntry>,
}

impl RegistryHashBuilder {
    /// Creates a new `RegistryHashBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an item to the hash computation.
    pub fn add_item(&mut self, id: impl Into<String>, max_stack_size: u32) -> &mut Self {
        let mut item = RegisteredItemEntry::new(id);
        item.max_stack_size = max_stack_size;
        self.items.push(item);
        self
    }

    /// Adds a block to the hash computation.
    pub fn add_block(&mut self, block: RegisteredBlockEntry) -> &mut Self {
        self.blocks.push(block);
        self
    }

    /// Adds a block entity to the hash computation.
    pub fn add_block_entity(
        &mut self,
        id: impl Into<String>,
        valid_blocks: &[&str],
        behavior_flags: u32,
    ) -> &mut Self {
        self.block_entities.push(RegisteredBlockEntityEntry {
            id: id.into(),
            valid_blocks: valid_blocks.iter().map(|s| s.to_string()).collect(),
            behavior_flags,
        });
        self
    }

    /// Adds a container menu to the hash computation.
    pub fn add_menu(
        &mut self,
        id: impl Into<String>,
        slots: usize,
        title: impl Into<String>,
    ) -> &mut Self {
        self.menus.push(RegisteredMenuEntry::new(id, slots, title));
        self
    }

    /// Computes the final SHA-256 hex digest.
    pub fn compute_hex(&self) -> String {
        compute_full_registry_hash_all(&self.items, &self.blocks, &self.block_entities, &self.menus)
    }
}

/// Computes a canonical SHA-256 hex hash from a slice of registered items.
pub fn compute_registry_hash(items: &[RegisteredItemEntry]) -> String {
    compute_full_registry_hash(items, &[])
}

/// Computes a canonical SHA-256 hex hash from slices of registered items, blocks, block entities, and menus.
pub fn compute_full_registry_hash_all(
    items: &[RegisteredItemEntry],
    blocks: &[RegisteredBlockEntry],
    block_entities: &[RegisteredBlockEntityEntry],
    menus: &[RegisteredMenuEntry],
) -> String {
    let mut sorted_entities: Vec<_> = block_entities.to_vec();
    sorted_entities.sort_by(|a, b| a.id.cmp(&b.id));

    let mut sorted_menus: Vec<_> = menus.to_vec();
    sorted_menus.sort_by(|a, b| a.id.cmp(&b.id));

    let mut hasher = RegistryHasher::new();
    for item in items {
        hasher.update_item(item);
    }
    for block in blocks {
        hasher.update_block(block);
    }
    for be in &sorted_entities {
        hasher.update_block_entity(be);
    }
    for menu in &sorted_menus {
        hasher.update_menu(menu);
    }
    hasher.finalize_hex()
}

/// Computes a canonical SHA-256 hex hash from slices of registered items, blocks, and block entities.
pub fn compute_full_registry_hash_with_entities(
    items: &[RegisteredItemEntry],
    blocks: &[RegisteredBlockEntry],
    block_entities: &[RegisteredBlockEntityEntry],
) -> String {
    compute_full_registry_hash_all(items, blocks, block_entities, &[])
}

/// Computes a canonical SHA-256 hex hash from slices of registered items and blocks.
pub fn compute_full_registry_hash(
    items: &[RegisteredItemEntry],
    blocks: &[RegisteredBlockEntry],
) -> String {
    compute_full_registry_hash_with_entities(items, blocks, &[])
}

/// Computes the registry content hash, returning `None` when no custom content
/// was registered.
pub fn compute_registry_content_hash_all(
    items: &[RegisteredItemEntry],
    blocks: &[RegisteredBlockEntry],
    block_entities: &[RegisteredBlockEntityEntry],
    menus: &[RegisteredMenuEntry],
) -> Option<String> {
    if items.is_empty() && blocks.is_empty() && block_entities.is_empty() && menus.is_empty() {
        None
    } else {
        Some(compute_full_registry_hash_all(
            items,
            blocks,
            block_entities,
            menus,
        ))
    }
}

/// Computes the registry content hash, returning `None` when no custom content
/// was registered.
pub fn compute_registry_content_hash_full(
    items: &[RegisteredItemEntry],
    blocks: &[RegisteredBlockEntry],
    block_entities: &[RegisteredBlockEntityEntry],
) -> Option<String> {
    compute_registry_content_hash_all(items, blocks, block_entities, &[])
}

/// Computes the registry content hash, returning `None` when no custom content
/// was registered.
pub fn compute_registry_content_hash(
    items: &[RegisteredItemEntry],
    blocks: &[RegisteredBlockEntry],
) -> Option<String> {
    compute_registry_content_hash_full(items, blocks, &[])
}

static REGISTRY_HASH: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Returns the computed registry content hash, if finalized.
pub fn get_registry_hash() -> Option<String> {
    REGISTRY_HASH.lock().unwrap().clone()
}

/// Sets the finalized registry content hash.
pub fn set_registry_hash(hash: String) {
    *REGISTRY_HASH.lock().unwrap() = Some(hash);
}

/// Clears the stored registry content hash (used when no custom content exists).
pub fn clear_registry_hash() {
    *REGISTRY_HASH.lock().unwrap() = None;
}
