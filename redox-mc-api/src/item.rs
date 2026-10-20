//! Declarative item definitions and registry for Redox-MC mods.

use std::sync::Mutex;

pub use redox_mc_abi::types::{
    ITEM_FLAG_HAS_FINISH_USING as FLAG_HAS_FINISH_USING, ITEM_FLAG_HAS_USE as FLAG_HAS_USE,
    ITEM_FLAG_HAS_USE_ON as FLAG_HAS_USE_ON,
};

/// Declarative builder for defining custom Minecraft items from Rust mods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDefinition {
    /// Namespaced identifier of the item (e.g. "testmod:ruby").
    pub id: String,
    /// Maximum stack size for this item (default 64, or 1 if damageable).
    pub max_stack_size: u32,
    /// Maximum durability for this item, or `None` if not damageable.
    pub durability: Option<u32>,
    /// Whether the item is fire/lava resistant.
    pub fire_resistant: bool,
    /// Whether the item defines custom right-click in air logic.
    pub has_use: bool,
    /// Whether the item defines custom right-click on block logic.
    pub has_use_on: bool,
    /// Whether the item defines custom finish using logic.
    pub has_finish_using: bool,
}

impl ItemDefinition {
    /// Creates a new `ItemDefinition` with default properties (stack size 64, non-damageable, passive).
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_stack_size: 64,
            durability: None,
            fire_resistant: false,
            has_use: false,
            has_use_on: false,
            has_finish_using: false,
        }
    }

    /// Sets the maximum stack size for this item.
    pub fn max_stack_size(mut self, size: u32) -> Self {
        self.max_stack_size = size;
        self
    }

    /// Sets the maximum durability for this item (also defaults max stack size to 1 if not overridden).
    pub fn durability(mut self, durability: u32) -> Self {
        self.durability = Some(durability);
        if self.max_stack_size == 64 {
            self.max_stack_size = 1;
        }
        self
    }

    /// Sets whether this item is immune to fire and lava damage.
    pub fn fire_resistant(mut self, fire_resistant: bool) -> Self {
        self.fire_resistant = fire_resistant;
        self
    }

    /// Sets whether this item should trigger active right-click in air (`use`) callbacks.
    pub fn has_use(mut self, has_use: bool) -> Self {
        self.has_use = has_use;
        self
    }

    /// Sets whether this item should trigger active right-click on block (`useOn`) callbacks.
    pub fn has_use_on(mut self, has_use_on: bool) -> Self {
        self.has_use_on = has_use_on;
        self
    }

    /// Sets whether this item should trigger active finish using callbacks.
    pub fn has_finish_using(mut self, has_finish_using: bool) -> Self {
        self.has_finish_using = has_finish_using;
        self
    }

    /// Returns the bitmask of active behavior flags for this item.
    pub fn behavior_flags(&self) -> u32 {
        let mut flags = 0;
        if self.has_use {
            flags |= FLAG_HAS_USE;
        }
        if self.has_use_on {
            flags |= FLAG_HAS_USE_ON;
        }
        if self.has_finish_using {
            flags |= FLAG_HAS_FINISH_USING;
        }
        flags
    }
}

static REGISTERED_ITEMS: Mutex<Vec<ItemDefinition>> = Mutex::new(Vec::new());

/// Registers a custom item definition from the current mod.
pub fn register_item(item: ItemDefinition) {
    let mut items = REGISTERED_ITEMS.lock().unwrap();
    items.push(item);
}

/// Returns a clone of all items registered so far in this mod.
pub fn get_registered_items() -> Vec<ItemDefinition> {
    REGISTERED_ITEMS.lock().unwrap().clone()
}

/// Clears the registered items registry (primarily for test isolation).
pub fn clear_registered_items() {
    REGISTERED_ITEMS.lock().unwrap().clear();
}

/// C-ABI exported symbol allowing the loader to query the number of registered items.
#[no_mangle]
pub extern "C" fn redox_mod_get_item_count() -> usize {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        get_registered_items().len()
    }))
    .unwrap_or(0)
}

/// C-ABI exported symbol allowing the loader to retrieve an item definition at `index`.
///
/// # Safety
/// Output pointers must point to valid, writable memory.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_get_item_at(
    index: usize,
    out_id: *mut redox_mc_abi::types::FfiStr,
    out_max_stack: *mut u32,
    out_durability: *mut u32,
    out_fire_resistant: *mut bool,
    out_behavior_flags: *mut u32,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_id.is_null()
            || out_max_stack.is_null()
            || out_durability.is_null()
            || out_fire_resistant.is_null()
            || out_behavior_flags.is_null()
        {
            return redox_mc_abi::types::FfiStatus::NullPointer;
        }

        let items = REGISTERED_ITEMS.lock().unwrap();
        if index >= items.len() {
            return redox_mc_abi::types::FfiStatus::NotFound;
        }

        let item = &items[index];
        *out_id = redox_mc_abi::types::FfiStr::from_str(item.id.as_str());
        *out_max_stack = item.max_stack_size;
        *out_durability = item.durability.unwrap_or(0);
        *out_fire_resistant = item.fire_resistant;
        *out_behavior_flags = item.behavior_flags();

        redox_mc_abi::types::FfiStatus::Success
    }));

    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}
