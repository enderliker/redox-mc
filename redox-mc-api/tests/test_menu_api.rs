use redox_mc_api::gui::{
    clear_handlers, clear_registered_menus, dispatch_close, dispatch_open, dispatch_slot_click,
    get_registered_menus, on_close, on_open, on_slot_click, redox_mod_get_menu_at,
    redox_mod_get_menu_count, redox_mod_on_menu_close, redox_mod_on_menu_open,
    redox_mod_on_menu_slot_click, register_menu, MenuAction, MenuClickEvent, MenuClickType,
    MenuDefinition,
};
use redox_mc_api::types::{FfiMenuAction, FfiMenuClickEvent, FfiStatus, FfiStr};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

#[test]
fn test_menu_definition_builder() {
    let menu = MenuDefinition::new("testmod:custom_menu")
        .slots(27)
        .title("Custom Chest");

    assert_eq!(menu.id, "testmod:custom_menu");
    assert_eq!(menu.slots, 27);
    assert_eq!(menu.title, "Custom Chest");

    let default_menu = MenuDefinition::new("testmod:default_menu");
    assert_eq!(default_menu.slots, 9);
    assert_eq!(default_menu.title, "testmod:default_menu");
}

#[test]
fn test_menu_registration_and_c_abi() {
    clear_registered_menus();

    let m1 = MenuDefinition::new("testmod:menu1")
        .slots(9)
        .title("Menu 1");
    let m2 = MenuDefinition::new("testmod:menu2")
        .slots(18)
        .title("Menu 2");

    register_menu(m1);
    register_menu(m2);

    let registered = get_registered_menus();
    assert_eq!(registered.len(), 2);
    assert_eq!(registered[0].id, "testmod:menu1");
    assert_eq!(registered[1].slots, 18);

    unsafe {
        assert_eq!(redox_mod_get_menu_count(), 2);

        let mut id = FfiStr::EMPTY;
        let mut slots = 0usize;
        let mut title = FfiStr::EMPTY;

        let status = redox_mod_get_menu_at(0, &mut id, &mut slots, &mut title);
        assert_eq!(status, FfiStatus::Success);
        assert_eq!(id.as_str().unwrap(), "testmod:menu1");
        assert_eq!(slots, 9);
        assert_eq!(title.as_str().unwrap(), "Menu 1");

        let status_oob = redox_mod_get_menu_at(99, &mut id, &mut slots, &mut title);
        assert_eq!(status_oob, FfiStatus::NotFound);

        let status_null = redox_mod_get_menu_at(0, std::ptr::null_mut(), &mut slots, &mut title);
        assert_eq!(status_null, FfiStatus::NullPointer);
    }

    clear_registered_menus();
}

#[test]
fn test_menu_lifecycle_and_slot_click_handlers() {
    clear_handlers();

    let open_called = Arc::new(AtomicBool::new(false));
    let open_flag = open_called.clone();
    on_open(move |menu_id, container_id, player_name| {
        if menu_id == "testmod:test_menu" && container_id == 42 && player_name == "Steve" {
            open_flag.store(true, Ordering::SeqCst);
        }
    });

    let close_called = Arc::new(AtomicBool::new(false));
    let close_flag = close_called.clone();
    on_close(move |menu_id, container_id, player_name| {
        if menu_id == "testmod:test_menu" && container_id == 42 && player_name == "Steve" {
            close_flag.store(true, Ordering::SeqCst);
        }
    });

    let click_count = Arc::new(AtomicI32::new(0));
    let click_acc = click_count.clone();
    on_slot_click(move |event| {
        click_acc.fetch_add(1, Ordering::SeqCst);
        assert_eq!(event.menu_id(), "testmod:test_menu");
        assert_eq!(event.container_id(), 42);
        assert_eq!(event.player_name(), "Steve");

        if event.slot_index() == 8 {
            MenuAction::Deny
        } else {
            MenuAction::Allow
        }
    });

    // Test open dispatch
    dispatch_open("testmod:test_menu", 42, "Steve");
    assert!(open_called.load(Ordering::SeqCst));

    // Test slot click dispatch via FFI
    let raw_event_allow = FfiMenuClickEvent::new(
        FfiStr::from_str("testmod:test_menu"),
        42,
        0,
        0,
        MenuClickType::Pickup,
        FfiStr::from_str("Steve"),
    );
    let event_allow = unsafe { MenuClickEvent::from_ffi(&raw_event_allow) };
    assert_eq!(dispatch_slot_click(&event_allow), MenuAction::Allow);

    let raw_event_deny = FfiMenuClickEvent::new(
        FfiStr::from_str("testmod:test_menu"),
        42,
        8,
        0,
        MenuClickType::Pickup,
        FfiStr::from_str("Steve"),
    );
    let event_deny = unsafe { MenuClickEvent::from_ffi(&raw_event_deny) };
    assert_eq!(dispatch_slot_click(&event_deny), MenuAction::Deny);

    // Test C-ABI exported functions directly
    unsafe {
        let ffi_status_open = redox_mod_on_menu_open(
            FfiStr::from_str("testmod:test_menu"),
            42,
            FfiStr::from_str("Steve"),
        );
        assert_eq!(ffi_status_open, FfiStatus::Success);

        let res_allow = redox_mod_on_menu_slot_click(&raw_event_allow);
        assert!(res_allow.is_ok());
        assert_eq!(res_allow.value, FfiMenuAction::Allow);

        let res_deny = redox_mod_on_menu_slot_click(&raw_event_deny);
        assert!(res_deny.is_ok());
        assert_eq!(res_deny.value, FfiMenuAction::Deny);

        let res_null = redox_mod_on_menu_slot_click(std::ptr::null());
        assert_eq!(res_null.status, FfiStatus::NullPointer);
        assert_eq!(res_null.value, FfiMenuAction::Deny);

        let ffi_status_close = redox_mod_on_menu_close(
            FfiStr::from_str("testmod:test_menu"),
            42,
            FfiStr::from_str("Steve"),
        );
        assert_eq!(ffi_status_close, FfiStatus::Success);
    }

    // Test close dispatch
    dispatch_close("testmod:test_menu", 42, "Steve");
    assert!(close_called.load(Ordering::SeqCst));

    clear_handlers();
}

