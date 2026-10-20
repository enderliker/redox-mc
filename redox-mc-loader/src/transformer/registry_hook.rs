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

    // Dispatch registration event to all loaded native mods
    crate::mod_loader::on_register_content();
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
}
