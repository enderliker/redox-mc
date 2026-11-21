use redox_mc_abi::types::{FfiBuffer, FfiStatus, FfiStr};
use redox_mc_api::block::{
    clear_registered_blocks, get_registered_blocks, redox_mod_free_buffer,
    redox_mod_get_block_entity_info_at, redox_mod_on_block_entity_load,
    redox_mod_on_block_entity_save, redox_mod_on_block_entity_tick, register_block,
    BlockDefinition,
};
use redox_mc_api::block_entity::{
    clear_handlers, dispatch_load, dispatch_save, dispatch_tick, on_load, on_save, on_tick,
    BlockEntityDefinition, FLAG_HAS_PERSISTENCE, FLAG_HAS_TICK,
};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

#[test]
fn test_block_entity_declarative_builder() {
    let be_default = BlockEntityDefinition::new("testmod:default_be");
    assert_eq!(be_default.id, "testmod:default_be");
    assert!(!be_default.has_tick);
    assert!(!be_default.has_persistence);
    assert_eq!(be_default.behavior_flags(), 0);

    let be = BlockEntityDefinition::new("testmod:counter_be")
        .has_tick(true)
        .has_persistence(true);

    let block1 = BlockDefinition::new("testmod:counter_block")
        .strength(2.0)
        .with_block_entity(be.clone());

    assert!(block1.block_entity.is_some());
    let be_def1 = block1.block_entity.as_ref().unwrap();
    assert_eq!(be_def1.id, "testmod:counter_be");
    assert!(be_def1.has_tick);
    assert!(be_def1.has_persistence);

    // Verify alias .block_entity(...)
    let block2 = BlockDefinition::new("testmod:counter_block_2")
        .strength(2.0)
        .block_entity(be);
    assert!(block2.block_entity.is_some());
    assert_eq!(block2.block_entity.unwrap().id, "testmod:counter_be");
}

#[test]
fn test_block_entity_handlers_and_flags() {
    clear_handlers();

    let def = BlockEntityDefinition::new("testmod:demo")
        .has_tick(true)
        .has_persistence(true);
    assert_eq!(def.behavior_flags(), FLAG_HAS_TICK | FLAG_HAS_PERSISTENCE);

    static TICK_CALLED: AtomicI32 = AtomicI32::new(0);
    on_tick(|id, x, y, z| {
        if id == "testmod:demo" && x == 10 && y == 20 && z == 30 {
            TICK_CALLED.store(1, Ordering::SeqCst);
        }
    });

    dispatch_tick("testmod:demo", 10, 20, 30);
    assert_eq!(TICK_CALLED.load(Ordering::SeqCst), 1);

    static SAVED_CONTENT: Mutex<Option<String>> = Mutex::new(None);
    on_save(|id, _x, _y, _z| {
        if id == "testmod:demo" {
            Some("counter=42".to_string())
        } else {
            None
        }
    });

    let saved = dispatch_save("testmod:demo", 0, 0, 0);
    assert_eq!(saved, Some("counter=42".to_string()));

    on_load(|id, _x, _y, _z, data| {
        if id == "testmod:demo" {
            *SAVED_CONTENT.lock().unwrap() = Some(data.to_string());
        }
    });

    dispatch_load("testmod:demo", 0, 0, 0, "counter=42");
    assert_eq!(
        *SAVED_CONTENT.lock().unwrap(),
        Some("counter=42".to_string())
    );
}

