use crate::class_injector::{
    self, define_class, define_game_bridge_classes, get_game_class_loader,
};
use jvmti_bindings::sys::jni;
use std::ffi::{c_void, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Explicitly registers native methods for net.redoxmc.bridge.RedoxBridge using JNI RegisterNatives.
///
/// # Safety
/// `env` must be a valid pointer to a live JNIEnv for the calling thread.
pub unsafe fn register_bridge_natives(env: *mut jni::JNIEnv) -> Result<(), String> {
    let class_name = CString::new("net/redoxmc/bridge/RedoxBridge").unwrap();
    let bridge_class = ((**env).FindClass)(env, class_name.as_ptr());
    if bridge_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Could not find class net/redoxmc/bridge/RedoxBridge".into());
    }

    let m_init = CString::new("init").unwrap();
    let sig_void = CString::new("()V").unwrap();

    let m_boot = CString::new("onBootstrapEntry").unwrap();
    let m_reg = CString::new("onRegisterContent").unwrap();
    let m_frozen = CString::new("onRegistriesFrozen").unwrap();

    let m_captured = CString::new("isClassLoaderCaptured").unwrap();
    let sig_bool = CString::new("()Z").unwrap();

    let m_get_cl = CString::new("getCapturedClassLoader").unwrap();
    let sig_get_cl = CString::new("()Ljava/lang/ClassLoader;").unwrap();

    let m_test_def = CString::new("testDefineClass").unwrap();
    let sig_test_def = CString::new("(Ljava/lang/String;[B)Z").unwrap();

    let m_tick = CString::new("onServerTick").unwrap();
    let sig_tick = CString::new("(J)V").unwrap();

    let m_chat = CString::new("onServerChat").unwrap();
    let sig_chat = CString::new("(Ljava/lang/String;Ljava/lang/String;)Z").unwrap();

    let m_item_use = CString::new("onItemUse").unwrap();
    let sig_item_use = CString::new("(Ljava/lang/String;I)I").unwrap();

    let m_item_use_on = CString::new("onItemUseOn").unwrap();
    let sig_item_use_on = CString::new("(Ljava/lang/String;IIIII)I").unwrap();

    let m_item_finish = CString::new("onItemFinishUsing").unwrap();
    let sig_item_finish = CString::new("(Ljava/lang/String;)V").unwrap();

    let m_block_use = CString::new("onBlockUseWithoutItem").unwrap();
    let sig_block_use = CString::new("(Ljava/lang/String;IIII)I").unwrap();

    let m_block_step = CString::new("onBlockStepOn").unwrap();
    let sig_block_step = CString::new("(Ljava/lang/String;III)V").unwrap();

    let m_block_destroy = CString::new("onBlockDestroy").unwrap();
    let sig_block_destroy = CString::new("(Ljava/lang/String;III)V").unwrap();

    let m_get_vresource = CString::new("getVirtualResource").unwrap();
    let sig_get_vresource =
        CString::new("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)[B").unwrap();

    let m_list_vresources = CString::new("listVirtualResources").unwrap();
    let sig_list_vresources =
        CString::new("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)[Ljava/lang/String;")
            .unwrap();

    let m_get_vnamespaces = CString::new("getVirtualNamespaces").unwrap();
    let sig_get_vnamespaces = CString::new("(Ljava/lang/String;)[Ljava/lang/String;").unwrap();

    let m_get_vroot = CString::new("getVirtualRootResource").unwrap();
    let sig_get_vroot = CString::new("(Ljava/lang/String;)[B").unwrap();

    let m_mod_payload = CString::new("onModCustomPayload").unwrap();
    let sig_mod_payload = CString::new("(Ljava/lang/String;Ljava/lang/String;[B)V").unwrap();

    let m_mod_payload_from = CString::new("onModCustomPayloadFrom").unwrap();
    let sig_mod_payload_from =
        CString::new("(Ljava/lang/String;[BLjava/lang/String;Ljava/lang/String;I)V").unwrap();

    let m_client_tick = CString::new("onClientTick").unwrap();

    let m_get_hash = CString::new("getRegistryHash").unwrap();
    let sig_get_hash = CString::new("()Ljava/lang/String;").unwrap();

    let m_get_mod_ids = CString::new("getLoadedModIds").unwrap();
    let sig_get_mod_ids = CString::new("()[Ljava/lang/String;").unwrap();

    let m_get_mod_versions = CString::new("getLoadedModVersions").unwrap();
    let sig_get_mod_versions = CString::new("()[Ljava/lang/String;").unwrap();

    let m_get_manifest = CString::new("getHandshakeManifest").unwrap();
    let sig_get_manifest = CString::new("()[Ljava/lang/String;").unwrap();

    let m_val_handshake = CString::new("validateClientHandshake").unwrap();
    let sig_val_handshake = CString::new(
        "(Ljava/lang/String;[Ljava/lang/String;[Ljava/lang/String;)Ljava/lang/String;",
    )
    .unwrap();

    let methods = [
        jni::JNINativeMethod {
            name: m_init.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_init as *mut _,
        },
        jni::JNINativeMethod {
            name: m_boot.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBootstrapEntry as *mut _,
        },
        jni::JNINativeMethod {
            name: m_reg.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onRegisterContent as *mut _,
        },
        jni::JNINativeMethod {
            name: m_frozen.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onRegistriesFrozen as *mut _,
        },
        jni::JNINativeMethod {
            name: m_tick.as_ptr() as *mut _,
            signature: sig_tick.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onServerTick as *mut _,
        },
        jni::JNINativeMethod {
            name: m_chat.as_ptr() as *mut _,
            signature: sig_chat.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onServerChat as *mut _,
        },
        jni::JNINativeMethod {
            name: m_item_use.as_ptr() as *mut _,
            signature: sig_item_use.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onItemUse as *mut _,
        },
        jni::JNINativeMethod {
            name: m_item_use_on.as_ptr() as *mut _,
            signature: sig_item_use_on.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onItemUseOn as *mut _,
        },
        jni::JNINativeMethod {
            name: m_item_finish.as_ptr() as *mut _,
            signature: sig_item_finish.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onItemFinishUsing as *mut _,
        },
        jni::JNINativeMethod {
            name: m_block_use.as_ptr() as *mut _,
            signature: sig_block_use.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBlockUseWithoutItem as *mut _,
        },
        jni::JNINativeMethod {
            name: m_block_step.as_ptr() as *mut _,
            signature: sig_block_step.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBlockStepOn as *mut _,
        },
        jni::JNINativeMethod {
            name: m_block_destroy.as_ptr() as *mut _,
            signature: sig_block_destroy.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBlockDestroy as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_vresource.as_ptr() as *mut _,
            signature: sig_get_vresource.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getVirtualResource as *mut _,
        },
        jni::JNINativeMethod {
            name: m_list_vresources.as_ptr() as *mut _,
            signature: sig_list_vresources.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_listVirtualResources as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_vnamespaces.as_ptr() as *mut _,
            signature: sig_get_vnamespaces.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getVirtualNamespaces as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_vroot.as_ptr() as *mut _,
            signature: sig_get_vroot.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getVirtualRootResource as *mut _,
        },
        jni::JNINativeMethod {
            name: m_mod_payload.as_ptr() as *mut _,
            signature: sig_mod_payload.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayload as *mut _,
        },
        jni::JNINativeMethod {
            name: m_mod_payload_from.as_ptr() as *mut _,
            signature: sig_mod_payload_from.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayloadFrom as *mut _,
        },
        jni::JNINativeMethod {
            name: m_client_tick.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onClientTick as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_hash.as_ptr() as *mut _,
            signature: sig_get_hash.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getRegistryHash as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_mod_ids.as_ptr() as *mut _,
            signature: sig_get_mod_ids.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getLoadedModIds as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_mod_versions.as_ptr() as *mut _,
            signature: sig_get_mod_versions.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getLoadedModVersions as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_manifest.as_ptr() as *mut _,
            signature: sig_get_manifest.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getHandshakeManifest as *mut _,
        },
        jni::JNINativeMethod {
            name: m_val_handshake.as_ptr() as *mut _,
            signature: sig_val_handshake.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_validateClientHandshake as *mut _,
        },
        jni::JNINativeMethod {
            name: m_captured.as_ptr() as *mut _,
            signature: sig_bool.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_isClassLoaderCaptured as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_cl.as_ptr() as *mut _,
            signature: sig_get_cl.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getCapturedClassLoader as *mut _,
        },
        jni::JNINativeMethod {
            name: m_test_def.as_ptr() as *mut _,
            signature: sig_test_def.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_testDefineClass as *mut _,
        },
    ];

    let res = ((**env).RegisterNatives)(
        env,
        bridge_class,
        methods.as_ptr(),
        methods.len() as jni::jint,
    );

    if res != jni::JNI_OK {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!("RegisterNatives failed with code {}", res));
    }

    println!("[redox-mc-loader] Successfully bound native methods to net.redoxmc.bridge.RedoxBridge via RegisterNatives.");
    Ok(())
}

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

