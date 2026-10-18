#![allow(clippy::missing_safety_doc)]

pub mod adapter;
pub mod bench;
pub mod bridge_jni;
pub mod class_injector;
pub mod events;
pub mod manifest;
pub mod mod_loader;
pub mod mod_resolver;
pub mod safety;
pub mod transformer;

use jvmti_bindings::prelude::*;
use jvmti_bindings::sys::jni;
use std::path::PathBuf;

// Embedded bridge-core.jar bytecode to ensure the agent is 100% self-contained
pub const BRIDGE_CORE_JAR_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../redox-mc-bridge/build/bridge-core.jar"
));

/// Extracts the embedded bridge-core.jar to disk (cache directory) and adds it
/// to the JVM Bootstrap ClassLoader Search path.
fn extract_and_inject_bridge_core(jvmti: &Jvmti) -> bool {
    let primary_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".redox-mc")
        .join("cache");

    let target_dir = match std::fs::create_dir_all(&primary_dir) {
        Ok(_) => primary_dir,
        Err(_) => {
            let unique_temp =
                std::env::temp_dir().join(format!("redoxmc_cache_{}", std::process::id()));
            let _ = std::fs::create_dir_all(&unique_temp);
            unique_temp
        }
    };

    let target_jar = target_dir.join("bridge-core.jar");

    if let Some(parent) = target_jar.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&target_jar, BRIDGE_CORE_JAR_BYTES) {
        eprintln!(
            "[redox-mc-loader] ERROR: Failed to extract embedded bridge-core.jar to '{:?}': {}",
            target_jar, e
        );
        return false;
    }

    let jar_path_str = target_jar.to_string_lossy();
    if let Err(e) = jvmti.add_to_bootstrap_class_loader_search(&jar_path_str) {
        eprintln!(
            "[redox-mc-loader] ERROR: AddToBootstrapClassLoaderSearch failed for '{}': {:?}",
            jar_path_str, e
        );
        return false;
    }

    println!(
        "[redox-mc-loader] Extracted bridge-core.jar to '{}' and added to BootstrapClassLoader search path.",
        jar_path_str
    );
    true
}

#[derive(Default)]
pub struct RedoxMCAgent;

impl Agent for RedoxMCAgent {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[redox-mc-loader] ==================================================");
        println!("[redox-mc-loader] Redox-MC Native Mod Loader for Minecraft Java");
        let adapter = adapter::active_adapter();
        println!(
            "[redox-mc-loader] Active Version Adapter: Minecraft {} (Protocol {}, Classfile {})",
            adapter.version_id(),
            adapter.protocol_version(),
            adapter.classfile_version()
        );
        println!(
            "[redox-mc-loader] Registered {} hook targets from adapter table.",
            adapter.hook_targets().len()
        );
        println!("[redox-mc-loader] JVMTI Agent initialized successfully.");
        println!("[redox-mc-loader] ==================================================");

        let Ok(jvmti) = context.vm().jvmti() else {
            eprintln!("[redox-mc-loader] ERROR: Failed to obtain JVMTI environment.");
            return jni::JNI_ERR;
        };

        // Enable class file load hook capability
        let mut caps = jvmti::jvmtiCapabilities::default();
        caps.set_can_generate_all_class_hook_events(true);
        if let Err(err) = jvmti.add_capabilities(&caps) {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed to add JVMTI capabilities: {:?}",
                err
            );
            return jni::JNI_ERR;
        }

        // Configure default trampolines for Agent trait callbacks
        if let Err(err) = jvmti.set_default_agent_callbacks() {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed to set default agent callbacks: {:?}",
                err
            );
            return jni::JNI_ERR;
        }

        // Enable global lifecycle events
        if let Err(err) = jvmti.enable_events_global(&[
            jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
            jvmti::JVMTI_EVENT_CLASS_PREPARE,
            jvmti::JVMTI_EVENT_VM_INIT,
            jvmti::JVMTI_EVENT_VM_DEATH,
        ]) {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed to enable global events: {:?}",
                err
            );
            return jni::JNI_ERR;
        }

        // Extract embedded bridge-core.jar and add to Bootstrap ClassLoader search path
        if !extract_and_inject_bridge_core(&jvmti) {
            eprintln!("[redox-mc-loader] Warning: bridge-core could not be added to bootstrap classloader search path.");
        }

        jni::JNI_OK
    }

    fn vm_init(&self, context: CallbackContext<'_>, _event: ThreadEvent) {
        if let Some(jni) = context.jni() {
            unsafe {
                if let Err(e) = bridge_jni::register_bridge_natives(jni.raw()) {
                    eprintln!(
                        "[redox-mc-loader] Warning registering bridge natives in vm_init: {}",
                        e
                    );
                }
            }
        }
    }

    fn class_prepare(&self, context: CallbackContext<'_>, event: ClassEvent) {
        let Ok((signature, _)) = (unsafe { context.jvmti().get_class_signature(event.class()) })
        else {
            return;
        };

        let target_sig = format!("L{};", adapter::active_adapter().server_main_class());
        if signature == target_sig {
            if let Some(jni) = context.jni() {
                unsafe {
                    println!(
                        "[redox-mc-loader] {} prepared. Verifying real Minecraft subclass in game classloader...",
                        adapter::active_adapter().server_main_class()
                    );
                    if let Err(e) = class_injector::verify_real_minecraft_subclass(jni.raw()) {
                        eprintln!(
                            "[redox-mc-loader] ERROR in verify_real_minecraft_subclass: {}",
                            e
                        );
                    }
                }
            }
        }
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        println!("[redox-mc-loader] JVM is shutting down (VMDeath). Releasing resources cleanly.");
    }

    fn class_file_load_hook<'callback>(
        &self,
        context: CallbackContext<'callback>,
        mut event: ClassFileLoadHookEvent<'callback>,
    ) {
        let name_opt = event.name_str().ok().flatten();

        // 1. Capture global reference to the game classloader on the FIRST net/minecraft/* class
        if let Some(jni) = context.jni() {
            if let Some(ref name) = name_opt {
                unsafe {
                    class_injector::on_class_file_load(jni.raw(), event.loader(), name);
                }
            }
        }

        // 2. Fast-path: check if this class matches any hook in the active version adapter
        let hooks = adapter::active_adapter().hook_targets();
        if let Some(ref name) = name_opt {
            if !hooks.iter().any(|h| h.class_name == name.as_ref()) {
                return;
            }
        }

        let class_bytes = event.class_data();
        if class_bytes.is_empty() {
            return;
        }

        match transformer::transform_class(class_bytes, hooks) {
            Ok(Some(transformed_bytes)) => {
                if let Some(ref name) = name_opt {
                    println!(
                        "[redox-mc-loader] Successfully transformed class '{}' ({} -> {} bytes)",
                        name,
                        class_bytes.len(),
                        transformed_bytes.len()
                    );
                }
                if let Err(err) = event.set_transformed_class(&context, &transformed_bytes) {
                    eprintln!(
                        "[redox-mc-loader] Failed to set transformed class in JVMTI: {:?}",
                        err
                    );
                }
            }
            Ok(None) => {
                // Class was not a target or did not need transformation
            }
            Err(err) => {
                // Never crash the JVM on transformer error: fallback gracefully to original class
                if let Some(ref name) = name_opt {
                    eprintln!(
                        "[redox-mc-loader] Warning: Bytecode transformation failed for '{}': {}. Falling back to original bytecode.",
                        name, err
                    );
                }
            }
        }
    }
}

export_agent!(RedoxMCAgent);
