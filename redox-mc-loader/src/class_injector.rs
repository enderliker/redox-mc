use jvmti_bindings::sys::jni;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;

// Global reference to the game's ClassLoader (captured from the first net/minecraft/ class)
static GAME_CLASS_LOADER: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());
static FIRST_CAPTURED_CLASS: Mutex<Option<String>> = Mutex::new(None);
static GAME_CLASSES_DEFINED: AtomicBool = AtomicBool::new(false);

// Embedded bytecode of bridge-game classes, compiled by build.rs
pub const GAME_BRIDGE_BASE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/game/RedoxGameBridgeBase.class"
));

pub const GAME_BRIDGE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/game/RedoxGameBridge.class"
));

/// Called inside ClassFileLoadHook when a class is being loaded.
/// If the class name starts with the game prefix and no loader has been captured yet,
/// creates and stores a JNI GlobalReference to the loader.
/// Queries the version adapter's hook table and logs detected hook targets.
/// NOTE: JNI DefineClass is NOT called here; only the reference is captured.
///
/// # Safety
/// `raw_env` must be a valid pointer to a live JNIEnv for the calling thread,
/// and `loader` must be a valid JNI jobject reference (or null).
pub unsafe fn on_class_file_load(
    raw_env: *mut jni::JNIEnv,
    loader: jni::jobject,
    class_name: &str,
) {
    let adapter = crate::adapter::active_adapter();
    let prefix = adapter.game_package_prefix();

    if !class_name.starts_with(prefix) || class_name.starts_with("net/minecraft/bundler/") {
        return;
    }

    // 1. Capture the game classloader reference on the first game class
    if !loader.is_null() && GAME_CLASS_LOADER.load(Ordering::Acquire).is_null() {
        let global_ref = unsafe { ((**raw_env).NewGlobalRef)(raw_env, loader) };
        if !global_ref.is_null() {
            match GAME_CLASS_LOADER.compare_exchange(
                ptr::null_mut(),
                global_ref,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let mut first_name = FIRST_CAPTURED_CLASS.lock().unwrap();
                    *first_name = Some(class_name.to_string());
                    println!(
                        "[redox-mc-loader] Captured game classloader from first '{}' class: '{}'",
                        prefix, class_name
                    );
                }
                Err(_) => {
                    unsafe { ((**raw_env).DeleteGlobalRef)(raw_env, global_ref) };
                }
            }
        }
    }

    // 2. Query hook table for this class (Task 1.3 minimal integration)
    if let Some(hook) = adapter.find_hook(class_name) {
        println!(
            "[redox-mc-loader] Detected hook target class: '{}', method: '{}', desc: '{}' -> bridge: '{}::{}'",
            hook.class_name, hook.method_name, hook.method_desc, hook.bridge_class, hook.bridge_method
        );
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
///
/// # Safety
/// `env` must be a valid pointer to a live JNIEnv for the calling thread,
/// `loader` must be a valid classloader object or null, and `class_bytes` must
/// be a valid byte slice containing Java classfile bytecode.
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
            "[redox-mc-loader] ERROR: Failed to define class '{}' in game classloader.",
            name
        );
        unsafe {
            // Clear the pending exception so JVM does not crash
            ((**env).ExceptionClear)(env);
        }
        return Err(format!(
            "Failed to define class '{}' (exception cleared)",
            name
        ));
    }

    if defined_class.is_null() {
        eprintln!(
            "[redox-mc-loader] ERROR: DefineClass returned null for class '{}' without throwing.",
            name
        );
        return Err(format!("DefineClass returned null for '{}'", name));
    }

    Ok(defined_class)
}

