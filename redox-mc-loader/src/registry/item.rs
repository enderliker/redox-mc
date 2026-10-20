//! JNI item registration into Minecraft's `BuiltInRegistries.ITEM`.

use super::hash::RegisteredItemEntry;
use jvmti_bindings::sys::jni;
use std::ffi::CString;

/// Registers a custom mod item into the vanilla Minecraft `BuiltInRegistries.ITEM`
/// by calling `net.redoxmc.bridge.item.GenericRustItem.register(...)`.
///
/// # Safety
/// `env` must be a valid pointer to a live `JNIEnv` attached to the current JVM thread,
/// and the call must occur while `BuiltInRegistries` is open for registration (e.g. during `onRegisterContent`).
pub unsafe fn register_item_in_game(
    env: *mut jni::JNIEnv,
    item: &RegisteredItemEntry,
) -> Result<(), String> {
    if env.is_null() {
        return Err("JNIEnv pointer is null".into());
    }

    if ((**env).PushLocalFrame)(env, 32) != jni::JNI_OK {
        return Err("Failed to push JNI local frame".into());
    }

    let result = register_item_in_game_inner(env, item);

    ((**env).PopLocalFrame)(env, std::ptr::null_mut());

    result
}

unsafe fn register_item_in_game_inner(
    env: *mut jni::JNIEnv,
    item: &RegisteredItemEntry,
) -> Result<(), String> {
    let loader = crate::class_injector::get_game_class_loader();
    if loader.is_null() {
        return Err("Cannot register item: game ClassLoader is null".into());
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

    let item_class_name = CString::new("net.redoxmc.bridge.item.GenericRustItem").unwrap();
    let j_name = ((**env).NewStringUTF)(env, item_class_name.as_ptr());
    let item_class = ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || item_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to load GenericRustItem class".into());
    }

    let register_method_name = CString::new("register").unwrap();
    let register_sig =
        CString::new("(Ljava/lang/String;IIZI)Lnet/redoxmc/bridge/item/GenericRustItem;").unwrap();
    let register_mid = ((**env).GetStaticMethodID)(
        env,
        item_class,
        register_method_name.as_ptr(),
        register_sig.as_ptr(),
    );
    if register_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to find GenericRustItem.register method".into());
    }

    let c_item_id = CString::new(item.id.as_str()).unwrap();
    let j_item_id = ((**env).NewStringUTF)(env, c_item_id.as_ptr());
    let j_max_stack = item.max_stack_size as jni::jint;
    let j_durability = item.durability.unwrap_or(0) as jni::jint;
    let j_fire = if item.fire_resistant {
        jni::JNI_TRUE
    } else {
        jni::JNI_FALSE
    };
    let j_flags = item.behavior_flags as jni::jint;

    let res = ((**env).CallStaticObjectMethod)(
        env,
        item_class,
        register_mid,
        j_item_id,
        j_max_stack,
        j_durability,
        j_fire as std::os::raw::c_uint,
        j_flags,
    );

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || res.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!(
            "Exception during GenericRustItem.register for '{}'",
            item.id
        ));
    }

    println!(
        "[redox-mc-loader] Registered custom item into BuiltInRegistries.ITEM: '{}' (max_stack={}, durability={:?}, fire_resistant={}, flags={})",
        item.id, item.max_stack_size, item.durability, item.fire_resistant, item.behavior_flags
    );
    Ok(())
}