#[test]
fn test_menu_click_event_safe_builder() {
    let event = MenuClickEvent::new("testmod:custom", 10, 3, 1, MenuClickType::QuickMove, "Alex");
    assert_eq!(event.menu_id(), "testmod:custom");
    assert_eq!(event.container_id(), 10);
    assert_eq!(event.slot_index(), 3);
    assert_eq!(event.button(), 1);
    assert_eq!(event.click_type(), MenuClickType::QuickMove);
    assert_eq!(event.player_name(), "Alex");
}

#[test]
fn test_multiple_handlers_deny_takes_precedence() {
    clear_handlers();

    on_slot_click(|_event| MenuAction::Allow);
    on_slot_click(|_event| MenuAction::Deny);
    on_slot_click(|_event| MenuAction::Allow);

    let event = MenuClickEvent::new("testmod:m", 1, 0, 0, MenuClickType::Pickup, "Player");
    assert_eq!(dispatch_slot_click(&event), MenuAction::Deny);

    clear_handlers();
}

#[test]
fn test_panic_containment_in_menu_slot_click() {
    clear_handlers();

    on_slot_click(|_event| {
        panic!("simulated panic inside mod slot click handler");
    });

    let raw_event = FfiMenuClickEvent::new(
        FfiStr::from_str("testmod:panicking_menu"),
        1,
        0,
        0,
        MenuClickType::Pickup,
        FfiStr::from_str("Player"),
    );

    unsafe {
        let res = redox_mod_on_menu_slot_click(&raw_event);
        assert_eq!(res.status, FfiStatus::Panic);
        assert_eq!(res.value, FfiMenuAction::Deny);
    }

    clear_handlers();
}

#[test]
fn test_invalid_utf8_in_menu_callbacks() {
    clear_handlers();

    let invalid_bytes: &[u8] = &[0xFF, 0xFE, 0xFD];
    let invalid_str = FfiStr {
        ptr: invalid_bytes.as_ptr(),
        len: invalid_bytes.len(),
    };
    let valid_str = FfiStr::from_str("valid");

    unsafe {
        let status_open1 = redox_mod_on_menu_open(invalid_str, 1, valid_str);
        assert_eq!(status_open1, FfiStatus::InvalidArgument);

        let status_open2 = redox_mod_on_menu_open(valid_str, 1, invalid_str);
        assert_eq!(status_open2, FfiStatus::InvalidArgument);

        let status_close1 = redox_mod_on_menu_close(invalid_str, 1, valid_str);
        assert_eq!(status_close1, FfiStatus::InvalidArgument);

        let status_close2 = redox_mod_on_menu_close(valid_str, 1, invalid_str);
        assert_eq!(status_close2, FfiStatus::InvalidArgument);
    }
}
