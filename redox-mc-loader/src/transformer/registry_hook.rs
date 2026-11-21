//! Hook handler for Minecraft `BuiltInRegistries.freeze()`.
//!
//! Intercepts the registry sealing process right after vanilla registries
//! are populated (`createContents()`) and right before they are permanently sealed (`freeze$original()`).

use jvmti_bindings::sys::jni;
use std::ffi::CString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryStatus {
    pub is_frozen: bool,
    pub item_count: i32,
}

/// Inspects the current state of Minecraft's `BuiltInRegistries.ITEM`.
///
/// # Safety
/// `env` must be a valid pointer to a live `JNIEnv` attached to the current JVM thread.
pub unsafe fn query_registry_status(env: *mut jni::JNIEnv) -> Result<RegistryStatus, String> {
    if env.is_null() {
        return Err("JNIEnv pointer is null".into());
    }

    if ((**env).PushLocalFrame)(env, 32) != jni::JNI_OK {
        return Err("Failed to push JNI local frame".into());
    }

    let result = query_registry_status_inner(env);

    ((**env).PopLocalFrame)(env, std::ptr::null_mut());

    result
}

unsafe fn query_registry_status_inner(env: *mut jni::JNIEnv) -> Result<RegistryStatus, String> {
    // 1. Locate BuiltInRegistries class
    let loader = crate::class_injector::get_game_class_loader();
    let built_in_registries_class = if !loader.is_null() {
        let cl_class = ((**env).GetObjectClass)(env, loader);
        let load_class_name = CString::new("loadClass").unwrap();
        let load_class_sig = CString::new("(Ljava/lang/String;)Ljava/lang/Class;").unwrap();
        let load_class_mid = ((**env).GetMethodID)(
            env,
            cl_class,
            load_class_name.as_ptr(),
            load_class_sig.as_ptr(),
        );
        if load_class_mid.is_null() {
            return Err("Failed to find loadClass method on ClassLoader".into());
        }
        let class_name_str =
            CString::new("net.minecraft.core.registries.BuiltInRegistries").unwrap();
        let j_name = ((**env).NewStringUTF)(env, class_name_str.as_ptr());
        let cls = ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || cls.is_null() {
            ((**env).ExceptionClear)(env);
            return Err("Failed to load BuiltInRegistries via game ClassLoader".into());
        }
        cls
    } else {
        let c_name = CString::new("net/minecraft/core/registries/BuiltInRegistries").unwrap();
        let cls = ((**env).FindClass)(env, c_name.as_ptr());
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || cls.is_null() {
            ((**env).ExceptionClear)(env);
            return Err("Failed to find BuiltInRegistries class via FindClass".into());
        }
        cls
    };

    // 2. Get static field `ITEM` (DefaultedRegistry)
    let item_field_name = CString::new("ITEM").unwrap();
    let item_field_sig = CString::new("Lnet/minecraft/core/DefaultedRegistry;").unwrap();
    let item_fid = ((**env).GetStaticFieldID)(
        env,
        built_in_registries_class,
        item_field_name.as_ptr(),
        item_field_sig.as_ptr(),
    );
    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || item_fid.is_null() {
        ((**env).ExceptionClear)(env);
        return Err("Failed to get static field ITEM from BuiltInRegistries".into());
    }

    let item_reg_obj = ((**env).GetStaticObjectField)(env, built_in_registries_class, item_fid);
    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || item_reg_obj.is_null() {
        ((**env).ExceptionClear)(env);
        return Err("BuiltInRegistries.ITEM registry object is null".into());
    }

    // 3. Inspect target class (DefaultedMappedRegistry -> superclass MappedRegistry)
    let item_class = ((**env).GetObjectClass)(env, item_reg_obj);
    let mapped_reg_class = ((**env).GetSuperclass)(env, item_class);
    let target_class = if mapped_reg_class.is_null() {
        item_class
    } else {
        mapped_reg_class
    };

    // 4. Query boolean `frozen` field
    let frozen_field_name = CString::new("frozen").unwrap();
    let frozen_field_sig = CString::new("Z").unwrap();
    let frozen_fid = ((**env).GetFieldID)(
        env,
        target_class,
        frozen_field_name.as_ptr(),
        frozen_field_sig.as_ptr(),
    );
    let is_frozen = if !frozen_fid.is_null() {
        let val = ((**env).GetBooleanField)(env, item_reg_obj, frozen_fid);
        val == jni::JNI_TRUE
    } else {
        ((**env).ExceptionClear)(env);
        // Fallback: try on item_class
        let fid2 = ((**env).GetFieldID)(
            env,
            item_class,
            frozen_field_name.as_ptr(),
            frozen_field_sig.as_ptr(),
        );
        if !fid2.is_null() {
            let val = ((**env).GetBooleanField)(env, item_reg_obj, fid2);
            val == jni::JNI_TRUE
        } else {
            ((**env).ExceptionClear)(env);
            false
        }
    };

    // 5. Query `size()` method
    let size_method_name = CString::new("size").unwrap();
    let size_method_sig = CString::new("()I").unwrap();
    let size_mid = ((**env).GetMethodID)(
        env,
        target_class,
        size_method_name.as_ptr(),
        size_method_sig.as_ptr(),
    );
    let item_count = if !size_mid.is_null() {
        ((**env).CallIntMethod)(env, item_reg_obj, size_mid)
    } else {
        ((**env).ExceptionClear)(env);
        let mid2 = ((**env).GetMethodID)(
            env,
            item_class,
            size_method_name.as_ptr(),
            size_method_sig.as_ptr(),
        );
        if !mid2.is_null() {
            ((**env).CallIntMethod)(env, item_reg_obj, mid2)
        } else {
            ((**env).ExceptionClear)(env);
            -1
        }
    };

    Ok(RegistryStatus {
        is_frozen,
        item_count,
    })
}

