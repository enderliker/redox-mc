use jvmti_bindings::sys::jni;
use std::ffi::CString;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

// Global reference to the game's ClassLoader (captured from the first net/minecraft/ class)
static GAME_CLASS_LOADER: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());
static FIRST_CAPTURED_CLASS: Mutex<Option<String>> = Mutex::new(None);
static GAME_CLASSES_DEFINED: AtomicBool = AtomicBool::new(false);

// Embedded bytecode of bridge-game classes, compiled by build.rs
pub const GAME_BRIDGE_BASE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustmc-bridge/build/game-classes/net/rustmc/bridge/game/RustGameBridgeBase.class"
));

pub const GAME_BRIDGE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustmc-bridge/build/game-classes/net/rustmc/bridge/game/RustGameBridge.class"
));

/// Called inside ClassFileLoadHook when a class is being loaded.
/// If the class name starts with the game prefix and no loader has been captured yet,
/// creates and stores a JNI GlobalReference to the loader.
/// Queries the version adapter's hook table and logs detected hook targets.
/// NOTE: JNI DefineClass is NOT called here; only the reference is captured.
pub unsafe fn on_class_file_load(
    raw_env: *mut jni::JNIEnv,
    loader: jni::jobject,
    class_name: &str,
) {
    if !class_name.starts_with("net/minecraft/") || class_name.starts_with("net/minecraft/bundler/") {
        return;
    }

    // 1. Capture the game classloader reference on the first game class
    if !loader.is_null() && GAME_CLASS_LOADER.load(Ordering::Acquire).is_null() {
        let global_ref = unsafe { ((**raw_env).NewGlobalRef)(raw_env, loader) };
        if !global_ref.is_null() {
            match GAME_CLASS_LOADER.compare_exchange(
                ptr::null_mut(),
                global_ref as *mut std::ffi::c_void,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let mut first_name = FIRST_CAPTURED_CLASS.lock().unwrap();
                    *first_name = Some(class_name.to_string());
                    println!(
                        "[rustmc-loader] Captured game classloader from first 'net/minecraft/' class: '{}'",
                        class_name
                    );
                }
                Err(_) => {
                    unsafe { ((**raw_env).DeleteGlobalRef)(raw_env, global_ref) };
                }
            }
        }
    }
}

/// Returns true if the game ClassLoader has been captured.
pub fn is_class_loader_captured() -> bool {
    !GAME_CLASS_LOADER.load(Ordering::Acquire).is_null()
}

/// Returns the captured game ClassLoader as a JNI jobject.
pub fn get_game_class_loader() -> jni::jobject {
    GAME_CLASS_LOADER.load(Ordering::Acquire) as jni::jobject
}

/// Returns the name of the first class that triggered classloader capture.
pub fn get_first_captured_class() -> Option<String> {
    FIRST_CAPTURED_CLASS.lock().unwrap().clone()
}

/// Defines a class into the given classloader using JNI DefineClass.
/// If DefineClass fails, the error is logged with the class name and the pending
/// Java exception is cleared, preventing a JVM crash.
pub unsafe fn define_class(
    env: *mut jni::JNIEnv,
    name: &str,
    loader: jni::jobject,
    class_bytes: &[u8],
) -> Result<jni::jclass, String> {
    let c_name = CString::new(name).map_err(|e| format!("Invalid class name '{}': {}", name, e))?;

    // Clear any pre-existing exception
    if unsafe { ((**env).ExceptionCheck)(env) } == jni::JNI_TRUE {
        unsafe { ((**env).ExceptionClear)(env) };
    }

    let defined_class = unsafe {
        ((**env).DefineClass)(
            env,
            c_name.as_ptr(),
            loader,
            class_bytes.as_ptr() as *const jni::jbyte,
            class_bytes.len() as jni::jsize,
        )
    };

    // Check if an exception occurred during DefineClass
    if unsafe { ((**env).ExceptionCheck)(env) } == jni::JNI_TRUE {
        eprintln!(
            "[rustmc-loader] ERROR: Failed to define class '{}' in game classloader.",
            name
        );
        unsafe {
            // Print stacktrace for diagnostics and CLEAR the exception so JVM does not crash
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!("Failed to define class '{}' (exception cleared)", name));
    }

    if defined_class.is_null() {
        eprintln!(
            "[rustmc-loader] ERROR: DefineClass returned null for class '{}' without throwing.",
            name
        );
        return Err(format!("DefineClass returned null for '{}'", name));
    }

    Ok(defined_class)
}