/// Defines the bridge-game classes into the captured classloader,
/// ensuring superclasses are defined before subclasses.
///
/// # Safety
/// `env` must be a valid pointer to a live JNIEnv for the calling thread.
pub unsafe fn define_game_bridge_classes(env: *mut jni::JNIEnv) -> Result<(), String> {
    if GAME_CLASSES_DEFINED.load(Ordering::Acquire) {
        return Ok(());
    }

    let loader = get_game_class_loader();
    if loader.is_null() {
        return Err(
            "Cannot define bridge-game classes: game classloader has not been captured yet!".into(),
        );
    }

    println!("[redox-mc-loader] Defining bridge-game classes in game classloader (superclasses first)...");

    // 1. Superclass first: RedoxGameBridgeBase
    let base_name = "net/redoxmc/bridge/game/RedoxGameBridgeBase";
    define_class(env, base_name, loader, GAME_BRIDGE_BASE_BYTES)?;
    println!("[redox-mc-loader] Defined superclass: {}", base_name);

    // 2. Subclass second: RedoxGameBridge
    let child_name = "net/redoxmc/bridge/game/RedoxGameBridge";
    define_class(env, child_name, loader, GAME_BRIDGE_BYTES)?;
    println!("[redox-mc-loader] Defined child class: {}", child_name);

    // 3. Real Minecraft SavedData Subclass: TestMinecraftSavedData
    let item_name = "net/redoxmc/bridge/game/TestMinecraftSavedData";
    define_class(env, item_name, loader, TEST_MINECRAFT_SAVEDDATA_BYTES)?;
    println!("[redox-mc-loader] Defined real Minecraft subclass: {} (extends net.minecraft.world.level.saveddata.SavedData)", item_name);

    // 4. Server events bridge: ServerEvents
    let server_events_name = "net/redoxmc/bridge/event/ServerEvents";
    define_class(env, server_events_name, loader, SERVER_EVENTS_BYTES)?;
    println!(
        "[redox-mc-loader] Defined server events class: {}",
        server_events_name
    );

    // 5. Generic item bridge: GenericRustItem
    let generic_item_name = "net/redoxmc/bridge/item/GenericRustItem";
    define_class(env, generic_item_name, loader, GENERIC_RUST_ITEM_BYTES)?;
    println!(
        "[redox-mc-loader] Defined generic item class: {}",
        generic_item_name
    );

    // 6. Generic block bridge: GenericRedoxBlock
    let generic_block_name = "net/redoxmc/bridge/block/GenericRedoxBlock";
    define_class(env, generic_block_name, loader, GENERIC_REDOX_BLOCK_BYTES)?;
    println!(
        "[redox-mc-loader] Defined generic block class: {}",
        generic_block_name
    );

    // 7. Virtual pack resources: VirtualModPackResources
    let virtual_pack_name = "net/redoxmc/bridge/resource/VirtualModPackResources";
    define_class(
        env,
        virtual_pack_name,
        loader,
        VIRTUAL_MOD_PACK_RESOURCES_BYTES,
    )?;
    println!(
        "[redox-mc-loader] Defined virtual pack resources class: {}",
        virtual_pack_name
    );

    // 8. Virtual pack repository source inner supplier: VirtualModPackRepositorySource$1
    let repo_source_inner_name = "net/redoxmc/bridge/resource/VirtualModPackRepositorySource$1";
    define_class(
        env,
        repo_source_inner_name,
        loader,
        VIRTUAL_MOD_PACK_REPOSITORY_SOURCE_INNER_BYTES,
    )?;
    println!(
        "[redox-mc-loader] Defined repository source inner class: {}",
        repo_source_inner_name
    );

    // 9. Virtual pack repository source: VirtualModPackRepositorySource
    let repo_source_name = "net/redoxmc/bridge/resource/VirtualModPackRepositorySource";
    define_class(
        env,
        repo_source_name,
        loader,
        VIRTUAL_MOD_PACK_REPOSITORY_SOURCE_BYTES,
    )?;
    println!(
        "[redox-mc-loader] Defined repository source class: {}",
        repo_source_name
    );

    // 10. Mod raw payload: ModRawPayload
    let mod_raw_payload_name = "net/redoxmc/bridge/network/ModRawPayload";
    define_class(env, mod_raw_payload_name, loader, MOD_RAW_PAYLOAD_BYTES)?;
    println!(
        "[redox-mc-loader] Defined mod raw payload class: {}",
        mod_raw_payload_name
    );

    // 11. Mod network bridge inner StreamCodec: ModNetworkBridge$1
    let network_bridge_inner_name = "net/redoxmc/bridge/network/ModNetworkBridge$1";
    define_class(
        env,
        network_bridge_inner_name,
        loader,
        MOD_NETWORK_BRIDGE_INNER_BYTES,
    )?;
    println!(
        "[redox-mc-loader] Defined network bridge inner class: {}",
        network_bridge_inner_name
    );

    // 12. Mod network bridge: ModNetworkBridge
    let network_bridge_name = "net/redoxmc/bridge/network/ModNetworkBridge";
    define_class(env, network_bridge_name, loader, MOD_NETWORK_BRIDGE_BYTES)?;
    println!(
        "[redox-mc-loader] Defined network bridge class: {}",
        network_bridge_name
    );

    // 13. Mod handshake configuration task: ModHandshakeConfigurationTask
    let handshake_task_name = "net/redoxmc/bridge/network/ModHandshakeConfigurationTask";
    define_class(
        env,
        handshake_task_name,
        loader,
        MOD_HANDSHAKE_CONFIGURATION_TASK_BYTES,
    )?;
    println!(
        "[redox-mc-loader] Defined handshake configuration task class: {}",
        handshake_task_name
    );

    GAME_CLASSES_DEFINED.store(true, Ordering::Release);
    println!("[redox-mc-loader] All bridge-game classes defined successfully.");
    Ok(())
}

pub const SERVER_EVENTS_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/event/ServerEvents.class"
));

pub const TEST_MINECRAFT_SAVEDDATA_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/game/TestMinecraftSavedData.class"
));

pub const GENERIC_RUST_ITEM_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/item/GenericRustItem.class"
));