/// Converts a JNI jstring to an owned Rust String.
unsafe fn jstring_to_string(env: *mut jni::JNIEnv, j_str: jni::jstring) -> String {
    if j_str.is_null() {
        return String::new();
    }
    let chars = ((**env).GetStringUTFChars)(env, j_str, ptr::null_mut());
    if chars.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(chars).to_string_lossy().to_string();
    ((**env).ReleaseStringUTFChars)(env, j_str, chars);
    s
}

/// Converts a Rust byte slice into a JNI jbyteArray.
unsafe fn bytes_to_jbyte_array(env: *mut jni::JNIEnv, data: &[u8]) -> jni::jbyteArray {
    let arr = ((**env).NewByteArray)(env, data.len() as jni::jsize);
    if arr.is_null() {
        return ptr::null_mut();
    }
    ((**env).SetByteArrayRegion)(
        env,
        arr,
        0,
        data.len() as jni::jsize,
        data.as_ptr() as *const jni::jbyte,
    );
    arr
}

/// Converts a slice of Rust Strings into a JNI jobjectArray of java/lang/String.
unsafe fn string_vec_to_jobject_array(
    env: *mut jni::JNIEnv,
    strings: &[String],
) -> jni::jobjectArray {
    let string_class_name = CString::new("java/lang/String").unwrap();
    let string_class = ((**env).FindClass)(env, string_class_name.as_ptr());
    if string_class.is_null() {
        return ptr::null_mut();
    }

    let arr = ((**env).NewObjectArray)(
        env,
        strings.len() as jni::jsize,
        string_class,
        ptr::null_mut(),
    );
    if arr.is_null() {
        ((**env).DeleteLocalRef)(env, string_class as jni::jobject);
        return ptr::null_mut();
    }

    for (i, s) in strings.iter().enumerate() {
        let c_str = CString::new(s.as_str()).unwrap_or_default();
        let j_str = ((**env).NewStringUTF)(env, c_str.as_ptr());
        if !j_str.is_null() {
            ((**env).SetObjectArrayElement)(env, arr, i as jni::jsize, j_str as jni::jobject);
            ((**env).DeleteLocalRef)(env, j_str as jni::jobject);
        }
    }

    ((**env).DeleteLocalRef)(env, string_class as jni::jobject);
    arr
}

