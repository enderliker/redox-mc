pub mod bridge_jni;
pub mod class_injector;

use jvmti_bindings::prelude::*;
use jvmti_bindings::sys::jni;
use std::path::PathBuf;

// Embedded bridge-core.jar bytecode to ensure the agent is 100% self-contained
pub const BRIDGE_CORE_JAR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustmc-bridge/build/bridge-core.jar"
));

/// Extracts the embedded bridge-core.jar to disk (cache directory) and adds it
/// to the JVM Bootstrap ClassLoader Search path.
fn extract_and_inject_bridge_core(jvmti: &Jvmti) -> bool {
    let primary_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".rustmc")
        .join("cache");

    let target_jar = match std::fs::create_dir_all(&primary_dir) {
        Ok(_) => primary_dir.join("bridge-core.jar"),
        Err(_) => std::env::temp_dir().join("rustmc_cache").join("bridge-core.jar"),
    };

    if let Some(parent) = target_jar.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&target_jar, BRIDGE_CORE_JAR_BYTES) {
        eprintln!(
            "[rustmc-loader] ERROR: Failed to extract embedded bridge-core.jar to '{:?}': {}",
            target_jar, e
        );
        return false;
    }

    let jar_path_str = target_jar.to_string_lossy();
    if let Err(e) = jvmti.add_to_bootstrap_class_loader_search(&jar_path_str) {
        eprintln!(
            "[rustmc-loader] ERROR: AddToBootstrapClassLoaderSearch failed for '{}': {:?}",
            jar_path_str, e
        );
        return false;
    }

    println!(
        "[rustmc-loader] Extracted bridge-core.jar to '{}' and added to BootstrapClassLoader search path.",
        jar_path_str
    );
    true
}

#[derive(Default)]
pub struct RustmcAgent;

impl Agent for RustmcAgent {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[rustmc-loader] ==================================================");
        println!("[rustmc-loader] RustMC Native Mod Loader for Minecraft Java 26.3");
        println!("[rustmc-loader] JVMTI Agent initialized successfully.");
        println!("[rustmc-loader] ==================================================");

        let Ok(jvmti) = context.vm().jvmti() else {
            eprintln!("[rustmc-loader] ERROR: Failed to obtain JVMTI environment.");
            return jni::JNI_ERR;
        };

        // Enable class file load hook capability
        let mut caps = jvmti::jvmtiCapabilities::default();
        caps.set_can_generate_all_class_hook_events(true);
        if let Err(err) = jvmti.add_capabilities(&caps) {
            eprintln!("[rustmc-loader] ERROR: Failed to add JVMTI capabilities: {:?}", err);
            return jni::JNI_ERR;
        }

        // Configure default trampolines for Agent trait callbacks
        if let Err(err) = jvmti.set_default_agent_callbacks() {
            eprintln!("[rustmc-loader] ERROR: Failed to set default agent callbacks: {:?}", err);
            return jni::JNI_ERR;
        }

        // Enable global lifecycle events
        if let Err(err) = jvmti.enable_events_global(&[
            jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
            jvmti::JVMTI_EVENT_CLASS_PREPARE,
            jvmti::JVMTI_EVENT_VM_INIT,
            jvmti::JVMTI_EVENT_VM_DEATH,
        ]) {
            eprintln!("[rustmc-loader] ERROR: Failed to enable global events: {:?}", err);
            return jni::JNI_ERR;
        }

        // Extract embedded bridge-core.jar and add to Bootstrap ClassLoader search path
        if !extract_and_inject_bridge_core(&jvmti) {
            eprintln!("[rustmc-loader] Warning: bridge-core could not be added to bootstrap classloader search path.");
        }

        jni::JNI_OK
    }

    fn vm_init(&self, context: CallbackContext<'_>, _event: ThreadEvent) {
        if let Some(jni) = context.jni() {
            unsafe {
                if let Err(e) = bridge_jni::register_bridge_natives(jni.raw()) {
                    eprintln!("[rustmc-loader] Warning registering bridge natives in vm_init: {}", e);
                }
            }
        }
    }

    fn class_prepare(&self, context: CallbackContext<'_>, event: ClassEvent) {
        let Ok((signature, _)) = (unsafe { context.jvmti().get_class_signature(event.class()) }) else {
            return;
        };

        if signature == "Lnet/minecraft/server/Main;" {
            if let Some(jni) = context.jni() {
                unsafe {
                    println!("[rustmc-loader] net/minecraft/server/Main prepared. Verifying real Minecraft subclass in game classloader...");
                    if let Err(e) = class_injector::verify_real_minecraft_subclass(jni.raw()) {
                        eprintln!("[rustmc-loader] ERROR in verify_real_minecraft_subclass: {}", e);
                    }
                }
            }
        }
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        println!("[rustmc-loader] JVM is shutting down (VMDeath). Releasing resources cleanly.");
    }

    fn class_file_load_hook<'callback>(
        &self,
        context: CallbackContext<'callback>,
        event: ClassFileLoadHookEvent<'callback>,
    ) {
        // Capture global reference to the game classloader on the FIRST net/minecraft/* class
        if let Some(jni) = context.jni() {
            if let Some(name_cow) = event.name_str().ok().flatten() {
                unsafe {
                    class_injector::on_class_file_load(jni.raw(), event.loader(), &name_cow);
                }
            }
        }
    }
}

export_agent!(RustmcAgent);
