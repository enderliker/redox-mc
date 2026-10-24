use super::network::drain_mod_network_outboxes;
use super::strings::clear_pending_exception;
use crate::class_injector::{
    self, define_class, define_game_bridge_classes, get_game_class_loader,
};
use jvmti_bindings::sys::jni;
use std::ffi::{CStr, CString};

/// JNI native: net.redoxmc.bridge.RedoxBridge.init()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_init(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        println!("[redox-mc-loader] Native RedoxBridge.init() invoked from Java.");
        if let Err(err) = define_game_bridge_classes(env) {
            eprintln!(
                "[redox-mc-loader] Warning during RedoxBridge.init(): {}",
                err
            );
        }
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onBootstrapEntry()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onBootstrapEntry(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        println!("[redox-mc-loader] Native RedoxBridge.onBootstrapEntry() invoked from Bootstrap.bootStrap().");
        if let Err(err) = define_game_bridge_classes(env) {
            eprintln!("[redox-mc-loader] Warning in onBootstrapEntry: {}", err);
        }

        // Discover, load native mods in deterministic order and dispatch bootstrap event
        let running_side = detect_running_side(env);
        crate::mod_loader::on_bootstrap_entry(running_side);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onRegisterContent()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onRegisterContent(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::transformer::registry_hook::handle_on_register_content(env);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onRegistriesFrozen()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onRegistriesFrozen(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::transformer::registry_hook::handle_on_registries_frozen(env);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onServerTick(long tickCount)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onServerTick(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    tick_count: jni::jlong,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::events::dispatch_server_tick(tick_count);
        drain_mod_network_outboxes(env, crate::manifest::ModSide::Server);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onClientTick()
///
/// Flushes queued outbound mod messages on the client thread.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onClientTick(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drain_mod_network_outboxes(env, crate::manifest::ModSide::Client);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.reloadMods()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_reloadMods(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jstring {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || match crate::reload::reload_mods() {
                Ok(outcome) => outcome.summary(),
                Err(err) => format!("Redox reload failed: {err}"),
            },
        ));
    let message =
        result.unwrap_or_else(|_| "Redox reload failed: native panic inside loader".to_string());
    super::strings::string_to_jstring(env, message)
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onServerChat(String playerName, String message)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string references.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onServerChat(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    player_str: jni::jstring,
    msg_str: jni::jstring,
) -> jni::jboolean {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if player_str.is_null() || msg_str.is_null() {
            return jni::JNI_TRUE;
        }

        let p_chars = ((**env).GetStringUTFChars)(env, player_str, std::ptr::null_mut());
        let m_chars = ((**env).GetStringUTFChars)(env, msg_str, std::ptr::null_mut());

        if p_chars.is_null() || m_chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            if !p_chars.is_null() {
                ((**env).ReleaseStringUTFChars)(env, player_str, p_chars);
            }
            if !m_chars.is_null() {
                ((**env).ReleaseStringUTFChars)(env, msg_str, m_chars);
            }
            return jni::JNI_TRUE;
        }

        let player_name = CStr::from_ptr(p_chars).to_string_lossy();
        let message = CStr::from_ptr(m_chars).to_string_lossy();

        let allow = crate::events::dispatch_server_chat(&player_name, &message);

        ((**env).ReleaseStringUTFChars)(env, player_str, p_chars);
        ((**env).ReleaseStringUTFChars)(env, msg_str, m_chars);

        if allow {
            jni::JNI_TRUE
        } else {
            jni::JNI_FALSE
        }
    }));

    result.unwrap_or(jni::JNI_TRUE)
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onItemUse(String itemId, int hand)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string reference.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onItemUse(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    item_str: jni::jstring,
    hand: jni::jint,
) -> jni::jint {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if item_str.is_null() {
            return 0;
        }
        let chars = ((**env).GetStringUTFChars)(env, item_str, std::ptr::null_mut());
        if chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return 0;
        }
        let item_id = CStr::from_ptr(chars).to_string_lossy();
        let res = crate::events::dispatch_item_use(&item_id, hand);
        ((**env).ReleaseStringUTFChars)(env, item_str, chars);
        res
    }));
    result.unwrap_or(0)
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onItemUseOn(String itemId, int x, int y, int z, int face, int hand)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string reference.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onItemUseOn(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    item_str: jni::jstring,
    x: jni::jint,
    y: jni::jint,
    z: jni::jint,
    face: jni::jint,
    hand: jni::jint,
) -> jni::jint {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if item_str.is_null() {
            return 0;
        }
        let chars = ((**env).GetStringUTFChars)(env, item_str, std::ptr::null_mut());
        if chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return 0;
        }
        let item_id = CStr::from_ptr(chars).to_string_lossy();
        let res = crate::events::dispatch_item_use_on(&item_id, x, y, z, face, hand);
        ((**env).ReleaseStringUTFChars)(env, item_str, chars);
        res
    }));
    result.unwrap_or(0)
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onItemFinishUsing(String itemId)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string reference.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onItemFinishUsing(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    item_str: jni::jstring,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if item_str.is_null() {
            return;
        }
        let chars = ((**env).GetStringUTFChars)(env, item_str, std::ptr::null_mut());
        if chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return;
        }
        let item_id = CStr::from_ptr(chars).to_string_lossy();
        crate::events::dispatch_item_finish_using(&item_id);
        ((**env).ReleaseStringUTFChars)(env, item_str, chars);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onBlockUseWithoutItem(String blockId, int x, int y, int z, int direction)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string reference.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onBlockUseWithoutItem(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    block_str: jni::jstring,
    x: jni::jint,
    y: jni::jint,
    z: jni::jint,
    direction: jni::jint,
) -> jni::jint {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if block_str.is_null() {
            return 0;
        }
        let chars = ((**env).GetStringUTFChars)(env, block_str, std::ptr::null_mut());
        if chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return 0;
        }
        let block_id = CStr::from_ptr(chars).to_string_lossy();
        let res = crate::events::dispatch_block_use_without_item(&block_id, x, y, z, direction);
        ((**env).ReleaseStringUTFChars)(env, block_str, chars);
        res
    }));
    result.unwrap_or(0)
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onBlockStepOn(String blockId, int x, int y, int z)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string reference.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onBlockStepOn(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    block_str: jni::jstring,
    x: jni::jint,
    y: jni::jint,
    z: jni::jint,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if block_str.is_null() {
            return;
        }
        let chars = ((**env).GetStringUTFChars)(env, block_str, std::ptr::null_mut());
        if chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return;
        }
        let block_id = CStr::from_ptr(chars).to_string_lossy();
        crate::events::dispatch_block_step_on(&block_id, x, y, z);
        ((**env).ReleaseStringUTFChars)(env, block_str, chars);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onBlockDestroy(String blockId, int x, int y, int z)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java string reference.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onBlockDestroy(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    block_str: jni::jstring,
    x: jni::jint,
    y: jni::jint,
    z: jni::jint,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if block_str.is_null() {
            return;
        }
        let chars = ((**env).GetStringUTFChars)(env, block_str, std::ptr::null_mut());
        if chars.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return;
        }
        let block_id = CStr::from_ptr(chars).to_string_lossy();
        crate::events::dispatch_block_destroy(&block_id, x, y, z);
        ((**env).ReleaseStringUTFChars)(env, block_str, chars);
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.isClassLoaderCaptured()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_isClassLoaderCaptured(
    _env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jboolean {
    if class_injector::is_class_loader_captured() {
        jni::JNI_TRUE
    } else {
        jni::JNI_FALSE
    }
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getCapturedClassLoader()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getCapturedClassLoader(
    _env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobject {
    get_game_class_loader()
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.testDefineClass(String className, byte[] classBytes)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer and valid Java object references.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_testDefineClass(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    name_str: jni::jstring,
    bytes_arr: jni::jbyteArray,
) -> jni::jboolean {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if name_str.is_null() || bytes_arr.is_null() {
            return jni::JNI_FALSE;
        }

        let loader = get_game_class_loader();
        if loader.is_null() {
            eprintln!("[redox-mc-loader] testDefineClass: loader is null!");
            return jni::JNI_FALSE;
        }

        // Convert jstring to Rust &str
        let chars = ((**env).GetStringUTFChars)(env, name_str, std::ptr::null_mut());
        if chars.is_null() {
            return jni::JNI_FALSE;
        }
        let class_name = CStr::from_ptr(chars).to_string_lossy().to_string();
        ((**env).ReleaseStringUTFChars)(env, name_str, chars);

        // Convert jbyteArray to Rust Vec<u8>
        let length = ((**env).GetArrayLength)(env, bytes_arr) as usize;
        let mut buffer = vec![0u8; length];
        ((**env).GetByteArrayRegion)(
            env,
            bytes_arr,
            0,
            length as jni::jsize,
            buffer.as_mut_ptr() as *mut jni::jbyte,
        );

        // Perform define_class
        match define_class(env, &class_name, loader, &buffer) {
            Ok(_) => jni::JNI_TRUE,
            Err(_) => {
                // Error was logged and exception cleared in define_class, JVM survives
                jni::JNI_FALSE
            }
        }
    }));

    result.unwrap_or(jni::JNI_FALSE)
}

/// Detects whether this JVM is a client or a dedicated server.
///
/// `REDOX_MC_SIDE=client|server` overrides detection for tests and launchers.
/// Otherwise the game classloader is probed for the client-only
/// `net/minecraft/client/Minecraft.class` resource without initializing classes.
///
/// # Safety
/// `env` must be a valid JNI environment pointer (or null).
pub(crate) unsafe fn detect_running_side(env: *mut jni::JNIEnv) -> crate::manifest::ModSide {
    if let Ok(value) = std::env::var("REDOX_MC_SIDE") {
        match value.to_ascii_lowercase().as_str() {
            "client" => return crate::manifest::ModSide::Client,
            "server" => return crate::manifest::ModSide::Server,
            other => eprintln!(
                "[redox-mc-loader] Ignoring invalid REDOX_MC_SIDE='{}' (expected 'client' or 'server').",
                other
            ),
        }
    }

    if env.is_null() || !class_injector::is_class_loader_captured() {
        return crate::manifest::ModSide::Server;
    }

    let loader = get_game_class_loader();
    if loader.is_null() {
        return crate::manifest::ModSide::Server;
    }

    let loader_class = ((**env).GetObjectClass)(env, loader);
    if loader_class.is_null() {
        clear_pending_exception(env);
        return crate::manifest::ModSide::Server;
    }

    let method_name = CString::new("getResource").unwrap();
    let method_sig = CString::new("(Ljava/lang/String;)Ljava/net/URL;").unwrap();
    let method_id =
        ((**env).GetMethodID)(env, loader_class, method_name.as_ptr(), method_sig.as_ptr());
    if method_id.is_null() {
        clear_pending_exception(env);
        ((**env).DeleteLocalRef)(env, loader_class as jni::jobject);
        return crate::manifest::ModSide::Server;
    }

    let resource = CString::new("net/minecraft/client/Minecraft.class").unwrap();
    let j_resource = ((**env).NewStringUTF)(env, resource.as_ptr());
    if j_resource.is_null() {
        clear_pending_exception(env);
        ((**env).DeleteLocalRef)(env, loader_class as jni::jobject);
        return crate::manifest::ModSide::Server;
    }

    let args = [jni::jvalue {
        l: j_resource as jni::jobject,
    }];
    let url = ((**env).CallObjectMethodA)(env, loader, method_id, args.as_ptr());

    ((**env).DeleteLocalRef)(env, j_resource as jni::jobject);
    ((**env).DeleteLocalRef)(env, loader_class as jni::jobject);
    if (url as jni::jobject).is_null() {
        clear_pending_exception(env);
        crate::manifest::ModSide::Server
    } else {
        ((**env).DeleteLocalRef)(env, url as jni::jobject);
        crate::manifest::ModSide::Client
    }
}