/// Converts a Rust `String` into a JNI string, returning null if the string
/// contains an interior NUL byte.
unsafe fn string_to_jstring(env: *mut jni::JNIEnv, value: String) -> jni::jstring {
    let c_value = CString::new(value).unwrap_or_default();
    ((**env).NewStringUTF)(env, c_value.as_ptr())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getVirtualResource(String type, String namespace, String path)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getVirtualResource(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_type: jni::jstring,
    j_namespace: jni::jstring,
    j_path: jni::jstring,
) -> jni::jbyteArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_type.is_null() || j_namespace.is_null() || j_path.is_null() {
            return ptr::null_mut();
        }

        let type_str = jstring_to_string(env, j_type);
        let ns_str = jstring_to_string(env, j_namespace);
        let path_str = jstring_to_string(env, j_path);

        if let Some(data) =
            crate::registry::resource::get_virtual_resource(&type_str, &ns_str, &path_str)
        {
            bytes_to_jbyte_array(env, &data)
        } else {
            ptr::null_mut()
        }
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.listVirtualResources(String type, String namespace, String directory)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_listVirtualResources(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_type: jni::jstring,
    j_namespace: jni::jstring,
    j_dir: jni::jstring,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_type.is_null() || j_namespace.is_null() || j_dir.is_null() {
            return ptr::null_mut();
        }

        let type_str = jstring_to_string(env, j_type);
        let ns_str = jstring_to_string(env, j_namespace);
        let dir_str = jstring_to_string(env, j_dir);

        let list = crate::registry::resource::list_virtual_resources(&type_str, &ns_str, &dir_str);
        string_vec_to_jobject_array(env, &list)
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getVirtualNamespaces(String type)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getVirtualNamespaces(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_type: jni::jstring,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_type.is_null() {
            return ptr::null_mut();
        }

        let type_str = jstring_to_string(env, j_type);
        let list = crate::registry::resource::get_virtual_namespaces(&type_str);
        string_vec_to_jobject_array(env, &list)
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getVirtualRootResource(String path)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getVirtualRootResource(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_path: jni::jstring,
) -> jni::jbyteArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_path.is_null() {
            return ptr::null_mut();
        }

        let path_str = jstring_to_string(env, j_path);
        if let Some(data) = crate::registry::resource::get_virtual_root_resource(&path_str) {
            bytes_to_jbyte_array(env, &data)
        } else {
            ptr::null_mut()
        }
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onModCustomPayload(String namespace, String path, byte[] data)
///
/// Legacy compatibility entry point that does not carry sender identity.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayload(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_namespace: jni::jstring,
    j_path: jni::jstring,
    j_data: jni::jbyteArray,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if env.is_null() || j_namespace.is_null() || j_path.is_null() || j_data.is_null() {
            return;
        }

        let namespace = jstring_to_string(env, j_namespace);
        let path = jstring_to_string(env, j_path);
        let channel = format!("{}:{}", namespace, path);

        let length = ((**env).GetArrayLength)(env, j_data) as usize;
        let elements = ((**env).GetByteArrayElements)(env, j_data, ptr::null_mut());
        if !elements.is_null() {
            let slice: &[u8] = std::slice::from_raw_parts(elements as *const u8, length);
            let side = crate::mod_loader::running_side();
            crate::mod_loader::dispatch_network_payload(&channel, slice, "", "", side);
            ((**env).ReleaseByteArrayElements)(env, j_data, elements, jni::JNI_ABORT);
            drain_mod_network_outboxes(env, side);
        }
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onModCustomPayloadFrom(
///     String channel, byte[] data, String senderName, String senderUuid, int side)
///
/// Dispatches an inbound mod payload with full sender identity and side context.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayloadFrom(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_channel: jni::jstring,
    j_data: jni::jbyteArray,
    j_sender_name: jni::jstring,
    j_sender_uuid: jni::jstring,
    j_side: jni::jint,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if env.is_null() || j_channel.is_null() || j_data.is_null() {
            return;
        }

        let channel = jstring_to_string(env, j_channel);
        let sender_name = jstring_to_string(env, j_sender_name);
        let sender_uuid = jstring_to_string(env, j_sender_uuid);
        let side = crate::network::side_from_abi(j_side as u32)
            .unwrap_or_else(crate::mod_loader::running_side);

        let length = ((**env).GetArrayLength)(env, j_data) as usize;
        let elements = ((**env).GetByteArrayElements)(env, j_data, ptr::null_mut());
        if elements.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return;
        }

        {
            let slice: &[u8] = std::slice::from_raw_parts(elements as *const u8, length);
            crate::mod_loader::dispatch_network_payload(
                &channel,
                slice,
                &sender_name,
                &sender_uuid,
                side,
            );
        }
        ((**env).ReleaseByteArrayElements)(env, j_data, elements, jni::JNI_ABORT);

        // Low-latency reply path: flush messages queued by the receive callback.
        drain_mod_network_outboxes(env, side);
    }));
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

