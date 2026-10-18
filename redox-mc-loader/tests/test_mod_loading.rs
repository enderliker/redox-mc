use redox_mc_loader::manifest::ModSide;
use redox_mc_loader::mod_loader::ModManager;
use semver::Version;
use std::fs;
use std::process::Command;

#[test]
fn test_dynamic_mod_discovery_and_loading() {
    // 1. Build test-mod cdylib first
    let build_status = Command::new("cargo")
        .args(["build", "-p", "test-mod"])
        .status()
        .expect("cargo build -p test-mod failed");
    assert!(build_status.success());

    let target_so = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../target/debug/libtest_mod.so"
    );
    assert!(
        fs::metadata(target_so).is_ok(),
        "libtest_mod.so must exist at {}",
        target_so
    );

    let test_mod_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/test-mod");
    let mod_toml_path = format!("{}/mod.toml", test_mod_dir);

    // 2. Setup a temporary mods/ directory structure
    let temp_dir = std::env::temp_dir().join("redox_test_mods_env");
    let mods_dir = temp_dir.join("mods");
    let mod_subdir = mods_dir.join("test_mod");

    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&mod_subdir).unwrap();

    // Copy mod.toml and libtest_mod.so into mods/test_mod/
    fs::copy(&mod_toml_path, mod_subdir.join("mod.toml")).unwrap();
    fs::copy(target_so, mod_subdir.join("libtest_mod.so")).unwrap();

    // 3. Test ModManager discovery and loading
    let mut manager = ModManager::new();
    let loader_ver = Version::parse("0.1.0").unwrap();

    let count = manager
        .load_mods(&mods_dir, ModSide::Both, &loader_ver)
        .expect("Mod loading must succeed");

    assert_eq!(count, 1, "Expected 1 mod to be discovered and loaded");
    assert_eq!(manager.loaded_mods().len(), 1);

    let loaded = &manager.loaded_mods()[0];
    assert_eq!(loaded.manifest.id, "test_mod");
    assert_eq!(loaded.manifest.version, Version::parse("0.1.0").unwrap());
    assert!(loaded.on_bootstrap_fn.is_some(), "redox_mod_on_bootstrap symbol must be resolved");

    // 4. Test dispatch_bootstrap
    manager.dispatch_bootstrap();

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
