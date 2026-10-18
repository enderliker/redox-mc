use crate::manifest::{ManifestError, ModManifest, ModSide};
use crate::mod_resolver::{resolve_load_order, ResolverError};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModLoaderError {
    #[error("Manifest error in '{0}': {1}")]
    Manifest(PathBuf, ManifestError),

    #[error("Failed to read manifest file '{0}': {1}")]
    ManifestIo(PathBuf, std::io::Error),

    #[error("Dependency resolution failed: {0}")]
    Resolver(#[from] ResolverError),

    #[error("Failed to dynamically load mod library '{0}': {1}")]
    LibraryLoad(PathBuf, libloading::Error),

    #[error("Mod '{0}' initialization function 'redox_mod_init' failed with exit code {1}")]
    InitFailed(String, i32),
}

/// Represents an unverified mod candidate found on disk.
#[derive(Debug, Clone)]
pub struct ModCandidate {
    pub manifest: ModManifest,
    pub binary_path: PathBuf,
    pub manifest_path: PathBuf,
}

/// Represents a dynamically loaded mod active in memory.
pub struct LoadedMod {
    pub manifest: ModManifest,
    pub binary_path: PathBuf,
    // Store library in memory for the life of the JVM (never dlclose)
    pub _library: Box<libloading::Library>,
    pub on_bootstrap_fn: Option<unsafe extern "C" fn()>,
}

pub struct ModManager {
    loaded_mods: Vec<LoadedMod>,
    initialized: bool,
}

impl ModManager {
    pub fn new() -> Self {
        Self {
            loaded_mods: Vec::new(),
            initialized: false,
        }
    }

    /// Discovers mod candidates from the given directory (e.g. "mods/").
    pub fn discover_candidates(
        &self,
        mods_dir: &Path,
    ) -> Result<Vec<ModCandidate>, ModLoaderError> {
        if !mods_dir.exists() {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();
        let entries = match fs::read_dir(mods_dir) {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Case 1: Subdirectory containing mod.toml and a native binary
                let manifest_path = path.join("mod.toml");
                if manifest_path.is_file() {
                    if let Some(binary_path) = find_binary_in_dir(&path) {
                        let toml_str = fs::read_to_string(&manifest_path)
                            .map_err(|e| ModLoaderError::ManifestIo(manifest_path.clone(), e))?;
                        let manifest = ModManifest::from_toml_str(&toml_str)
                            .map_err(|e| ModLoaderError::Manifest(manifest_path.clone(), e))?;
                        candidates.push(ModCandidate {
                            manifest,
                            binary_path,
                            manifest_path,
                        });
                    }
                }
            } else if path.is_file() && is_native_library(&path) {
                // Case 2: Flat binary in mods/ with a corresponding .toml
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let clean_stem = file_stem.strip_prefix("lib").unwrap_or(file_stem);

                // Try <clean_stem>.toml, <file_stem>.toml, mod.toml
                let possible_manifests = [
                    mods_dir.join(format!("{}.toml", clean_stem)),
                    mods_dir.join(format!("{}.toml", file_stem)),
                    mods_dir.join("mod.toml"),
                ];

                for manifest_path in &possible_manifests {
                    if manifest_path.is_file() {
                        let toml_str = fs::read_to_string(manifest_path)
                            .map_err(|e| ModLoaderError::ManifestIo(manifest_path.clone(), e))?;
                        if let Ok(manifest) = ModManifest::from_toml_str(&toml_str) {
                            candidates.push(ModCandidate {
                                manifest,
                                binary_path: path.clone(),
                                manifest_path: manifest_path.clone(),
                            });
                            break;
                        }
                    }
                }
            }
        }

        Ok(candidates)
    }

    /// Loads and initializes all discovered mods in strictly deterministic order.
    pub fn load_mods(
        &mut self,
        mods_dir: &Path,
        running_side: ModSide,
        loader_version: &Version,
    ) -> Result<usize, ModLoaderError> {
        if self.initialized {
            return Ok(self.loaded_mods.len());
        }

        println!(
            "[redox-mc-loader] Scanning for native mods in: {:?}",
            mods_dir.display()
        );

        let candidates = self.discover_candidates(mods_dir)?;
        if candidates.is_empty() {
            println!("[redox-mc-loader] No native mods found in mods directory.");
            self.initialized = true;
            return Ok(0);
        }

        // Map candidate manifests for dependency resolver
        let candidate_map: std::collections::HashMap<String, ModCandidate> = candidates
            .into_iter()
            .map(|c| (c.manifest.id.clone(), c))
            .collect();

        let manifests: Vec<ModManifest> =
            candidate_map.values().map(|c| c.manifest.clone()).collect();

        // Compute deterministic topological load order
        let resolved_order = resolve_load_order(manifests, running_side, loader_version)?;

        println!(
            "[redox-mc-loader] Resolved deterministic load order for {} mod(s):",
            resolved_order.len()
        );
        for (i, m) in resolved_order.iter().enumerate() {
            println!("  [{}] '{}' v{} ({:?})", i + 1, m.id, m.version, m.side);
        }

        // Load each mod in the resolved deterministic order
        for manifest in resolved_order {
            let candidate = candidate_map.get(&manifest.id).unwrap();

            println!(
                "[redox-mc-loader] Loading native mod '{}' from {:?}...",
                manifest.id,
                candidate.binary_path.file_name().unwrap_or_default()
            );

            // Load cdylib using libloading
            let lib = unsafe {
                libloading::Library::new(&candidate.binary_path)
                    .map_err(|e| ModLoaderError::LibraryLoad(candidate.binary_path.clone(), e))?
            };
            let boxed_lib = Box::new(lib);

            // Resolve and execute redox_mod_init
            type InitFn = unsafe extern "C" fn() -> i32;
            let init_symbol = unsafe { boxed_lib.get::<InitFn>(b"redox_mod_init\0") };

            if let Ok(init_fn) = init_symbol {
                let code = unsafe { init_fn() };
                if let Err(_safety_err) = crate::safety::check_ffi_code(&manifest.id, code) {
                    return Err(ModLoaderError::InitFailed(manifest.id.clone(), code));
                }
            } else {
                println!(
                    "[redox-mc-loader] Mod '{}' has no 'redox_mod_init' symbol. Proceeding.",
                    manifest.id
                );
            }

            // Resolve optional redox_mod_on_bootstrap
            type BootstrapFn = unsafe extern "C" fn();
            let bootstrap_symbol =
                unsafe { boxed_lib.get::<BootstrapFn>(b"redox_mod_on_bootstrap\0") };
            let on_bootstrap_fn = bootstrap_symbol.ok().map(|s| *s);

            self.loaded_mods.push(LoadedMod {
                manifest,
                binary_path: candidate.binary_path.clone(),
                _library: boxed_lib,
                on_bootstrap_fn,
            });
        }

        println!(
            "[redox-mc-loader] Successfully loaded {} native mod(s).",
            self.loaded_mods.len()
        );
        self.initialized = true;
        Ok(self.loaded_mods.len())
    }

    /// Dispatches the Minecraft bootstrap hook to all loaded mods in deterministic order.
    pub fn dispatch_bootstrap(&self) {
        if self.loaded_mods.is_empty() {
            return;
        }

        println!(
            "[redox-mc-loader] Dispatching Bootstrap.bootStrap() hook to {} loaded mod(s)...",
            self.loaded_mods.len()
        );

        for loaded in &self.loaded_mods {
            if let Some(on_bootstrap) = loaded.on_bootstrap_fn {
                unsafe {
                    on_bootstrap();
                }
            }
        }
    }

    pub fn loaded_mods(&self) -> &[LoadedMod] {
        &self.loaded_mods
    }
}

impl Default for ModManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global ModManager instance
static MOD_MANAGER: Mutex<Option<ModManager>> = Mutex::new(None);

pub fn get_or_init_manager() -> &'static Mutex<Option<ModManager>> {
    &MOD_MANAGER
}

/// Called from RedoxBridge.onBootstrapEntry()
pub fn on_bootstrap_entry() {
    let mut guard = MOD_MANAGER.lock().unwrap();
    let manager = guard.get_or_insert_with(ModManager::new);

    if !manager.initialized {
        let mods_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("mods");

        let loader_ver =
            Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 1, 0));
        let running_side = ModSide::Server; // Bootstrap on dedicated server

        if let Err(err) = manager.load_mods(&mods_dir, running_side, &loader_ver) {
            eprintln!(
                "[redox-mc-loader] ERROR: Failed during mod loading: {}",
                err
            );
        }
    }

    manager.dispatch_bootstrap();
}

fn find_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_native_library(&path) {
            return Some(path);
        }
    }
    None
}

fn is_native_library(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    matches!(ext, "so" | "dll" | "dylib")
}
