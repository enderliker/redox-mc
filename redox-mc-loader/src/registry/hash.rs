//! Deterministic content hasher for registered mod items and blocks.
//!
//! Computes a canonical SHA-256 digest over all declared game objects in
//! topological mod order, guaranteeing identical hashes across client and server
//! during network handshake negotiation.

use sha2::{Digest, Sha256};

pub use redox_mc_abi::types::{
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

/// Computes a canonical SHA-256 hex hash from a slice of registered items.
pub fn compute_registry_hash(items: &[RegisteredItemEntry]) -> String {
    let mut hasher = RegistryHasher::new();
    for item in items {
        hasher.update_item(item);
    }
    hasher.finalize_hex()
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
