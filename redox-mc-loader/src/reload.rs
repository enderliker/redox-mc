//! Safe hot-reload support for native mods.
//!
//! Reloading follows the pattern documented in `tasks/plan.md`:
//!
//! 1. Copy the freshly built project binary into `mod_cache/mod_<timestamp>...`.
//! 2. Load the cached copy with `libloading`.
//! 3. Initialize the new generation and resolve all callbacks.
//! 4. Publish the new callback table atomically through [`crate::dispatcher`].
//! 5. Retain the previous libraries forever instead of calling `dlclose`.
//!
//! Step 5 is deliberate. Rust thread-local destructors, locks, and (on Windows)
//! file mappings may still be referenced by callbacks that started before the
//! swap. Unloading the old library would turn those references into undefined
//! behaviour.

use crate::mod_loader::{ModLoaderError, MOD_MANAGER};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static CACHE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Result of a successful hot reload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadOutcome {
    /// Mod IDs in the newly published generation, in deterministic order.
    pub reloaded: Vec<String>,
    /// Mod IDs whose previous generation was retired but kept mapped.
    pub retired: Vec<String>,
    /// Directory holding the immutable per-generation binary copies.
    pub cache_dir: PathBuf,
}

impl ReloadOutcome {
    /// Human-readable summary returned to the `/redox reload` command.
    pub fn summary(&self) -> String {
        if self.reloaded.is_empty() {
            format!(
                "No native mods found. {} previous version(s) retained.",
                self.retired.len()
            )
        } else {
            format!(
                "Reloaded {} native mod(s); {} previous version(s) retained in {}.",
                self.reloaded.len(),
                self.retired.len(),
                self.cache_dir.display()
            )
        }
    }
}

/// Returns the cache directory that belongs to a `mods/` directory.
///
/// For the normal `<server>/mods` layout this is `<server>/mod_cache`.
pub fn cache_dir_for(mods_dir: &Path) -> PathBuf {
    let parent = mods_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join("mod_cache")
}

/// Copies a mod binary into the cache under a unique, timestamped name.
///
/// The returned path is safe to load with `libloading`: it is never the project
/// binary, so rebuilding and replacing the project binary cannot fail because
/// the JVM holds an exclusive mapping (a real concern on Windows).
pub fn copy_binary_to_cache(
    binary_path: &Path,
    cache_dir: &Path,
    mod_id: &str,
) -> Result<PathBuf, ModLoaderError> {
    fs::create_dir_all(cache_dir)
        .map_err(|err| ModLoaderError::CacheDir(cache_dir.to_path_buf(), err))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = CACHE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let extension = binary_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin");
    let cached_path = cache_dir.join(format!("mod_{timestamp}_{sequence}_{mod_id}.{extension}"));

    fs::copy(binary_path, &cached_path).map_err(|err| {
        ModLoaderError::CacheCopy(binary_path.to_path_buf(), cached_path.clone(), err)
    })?;

    Ok(cached_path)
}

/// Reloads the mods from `<current working directory>/mods`.
///
/// The manager lock is released before bootstrap callbacks are dispatched so a
/// mod that calls back into the loader cannot deadlock the JVM.
pub fn reload_mods() -> Result<ReloadOutcome, ModLoaderError> {
    let mods_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("mods");
    let loader_version =
        Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 1, 0));
    let side = crate::mod_loader::running_side();

    let outcome = {
        let mut guard = MOD_MANAGER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let manager = guard.as_mut().ok_or(ModLoaderError::NotInitialized)?;
        manager.reload_mods(&mods_dir, side, &loader_version)?
    };

    // Refresh virtual resources registered by mods (if any were modified or added)
    crate::mod_loader::collect_registered_resources();

    Ok(outcome)
}
