//! In-game registration of block entity types into `BuiltInRegistries.BLOCK_ENTITY_TYPE`.

use super::hash::RegisteredBlockEntityEntry;
use jvmti_bindings::sys::jni;
use std::ffi::CString;

/// Registers a custom block entity type into Minecraft's `BuiltInRegistries.BLOCK_ENTITY_TYPE`
/// by calling `net.redoxmc.bridge.block.GenericRedoxEntityBlock.registerBlockEntityType(...)`.
///
/// # Safety
/// `env` must be a valid pointer to a live `JNIEnv` attached to the current JVM thread,
/// and the call must occur while `BuiltInRegistries` is open for registration (e.g. during `onRegisterContent`).
pub unsafe fn register_block_entity_in_game(
    env: *mut jni::JNIEnv,
    entry: &RegisteredBlockEntityEntry,
) -> Result<(), String> {
    if env.is_null() {
        return Err("JNIEnv pointer is null".into());
    }

    if ((**env).PushLocalFrame)(env, 32) != jni::JNI_OK {
        return Err("Failed to push JNI local frame".into());
    }

    let result = register_block_entity_in_game_inner(env, entry);

    ((**env).PopLocalFrame)(env, std::ptr::null_mut());

    result
}

unsafe fn register_block_entity_in_game_inner(
    env: *mut jni::JNIEnv,
    entry: &RegisteredBlockEntityEntry,
) -> Result<(), String> {
    let loader = crate::class_injector::get_game_class_loader();
    if loader.is_null() {
        return Err("Cannot register block entity: game ClassLoader is null".into());
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
        return Err("Failed to get loadClass method ID".into());
    }

    let class_name = CString::new("net.redoxmc.bridge.block.GenericRedoxEntityBlock").unwrap();
    let j_name = ((**env).NewStringUTF)(env, class_name.as_ptr());
    let entity_block_class =
        ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || entity_block_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to load GenericRedoxEntityBlock class".into());
    }

    let register_method_name = CString::new("registerBlockEntityType").unwrap();
    let register_sig = CString::new(
        "(Ljava/lang/String;[Ljava/lang/String;I)Lnet/minecraft/world/level/block/entity/BlockEntityType;",
    )
    .unwrap();
    let register_mid = ((**env).GetStaticMethodID)(
        env,
        entity_block_class,
        register_method_name.as_ptr(),
        register_sig.as_ptr(),
    );
    if register_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to find GenericRedoxEntityBlock.registerBlockEntityType method".into());
    }

    let c_be_id = CString::new(entry.id.as_str())
        .map_err(|e| format!("Invalid CString for '{}': {}", entry.id, e))?;
    let j_be_id = ((**env).NewStringUTF)(env, c_be_id.as_ptr());

    // Create String[] array for valid blocks
    let string_class_name = CString::new("java/lang/String")
        .map_err(|e| format!("CString error for java/lang/String: {}", e))?;
    let string_class = ((**env).FindClass)(env, string_class_name.as_ptr());
    if string_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to find java/lang/String class".into());
    }
    let j_blocks_array = ((**env).NewObjectArray)(
        env,
        entry.valid_blocks.len() as jni::jsize,
        string_class,
        std::ptr::null_mut(),
    );
    for (i, b) in entry.valid_blocks.iter().enumerate() {
        let c_b = CString::new(b.as_str())
            .map_err(|e| format!("Invalid CString for block '{}': {}", b, e))?;
        let j_b = ((**env).NewStringUTF)(env, c_b.as_ptr());
        ((**env).SetObjectArrayElement)(env, j_blocks_array, i as jni::jsize, j_b);
        ((**env).DeleteLocalRef)(env, j_b);
    }

    let j_flags = entry.behavior_flags as jni::jint;

    let res = ((**env).CallStaticObjectMethod)(
        env,
        entity_block_class,
        register_mid,
        j_be_id,
        j_blocks_array,
        j_flags,
    );

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || res.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!(
            "Exception during GenericRedoxEntityBlock.registerBlockEntityType for '{}'",
            entry.id
        ));
    }

    println!(
        "[redox-mc-loader] Registered custom block entity type into BuiltInRegistries.BLOCK_ENTITY_TYPE: '{}' (flags={}, blocks={:?})",
        entry.id, entry.behavior_flags, entry.valid_blocks
    );
    Ok(())
}