pub const GENERIC_REDOX_BLOCK_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/block/GenericRedoxBlock.class"
));

pub const VIRTUAL_MOD_PACK_RESOURCES_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/resource/VirtualModPackResources.class"
));

pub const VIRTUAL_MOD_PACK_REPOSITORY_SOURCE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/resource/VirtualModPackRepositorySource.class"
));

pub const VIRTUAL_MOD_PACK_REPOSITORY_SOURCE_INNER_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/resource/VirtualModPackRepositorySource$1.class"
));

pub const MOD_RAW_PAYLOAD_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/network/ModRawPayload.class"
));

pub const MOD_NETWORK_BRIDGE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/network/ModNetworkBridge.class"
));

pub const MOD_NETWORK_BRIDGE_INNER_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/network/ModNetworkBridge$1.class"
));

pub const MOD_HANDSHAKE_CONFIGURATION_TASK_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/game-classes/net/redoxmc/bridge/network/ModHandshakeConfigurationTask.class"
));

/// Verifies that a real Minecraft subclass was defined, linked, and can be instantiated
/// in the captured game classloader without NoClassDefFoundError.
///
/// # Safety
/// `env` must be a valid pointer to a live JNIEnv for the calling thread.
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

    let class_name_str = CString::new("net.redoxmc.bridge.game.TestMinecraftSavedData").unwrap();
    let j_name = ((**env).NewStringUTF)(env, class_name_str.as_ptr());
    let item_class = ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || item_class.is_null() {
        eprintln!("[redox-mc-loader] ERROR: Exception while loading TestMinecraftSavedData via game ClassLoader!");
        ((**env).ExceptionDescribe)(env);
        ((**env).ExceptionClear)(env);
        return Err("Failed to load TestMinecraftSavedData".into());
    }

    // Call static method: TestMinecraftSavedData.getTestStatus()
    let status_name = CString::new("getTestStatus").unwrap();
    let status_sig = CString::new("()Ljava/lang/String;").unwrap();
    let status_mid =
        ((**env).GetStaticMethodID)(env, item_class, status_name.as_ptr(), status_sig.as_ptr());
    if status_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to get getTestStatus method ID".into());
    }

    let j_res = ((**env).CallStaticObjectMethod)(env, item_class, status_mid) as jni::jstring;
    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || j_res.is_null() {
        eprintln!(
            "[redox-mc-loader] ERROR: Exception during TestMinecraftSavedData.getTestStatus()!"
        );
        ((**env).ExceptionDescribe)(env);
        ((**env).ExceptionClear)(env);
        return Err("Failed to execute getTestStatus".into());
    }

    let c_chars = ((**env).GetStringUTFChars)(env, j_res, std::ptr::null_mut());
    let res_str = std::ffi::CStr::from_ptr(c_chars)
        .to_string_lossy()
        .to_string();
    ((**env).ReleaseStringUTFChars)(env, j_res, c_chars);

    println!("[redox-mc-loader] [VERIFY] {}", res_str);
    Ok(res_str)
}

/// Verifies that GenericRedoxBlock was defined, linked, and can be instantiated
/// in the captured game classloader without NoClassDefFoundError.
///
/// # Safety
/// `env` must be a valid pointer to a live JNIEnv for the calling thread.
pub unsafe fn verify_generic_redox_block(env: *mut jni::JNIEnv) -> Result<String, String> {
    define_game_bridge_classes(env)?;

    let loader = get_game_class_loader();
    if loader.is_null() {
        return Err("Game classloader is null".into());
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

    let class_name_str = CString::new("net.redoxmc.bridge.block.GenericRedoxBlock").unwrap();
    let j_name = ((**env).NewStringUTF)(env, class_name_str.as_ptr());
    let block_class =
        ((**env).CallObjectMethod)(env, loader, load_class_mid, j_name) as jni::jclass;

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || block_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to load GenericRedoxBlock class".into());
    }

    let verify_name = CString::new("verifyBlockInstantiation").unwrap();
    let verify_sig = CString::new("()Ljava/lang/String;").unwrap();
    let verify_mid =
        ((**env).GetStaticMethodID)(env, block_class, verify_name.as_ptr(), verify_sig.as_ptr());
    if verify_mid.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Failed to find verifyBlockInstantiation method".into());
    }

    let j_res = ((**env).CallStaticObjectMethod)(env, block_class, verify_mid) as jni::jstring;
    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE || j_res.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err("Exception during verifyBlockInstantiation".into());
    }

    let c_chars = ((**env).GetStringUTFChars)(env, j_res, std::ptr::null_mut());
    let res_str = std::ffi::CStr::from_ptr(c_chars)
        .to_string_lossy()
        .to_string();
    ((**env).ReleaseStringUTFChars)(env, j_res, c_chars);

    println!("[redox-mc-loader] [VERIFY] {}", res_str);
    Ok(res_str)
}
