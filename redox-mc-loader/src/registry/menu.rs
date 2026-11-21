//! In-game registration of menu types into `BuiltInRegistries.MENU`.

use jvmti_bindings::sys::jni;
use std::ffi::CString;

/// Canonical representation of a registered container menu for in-game registration and content hashing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredMenuEntry {
    /// Namespaced identifier of the menu (e.g. "testmod:counter_menu").
    pub id: String,
    /// Number of container slots.
    pub slots: usize,
    /// Display title for the menu window.
    pub title: String,
}

impl RegisteredMenuEntry {
    pub fn new(id: impl Into<String>, slots: usize, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            slots,
            title: title.into(),
        }
    }
}

/// Registers a custom MenuType into Minecraft's `BuiltInRegistries.MENU`
/// by calling `net.redoxmc.bridge.gui.GenericRedoxMenuHelper.registerMenuType(...)`.
///
/// # Safety
/// `env` must be a valid pointer to a live `JNIEnv` attached to the current JVM thread,
/// and the call must occur while `BuiltInRegistries` is open for registration (e.g. during `onRegisterContent`).
pub unsafe fn register_menu_in_game(
    env: *mut jni::JNIEnv,
    entry: &RegisteredMenuEntry,
) -> Result<(), String> {
    if env.is_null() {
        return Err("JNIEnv pointer is null".into());
    }

    if ((**env).PushLocalFrame)(env, 32) != jni::JNI_OK {
        return Err("Failed to push JNI local frame".into());
    }

    let result = register_menu_in_game_inner(env, entry);

    ((**env).PopLocalFrame)(env, std::ptr::null_mut());

    result
}

unsafe fn register_menu_in_game_inner(
    env: *mut jni::JNIEnv,
    entry: &RegisteredMenuEntry,
) -> Result<(), String> {
    let loader = crate::class_injector::get_game_class_loader();
    if loader.is_null() {
        return Err("Cannot register menu: game ClassLoader is null".into());
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

    let class_name = CString::new("net.redoxmc.bridge.gui.GenericRedoxMenuHelper").unwrap();
    let j_name = ((**env).NewStringUTF)(env, class_name.as_ptr());
    let helper_class =
        ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || helper_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to load GenericRedoxMenuHelper class".into());
    }

    let register_method_name = CString::new("registerMenuType").unwrap();
    let register_sig =
        CString::new("(Ljava/lang/String;I)Lnet/minecraft/world/inventory/MenuType;").unwrap();

    let register_mid = ((**env).GetStaticMethodID)(
        env,
        helper_class,
        register_method_name.as_ptr(),
        register_sig.as_ptr(),
    );
    if register_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to find registerMenuType method on GenericRedoxMenuHelper".into());
    }

    let c_id = CString::new(entry.id.as_str())
        .map_err(|e| format!("Invalid menu ID '{}': {}", entry.id, e))?;
    let j_id = ((**env).NewStringUTF)(env, c_id.as_ptr());
    let j_slots = entry.slots as jni::jint;

    let res = ((**env).CallStaticObjectMethod)(env, helper_class, register_mid, j_id, j_slots);

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || res.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!(
            "Exception occurred while calling registerMenuType for '{}'",
            entry.id
        ));
    }

    println!(
        "[redox-mc-loader] Registered custom MenuType in BuiltInRegistries.MENU: '{}' (slots={})",
        entry.id, entry.slots
    );

    Ok(())
}
