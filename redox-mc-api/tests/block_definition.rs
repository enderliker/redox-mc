use redox_mc_api::block::{
    clear_registered_blocks, get_registered_blocks, register_block, BlockDefinition,
    FLAG_HAS_DESTROY, FLAG_HAS_STEP_ON, FLAG_HAS_USE_WITHOUT_ITEM,
};
use std::sync::Mutex;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_block_definition_builder_and_flags() {
    let ruby_block = BlockDefinition::new("testmod:ruby_block")
        .strength(5.0)
        .requires_tool(true)
        .has_use_without_item(true);

    assert_eq!(ruby_block.id, "testmod:ruby_block");
    assert_eq!(ruby_block.destroy_time, 5.0);
    assert_eq!(ruby_block.explosion_resistance, 5.0);
    assert!(ruby_block.requires_tool);
    assert!(!ruby_block.no_collision);
    assert!(ruby_block.has_item);
    assert!(ruby_block.has_use_without_item);
    assert!(!ruby_block.has_step_on);
    assert!(!ruby_block.has_destroy);
    assert_eq!(ruby_block.behavior_flags(), FLAG_HAS_USE_WITHOUT_ITEM);

    let jump_pad = BlockDefinition::new("testmod:jump_pad")
        .strength_and_resistance(0.5, 1.0)
        .no_collision(true)
        .has_step_on(true)
        .has_destroy(true);

    assert_eq!(jump_pad.id, "testmod:jump_pad");
    assert_eq!(jump_pad.destroy_time, 0.5);
    assert_eq!(jump_pad.explosion_resistance, 1.0);
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
    let _lock = TEST_MUTEX.lock().unwrap();
    clear_registered_blocks();
    assert!(get_registered_blocks().is_empty());

    let block1 = BlockDefinition::new("testmod:ruby_block").strength(3.0);
    let block2 = BlockDefinition::new("testmod:sapphire_block").strength_and_resistance(4.0, 8.0);

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
    let _lock = TEST_MUTEX.lock().unwrap();
    clear_registered_blocks();

    let ruby_block = BlockDefinition::new("testmod:ruby_block")
        .strength_and_resistance(3.0, 6.0)
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

#[test]
fn test_block_declarative_cube_model_and_assets() {
    let _lock = TEST_MUTEX.lock().unwrap();
    clear_registered_blocks();
    redox_mc_api::resource::clear_registered_resources();

    let ruby_block = BlockDefinition::new("testmod:ruby_block")
        .strength(3.0)
        .cube_model("testmod:block/ruby_block")
        .drops("testmod:ruby");

    // Test asset generation directly on BlockDefinition
    let generated = ruby_block.generate_virtual_resources();
    assert_eq!(generated.len(), 6); // blockstate, block model, item def, item legacy model, loot table, loot tables alias

    let paths: Vec<String> = generated.iter().map(|r| r.path.clone()).collect();
    assert!(paths.contains(&"blockstates/ruby_block.json".to_string()));
    assert!(paths.contains(&"models/block/ruby_block.json".to_string()));
    assert!(paths.contains(&"items/ruby_block.json".to_string()));
    assert!(paths.contains(&"models/item/ruby_block.json".to_string()));
    assert!(paths.contains(&"loot_table/blocks/ruby_block.json".to_string()));
    assert!(paths.contains(&"loot_tables/blocks/ruby_block.json".to_string()));

    // Verify content of blockstate
    let bs = generated
        .iter()
        .find(|r| r.path == "blockstates/ruby_block.json")
        .unwrap();
    let bs_str = String::from_utf8(bs.content.clone()).unwrap();
    assert!(bs_str.contains(r#""model":"testmod:block/ruby_block""#));

    // Verify content of block model
    let bm = generated
        .iter()
        .find(|r| r.path == "models/block/ruby_block.json")
        .unwrap();
    let bm_str = String::from_utf8(bm.content.clone()).unwrap();
    assert!(bm_str.contains(r#""parent":"minecraft:block/cube_all""#));
    assert!(bm_str.contains(r#""all":"testmod:block/ruby_block""#));

    // Verify content of loot table
    let lt = generated
        .iter()
        .find(|r| r.path == "loot_table/blocks/ruby_block.json")
        .unwrap();
    let lt_str = String::from_utf8(lt.content.clone()).unwrap();
    assert!(lt_str.contains(r#""name":"testmod:ruby""#));
    assert!(lt_str.contains(r#""type":"minecraft:block""#));

    // Registering the block should automatically register the resources into virtual resource registry
    register_block(ruby_block);

    let res = redox_mc_api::resource::get_registered_resources();
    assert_eq!(res.len(), 6);

    clear_registered_blocks();
    redox_mc_api::resource::clear_registered_resources();
}

#[test]
fn test_split_identifier_edge_cases() {
    assert_eq!(
        redox_mc_api::split_identifier("testmod:ruby_block"),
        ("testmod", "ruby_block")
    );
    assert_eq!(
        redox_mc_api::split_identifier("stone"),
        ("minecraft", "stone")
    );
    assert_eq!(
        redox_mc_api::split_identifier(":stone"),
        ("minecraft", "stone")
    );
    assert_eq!(redox_mc_api::split_identifier(""), ("minecraft", ""));
}
