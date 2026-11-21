//! Declarative block entity definitions and handlers for Redox-MC mods.

use std::sync::Mutex;

pub use redox_mc_abi::types::{
    BLOCK_ENTITY_FLAG_HAS_CONTAINER as FLAG_HAS_CONTAINER,
    BLOCK_ENTITY_FLAG_HAS_PERSISTENCE as FLAG_HAS_PERSISTENCE,
    BLOCK_ENTITY_FLAG_HAS_TICK as FLAG_HAS_TICK,
};

/// Declarative builder for defining custom Minecraft block entities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEntityDefinition {
    /// Namespaced identifier of the block entity (e.g. "testmod:counter_be").
    pub id: String,
    /// Whether the block entity ticks every server tick.
    pub has_tick: bool,
    /// Whether the block entity serializes persistent state.
    pub has_persistence: bool,
    /// Whether the block entity holds a persistent item container.
    pub has_container: bool,
    /// Number of container slots (default 9).
    pub slot_count: u32,
    /// Menu identifier used to open container (defaults to `id`).
    pub menu_id: Option<String>,
    /// Optional custom title displayed on the container GUI.
    pub title: Option<String>,
}

impl BlockEntityDefinition {
    /// Creates a new `BlockEntityDefinition` with default flags.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            has_tick: false,
            has_persistence: false,
            has_container: false,
            slot_count: 9,
            menu_id: None,
            title: None,
        }
    }

    /// Sets whether this block entity ticks on the server.
    pub fn has_tick(mut self, val: bool) -> Self {
        self.has_tick = val;
        self
    }

    /// Sets whether this block entity persists state.
    pub fn has_persistence(mut self, val: bool) -> Self {
        self.has_persistence = val;
        self
    }

    /// Sets whether this block entity has a container.
    pub fn has_container(mut self, val: bool) -> Self {
        self.has_container = val;
        self
    }

    /// Configures container slots and associated menu identifier.
    pub fn with_container(mut self, slots: u32, menu_id: impl Into<String>) -> Self {
        self.has_container = true;
        self.slot_count = slots;
        self.menu_id = Some(menu_id.into());
        self
    }

    /// Sets a custom GUI display title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Returns the behavior flags bitmask.
    pub fn behavior_flags(&self) -> u32 {
        let mut flags = 0;
        if self.has_tick {
            flags |= FLAG_HAS_TICK;
        }
        if self.has_persistence {
            flags |= FLAG_HAS_PERSISTENCE;
        }
        if self.has_container {
            flags |= FLAG_HAS_CONTAINER;
        }
        flags
    }
}

type TickHandler = Box<dyn Fn(&str, i32, i32, i32) + Send + Sync>;
type SaveHandler = Box<dyn Fn(&str, i32, i32, i32) -> Option<String> + Send + Sync>;
type LoadHandler = Box<dyn Fn(&str, i32, i32, i32, &str) + Send + Sync>;

static TICK_HANDLERS: Mutex<Vec<TickHandler>> = Mutex::new(Vec::new());
static SAVE_HANDLERS: Mutex<Vec<SaveHandler>> = Mutex::new(Vec::new());
static LOAD_HANDLERS: Mutex<Vec<LoadHandler>> = Mutex::new(Vec::new());

/// Registers a callback to be invoked on every block entity tick.
pub fn on_tick<F>(handler: F)
where
    F: Fn(&str, i32, i32, i32) + Send + Sync + 'static,
{
    TICK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Box::new(handler));
}

/// Registers a callback to be invoked when saving block entity state.
pub fn on_save<F>(handler: F)
where
    F: Fn(&str, i32, i32, i32) -> Option<String> + Send + Sync + 'static,
{
    SAVE_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Box::new(handler));
}

/// Registers a callback to be invoked when loading block entity state.
pub fn on_load<F>(handler: F)
where
    F: Fn(&str, i32, i32, i32, &str) + Send + Sync + 'static,
{
    LOAD_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Box::new(handler));
}

/// Dispatches tick event to registered handlers.
pub fn dispatch_tick(id: &str, x: i32, y: i32, z: i32) {
    let handlers = TICK_HANDLERS.lock().unwrap_or_else(|e| e.into_inner());
    for h in handlers.iter() {
        h(id, x, y, z);
    }
}

/// Dispatches save event to registered handlers.
pub fn dispatch_save(id: &str, x: i32, y: i32, z: i32) -> Option<String> {
    let handlers = SAVE_HANDLERS.lock().unwrap_or_else(|e| e.into_inner());
    for h in handlers.iter() {
        if let Some(data) = h(id, x, y, z) {
            return Some(data);
        }
    }
    None
}

/// Dispatches load event to registered handlers.
pub fn dispatch_load(id: &str, x: i32, y: i32, z: i32, data: &str) {
    let handlers = LOAD_HANDLERS.lock().unwrap_or_else(|e| e.into_inner());
    for h in handlers.iter() {
        h(id, x, y, z, data);
    }
}

/// Clears registered handlers (primarily for test isolation).
pub fn clear_handlers() {
    TICK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    SAVE_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    LOAD_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}