unsafe fn clear_pending_exception(env: *mut jni::JNIEnv) {
    if !env.is_null() && ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
        ((**env).ExceptionClear)(env);
    }
}

/// Cached method ID for `ModNetworkBridge.sendModPayload(...)`.
static SEND_MOD_PAYLOAD_METHOD: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

struct NetworkDrainContext {
    env: *mut jni::JNIEnv,
    class: jni::jclass,
    method: jni::jmethodID,
    origin_side: crate::manifest::ModSide,
    sent: usize,
    failed: usize,
}

unsafe fn get_send_mod_payload_method(env: *mut jni::JNIEnv, class: jni::jclass) -> jni::jmethodID {
    let cached = SEND_MOD_PAYLOAD_METHOD.load(Ordering::Acquire);
    if !cached.is_null() {
        return cached as jni::jmethodID;
    }

    let name = CString::new("sendModPayload").unwrap();
    let sig =
        CString::new("(Ljava/lang/String;ILjava/lang/String;Ljava/lang/String;[BI)Z").unwrap();
    let method = ((**env).GetStaticMethodID)(env, class, name.as_ptr(), sig.as_ptr());
    if method.is_null() {
        clear_pending_exception(env);
        return ptr::null_mut();
    }

    match SEND_MOD_PAYLOAD_METHOD.compare_exchange(
        ptr::null_mut(),
        method,
        Ordering::Release,
        Ordering::Relaxed,
    ) {
        Ok(_) => method,
        Err(existing) => existing as jni::jmethodID,
    }
}