/// Invoked from JNI when `RedoxBridge.onRegisterContent()` is triggered right before `BuiltInRegistries.freeze()`.
///
/// # Safety
/// Valid JNI environment must be supplied.
pub unsafe fn handle_on_register_content(env: *mut jni::JNIEnv) {
    println!("[redox-mc-loader] Native RedoxBridge.onRegisterContent() invoked before BuiltInRegistries.freeze().");

    match query_registry_status(env) {
        Ok(status) => {
            println!(
                "[redox-mc-loader] BuiltInRegistries status: open for registration (frozen={}, item_count={}).",
                status.is_frozen, status.item_count
            );
            if status.is_frozen {
                eprintln!("[redox-mc-loader] WARNING: BuiltInRegistries is already frozen before freeze$original()!");
            }
        }
        Err(err) => {
            println!(
                "[redox-mc-loader] Notice: Could not inspect registry status via JNI (reason: {}). Proceeding.",
                err
            );
        }
    }

    // 1. Dispatch registration event to all loaded native mods
    crate::mod_loader::on_register_content();

    // 2. Collect registered items, blocks, and virtual resources from all mods in deterministic topological order
    let items = crate::mod_loader::collect_registered_items();
    let blocks = crate::mod_loader::collect_registered_blocks();
    let block_entities = crate::mod_loader::collect_registered_block_entities();
    crate::mod_loader::collect_registered_resources();

    // 3. Register each custom block into BuiltInRegistries.BLOCK (and associated BlockItem / BlockEntityType) via JNI
    for block in &blocks {
        if let Err(err) = crate::registry::register_block_in_game(env, block) {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed to register block '{}': {}",
                block.id, err
            );
        }
    }

    // 4. Register any stand-alone custom block entity types into BuiltInRegistries.BLOCK_ENTITY_TYPE
    for be in &block_entities {
        // Only register if not already registered as part of an entity block
        let already_registered = blocks
            .iter()
            .any(|b| b.block_entity.as_ref().map(|e| &e.id) == Some(&be.id));
        if !already_registered {
            if let Err(err) = crate::registry::register_block_entity_in_game(env, be) {
                eprintln!(
                    "[redox-mc-loader] ERROR: Failed to register block entity '{}': {}",
                    be.id, err
                );
            }
        }
    }

    // 5. Register each custom item into BuiltInRegistries.ITEM via JNI
    for item in &items {
        if let Err(err) = crate::registry::register_item_in_game(env, item) {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed to register item '{}': {}",
                item.id, err
            );
        }
    }

    // 6. Collect and register custom container menus into BuiltInRegistries.MENU
    let menus = crate::mod_loader::collect_registered_menus();
    for menu in &menus {
        if let Err(err) = crate::registry::register_menu_in_game(env, menu) {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed to register menu '{}': {}",
                menu.id, err
            );
        }
    }

    // 7. Compute and store deterministic registry content hash
    // Attached block entities are already serialized inside update_block; only feed standalone ones here to avoid double-hashing
    let standalone_block_entities: Vec<_> = block_entities
        .iter()
        .filter(|be| {
            !blocks
                .iter()
                .any(|b| b.block_entity.as_ref().map(|e| &e.id) == Some(&be.id))
        })
        .cloned()
        .collect();
    let hash = crate::registry::compute_registry_content_hash_all(
        &items,
        &blocks,
        &standalone_block_entities,
        &menus,
    );
    match &hash {
        Some(hash) => {
            crate::registry::set_registry_hash(hash.clone());
            println!(
                "[redox-mc-loader] Content registration complete: {} custom item(s), {} custom block(s), {} block entity type(s), {} menu type(s) registered. Registry Content Hash: {}",
                items.len(),
                blocks.len(),
                block_entities.len(),
                menus.len(),
                hash
            );
        }
        None => {
            crate::registry::clear_registry_hash();
            println!(
                "[redox-mc-loader] Content registration complete: no custom items, blocks, block entities, or menus registered; registry content hash is unset."
            );
        }
    }
}

