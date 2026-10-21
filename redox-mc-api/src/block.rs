//! Declarative block definitions and registry for Redox-MC mods.

use std::sync::Mutex;

pub use redox_mc_abi::types::{
    BLOCK_FLAG_HAS_DESTROY as FLAG_HAS_DESTROY, BLOCK_FLAG_HAS_STEP_ON as FLAG_HAS_STEP_ON,
    BLOCK_FLAG_HAS_USE_WITHOUT_ITEM as FLAG_HAS_USE_WITHOUT_ITEM,
};

/// Declarative builder for defining custom Minecraft blocks from Rust mods.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockDefinition {
    /// Namespaced identifier of the block (e.g. "testmod:ruby_block").
    pub id: String,
    /// Destroy time / hardness of the block (default 1.5).
    pub destroy_time: f32,
    /// Explosion resistance of the block (default 6.0).
    pub explosion_resistance: f32,
    /// Whether the block requires an appropriate tool to drop items.
    pub requires_tool: bool,
    /// Whether the block has no collision box (walk-through).
    pub no_collision: bool,
    /// Whether to automatically create and register an associated `BlockItem`.
    pub has_item: bool,
    /// Whether the block defines custom interaction when right-clicked without an item.
    pub has_use_without_item: bool,
    /// Whether the block defines custom logic when an entity steps on it.
    pub has_step_on: bool,
    /// Whether the block defines custom logic when destroyed.
    pub has_destroy: bool,
    /// Texture or model path for auto-generating cube assets (e.g. "testmod:block/ruby_block").
    pub cube_model_texture: Option<String>,
    /// Custom item dropped when the block is broken (defaults to `id` if None).
    pub drop_item: Option<String>,
    /// Whether to generate block loot table.
    pub generate_loot_table: bool,
}

impl BlockDefinition {
    /// Creates a new `BlockDefinition` with sensible default solid block properties.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            destroy_time: 1.5,
            explosion_resistance: 6.0,
            requires_tool: false,
            no_collision: false,
            has_item: true,
            has_use_without_item: false,
            has_step_on: false,
            has_destroy: false,
            cube_model_texture: None,
            drop_item: None,
            generate_loot_table: true,
        }
    }

    /// Sets both destroy time (hardness) and explosion resistance to the same value.
    pub fn strength(mut self, strength: f32) -> Self {
        self.destroy_time = strength;
        self.explosion_resistance = strength;
        self
    }

    /// Sets destroy time and explosion resistance independently.
    pub fn strength_and_resistance(mut self, destroy_time: f32, explosion_resistance: f32) -> Self {
        self.destroy_time = destroy_time;
        self.explosion_resistance = explosion_resistance;
        self
    }

    /// Sets the destroy time (hardness) of the block.
    pub fn destroy_time(mut self, destroy_time: f32) -> Self {
        self.destroy_time = destroy_time;
        self
    }

    /// Sets the explosion resistance of the block.
    pub fn explosion_resistance(mut self, explosion_resistance: f32) -> Self {
        self.explosion_resistance = explosion_resistance;
        self
    }

    /// Sets whether this block requires an appropriate tool for drops.
    pub fn requires_tool(mut self, requires: bool) -> Self {
        self.requires_tool = requires;
        self
    }

    /// Sets whether this block has collision.
    pub fn no_collision(mut self, no_collision: bool) -> Self {
        self.no_collision = no_collision;
        self
    }

    /// Sets whether to automatically register an associated BlockItem.
    pub fn has_item(mut self, has_item: bool) -> Self {
        self.has_item = has_item;
        self
    }

    /// Sets whether this block defines active `useWithoutItem` logic.
    pub fn has_use_without_item(mut self, val: bool) -> Self {
        self.has_use_without_item = val;
        self
    }

    /// Sets whether this block defines active `stepOn` logic.
    pub fn has_step_on(mut self, val: bool) -> Self {
        self.has_step_on = val;
        self
    }

    /// Sets whether this block defines active `destroy` logic.
    pub fn has_destroy(mut self, val: bool) -> Self {
        self.has_destroy = val;
        self
    }

    /// Configures standard cube block models and blockstate using the specified texture.
    ///
    /// Autogenerates:
    /// - `assets/<namespace>/blockstates/<id>.json`
    /// - `assets/<namespace>/models/block/<id>.json`
    /// - `assets/<namespace>/items/<id>.json` (if `has_item` is true)
    /// - `assets/<namespace>/models/item/<id>.json` (if `has_item` is true)
    /// - `data/<namespace>/loot_table/blocks/<id>.json` (if `generate_loot_table` is true)
    pub fn cube_model(mut self, texture: impl Into<String>) -> Self {
        self.cube_model_texture = Some(texture.into());
        self
    }

    /// Configures a custom item dropped when the block is mined/broken.
    pub fn drops(mut self, drop_item: impl Into<String>) -> Self {
        self.drop_item = Some(drop_item.into());
        self
    }

    /// Disables automatic generation of the block loot table.
    pub fn no_loot_table(mut self) -> Self {
        self.generate_loot_table = false;
        self
    }

    /// Generates in-memory virtual resources for this block if a cube model texture is configured.
    pub fn generate_virtual_resources(&self) -> Vec<crate::resource::VirtualResource> {
        if let Some(texture) = &self.cube_model_texture {
            crate::assets::generate_cube_block_assets(
                &self.id,
                texture,
                self.drop_item.as_deref(),
                self.has_item,
                self.generate_loot_table,
            )
        } else {
            Vec::new()
        }
    }

    /// Returns the bitmask of active behavior flags for this block.
    pub fn behavior_flags(&self) -> u32 {
        let mut flags = 0;
        if self.has_use_without_item {
            flags |= FLAG_HAS_USE_WITHOUT_ITEM;
        }
        if self.has_step_on {
            flags |= FLAG_HAS_STEP_ON;
        }
        if self.has_destroy {
            flags |= FLAG_HAS_DESTROY;
        }
        flags
    }
}

