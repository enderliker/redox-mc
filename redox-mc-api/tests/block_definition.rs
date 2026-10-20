use redox_mc_api::block::{
    clear_registered_blocks, get_registered_blocks, register_block, BlockDefinition,
    FLAG_HAS_DESTROY, FLAG_HAS_STEP_ON, FLAG_HAS_USE_WITHOUT_ITEM,
};

#[test]
fn test_block_definition_builder_and_flags() {
    let ruby_block = BlockDefinition::new("testmod:ruby_block")
        .strength(5.0, 10.0)
        .requires_tool(true)
        .has_use_without_item(true);

    assert_eq!(ruby_block.id, "testmod:ruby_block");
    assert_eq!(ruby_block.destroy_time, 5.0);
    assert_eq!(ruby_block.explosion_resistance, 10.0);
    assert!(ruby_block.requires_tool);
    assert!(!ruby_block.no_collision);
    assert!(ruby_block.has_item);
    assert!(ruby_block.has_use_without_item);
    assert!(!ruby_block.has_step_on);
    assert!(!ruby_block.has_destroy);
    assert_eq!(ruby_block.behavior_flags(), FLAG_HAS_USE_WITHOUT_ITEM);

    let jump_pad = BlockDefinition::new("testmod:jump_pad")
        .strength(0.5, 1.0)
        .no_collision(true)
        .has_step_on(true)
        .has_destroy(true);

    assert_eq!(jump_pad.id, "testmod:jump_pad");
    assert!(jump_pad.no_collision);
    assert!(jump_pad.has_step_on);
    assert!(jump_pad.has_destroy);
    assert_eq!(
        jump_pad.behavior_flags(),
        FLAG_HAS_STEP_ON | FLAG_HAS_DESTROY
    );
}

#[test]
fn test_block_registration_and_retrieval() {
    clear_registered_blocks();
    assert!(get_registered_blocks().is_empty());

    let block1 = BlockDefinition::new("testmod:ruby_block").strength(3.0, 6.0);
    let block2 = BlockDefinition::new("testmod:sapphire_block").strength(4.0, 8.0);

    register_block(block1.clone());
    register_block(block2.clone());

    let registered = get_registered_blocks();
    assert_eq!(registered.len(), 2);
    assert_eq!(registered[0], block1);
    assert_eq!(registered[1], block2);

    clear_registered_blocks();
    assert!(get_registered_blocks().is_empty());
}

#[test]
fn test_block_c_abi_export_symbols() {
    clear_registered_blocks();

    let ruby_block = BlockDefinition::new("testmod:ruby_block")
        .strength(3.0, 6.0)
        .requires_tool(true)
        .has_use_without_item(true);
    register_block(ruby_block);

    assert_eq!(redox_mc_api::block::redox_mod_get_block_count(), 1);

    let mut id = redox_mc_abi::types::FfiStr::EMPTY;
    let mut destroy_time = 0.0f32;
    let mut explosion_resistance = 0.0f32;
    let mut requires_tool = false;
    let mut no_collision = false;
    let mut behavior_flags = 0u32;
    let mut has_item = false;

    let status = unsafe {
        redox_mc_api::block::redox_mod_get_block_at(
            0,
            &mut id,
            &mut destroy_time,
            &mut explosion_resistance,
            &mut requires_tool,
            &mut no_collision,
            &mut behavior_flags,
            &mut has_item,
        )
    };

    assert_eq!(status, redox_mc_abi::types::FfiStatus::Success);
    assert_eq!(unsafe { id.as_str().unwrap() }, "testmod:ruby_block");
    assert_eq!(destroy_time, 3.0);
    assert_eq!(explosion_resistance, 6.0);
    assert!(requires_tool);
    assert!(!no_collision);
    assert_eq!(behavior_flags, FLAG_HAS_USE_WITHOUT_ITEM);
    assert!(has_item);

    // Out of bounds test
    let status_oob = unsafe {
        redox_mc_api::block::redox_mod_get_block_at(
            1,
            &mut id,
            &mut destroy_time,
            &mut explosion_resistance,
            &mut requires_tool,
            &mut no_collision,
            &mut behavior_flags,
            &mut has_item,
        )
    };
    assert_eq!(status_oob, redox_mc_abi::types::FfiStatus::NotFound);

    clear_registered_blocks();
}
