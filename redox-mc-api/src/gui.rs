//! Declarative container menu definitions and handlers for Redox-MC mods.

use redox_mc_abi::types::{FfiMenuAction, FfiMenuClickEvent, FfiResult, FfiStatus, FfiStr};
use std::sync::{Arc, Mutex};

pub use redox_mc_abi::types::FfiMenuClickType as MenuClickType;

/// Declarative builder for defining custom Minecraft container menus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuDefinition {
    /// Namespaced identifier of the menu (e.g. "testmod:counter_menu").
    pub id: String,
    /// Number of container slots (e.g. 9 for 3x3, 27 for standard chest).
    pub slots: usize,
    /// Display title for the menu window.
    pub title: String,
}

impl MenuDefinition {
    /// Creates a new `MenuDefinition` with default 9 slots.
    pub fn new(id: impl Into<String>) -> Self {
        let id_str = id.into();
        Self {
            id: id_str.clone(),
            slots: 9,
            title: id_str,
        }
    }

    /// Sets the number of container slots.
    pub fn slots(mut self, slots: usize) -> Self {
        self.slots = slots;
        self
    }

    /// Sets the display title for this menu.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

/// Outcome of intercepting a container menu slot click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Allow the click to proceed and modify the slot state.
    Allow,
    /// Reject/cancel the click and resync container state to client.
    Deny,
}

impl From<MenuAction> for FfiMenuAction {
    fn from(action: MenuAction) -> Self {
        match action {
            MenuAction::Allow => FfiMenuAction::Allow,
            MenuAction::Deny => FfiMenuAction::Deny,
        }
    }
}

impl From<FfiMenuAction> for MenuAction {
    fn from(ffi: FfiMenuAction) -> Self {
        match ffi {
            FfiMenuAction::Allow => MenuAction::Allow,
            FfiMenuAction::Deny => MenuAction::Deny,
        }
    }
}

/// Safe wrapper around an incoming container menu slot click event.
#[derive(Debug, Clone)]
pub struct MenuClickEvent<'a> {
    menu_id: &'a str,
    container_id: i32,
    slot_index: i32,
    button: i32,
    click_type: MenuClickType,
    player_name: &'a str,
}

impl<'a> MenuClickEvent<'a> {
    /// Constructs a `MenuClickEvent` with explicit safe fields (useful for testing and local dispatch).
    pub fn new(
        menu_id: &'a str,
        container_id: i32,
        slot_index: i32,
        button: i32,
        click_type: MenuClickType,
        player_name: &'a str,
    ) -> Self {
        Self {
            menu_id,
            container_id,
            slot_index,
            button,
            click_type,
            player_name,
        }
    }

    /// Constructs a `MenuClickEvent` from an FFI event struct.
    ///
    /// # Safety
    /// `raw` must point to a valid `FfiMenuClickEvent` whose string slices are valid UTF-8 and live for `'a`.
    pub unsafe fn from_ffi(raw: &'a FfiMenuClickEvent) -> Self {
        let menu_id = unsafe { raw.menu_id.as_str().unwrap_or("") };
        let player_name = unsafe { raw.player_name.as_str().unwrap_or("") };
        Self {
            menu_id,
            container_id: raw.container_id,
            slot_index: raw.slot_index,
            button: raw.button,
            click_type: raw.click_type,
            player_name,
        }
    }

    /// Namespaced identifier of the menu (e.g. "testmod:counter_menu").
    pub fn menu_id(&self) -> &str {
        self.menu_id
    }

    /// Window / container ID assigned by the server for this open session.
    pub fn container_id(&self) -> i32 {
        self.container_id
    }

    /// Slot index clicked within the container menu.
    pub fn slot_index(&self) -> i32 {
        self.slot_index
    }

    /// Mouse button pressed (0 = left, 1 = right, etc.).
    pub fn button(&self) -> i32 {
        self.button
    }

    /// Interaction click type (Pickup, QuickMove, Swap, etc.).
    pub fn click_type(&self) -> MenuClickType {
        self.click_type
    }

    /// Name of the player who clicked the slot.
    pub fn player_name(&self) -> &str {
        self.player_name
    }
}

static REGISTERED_MENUS: Mutex<Vec<MenuDefinition>> = Mutex::new(Vec::new());

/// Registers a custom container menu definition with the mod runtime.
pub fn register_menu(menu: MenuDefinition) {
    REGISTERED_MENUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(menu);
}

/// Returns a clone of all menus registered by this mod.
pub fn get_registered_menus() -> Vec<MenuDefinition> {
    REGISTERED_MENUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Clears all registered menus (primarily for test isolation).
pub fn clear_registered_menus() {
    REGISTERED_MENUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

type OpenHandler = Arc<dyn Fn(&str, i32, &str) + Send + Sync>;
type SlotClickHandler = Arc<dyn Fn(&MenuClickEvent) -> MenuAction + Send + Sync>;
type CloseHandler = Arc<dyn Fn(&str, i32, &str) + Send + Sync>;

static OPEN_HANDLERS: Mutex<Vec<OpenHandler>> = Mutex::new(Vec::new());
static SLOT_CLICK_HANDLERS: Mutex<Vec<SlotClickHandler>> = Mutex::new(Vec::new());
static CLOSE_HANDLERS: Mutex<Vec<CloseHandler>> = Mutex::new(Vec::new());

/// Registers a callback to be invoked when a container menu is opened.
pub fn on_open<F>(handler: F)
where
    F: Fn(&str, i32, &str) + Send + Sync + 'static,
{
    OPEN_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Arc::new(handler));
}

/// Registers a callback to intercept container menu slot clicks.
pub fn on_slot_click<F>(handler: F)
where
    F: Fn(&MenuClickEvent) -> MenuAction + Send + Sync + 'static,
{
    SLOT_CLICK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Arc::new(handler));
}

