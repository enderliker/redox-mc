use redox_mc_abi::types::{FfiStatus, FfiStr};
use redox_mc_loader::manifest::ModSide;
use redox_mc_loader::mod_loader::{ModLoaderError, ModManager};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_MANIFEST: &str = r#"
id = "reload_fixture"
name = "Hot Reload Fixture"
version = "1.0.0"
side = "both"
"#;

fn fixture_library_name(stem: &str) -> String {
    format!(
        "{}{}{}",
        std::env::consts::DLL_PREFIX,
        stem,
        std::env::consts::DLL_SUFFIX
    )
}

fn build_fixtures() -> (PathBuf, PathBuf) {
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "redox-reload-fixture-v1",
            "-p",
            "redox-reload-fixture-v2",
        ])
        .status()
        .expect("cargo build for reload fixtures failed to start");
    assert!(status.success(), "cargo build for reload fixtures failed");

    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/debug");
    let v1 = target_dir.join(fixture_library_name("reload_mod_v1"));
    let v2 = target_dir.join(fixture_library_name("reload_mod_v2"));

    assert!(v1.exists(), "v1 fixture library must exist at {:?}", v1);
    assert!(v2.exists(), "v2 fixture library must exist at {:?}", v2);

    (v1, v2)
}

fn prepare_mods_dir(v1: &Path) -> (PathBuf, PathBuf) {
    let temp_dir = std::env::temp_dir().join(format!(
        "redox_hot_reload_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mods_dir = temp_dir.join("mods");
    let mod_dir = mods_dir.join("reload_fixture");

    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&mod_dir).expect("create fixture mods directory");
    fs::write(mod_dir.join("mod.toml"), FIXTURE_MANIFEST).expect("write fixture manifest");

    let original_binary = mod_dir.join(fixture_library_name("reload_mod"));
    fs::copy(v1, &original_binary).expect("copy v1 fixture into mods directory");

    (temp_dir, original_binary)
}

#[test]
fn test_hot_reload_swaps_chat_behavior_and_retains_old_library() {
    let (v1, v2) = build_fixtures();
    let (temp_dir, original_binary) = prepare_mods_dir(&v1);
    let mods_dir = temp_dir.join("mods");
    let cache_dir = temp_dir.join("mod_cache");
    let loader_version = Version::parse("0.1.0").unwrap();

    let mut manager = ModManager::new();
    let count = manager
        .load_mods(&mods_dir, ModSide::Server, &loader_version)
        .expect("initial mod loading must succeed");
    assert_eq!(count, 1);

    let loaded = &manager.loaded_mods()[0];
    assert!(
        loaded.binary_path.starts_with(&cache_dir),
        "the loader must load the cached copy, not the project binary: {:?}",
        loaded.binary_path
    );
    assert_ne!(
        loaded.binary_path, original_binary,
        "the original mod binary must never be loaded directly"
    );

    assert!(
        !manager.dispatch_server_chat("Steve", "!blocked-by-v1"),
        "v1 must cancel messages prefixed with '!'"
    );
    assert!(
        manager.dispatch_server_chat("Steve", "?allowed-by-v1"),
        "v1 must allow messages prefixed with '?'"
    );

    let old_chat_callback = manager.loaded_mods()[0]
        .on_server_chat_fn
        .expect("v1 must export a chat callback");

    // On Windows, the JVM must not keep the project's original binary locked.
    #[cfg(windows)]
    {
        let renamed = original_binary.with_extension("lock-check");
        fs::rename(&original_binary, &renamed)
            .expect("the original mod binary must not be locked by the loader");
        fs::rename(&renamed, &original_binary).expect("restore the original mod binary");
    }

    fs::copy(&v2, &original_binary).expect("replace the project binary with v2");

    let outcome = manager
        .reload_mods(&mods_dir, ModSide::Server, &loader_version)
        .expect("hot reload must succeed");

    assert_eq!(outcome.reloaded, vec!["reload_fixture".to_string()]);
    assert_eq!(outcome.retired, vec!["reload_fixture".to_string()]);
    assert_eq!(outcome.cache_dir, cache_dir);
    assert_eq!(
        manager.retired_mod_count(),
        1,
        "the previous generation must be retained to avoid dlclose"
    );

    assert!(
        manager.dispatch_server_chat("Steve", "!allowed-by-v2"),
        "v2 must allow messages prefixed with '!' after the reload"
    );
    assert!(
        !manager.dispatch_server_chat("Steve", "?blocked-by-v2"),
        "v2 must cancel messages prefixed with '?' after the reload"
    );

    // The previous library is retained deliberately. Invoking its callback after
    // the swap must remain safe and must still execute the v1 implementation.
    let old_result = unsafe {
        old_chat_callback(
            FfiStr::from_str("Steve"),
            FfiStr::from_str("!old-generation"),
        )
    };
    assert_eq!(old_result.status, FfiStatus::Success);
    assert!(
        !old_result.value,
        "the retired v1 callback must remain callable without crashing"
    );

    let cache_files: Vec<PathBuf> = fs::read_dir(&cache_dir)
        .expect("mod_cache must exist")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect();
    assert!(
        cache_files.len() >= 2,
        "each load generation must get its own cached copy, found {:?}",
        cache_files
    );
    for path in &cache_files {
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("mod_"),
            "cached mod copies must follow the mod_<timestamp> naming scheme: {}",
            name
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_reload_before_initialization_is_rejected() {
    let temp_dir = std::env::temp_dir().join(format!(
        "redox_hot_reload_uninitialized_{}",
        std::process::id()
    ));
    let mods_dir = temp_dir.join("mods");
    fs::create_dir_all(&mods_dir).expect("create mods directory");

    let mut manager = ModManager::new();
    let err = manager
        .reload_mods(
            &mods_dir,
            ModSide::Server,
            &Version::parse("0.1.0").unwrap(),
        )
        .expect_err("reload must be rejected before the initial load");

    assert!(
        matches!(err, ModLoaderError::NotInitialized),
        "unexpected error: {err}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
