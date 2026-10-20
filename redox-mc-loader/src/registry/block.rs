//! JNI block registration into Minecraft's `BuiltInRegistries.BLOCK`.

use super::hash::RegisteredBlockEntry;
use jvmti_bindings::sys::jni;
use std::ffi::CString;

/// Registers a custom mod block into the vanilla Minecraft `BuiltInRegistries.BLOCK`,
/// registers its states into `Block.BLOCK_STATE_REGISTRY`, and creates its `BlockItem`
/// by calling `net.redoxmc.bridge.block.GenericRedoxBlock.register(...)`.
///
/// # Safety
/// `env` must be a valid pointer to a live `JNIEnv` attached to the current JVM thread,
/// and the call must occur while `BuiltInRegistries` is open for registration (e.g. during `onRegisterContent`).
pub unsafe fn register_block_in_game(
    env: *mut jni::JNIEnv,
    block: &RegisteredBlockEntry,
) -> Result<(), String> {
    if env.is_null() {
        return Err("JNIEnv pointer is null".into());
    }

    if ((**env).PushLocalFrame)(env, 32) != jni::JNI_OK {
        return Err("Failed to push JNI local frame".into());
    }

    let result = register_block_in_game_inner(env, block);

    ((**env).PopLocalFrame)(env, std::ptr::null_mut());

    result
}

unsafe fn register_block_in_game_inner(
    env: *mut jni::JNIEnv,
    block: &RegisteredBlockEntry,
) -> Result<(), String> {
    let loader = crate::class_injector::get_game_class_loader();
    if loader.is_null() {
        return Err("Cannot register block: game ClassLoader is null".into());
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

    let block_class_name = CString::new("net.redoxmc.bridge.block.GenericRedoxBlock").unwrap();
    let j_name = ((**env).NewStringUTF)(env, block_class_name.as_ptr());
    let block_class =
        ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || block_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to load GenericRedoxBlock class".into());
    }

    let register_method_name = CString::new("register").unwrap();
    let register_sig =
        CString::new("(Ljava/lang/String;FFZZIZ)Lnet/redoxmc/bridge/block/GenericRedoxBlock;")
            .unwrap();
    let register_mid = ((**env).GetStaticMethodID)(
        env,
        block_class,
        register_method_name.as_ptr(),
        register_sig.as_ptr(),
    );
    if register_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to find GenericRedoxBlock.register method".into());
    }

    let c_block_id = CString::new(block.id.as_str()).unwrap();
    let j_block_id = ((**env).NewStringUTF)(env, c_block_id.as_ptr());
    let j_destroy_time = block.destroy_time as jni::jfloat;
    let j_explosion_resistance = block.explosion_resistance as jni::jfloat;
    let j_requires_tool = if block.requires_tool {
        jni::JNI_TRUE
    } else {
        jni::JNI_FALSE
    };
    let j_no_collision = if block.no_collision {
        jni::JNI_TRUE
    } else {
        jni::JNI_FALSE
    };
    let j_flags = block.behavior_flags as jni::jint;
    let j_register_item = if block.has_item {
        jni::JNI_TRUE
    } else {
        jni::JNI_FALSE
    };

    let res = ((**env).CallStaticObjectMethod)(
        env,
        block_class,
        register_mid,
        j_block_id,
        j_destroy_time as std::os::raw::c_double,
        j_explosion_resistance as std::os::raw::c_double,
        j_requires_tool as std::os::raw::c_uint,
        j_no_collision as std::os::raw::c_uint,
        j_flags,
        j_register_item as std::os::raw::c_uint,
    );

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || res.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!(
            "Exception during GenericRedoxBlock.register for '{}'",
            block.id
        ));
    }

    println!(
        "[redox-mc-loader] Registered custom block into BuiltInRegistries.BLOCK: '{}' (destroy_time={}, resistance={}, requires_tool={}, flags={})",
        block.id, block.destroy_time, block.explosion_resistance, block.requires_tool, block.behavior_flags
    );
    Ok(())
}
