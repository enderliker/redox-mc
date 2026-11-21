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
    /// Optional block entity attached to this block.
    pub block_entity: Option<crate::block_entity::BlockEntityDefinition>,
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
            block_entity: None,
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

    /// Associates a block entity with this block.
    pub fn with_block_entity(mut self, be: crate::block_entity::BlockEntityDefinition) -> Self {
        self.block_entity = Some(be);
        self
    }

    /// Associates a block entity with this block.
    pub fn block_entity(mut self, be: crate::block_entity::BlockEntityDefinition) -> Self {
        self.block_entity = Some(be);
        self
    }

    /// Attaches a container block entity with custom slot count and menu identifier.
    pub fn with_container_block_entity(
        mut self,
        be_id: impl Into<String>,
        slots: u32,
        menu_id: impl Into<String>,
    ) -> Self {
        let be_id_str = be_id.into();
        let menu_id_str = menu_id.into();
        self.block_entity = Some(
            crate::block_entity::BlockEntityDefinition::new(be_id_str)
                .with_container(slots, menu_id_str),
        );
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

        let blocks = REGISTERED_BLOCKS.lock().unwrap_or_else(|e| e.into_inner());
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

/// C-ABI exported symbol allowing the loader to query the block entity associated with a block at `index`.
///
/// # Safety
/// Output pointers must point to valid, writable memory.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_get_block_entity_info_at(
    index: usize,
    out_has_entity: *mut bool,
    out_id: *mut redox_mc_abi::types::FfiStr,
    out_flags: *mut u32,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_has_entity.is_null() || out_id.is_null() || out_flags.is_null() {
            return redox_mc_abi::types::FfiStatus::NullPointer;
        }

        let blocks = REGISTERED_BLOCKS.lock().unwrap_or_else(|e| e.into_inner());
        if index >= blocks.len() {
            return redox_mc_abi::types::FfiStatus::NotFound;
        }

        let block = &blocks[index];
        if let Some(be) = &block.block_entity {
            *out_has_entity = true;
            *out_id = redox_mc_abi::types::FfiStr::from_str(be.id.as_str());
            *out_flags = be.behavior_flags();
        } else {
            *out_has_entity = false;
            *out_id = redox_mc_abi::types::FfiStr::EMPTY;
            *out_flags = 0;
        }

        redox_mc_abi::types::FfiStatus::Success
    }));

    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}

/// C-ABI exported symbol allowing the loader to query container info for the block entity at `index`.
///
/// # Safety
/// Output pointers must point to valid, writable memory.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_get_block_entity_container_info_at(
    index: usize,
    out_has_container: *mut bool,
    out_slots: *mut u32,
    out_menu_id: *mut redox_mc_abi::types::FfiStr,
    out_title: *mut redox_mc_abi::types::FfiStr,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_has_container.is_null()
            || out_slots.is_null()
            || out_menu_id.is_null()
            || out_title.is_null()
        {
            return redox_mc_abi::types::FfiStatus::NullPointer;
        }

        let blocks = REGISTERED_BLOCKS.lock().unwrap_or_else(|e| e.into_inner());
        if index >= blocks.len() {
            return redox_mc_abi::types::FfiStatus::NotFound;
        }

        let block = &blocks[index];
        if let Some(be) = &block.block_entity {
            *out_has_container = be.has_container;
            *out_slots = be.slot_count;
            if let Some(m) = &be.menu_id {
                *out_menu_id = redox_mc_abi::types::FfiStr::from_str(m.as_str());
            } else {
                *out_menu_id = redox_mc_abi::types::FfiStr::EMPTY;
            }
            if let Some(t) = &be.title {
                *out_title = redox_mc_abi::types::FfiStr::from_str(t.as_str());
            } else {
                *out_title = redox_mc_abi::types::FfiStr::EMPTY;
            }
        } else {
            *out_has_container = false;
            *out_slots = 0;
            *out_menu_id = redox_mc_abi::types::FfiStr::EMPTY;
            *out_title = redox_mc_abi::types::FfiStr::EMPTY;
        }

        redox_mc_abi::types::FfiStatus::Success
    }));

    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}

/// Fallback C-ABI exported symbol for block entity tick if mod does not provide a vtable entry.
///
/// # Safety
/// Invoked by loader when a block entity ticks.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_on_block_entity_tick(
    id: redox_mc_abi::types::FfiStr,
    x: i32,
    y: i32,
    z: i32,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let id_str = match id.as_str() {
            Ok(s) => s,
            Err(_) => return redox_mc_abi::types::FfiStatus::InvalidArgument,
        };
        crate::block_entity::dispatch_tick(id_str, x, y, z);
        redox_mc_abi::types::FfiStatus::Success
    }));
    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}

/// Fallback C-ABI exported symbol for block entity save if mod does not provide a vtable entry.
///
/// # Safety
/// Invoked by loader when a block entity saves state.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_on_block_entity_save(
    id: redox_mc_abi::types::FfiStr,
    x: i32,
    y: i32,
    z: i32,
    out_buf: *mut redox_mc_abi::types::FfiBuffer,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_buf.is_null() {
            return redox_mc_abi::types::FfiStatus::NullPointer;
        }
        *out_buf = redox_mc_abi::types::FfiBuffer::NULL;
        let id_str = match id.as_str() {
            Ok(s) => s,
            Err(_) => return redox_mc_abi::types::FfiStatus::InvalidArgument,
        };
        if let Some(data) = crate::block_entity::dispatch_save(id_str, x, y, z) {
            *out_buf = redox_mc_abi::types::FfiBuffer::from_vec(data.into_bytes());
        }
        redox_mc_abi::types::FfiStatus::Success
    }));
    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}

/// Fallback C-ABI exported symbol for block entity load if mod does not provide a vtable entry.
///
/// # Safety
/// Invoked by loader when a block entity loads state.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_on_block_entity_load(
    id: redox_mc_abi::types::FfiStr,
    x: i32,
    y: i32,
    z: i32,
    data: redox_mc_abi::types::FfiStr,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let id_str = match id.as_str() {
            Ok(s) => s,
            Err(_) => return redox_mc_abi::types::FfiStatus::InvalidArgument,
        };
        let data_str = match data.as_str() {
            Ok(s) => s,
            Err(_) => return redox_mc_abi::types::FfiStatus::InvalidArgument,
        };
        crate::block_entity::dispatch_load(id_str, x, y, z, data_str);
        redox_mc_abi::types::FfiStatus::Success
    }));
    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}

/// C-ABI exported symbol to free buffers allocated by this mod.
///
/// # Safety
/// `b` must have been allocated in this mod's heap.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_free_buffer(b: redox_mc_abi::types::FfiBuffer) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        redox_mc_abi::memory::redox_mod_free_buffer(b);
    }));
}