/// Defines the bridge-game classes into the captured classloader,
/// ensuring superclasses are defined before subclasses.
pub unsafe fn define_game_bridge_classes(env: *mut jni::JNIEnv) -> Result<(), String> {
    if GAME_CLASSES_DEFINED.load(Ordering::Acquire) {
        return Ok(());
    }

    let loader = get_game_class_loader();
    if loader.is_null() {
        return Err("Cannot define bridge-game classes: game classloader has not been captured yet!".into());
    }

    println!("[rustmc-loader] Defining bridge-game classes in game classloader (superclasses first)...");

    // 1. Superclass first: RustGameBridgeBase
    let base_name = "net/rustmc/bridge/game/RustGameBridgeBase";
    define_class(env, base_name, loader, GAME_BRIDGE_BASE_BYTES)?;
    println!("[rustmc-loader] Defined superclass: {}", base_name);

    // 2. Subclass second: RustGameBridge
    let child_name = "net/rustmc/bridge/game/RustGameBridge";
    define_class(env, child_name, loader, GAME_BRIDGE_BYTES)?;
    println!("[rustmc-loader] Defined child class: {}", child_name);

    // 3. Real Minecraft SavedData Subclass: TestMinecraftSavedData
    let item_name = "net/rustmc/bridge/game/TestMinecraftSavedData";
    define_class(env, item_name, loader, TEST_MINECRAFT_SAVEDDATA_BYTES)?;
    println!("[rustmc-loader] Defined real Minecraft subclass: {} (extends net.minecraft.world.level.saveddata.SavedData)", item_name);

    GAME_CLASSES_DEFINED.store(true, Ordering::Release);
    println!("[rustmc-loader] All bridge-game classes defined successfully.");
    Ok(())
}

pub const TEST_MINECRAFT_SAVEDDATA_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustmc-bridge/build/game-classes/net/rustmc/bridge/game/TestMinecraftSavedData.class"
));

/// Verifies that a real Minecraft subclass was defined, linked, and can be instantiated
/// in the captured game classloader without NoClassDefFoundError.
pub unsafe fn verify_real_minecraft_subclass(env: *mut jni::JNIEnv) -> Result<String, String> {
    define_game_bridge_classes(env)?;

    let loader = get_game_class_loader();
    if loader.is_null() {
        return Err("Game classloader is null".into());
    }

    // Call loadClass on the game classloader to get the Class object for TestMinecraftSavedData
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

    let class_name_str = CString::new("net.rustmc.bridge.game.TestMinecraftSavedData").unwrap();
    let j_name = ((**env).NewStringUTF)(env, class_name_str.as_ptr());
    let item_class = ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || item_class.is_null() {
        eprintln!("[rustmc-loader] ERROR: Exception while loading TestMinecraftSavedData via game ClassLoader!");
        ((**env).ExceptionDescribe)(env);
        ((**env).ExceptionClear)(env);
        return Err("Failed to load TestMinecraftSavedData".into());
    }

    // Call static method: TestMinecraftSavedData.getTestStatus()
    let status_name = CString::new("getTestStatus").unwrap();
    let status_sig = CString::new("()Ljava/lang/String;").unwrap();
    let status_mid = ((**env).GetStaticMethodID)(
        env,
        item_class,
        status_name.as_ptr(),
        status_sig.as_ptr(),
    );
    if status_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to get getTestStatus method ID".into());
    }

    let j_res = ((**env).CallStaticObjectMethod)(env, item_class, status_mid) as jni::jstring;
    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || j_res.is_null() {
        eprintln!("[rustmc-loader] ERROR: Exception during TestMinecraftSavedData.getTestStatus()!");
        ((**env).ExceptionDescribe)(env);
        ((**env).ExceptionClear)(env);
        return Err("Failed to execute getTestStatus".into());
    }

    let c_chars = ((**env).GetStringUTFChars)(env, j_res, std::ptr::null_mut());
    let res_str = std::ffi::CStr::from_ptr(c_chars).to_string_lossy().to_string();
    ((**env).ReleaseStringUTFChars)(env, j_res, c_chars);

    println!("[rustmc-loader] [VERIFY] {}", res_str);
    Ok(res_str)
}