/// Invoked from JNI when `RedoxBridge.onRegistriesFrozen()` is triggered immediately after `BuiltInRegistries.freeze$original()`.
///
/// # Safety
/// Valid JNI environment must be supplied.
pub unsafe fn handle_on_registries_frozen(env: *mut jni::JNIEnv) {
    println!("[redox-mc-loader] Native RedoxBridge.onRegistriesFrozen() invoked after BuiltInRegistries.freeze().");

    match query_registry_status(env) {
        Ok(status) => {
            println!(
                "[redox-mc-loader] BuiltInRegistries status: sealed (frozen={}, item_count={}).",
                status.is_frozen, status.item_count
            );
            if !status.is_frozen {
                eprintln!("[redox-mc-loader] WARNING: BuiltInRegistries is NOT frozen after freeze$original()!");
            }
        }
        Err(err) => {
            println!(
                "[redox-mc-loader] Notice: Could not inspect sealed registry status via JNI (reason: {}). Proceeding.",
                err
            );
        }
    }

    // If testmod:counter_menu is registered, exercise its lifecycle in game classloader
    let menus = crate::mod_loader::collect_registered_menus();
    if menus.iter().any(|m| m.id == "testmod:counter_menu") {
        if let Err(err) = exercise_menu_lifecycle(env, "testmod:counter_menu") {
            eprintln!(
                "[redox-mc-loader] Notice: testMenuSlotClickLifecycle: {}",
                err
            );
        }
    }
}

unsafe fn exercise_menu_lifecycle(env: *mut jni::JNIEnv, menu_id: &str) -> Result<(), String> {
    if env.is_null() {
        return Err("JNIEnv is null".into());
    }
    if ((**env).PushLocalFrame)(env, 16) != jni::JNI_OK {
        return Err("Failed to push JNI local frame".into());
    }

    let loader = crate::class_injector::get_game_class_loader();
    if loader.is_null() {
        ((**env).PopLocalFrame)(env, std::ptr::null_mut());
        return Err("Game classloader is null".into());
    }
    let cl_class = ((**env).GetObjectClass)(env, loader);
    let load_class_name = CString::new("loadClass").unwrap();
    let load_class_sig = CString::new("(Ljava/lang/String;)Ljava/lang/Class;").unwrap();
    let load_class_mid = ((**env).GetMethodID)(
        env,
        cl_class,
        load_class_name.as_ptr(),
        load_class_sig.as_ptr(),
    );
    if load_class_mid.is_null() {
        ((**env).PopLocalFrame)(env, std::ptr::null_mut());
        return Err("Failed to get loadClass method ID".into());
    }

    let class_name = CString::new("net.redoxmc.bridge.gui.GenericRedoxMenuHelper").unwrap();
    let j_name = ((**env).NewStringUTF)(env, class_name.as_ptr());
    let helper_class =
        ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || helper_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        ((**env).PopLocalFrame)(env, std::ptr::null_mut());
        return Err("Failed to load GenericRedoxMenuHelper".into());
    }

    let test_name = CString::new("testMenuSlotClickLifecycle").unwrap();
    let test_sig = CString::new("(Ljava/lang/String;)V").unwrap();
    let test_mid =
        ((**env).GetStaticMethodID)(env, helper_class, test_name.as_ptr(), test_sig.as_ptr());
    if test_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        ((**env).PopLocalFrame)(env, std::ptr::null_mut());
        return Err("Failed to find testMenuSlotClickLifecycle method".into());
    }

    let c_menu_id = CString::new(menu_id).map_err(|e| e.to_string())?;
    let j_menu_id = ((**env).NewStringUTF)(env, c_menu_id.as_ptr());
    ((**env).CallStaticVoidMethod)(env, helper_class, test_mid, j_menu_id);

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
        ((**env).ExceptionDescribe)(env);
        ((**env).ExceptionClear)(env);
        ((**env).PopLocalFrame)(env, std::ptr::null_mut());
        return Err("Exception in testMenuSlotClickLifecycle".into());
    }

    ((**env).PopLocalFrame)(env, std::ptr::null_mut());
    Ok(())
}
