use redox_mc_adapter_26_3::HOOK_TARGETS;
use redox_mc_loader::manifest::ModSide;
use redox_mc_loader::mod_loader::ModManager;
use redox_mc_loader::transformer::transform_class;
use ristretto_classfile::ClassFile;
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_minecraft_server_jar() -> Option<PathBuf> {
    let candidates = [
        "../versions/26.3/server-26.3.jar",
        "versions/26.3/server-26.3.jar",
        "/home/ender/rustmc/versions/26.3/server-26.3.jar",
    ];

    for candidate in &candidates {
        let p = Path::new(candidate);
        if p.exists() {
            return Some(
                p.canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(candidate)),
            );
        }
    }
    None
}

#[test]
fn test_builtin_registries_wrapper_transformation_and_bytecode_verification() {
    let Some(jar_path) = find_minecraft_server_jar() else {
        println!("SKIPPED: Minecraft server jar not found at versions/26.3/server-26.3.jar");
        return;
    };

    // 1. Extract net/minecraft/core/registries/BuiltInRegistries.class from server-26.3.jar
    let original_bytes = match Command::new("unzip")
        .args([
            "-p",
            jar_path.to_str().unwrap(),
            "net/minecraft/core/registries/BuiltInRegistries.class",
        ])
        .output()
    {
        Ok(out) if out.status.success() => out.stdout,
        _ => {
            let temp_dir = std::env::temp_dir().join("redox_extract_builtin_registries");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();
            let jar_status = Command::new("jar")
                .args([
                    "xf",
                    jar_path.to_str().unwrap(),
                    "net/minecraft/core/registries/BuiltInRegistries.class",
                ])
                .current_dir(&temp_dir)
                .status();
            match jar_status {
                Ok(st) if st.success() => {
                    fs::read(temp_dir.join("net/minecraft/core/registries/BuiltInRegistries.class"))
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            }
        }
    };

    if original_bytes.is_empty() {
        println!("SKIPPED: Failed to extract BuiltInRegistries.class from server jar");
        return;
    }

    // 2. Transform with canonical HOOK_TARGETS
    let transformed = transform_class(&original_bytes, HOOK_TARGETS)
        .expect("Transformation must succeed")
        .expect("BuiltInRegistries must be transformed");

    // 3. Inspect transformed class structure
    let class_file = ClassFile::from_bytes(&transformed).expect("Parse transformed class");

    let mut found_original = false;
    let mut found_wrapper = false;

    for method in &class_file.methods {
        let name = class_file
            .constant_pool
            .try_get_utf8(method.name_index)
            .unwrap();
        let desc = class_file
            .constant_pool
            .try_get_utf8(method.descriptor_index)
            .unwrap();

        if name == "freeze$original" && desc == "()V" {
            found_original = true;
        }
        if name == "freeze" && desc == "()V" {
            found_wrapper = true;
        }
    }

    assert!(
        found_original,
        "Method 'freeze$original()V' must exist in transformed BuiltInRegistries.class"
    );
    assert!(
        found_wrapper,
        "Wrapper method 'freeze()V' must exist in transformed BuiltInRegistries.class"
    );

    // 4. Validate bytecode with javap -v -p
    let temp_dir = std::env::temp_dir().join("redox_test_builtin_registries");
    fs::create_dir_all(&temp_dir).unwrap();
    let temp_class = temp_dir.join("BuiltInRegistries.class");
    fs::write(&temp_class, &transformed).unwrap();

    let javap_output = Command::new("javap")
        .args(["-v", "-p", temp_class.to_str().unwrap()])
        .output()
        .expect("Failed to execute javap");

    assert!(
        javap_output.status.success(),
        "javap validation failed: {}",
        String::from_utf8_lossy(&javap_output.stderr)
    );

    let javap_stdout = String::from_utf8_lossy(&javap_output.stdout);

    // Must contain original method
    assert!(
        javap_stdout.contains("freeze$original"),
        "javap output must contain 'freeze$original'"
    );

    // Must contain invocation of pre-freeze hook
    assert!(
        javap_stdout.contains("onRegisterContent"),
        "javap output must contain RedoxBridge.onRegisterContent"
    );

    // Must contain invocation of post-freeze hook
    assert!(
        javap_stdout.contains("onRegistriesFrozen"),
        "javap output must contain RedoxBridge.onRegistriesFrozen"
    );

    println!("[TEST] BuiltInRegistries bytecode verification passed: wrapper contains both pre-hook and post-hook!");
}

#[test]
fn test_mod_manager_dispatches_register_content_to_loaded_mods() {
    // 1. Build test-mod cdylib first
    let build_status = Command::new("cargo")
        .args(["build", "-p", "test-mod"])
        .status()
        .expect("cargo build -p test-mod failed");
    assert!(build_status.success());

    let binary_name = format!(
        "{}test_mod{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let target_binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/debug")
        .join(&binary_name);
    assert!(target_binary.exists(), "{} must exist", binary_name);

    let test_mod_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/test-mod");
    let mod_toml_path = format!("{}/mod.toml", test_mod_dir);

    // 2. Setup temporary mods environment
    let temp_dir = std::env::temp_dir().join("redox_test_reg_hook_mods");
    let mods_dir = temp_dir.join("mods");
    let mod_subdir = mods_dir.join("test_mod");

    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&mod_subdir).unwrap();
    fs::copy(&mod_toml_path, mod_subdir.join("mod.toml")).unwrap();
    fs::copy(&target_binary, mod_subdir.join(&binary_name)).unwrap();

    // 3. Load mod and verify callbacks are registered
    let mut manager = ModManager::new();
    let loader_ver = Version::parse("0.1.0").unwrap();

    let count = manager
        .load_mods(&mods_dir, ModSide::Both, &loader_ver)
        .expect("Mod loading must succeed");
    assert_eq!(count, 1);

    let loaded = &manager.loaded_mods()[0];
    assert!(
        loaded.on_register_content_fn.is_some(),
        "redox_mod_on_register_content symbol must be resolved"
    );

    // 4. Dispatch register_content and ensure no panics/errors
    manager.dispatch_register_content();

    let _ = fs::remove_dir_all(&temp_dir);
}