static REGISTERED_BLOCKS: Mutex<Vec<BlockDefinition>> = Mutex::new(Vec::new());

/// Registers a custom block definition from the current mod.
///
/// If the block has a cube model configured via `.cube_model(...)`,
/// its corresponding virtual assets and data pack entries (blockstate, block model,
/// item models, and loot table) are automatically generated and registered.
pub fn register_block(block: BlockDefinition) {
    let resources = block.generate_virtual_resources();
    for res in resources {
        crate::resource::register_resource(res.pack_type, res.namespace, res.path, res.content);
    }

    let mut blocks = REGISTERED_BLOCKS.lock().unwrap();
    blocks.push(block);
}

/// Returns a clone of all blocks registered so far in this mod.
pub fn get_registered_blocks() -> Vec<BlockDefinition> {
    REGISTERED_BLOCKS.lock().unwrap().clone()
}

/// Clears the registered blocks registry (primarily for test isolation).
pub fn clear_registered_blocks() {
    REGISTERED_BLOCKS.lock().unwrap().clear();
}

/// C-ABI exported symbol allowing the loader to query the number of registered blocks.
#[no_mangle]
pub extern "C" fn redox_mod_get_block_count() -> usize {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        get_registered_blocks().len()
    }))
    .unwrap_or(0)
}

/// C-ABI exported symbol allowing the loader to retrieve a block definition at `index`.
///
/// # Safety
/// Output pointers must point to valid, writable memory.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_get_block_at(
    index: usize,
    out_id: *mut redox_mc_abi::types::FfiStr,
    out_destroy_time: *mut f32,
    out_explosion_resistance: *mut f32,
    out_requires_tool: *mut bool,
    out_no_collision: *mut bool,
    out_behavior_flags: *mut u32,
    out_has_item: *mut bool,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_id.is_null()
            || out_destroy_time.is_null()
            || out_explosion_resistance.is_null()
            || out_requires_tool.is_null()
            || out_no_collision.is_null()
            || out_behavior_flags.is_null()
            || out_has_item.is_null()
        {
            return redox_mc_abi::types::FfiStatus::NullPointer;
        }

        let blocks = REGISTERED_BLOCKS.lock().unwrap();
        if index >= blocks.len() {
            return redox_mc_abi::types::FfiStatus::NotFound;
        }

        let block = &blocks[index];
        *out_id = redox_mc_abi::types::FfiStr::from_str(block.id.as_str());
        *out_destroy_time = block.destroy_time;
        *out_explosion_resistance = block.explosion_resistance;
        *out_requires_tool = block.requires_tool;
        *out_no_collision = block.no_collision;
        *out_behavior_flags = block.behavior_flags();
        *out_has_item = block.has_item;

        redox_mc_abi::types::FfiStatus::Success
    }));

    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}