/// Registers a callback to be invoked when a container menu is closed.
pub fn on_close<F>(handler: F)
where
    F: Fn(&str, i32, &str) + Send + Sync + 'static,
{
    CLOSE_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Arc::new(handler));
}

/// Dispatches menu open event to registered handlers without holding the lock.
pub fn dispatch_open(menu_id: &str, container_id: i32, player_name: &str) {
    let handlers = {
        let guard = OPEN_HANDLERS.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    for h in handlers {
        h(menu_id, container_id, player_name);
    }
}

/// Dispatches menu slot click event to registered handlers without holding the lock.
/// If any handler returns Deny, returns Deny. Otherwise Allow.
pub fn dispatch_slot_click(event: &MenuClickEvent) -> MenuAction {
    let handlers = {
        let guard = SLOT_CLICK_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    let mut final_action = MenuAction::Allow;
    for h in handlers {
        if h(event) == MenuAction::Deny {
            final_action = MenuAction::Deny;
        }
    }
    final_action
}

/// Dispatches menu close event to registered handlers without holding the lock.
pub fn dispatch_close(menu_id: &str, container_id: i32, player_name: &str) {
    let handlers = {
        let guard = CLOSE_HANDLERS.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    for h in handlers {
        h(menu_id, container_id, player_name);
    }
}

/// Clears registered menu handlers (primarily for test isolation).
pub fn clear_handlers() {
    OPEN_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    SLOT_CLICK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    CLOSE_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

// ---------------------------------------------------------------------------
// C-ABI Export Symbols
// ---------------------------------------------------------------------------

/// C-ABI exported symbol returning the number of declared custom menus.
#[no_mangle]
pub extern "C" fn redox_mod_get_menu_count() -> usize {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        REGISTERED_MENUS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }));
    result.unwrap_or_else(|err| {
        crate::macros::handle_panic("redox_mod_get_menu_count", err);
        0
    })
}

/// C-ABI exported symbol retrieving the definition of a custom menu by index.
///
/// # Safety
/// Output pointers must point to valid, writable memory.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_get_menu_at(
    index: usize,
    out_id: *mut FfiStr,
    out_slots: *mut usize,
    out_title: *mut FfiStr,
) -> FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_id.is_null() || out_slots.is_null() || out_title.is_null() {
            return FfiStatus::NullPointer;
        }
        let menus = REGISTERED_MENUS.lock().unwrap_or_else(|e| e.into_inner());
        if index >= menus.len() {
            return FfiStatus::NotFound;
        }

        let menu = &menus[index];
        *out_id = FfiStr::from_str(&menu.id);
        *out_slots = menu.slots;
        *out_title = FfiStr::from_str(&menu.title);
        FfiStatus::Success
    }));
    result.unwrap_or_else(|err| {
        crate::macros::handle_panic("redox_mod_get_menu_at", err);
        FfiStatus::Panic
    })
}

/// C-ABI exported symbol for menu open callback.
///
/// # Safety
/// `menu_id` and `player_name` must represent valid FFI string references.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_on_menu_open(
    menu_id: FfiStr,
    container_id: i32,
    player_name: FfiStr,
) -> FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let id_str = match menu_id.as_str() {
            Ok(s) => s,
            Err(_) => return FfiStatus::InvalidArgument,
        };
        let player_str = match player_name.as_str() {
            Ok(s) => s,
            Err(_) => return FfiStatus::InvalidArgument,
        };
        dispatch_open(id_str, container_id, player_str);
        FfiStatus::Success
    }));
    result.unwrap_or_else(|err| {
        crate::macros::handle_panic("redox_mod_on_menu_open", err);
        FfiStatus::Panic
    })
}

/// C-ABI exported symbol for menu slot click interception.
///
/// # Safety
/// `event` must point to a valid, initialized `FfiMenuClickEvent`.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_on_menu_slot_click(
    event: *const FfiMenuClickEvent,
) -> FfiResult<FfiMenuAction> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if event.is_null() {
            return FfiResult::err(FfiStatus::NullPointer, FfiMenuAction::Deny);
        }
        let raw = &*event;
        let click_event = MenuClickEvent::from_ffi(raw);
        let action = dispatch_slot_click(&click_event);
        FfiResult::ok(action.into())
    }));
    result.unwrap_or_else(|err| {
        crate::macros::handle_panic("redox_mod_on_menu_slot_click", err);
        FfiResult::err(FfiStatus::Panic, FfiMenuAction::Deny)
    })
}

/// C-ABI exported symbol for menu close callback.
///
/// # Safety
/// `menu_id` and `player_name` must represent valid FFI string references.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_on_menu_close(
    menu_id: FfiStr,
    container_id: i32,
    player_name: FfiStr,
) -> FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let id_str = match menu_id.as_str() {
            Ok(s) => s,
            Err(_) => return FfiStatus::InvalidArgument,
        };
        let player_str = match player_name.as_str() {
            Ok(s) => s,
            Err(_) => return FfiStatus::InvalidArgument,
        };
        dispatch_close(id_str, container_id, player_str);
        FfiStatus::Success
    }));
    result.unwrap_or_else(|err| {
        crate::macros::handle_panic("redox_mod_on_menu_close", err);
        FfiStatus::Panic
    })
}
