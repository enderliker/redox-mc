use crate::class_injector::{
    self, define_class, define_game_bridge_classes, get_game_class_loader,
};
use jvmti_bindings::sys::jni;
use std::ffi::{CStr, CString};

/// Explicitly registers native methods for net.redoxmc.bridge.RedoxBridge using JNI RegisterNatives.
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

    let m_captured = CString::new("isClassLoaderCaptured").unwrap();
    let sig_bool = CString::new("()Z").unwrap();

    let m_get_cl = CString::new("getCapturedClassLoader").unwrap();
    let sig_get_cl = CString::new("()Ljava/lang/ClassLoader;").unwrap();

    let m_test_def = CString::new("testDefineClass").unwrap();
    let sig_test_def = CString::new("(Ljava/lang/String;[B)Z").unwrap();

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
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_init(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    println!("[redox-mc-loader] Native RedoxBridge.init() invoked from Java.");
    if let Err(err) = define_game_bridge_classes(env) {
        eprintln!(
            "[redox-mc-loader] Warning during RedoxBridge.init(): {}",
            err
        );
    }
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onBootstrapEntry()
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onBootstrapEntry(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    println!("[redox-mc-loader] Native RedoxBridge.onBootstrapEntry() invoked from Bootstrap.bootStrap().");
    if let Err(err) = define_game_bridge_classes(env) {
        eprintln!("[redox-mc-loader] Warning in onBootstrapEntry: {}", err);
    }

    // Discover, load native mods in deterministic order and dispatch bootstrap event
    crate::mod_loader::on_bootstrap_entry();
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onRegisterContent()
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onRegisterContent(
    _env: *mut jni::JNIEnv,
    _class: jni::jclass,
) {
    println!("[redox-mc-loader] Native RedoxBridge.onRegisterContent() invoked before BuiltInRegistries.freeze().");
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.isClassLoaderCaptured()
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
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getCapturedClassLoader(
    _env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobject {
    get_game_class_loader()
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.testDefineClass(String className, byte[] classBytes)
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_testDefineClass(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    name_str: jni::jstring,
    bytes_arr: jni::jbyteArray,
) -> jni::jboolean {
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
}