#[test]
fn test_c_abi_block_entity_exported_symbols() {
    clear_handlers();
    clear_registered_blocks();

    // Register a block with block entity for index lookup test
    let be = BlockEntityDefinition::new("testmod:c_abi_be")
        .has_tick(true)
        .has_persistence(true);
    let block = BlockDefinition::new("testmod:c_abi_block").with_block_entity(be);
    register_block(block);
    assert_eq!(get_registered_blocks().len(), 1);

    // 1. Test redox_mod_get_block_entity_info_at
    unsafe {
        let mut out_has_entity = false;
        let mut out_id = FfiStr::EMPTY;
        let mut out_flags = 0u32;

        // Null pointer check
        let status = redox_mod_get_block_entity_info_at(
            0,
            std::ptr::null_mut(),
            &mut out_id,
            &mut out_flags,
        );
        assert_eq!(status, FfiStatus::NullPointer);

        let status = redox_mod_get_block_entity_info_at(
            0,
            &mut out_has_entity,
            std::ptr::null_mut(),
            &mut out_flags,
        );
        assert_eq!(status, FfiStatus::NullPointer);

        let status = redox_mod_get_block_entity_info_at(
            0,
            &mut out_has_entity,
            &mut out_id,
            std::ptr::null_mut(),
        );
        assert_eq!(status, FfiStatus::NullPointer);

        // Out of bounds check
        let status = redox_mod_get_block_entity_info_at(
            999,
            &mut out_has_entity,
            &mut out_id,
            &mut out_flags,
        );
        assert_eq!(status, FfiStatus::NotFound);

        // Valid query at index 0
        let status =
            redox_mod_get_block_entity_info_at(0, &mut out_has_entity, &mut out_id, &mut out_flags);
        assert_eq!(status, FfiStatus::Success);
        assert!(out_has_entity);
        assert_eq!(out_id.as_str().unwrap(), "testmod:c_abi_be");
        assert_eq!(out_flags, FLAG_HAS_TICK | FLAG_HAS_PERSISTENCE);
    }

    // 2. Test redox_mod_on_block_entity_tick
    static C_ABI_TICK: AtomicI32 = AtomicI32::new(0);
    on_tick(|id, x, y, z| {
        if id == "testmod:c_abi_be" && x == 1 && y == 2 && z == 3 {
            C_ABI_TICK.store(42, Ordering::SeqCst);
        }
    });

    unsafe {
        let valid_id = FfiStr::from_str("testmod:c_abi_be");
        let status = redox_mod_on_block_entity_tick(valid_id, 1, 2, 3);
        assert_eq!(status, FfiStatus::Success);
        assert_eq!(C_ABI_TICK.load(Ordering::SeqCst), 42);

        // Invalid UTF-8 handling
        let invalid_utf8 = [0xFF, 0xFE, 0xFD];
        let invalid_id = FfiStr {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };
        let status = redox_mod_on_block_entity_tick(invalid_id, 1, 2, 3);
        assert_eq!(status, FfiStatus::InvalidArgument);
    }

    // 3. Test redox_mod_on_block_entity_save and redox_mod_free_buffer
    on_save(|id, _x, _y, _z| {
        if id == "testmod:c_abi_be" {
            Some("state=active".to_string())
        } else {
            None
        }
    });

    unsafe {
        let valid_id = FfiStr::from_str("testmod:c_abi_be");

        // Null buffer pointer check
        let status = redox_mod_on_block_entity_save(valid_id, 0, 0, 0, std::ptr::null_mut());
        assert_eq!(status, FfiStatus::NullPointer);

        // Invalid UTF-8 check
        let mut out_buf = FfiBuffer::NULL;
        let invalid_utf8 = [0xFF, 0xFE, 0xFD];
        let invalid_id = FfiStr {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };
        let status = redox_mod_on_block_entity_save(invalid_id, 0, 0, 0, &mut out_buf);
        assert_eq!(status, FfiStatus::InvalidArgument);
        assert!(out_buf.is_null());

        // Valid save call
        let status = redox_mod_on_block_entity_save(valid_id, 0, 0, 0, &mut out_buf);
        assert_eq!(status, FfiStatus::Success);
        assert!(!out_buf.is_null());
        assert_eq!(out_buf.len, "state=active".len());

        let slice = std::slice::from_raw_parts(out_buf.ptr, out_buf.len);
        assert_eq!(std::str::from_utf8(slice).unwrap(), "state=active");

        // Free the buffer using exported free function
        redox_mod_free_buffer(out_buf);
    }

    // 4. Test redox_mod_on_block_entity_load
    static C_ABI_LOAD: Mutex<Option<String>> = Mutex::new(None);
    on_load(|id, _x, _y, _z, data| {
        if id == "testmod:c_abi_be" {
            *C_ABI_LOAD.lock().unwrap() = Some(data.to_string());
        }
    });

    unsafe {
        let valid_id = FfiStr::from_str("testmod:c_abi_be");
        let valid_data = FfiStr::from_str("state=active");
        let status = redox_mod_on_block_entity_load(valid_id, 0, 0, 0, valid_data);
        assert_eq!(status, FfiStatus::Success);
        assert_eq!(
            *C_ABI_LOAD.lock().unwrap(),
            Some("state=active".to_string())
        );

        // Invalid UTF-8 on data
        let invalid_utf8 = [0xFF, 0xFE, 0xFD];
        let invalid_data = FfiStr {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };
        let status = redox_mod_on_block_entity_load(valid_id, 0, 0, 0, invalid_data);
        assert_eq!(status, FfiStatus::InvalidArgument);
    }
}
