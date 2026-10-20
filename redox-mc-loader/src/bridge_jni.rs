use crate::class_injector::{
    self, define_class, define_game_bridge_classes, get_game_class_loader,
};
use jvmti_bindings::sys::jni;
use std::ffi::{CStr, CString};

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
        crate::mod_loader::on_bootstrap_entry();
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
    _env: *mut jni::JNIEnv,
    _class: jni::jclass,
    tick_count: jni::jlong,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::events::dispatch_server_tick(tick_count);
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