/// Drains every loaded mod's outbound network queue and forwards each message
/// to `ModNetworkBridge.sendModPayload(...)`.
///
/// `origin_side` tells the Java bridge whether the message must travel
/// server-to-client or client-to-server (important in integrated servers).
///
/// # Safety
/// `env` must be a valid JNI environment pointer.
pub(crate) unsafe fn drain_mod_network_outboxes(
    env: *mut jni::JNIEnv,
    origin_side: crate::manifest::ModSide,
) {
    if env.is_null() {
        return;
    }

    let class = class_injector::get_mod_network_bridge_class();
    if class.is_null() {
        return;
    }

    let method = get_send_mod_payload_method(env, class);
    if method.is_null() {
        return;
    }

    let callbacks = crate::mod_loader::network_drain_callbacks();
    let abi_side = crate::network::side_to_abi(origin_side);
    for (id, drain) in callbacks {
        let mut context = NetworkDrainContext {
            env,
            class,
            method,
            origin_side,
            sent: 0,
            failed: 0,
        };

        let status = drain(
            abi_side,
            network_outbound_visitor,
            (&mut context as *mut NetworkDrainContext) as *mut c_void,
        );

        if let Err(safety_err) = crate::safety::check_ffi_status(&id, status) {
            eprintln!(
                "[redox-mc-loader] ERROR: Mod '{}' network outbox drain failed: {}",
                id, safety_err
            );
        }
        if context.sent > 0 {
            println!(
                "[redox-mc-loader] Mod '{}' sent {} outbound network message(s) from {:?}.",
                id, context.sent, origin_side
            );
        }
        if context.failed > 0 {
            eprintln!(
                "[redox-mc-loader] Mod '{}' failed to deliver {} outbound network message(s).",
                id, context.failed
            );
        }
    }
}

unsafe extern "C" fn network_outbound_visitor(
    user_data: *mut c_void,
    channel: redox_mc_abi::types::FfiStr,
    target_kind: u32,
    target_name: redox_mc_abi::types::FfiStr,
    target_uuid: redox_mc_abi::types::FfiStr,
    data: redox_mc_abi::types::FfiSlice<u8>,
) -> redox_mc_abi::types::FfiStatus {
    use redox_mc_abi::types::FfiStatus;

    if user_data.is_null() {
        return FfiStatus::NullPointer;
    }
    let context = &mut *(user_data as *mut NetworkDrainContext);

    let Ok(channel) = channel.as_str() else {
        context.failed += 1;
        return FfiStatus::InvalidArgument;
    };
    if channel.is_empty() {
        context.failed += 1;
        return FfiStatus::InvalidArgument;
    }

    let target_name = target_name.as_str().unwrap_or("");
    let target_uuid = target_uuid.as_str().unwrap_or("");
    let data = data.as_slice();

    let channel_c = match CString::new(channel) {
        Ok(value) => value,
        Err(_) => {
            context.failed += 1;
            return FfiStatus::InvalidArgument;
        }
    };
    let target_name_c = CString::new(target_name).unwrap_or_default();
    let target_uuid_c = CString::new(target_uuid).unwrap_or_default();

    let env = context.env;
    let j_channel = ((**env).NewStringUTF)(env, channel_c.as_ptr());
    let j_target_name = ((**env).NewStringUTF)(env, target_name_c.as_ptr());
    let j_target_uuid = ((**env).NewStringUTF)(env, target_uuid_c.as_ptr());
    let j_data = bytes_to_jbyte_array(env, data);

    if j_channel.is_null() || j_target_name.is_null() || j_target_uuid.is_null() || j_data.is_null()
    {
        clear_pending_exception(env);
        delete_local_ref(env, j_channel as jni::jobject);
        delete_local_ref(env, j_target_name as jni::jobject);
        delete_local_ref(env, j_target_uuid as jni::jobject);
        delete_local_ref(env, j_data as jni::jobject);
        context.failed += 1;
        return FfiStatus::GenericError;
    }

    let args = [
        jni::jvalue {
            l: j_channel as jni::jobject,
        },
        jni::jvalue {
            i: target_kind as jni::jint,
        },
        jni::jvalue {
            l: j_target_name as jni::jobject,
        },
        jni::jvalue {
            l: j_target_uuid as jni::jobject,
        },
        jni::jvalue {
            l: j_data as jni::jobject,
        },
        jni::jvalue {
            i: crate::network::side_to_abi(context.origin_side) as jni::jint,
        },
    ];

    let accepted =
        ((**env).CallStaticBooleanMethodA)(env, context.class, context.method, args.as_ptr());

    delete_local_ref(env, j_channel as jni::jobject);
    delete_local_ref(env, j_target_name as jni::jobject);
    delete_local_ref(env, j_target_uuid as jni::jobject);
    delete_local_ref(env, j_data as jni::jobject);

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
        clear_pending_exception(env);
        context.failed += 1;
        return FfiStatus::Success;
    }

    if accepted == jni::JNI_TRUE {
        context.sent += 1;
    } else {
        context.failed += 1;
    }

    FfiStatus::Success
}

