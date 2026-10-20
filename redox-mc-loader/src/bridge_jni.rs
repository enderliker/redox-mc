use crate::class_injector::{
    self, define_class, define_game_bridge_classes, get_game_class_loader,
};
use jvmti_bindings::sys::jni;
use std::ffi::{CStr, CString};
use std::ptr;

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
