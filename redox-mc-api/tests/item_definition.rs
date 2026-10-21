use redox_mc_api::item::{
    clear_registered_items, get_registered_items, register_item, ItemDefinition,
    FLAG_HAS_FINISH_USING, FLAG_HAS_USE, FLAG_HAS_USE_ON,
};
use std::sync::Mutex;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_item_definition_builder_and_flags() {
    let ruby = ItemDefinition::new("testmod:ruby")
        .max_stack_size(64)
        .fire_resistant(true)
        .has_use(true);

    assert_eq!(ruby.id, "testmod:ruby");
    assert_eq!(ruby.max_stack_size, 64);
    assert_eq!(ruby.durability, None);
    assert!(ruby.fire_resistant);
    assert!(ruby.has_use);
    assert!(!ruby.has_use_on);
    assert!(!ruby.has_finish_using);
    assert_eq!(ruby.behavior_flags(), FLAG_HAS_USE);

    let pickaxe = ItemDefinition::new("testmod:ruby_pickaxe")
        .durability(1500)
        .has_use_on(true);

    assert_eq!(pickaxe.id, "testmod:ruby_pickaxe");
    assert_eq!(pickaxe.max_stack_size, 1); // automatically defaults to 1 for durability
    assert_eq!(pickaxe.durability, Some(1500));
    assert_eq!(pickaxe.behavior_flags(), FLAG_HAS_USE_ON);

    let potion = ItemDefinition::new("testmod:ruby_elixir")
        .max_stack_size(16)
        .has_finish_using(true);

    assert_eq!(potion.behavior_flags(), FLAG_HAS_FINISH_USING);
}

#[test]
fn test_item_registration_and_retrieval() {
    let _lock = TEST_MUTEX.lock().unwrap();
    clear_registered_items();
    assert!(get_registered_items().is_empty());

    let item1 = ItemDefinition::new("testmod:ruby").max_stack_size(64);
    let item2 = ItemDefinition::new("testmod:sapphire").max_stack_size(32);

    register_item(item1.clone());
    register_item(item2.clone());

    let registered = get_registered_items();
    assert_eq!(registered.len(), 2);
    assert_eq!(registered[0], item1);
    assert_eq!(registered[1], item2);

    clear_registered_items();
    assert!(get_registered_items().is_empty());
}

#[test]
fn test_c_abi_export_symbols() {
    let _lock = TEST_MUTEX.lock().unwrap();
    clear_registered_items();

    let ruby = ItemDefinition::new("testmod:ruby")
        .max_stack_size(64)
        .fire_resistant(true)
        .has_use(true);
    register_item(ruby);

    assert_eq!(redox_mc_api::item::redox_mod_get_item_count(), 1);

    let mut id = redox_mc_abi::types::FfiStr::EMPTY;
    let mut max_stack = 0u32;
    let mut durability = 0u32;
    let mut fire_resistant = false;
    let mut behavior_flags = 0u32;

    let status = unsafe {
        redox_mc_api::item::redox_mod_get_item_at(
            0,
            &mut id,
            &mut max_stack,
            &mut durability,
            &mut fire_resistant,
            &mut behavior_flags,
        )
    };

    assert_eq!(status, redox_mc_abi::types::FfiStatus::Success);
    assert_eq!(unsafe { id.as_str().unwrap() }, "testmod:ruby");
    assert_eq!(max_stack, 64);
    assert_eq!(durability, 0);
    assert!(fire_resistant);
    assert_eq!(behavior_flags, FLAG_HAS_USE);

    // Out of bounds test
    let status_oob = unsafe {
        redox_mc_api::item::redox_mod_get_item_at(
            1,
            &mut id,
            &mut max_stack,
            &mut durability,
            &mut fire_resistant,
            &mut behavior_flags,
        )
    };
    assert_eq!(status_oob, redox_mc_abi::types::FfiStatus::NotFound);

    clear_registered_items();
}