unsafe fn delete_local_ref(env: *mut jni::JNIEnv, reference: jni::jobject) {
    if !reference.is_null() {
        ((**env).DeleteLocalRef)(env, reference);
    }
}

/// Converts a JNI jobjectArray of java/lang/String to a Rust Vec<String>.
unsafe fn jobject_array_to_string_vec(
    env: *mut jni::JNIEnv,
    arr: jni::jobjectArray,
) -> Vec<String> {
    if arr.is_null() {
        return Vec::new();
    }
    let len = ((**env).GetArrayLength)(env, arr as jni::jarray) as usize;
    let mut vec = Vec::with_capacity(len);
    for i in 0..len {
        let elem = ((**env).GetObjectArrayElement)(env, arr, i as jni::jsize) as jni::jstring;
        if !elem.is_null() {
            vec.push(jstring_to_string(env, elem));
            ((**env).DeleteLocalRef)(env, elem as jni::jobject);
        } else {
            vec.push(String::new());
        }
    }
    vec
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getRegistryHash()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getRegistryHash(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let hash = crate::registry::get_registry_hash().unwrap_or_default();
        let c_str = CString::new(hash).unwrap_or_default();
        ((**env).NewStringUTF)(env, c_str.as_ptr())
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getLoadedModIds()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getLoadedModIds(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mods = crate::mod_loader::get_handshake_mods_info();
        let ids: Vec<String> = mods.into_iter().map(|(id, _)| id).collect();
        string_vec_to_jobject_array(env, &ids)
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getLoadedModVersions()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getLoadedModVersions(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mods = crate::mod_loader::get_handshake_mods_info();
        let versions: Vec<String> = mods.into_iter().map(|(_, ver)| ver).collect();
        string_vec_to_jobject_array(env, &versions)
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getHandshakeManifest()
///
/// Returns a single consistent snapshot shaped as `[hash, id0, version0, ...]`.
/// The mod list is filtered to `side = "both"` mods, which are the only ones the
/// remote side must also have installed.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getHandshakeManifest(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let hash = crate::registry::get_registry_hash().unwrap_or_default();
        let mods = crate::mod_loader::get_handshake_mods_info();
        let mut manifest = Vec::with_capacity(1 + mods.len() * 2);
        manifest.push(hash);
        for (id, version) in mods {
            manifest.push(id);
            manifest.push(version);
        }
        string_vec_to_jobject_array(env, &manifest)
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.validateClientHandshake(...)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_validateClientHandshake(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_client_hash: jni::jstring,
    j_client_mod_ids: jni::jobjectArray,
    j_client_mod_versions: jni::jobjectArray,
) -> jni::jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let client_hash = jstring_to_string(env, j_client_hash);
        let client_mod_ids = jobject_array_to_string_vec(env, j_client_mod_ids);
        let client_mod_versions = jobject_array_to_string_vec(env, j_client_mod_versions);
        if client_mod_ids.len() != client_mod_versions.len() {
            return string_to_jstring(
                env,
                "Malformed client handshake: mod id and version arrays differ in length."
                    .to_string(),
            );
        }

        let client_mods: Vec<crate::network::handshake::HandshakeModInfo> = client_mod_ids
            .into_iter()
            .zip(client_mod_versions)
            .map(|(id, version)| crate::network::handshake::HandshakeModInfo { id, version })
            .collect();

        let server_hash = crate::registry::get_registry_hash().unwrap_or_default();
        let server_mods: Vec<crate::network::handshake::HandshakeModInfo> =
            crate::mod_loader::get_handshake_mods_info()
                .into_iter()
                .map(|(id, version)| crate::network::handshake::HandshakeModInfo { id, version })
                .collect();

        match crate::network::handshake::validate_client_handshake(
            &server_hash,
            &server_mods,
            &client_hash,
            &client_mods,
        ) {
            Ok(()) => ptr::null_mut(),
            Err(reason) => string_to_jstring(env, reason),
        }
    }));
    result.unwrap_or(ptr::null_mut())
}
