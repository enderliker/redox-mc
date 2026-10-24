use crate::dispatcher;
use crate::manifest::{ManifestError, ModManifest, ModSide};
use crate::mod_resolver::{resolve_load_order, ResolverError};
use crate::reload::{cache_dir_for, copy_binary_to_cache, ReloadOutcome};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
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

    #[error("Hot reload requested before the initial mod load completed")]
    NotInitialized,

    #[error("The mod manager lock was poisoned")]
    LockPoisoned,

    #[error("Failed to create mod cache directory '{0}': {1}")]
    CacheDir(PathBuf, std::io::Error),

    #[error("Failed to copy mod binary '{0}' to cache '{1}': {2}")]
    CacheCopy(PathBuf, PathBuf, std::io::Error),
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
    pub on_bootstrap_fn: Option<unsafe extern "C" fn() -> redox_mc_abi::types::FfiStatus>,
    pub on_register_content_fn: Option<unsafe extern "C" fn() -> redox_mc_abi::types::FfiStatus>,
    pub on_server_tick_fn: Option<unsafe extern "C" fn(i64) -> redox_mc_abi::types::FfiStatus>,
    pub on_server_chat_fn: Option<
        unsafe extern "C" fn(
            redox_mc_abi::types::FfiStr,
            redox_mc_abi::types::FfiStr,
        ) -> redox_mc_abi::types::FfiResult<bool>,
    >,
    pub on_item_use_fn: Option<unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32) -> i32>,
    pub on_item_use_on_fn:
        Option<unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32, i32, i32) -> i32>,
    pub on_item_finish_using_fn: Option<unsafe extern "C" fn(redox_mc_abi::types::FfiStr)>,
    pub on_block_use_without_item_fn:
        Option<unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32, i32) -> i32>,
    pub on_block_step_on_fn:
        Option<unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32)>,
    pub on_block_destroy_fn:
        Option<unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32)>,
    /// Receives inbound mod channel payloads together with sender identity.
    pub dispatch_network_payload_fn: Option<redox_mc_abi::types::NetworkDispatchFn>,
    /// Drains the mod's queued outbound network messages.
    pub drain_network_outbox_fn: Option<redox_mc_abi::types::NetworkOutboxDrainFn>,
    /// Sets the process-wide side used outside callbacks.
    pub set_running_side_fn: Option<unsafe extern "C" fn(u32) -> redox_mc_abi::types::FfiStatus>,
    /// Sets the side associated with the current callback thread.
    pub set_network_side_fn: Option<unsafe extern "C" fn(u32) -> redox_mc_abi::types::FfiStatus>,
}

pub struct ModManager {
    loaded_mods: Vec<Arc<LoadedMod>>,
    /// Retired generations are intentionally kept alive for the entire process.
    /// Dropping a mod library would call `dlclose`, invalidating callbacks that
    /// may still be executing on another thread.
    retired_mods: Vec<Arc<LoadedMod>>,
    initialized: bool,
}

impl ModManager {
    pub fn new() -> Self {
        Self {
            loaded_mods: Vec::new(),
            retired_mods: Vec::new(),
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
    ///
    /// Every binary is first copied into `mod_cache/`. The JVM therefore never
    /// maps the project's original `.so`/`.dll`, which keeps recompilation and
    /// replacement possible on Windows.
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
            dispatcher::publish(Vec::new());
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

        let cache_dir = cache_dir_for(mods_dir);
        let mut loaded_mods = Vec::with_capacity(resolved_order.len());

        for manifest in resolved_order {
            let candidate = candidate_map
                .get(&manifest.id)
                .expect("resolved manifest must have a matching candidate");

            match load_candidate(candidate, &cache_dir, running_side) {
                Ok(loaded) => loaded_mods.push(Arc::new(loaded)),
                Err(err) => {
                    // A later candidate failed. Keep already-initialized libraries
                    // mapped instead of running their destructors via dlclose.
                    self.retired_mods.extend(loaded_mods);
                    return Err(err);
                }
            }
        }

        self.loaded_mods = loaded_mods;
        self.initialized = true;
        dispatcher::publish(self.loaded_mods.clone());

        println!(
            "[redox-mc-loader] Successfully loaded {} native mod(s).",
            self.loaded_mods.len()
        );
        Ok(self.loaded_mods.len())
    }

    /// Reloads every native mod from disk using fresh cached copies.
    ///
    /// The new generation is fully loaded and initialized before it is published.
    /// If any candidate fails, the previous generation remains active. Old
    /// libraries are retained forever (never `dlclose`d) so callbacks already
    /// running on other threads remain valid.
    pub fn reload_mods(
        &mut self,
        mods_dir: &Path,
        running_side: ModSide,
        loader_version: &Version,
    ) -> Result<ReloadOutcome, ModLoaderError> {
        if !self.initialized {
            return Err(ModLoaderError::NotInitialized);
        }

        println!(
            "[redox-mc-loader] Reloading native mods from: {:?}",
            mods_dir.display()
        );

        let candidates = self.discover_candidates(mods_dir)?;
        let cache_dir = cache_dir_for(mods_dir);

        if candidates.is_empty() {
            let retired_mods = std::mem::take(&mut self.loaded_mods);
            let retired: Vec<String> = retired_mods
                .iter()
                .map(|loaded| loaded.manifest.id.clone())
                .collect();
            self.retired_mods.extend(retired_mods);
            dispatcher::publish(Vec::new());
            println!(
                "[redox-mc-loader] Hot reload complete: no mods found, {} previous version(s) retained.",
                retired.len()
            );
            return Ok(ReloadOutcome {
                reloaded: Vec::new(),
                retired,
                cache_dir,
            });
        }

        let candidate_map: std::collections::HashMap<String, ModCandidate> = candidates
            .into_iter()
            .map(|c| (c.manifest.id.clone(), c))
            .collect();

        let manifests: Vec<ModManifest> =
            candidate_map.values().map(|c| c.manifest.clone()).collect();

        let resolved_order = resolve_load_order(manifests, running_side, loader_version)?;

        let mut new_mods = Vec::with_capacity(resolved_order.len());
        for manifest in resolved_order {
            let candidate = candidate_map
                .get(&manifest.id)
                .expect("resolved manifest must have a matching candidate");

            match load_candidate(candidate, &cache_dir, running_side) {
                Ok(loaded) => new_mods.push(Arc::new(loaded)),
                Err(err) => {
                    // Keep partially loaded generations mapped as well.
                    self.retired_mods.extend(new_mods);
                    return Err(err);
                }
            }
        }

        // Initialize running side and bootstrap on the new generation BEFORE
        // publishing it to the dispatcher. This ensures concurrent server tick
        // and packet threads never observe an uninitialized generation in
        // dispatcher::snapshot().
        let abi_side = crate::network::side_to_abi(running_side);
        for loaded in &new_mods {
            if let Some(set_side) = loaded.set_running_side_fn {
                let status = unsafe { set_side(abi_side) };
                if let Err(safety_err) =
                    crate::safety::check_ffi_status(&loaded.manifest.id, status)
                {
                    eprintln!(
                        "[redox-mc-loader] ERROR: Mod '{}' failed to set running side on reload: {}",
                        loaded.manifest.id, safety_err
                    );
                }
            }
            if let Some(on_bootstrap) = loaded.on_bootstrap_fn {
                let status = unsafe { on_bootstrap() };
                if let Err(safety_err) =
                    crate::safety::check_ffi_status(&loaded.manifest.id, status)
                {
                    eprintln!(
                        "[redox-mc-loader] ERROR: Mod '{}' on_bootstrap failed on reload: {}",
                        loaded.manifest.id, safety_err
                    );
                }
            }
        }

        let old_mods = std::mem::replace(&mut self.loaded_mods, new_mods);
        let retired: Vec<String> = old_mods
            .iter()
            .map(|loaded| loaded.manifest.id.clone())
            .collect();
        self.retired_mods.extend(old_mods);

        dispatcher::publish(self.loaded_mods.clone());

        let reloaded: Vec<String> = self
            .loaded_mods
            .iter()
            .map(|loaded| loaded.manifest.id.clone())
            .collect();

        println!(
            "[redox-mc-loader] Hot reload complete: {} mod(s) reloaded, {} previous version(s) retained.",
            reloaded.len(),
            retired.len()
        );

        Ok(ReloadOutcome {
            reloaded,
            retired,
            cache_dir,
        })
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
                let status = unsafe { on_bootstrap() };
                if let Err(safety_err) =
                    crate::safety::check_ffi_status(&loaded.manifest.id, status)
                {
                    eprintln!(
                        "[redox-mc-loader] ERROR: Mod '{}' on_bootstrap failed: {}",
                        loaded.manifest.id, safety_err
                    );
                }
            }
        }
    }

    /// Dispatches the BuiltInRegistries.freeze() hook to all loaded mods in deterministic order.
    pub fn dispatch_register_content(&self) {
        if self.loaded_mods.is_empty() {
            return;
        }

        println!(
            "[redox-mc-loader] Dispatching BuiltInRegistries.freeze() hook to {} loaded mod(s)...",
            self.loaded_mods.len()
        );

        for loaded in &self.loaded_mods {
            if let Some(on_register) = loaded.on_register_content_fn {
                let status = unsafe { on_register() };
                if let Err(safety_err) =
                    crate::safety::check_ffi_status(&loaded.manifest.id, status)
                {
                    eprintln!(
                        "[redox-mc-loader] ERROR: Mod '{}' on_register_content failed: {}",
                        loaded.manifest.id, safety_err
                    );
                }
            }
        }
    }

    /// Dispatches a server tick event to all loaded mods.
    pub fn dispatch_server_tick(&self, tick_count: i64) {
        for loaded in &self.loaded_mods {
            if let Some(cb) = loaded.on_server_tick_fn {
                let status = unsafe { cb(tick_count) };
                if let Err(safety_err) =
                    crate::safety::check_ffi_status(&loaded.manifest.id, status)
                {
                    eprintln!(
                        "[redox-mc-loader] ERROR: Mod '{}' on_server_tick failed: {}",
                        loaded.manifest.id, safety_err
                    );
                }
            }
        }
    }

    /// Dispatches a player chat event to all loaded mods.
    /// Returns true if allowed, false if cancelled by any loaded mod.
    pub fn dispatch_server_chat(&self, player_name: &str, message: &str) -> bool {
        let ffi_player = redox_mc_abi::types::FfiStr::from_str(player_name);
        let ffi_msg = redox_mc_abi::types::FfiStr::from_str(message);

        for loaded in &self.loaded_mods {
            if let Some(cb) = loaded.on_server_chat_fn {
                let res = unsafe { cb(ffi_player, ffi_msg) };
                match crate::safety::check_ffi_result(&loaded.manifest.id, res) {
                    Ok(allow) => {
                        if !allow {
                            return false;
                        }
                    }
                    Err(safety_err) => {
                        eprintln!(
                            "[redox-mc-loader] ERROR: Mod '{}' on_server_chat failed: {}",
                            loaded.manifest.id, safety_err
                        );
                    }
                }
            }
        }
        true
    }

    pub fn loaded_mods(&self) -> &[Arc<LoadedMod>] {
        &self.loaded_mods
    }

    /// Number of retired generations kept mapped for process lifetime.
    pub fn retired_mod_count(&self) -> usize {
        self.retired_mods.len()
    }

    /// Collects all registered item definitions from loaded mods in deterministic order.
    pub fn collect_registered_items(&self) -> Vec<crate::registry::RegisteredItemEntry> {
        type GetItemCountFn = unsafe extern "C" fn() -> usize;
        type GetItemAtFn = unsafe extern "C" fn(
            usize,
            *mut redox_mc_abi::types::FfiStr,
            *mut u32,
            *mut u32,
            *mut bool,
            *mut u32,
        ) -> redox_mc_abi::types::FfiStatus;

        let mut items = Vec::new();

        for loaded in &self.loaded_mods {
            let get_count_sym = unsafe {
                loaded
                    ._library
                    .get::<GetItemCountFn>(b"redox_mod_get_item_count\0")
            };
            let get_at_sym = unsafe {
                loaded
                    ._library
                    .get::<GetItemAtFn>(b"redox_mod_get_item_at\0")
            };

            if let (Ok(get_count), Ok(get_at)) = (get_count_sym, get_at_sym) {
                let count = unsafe { get_count() };
                for i in 0..count {
                    let mut ffi_id = redox_mc_abi::types::FfiStr::EMPTY;
                    let mut max_stack = 64u32;
                    let mut durability = 0u32;
                    let mut fire_resistant = false;
                    let mut behavior_flags = 0u32;

                    let status = unsafe {
                        get_at(
                            i,
                            &mut ffi_id,
                            &mut max_stack,
                            &mut durability,
                            &mut fire_resistant,
                            &mut behavior_flags,
                        )
                    };

                    if status.is_success() {
                        if let Ok(id_str) = unsafe { ffi_id.as_str() } {
                            items.push(crate::registry::RegisteredItemEntry {
                                id: id_str.to_string(),
                                max_stack_size: max_stack,
                                durability: if durability > 0 {
                                    Some(durability)
                                } else {
                                    None
                                },
                                fire_resistant,
                                behavior_flags,
                            });
                        }
                    } else {
                        eprintln!(
                            "[redox-mc-loader] ERROR: Mod '{}' redox_mod_get_item_at({}) returned status {:?}",
                            loaded.manifest.id, i, status
                        );
                    }
                }
            }
        }

        items
    }

    /// Collects all registered block definitions from loaded mods in deterministic order.
    pub fn collect_registered_blocks(&self) -> Vec<crate::registry::RegisteredBlockEntry> {
        type GetBlockCountFn = unsafe extern "C" fn() -> usize;
        type GetBlockAtFn = unsafe extern "C" fn(
            usize,
            *mut redox_mc_abi::types::FfiStr,
            *mut f32,
            *mut f32,
            *mut bool,
            *mut bool,
            *mut u32,
            *mut bool,
        ) -> redox_mc_abi::types::FfiStatus;

        let mut blocks = Vec::new();

        for loaded in &self.loaded_mods {
            let get_count_sym = unsafe {
                loaded
                    ._library
                    .get::<GetBlockCountFn>(b"redox_mod_get_block_count\0")
            };
            let get_at_sym = unsafe {
                loaded
                    ._library
                    .get::<GetBlockAtFn>(b"redox_mod_get_block_at\0")
            };

            if let (Ok(get_count), Ok(get_at)) = (get_count_sym, get_at_sym) {
                let count = unsafe { get_count() };
                for i in 0..count {
                    let mut ffi_id = redox_mc_abi::types::FfiStr::EMPTY;
                    let mut destroy_time = 1.5f32;
                    let mut explosion_resistance = 6.0f32;
                    let mut requires_tool = false;
                    let mut no_collision = false;
                    let mut behavior_flags = 0u32;
                    let mut has_item = true;

                    let status = unsafe {
                        get_at(
                            i,
                            &mut ffi_id,
                            &mut destroy_time,
                            &mut explosion_resistance,
                            &mut requires_tool,
                            &mut no_collision,
                            &mut behavior_flags,
                            &mut has_item,
                        )
                    };

                    if status.is_success() {
                        if let Ok(id_str) = unsafe { ffi_id.as_str() } {
                            blocks.push(crate::registry::RegisteredBlockEntry {
                                id: id_str.to_string(),
                                destroy_time,
                                explosion_resistance,
                                requires_tool,
                                no_collision,
                                behavior_flags,
                                has_item,
                            });
                        }
                    } else {
                        eprintln!(
                            "[redox-mc-loader] ERROR: Mod '{}' redox_mod_get_block_at({}) returned status {:?}",
                            loaded.manifest.id, i, status
                        );
                    }
                }
            }
        }

        blocks
    }

    /// Collects and registers all virtual resources from loaded mods in deterministic order.
    pub fn collect_registered_resources(&self) {
        type GetResourceCountFn = unsafe extern "C" fn() -> usize;
        type GetResourceAtFn = unsafe extern "C" fn(
            usize,
            *mut redox_mc_abi::types::FfiStr,
            *mut redox_mc_abi::types::FfiStr,
            *mut redox_mc_abi::types::FfiStr,
            *mut *const u8,
            *mut usize,
        ) -> redox_mc_abi::types::FfiStatus;

        for loaded in &self.loaded_mods {
            let get_count_sym = unsafe {
                loaded
                    ._library
                    .get::<GetResourceCountFn>(b"redox_mod_get_resource_count\0")
            };
            let get_at_sym = unsafe {
                loaded
                    ._library
                    .get::<GetResourceAtFn>(b"redox_mod_get_resource_at\0")
            };

            if let (Ok(get_count), Ok(get_at)) = (get_count_sym, get_at_sym) {
                let count = unsafe { get_count() };
                for i in 0..count {
                    let mut pack_type = redox_mc_abi::types::FfiStr::EMPTY;
                    let mut namespace = redox_mc_abi::types::FfiStr::EMPTY;
                    let mut path = redox_mc_abi::types::FfiStr::EMPTY;
                    let mut data_ptr: *const u8 = std::ptr::null();
                    let mut data_len: usize = 0;

                    let status = unsafe {
                        get_at(
                            i,
                            &mut pack_type,
                            &mut namespace,
                            &mut path,
                            &mut data_ptr,
                            &mut data_len,
                        )
                    };

                    if status == redox_mc_abi::types::FfiStatus::Success && !data_ptr.is_null() {
                        let pt = unsafe { pack_type.as_str().unwrap_or("data") };
                        let ns = unsafe { namespace.as_str().unwrap_or("") };
                        let p = unsafe { path.as_str().unwrap_or("") };
                        let bytes = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };

                        crate::registry::resource::register_virtual_resource(
                            pt,
                            ns,
                            p,
                            bytes.to_vec(),
                        );
                        println!(
                            "[redox-mc-loader] Mod '{}' registered virtual resource: [{}] {}/{}",
                            loaded.manifest.id, pt, ns, p
                        );
                    } else {
                        eprintln!(
                            "[redox-mc-loader] ERROR: Mod '{}' redox_mod_get_resource_at({}) returned status {:?}",
                            loaded.manifest.id, i, status
                        );
                    }
                }
            }
        }
    }
}

/// Copies a candidate binary into the mod cache, loads it, and resolves every
/// optional callback exported by the mod.
fn load_candidate(
    candidate: &ModCandidate,
    cache_dir: &Path,
    running_side: ModSide,
) -> Result<LoadedMod, ModLoaderError> {
    let manifest = &candidate.manifest;
    let cached_path = copy_binary_to_cache(&candidate.binary_path, cache_dir, &manifest.id)?;

    println!(
        "[redox-mc-loader] Loading native mod '{}' from {:?} (cached as {:?})...",
        manifest.id,
        candidate.binary_path.file_name().unwrap_or_default(),
        cached_path.file_name().unwrap_or_default()
    );

    // Load the cached copy, never the project binary. This keeps the original
    // file replaceable while the JVM is running, including on Windows.
    let lib = unsafe {
        libloading::Library::new(&cached_path)
            .map_err(|e| ModLoaderError::LibraryLoad(cached_path.clone(), e))?
    };
    let boxed_lib = Box::new(lib);

    // 1. Check for modern v1 ABI entrypoint: redox_mod_init_v1 (ModInitFn)
    let init_v1_symbol =
        unsafe { boxed_lib.get::<redox_mc_abi::vtable::ModInitFn>(b"redox_mod_init_v1\0") };

    let mut vtable_opt: Option<&'static redox_mc_abi::vtable::ModVTable> = None;

    if let Ok(init_v1_fn) = init_v1_symbol {
        static LOADER_VTABLE: redox_mc_abi::vtable::LoaderVTable =
            redox_mc_abi::vtable::LoaderVTable {
                log: default_loader_log,
                register_event_listener: default_register_event_listener,
                free_string: redox_mc_abi::memory::redox_loader_free_string,
                free_buffer: redox_mc_abi::memory::redox_loader_free_buffer,
            };

        let context = redox_mc_abi::vtable::LoaderContext {
            abi_version: redox_mc_abi::REDOX_ABI_VERSION,
            loader_version_major: 0,
            loader_version_minor: 1,
            loader_version_patch: 0,
            side: match running_side {
                ModSide::Both => redox_mc_abi::types::FfiModSide::Both,
                ModSide::Client => redox_mc_abi::types::FfiModSide::Client,
                ModSide::Server => redox_mc_abi::types::FfiModSide::Server,
            },
            _reserved: [0; 7],
            vtable: &LOADER_VTABLE,
            loader_handle: std::ptr::null_mut(),
        };

        let res = unsafe { init_v1_fn(&context) };
        if let Err(_safety_err) = crate::safety::check_ffi_result(&manifest.id, res) {
            return Err(ModLoaderError::InitFailed(
                manifest.id.clone(),
                res.status as i32,
            ));
        }

        if res.is_ok() && !res.value.is_null() {
            let vtable_ref = unsafe { &*res.value };
            vtable_opt = Some(vtable_ref);
        }
    } else {
        // 2. Fallback to Phase 1 redox_mod_init() -> i32
        type InitFn = unsafe extern "C" fn() -> i32;
        let init_symbol = unsafe { boxed_lib.get::<InitFn>(b"redox_mod_init\0") };

        if let Ok(init_fn) = init_symbol {
            let code = unsafe { init_fn() };
            if let Err(_safety_err) = crate::safety::check_ffi_code(&manifest.id, code) {
                return Err(ModLoaderError::InitFailed(manifest.id.clone(), code));
            }
        } else {
            println!(
                "[redox-mc-loader] Mod '{}' has no 'redox_mod_init' or 'redox_mod_init_v1' symbol. Proceeding.",
                manifest.id
            );
        }
    }

    // Resolve bootstrap callback (from vtable or explicit symbol)
    let on_bootstrap_fn = if let Some(vtable) = vtable_opt {
        vtable.on_bootstrap
    } else {
        type BootstrapFn = unsafe extern "C" fn() -> redox_mc_abi::types::FfiStatus;
        let bootstrap_symbol = unsafe { boxed_lib.get::<BootstrapFn>(b"redox_mod_on_bootstrap\0") };
        bootstrap_symbol.ok().map(|s| *s)
    };

    // Resolve register_content callback (from vtable or explicit symbol)
    let on_register_content_fn = if let Some(vtable) = vtable_opt {
        vtable.on_register_content
    } else {
        type RegFn = unsafe extern "C" fn() -> redox_mc_abi::types::FfiStatus;
        let reg_symbol = unsafe { boxed_lib.get::<RegFn>(b"redox_mod_on_register_content\0") };
        reg_symbol.ok().map(|s| *s)
    };

    // Resolve server_tick callback (from vtable or explicit symbol)
    let on_server_tick_fn = if let Some(vtable) = vtable_opt {
        vtable.on_server_tick
    } else {
        type TickFn = unsafe extern "C" fn(i64) -> redox_mc_abi::types::FfiStatus;
        let tick_symbol = unsafe { boxed_lib.get::<TickFn>(b"redox_mod_on_server_tick\0") };
        tick_symbol.ok().map(|s| *s)
    };

    // Resolve server_chat callback (from vtable or explicit symbol)
    let on_server_chat_fn = if let Some(vtable) = vtable_opt {
        vtable.on_server_chat
    } else {
        type ChatFn = unsafe extern "C" fn(
            redox_mc_abi::types::FfiStr,
            redox_mc_abi::types::FfiStr,
        ) -> redox_mc_abi::types::FfiResult<bool>;
        let chat_symbol = unsafe { boxed_lib.get::<ChatFn>(b"redox_mod_on_server_chat\0") };
        chat_symbol.ok().map(|s| *s)
    };

    // Resolve item interaction callbacks
    type ItemUseFn = unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32) -> i32;
    let on_item_use_fn = unsafe {
        boxed_lib
            .get::<ItemUseFn>(b"redox_mod_on_item_use\0")
            .ok()
            .map(|s| *s)
    };

    type ItemUseOnFn =
        unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32, i32, i32) -> i32;
    let on_item_use_on_fn = unsafe {
        boxed_lib
            .get::<ItemUseOnFn>(b"redox_mod_on_item_use_on\0")
            .ok()
            .map(|s| *s)
    };

    type ItemFinishFn = unsafe extern "C" fn(redox_mc_abi::types::FfiStr);
    let on_item_finish_using_fn = unsafe {
        boxed_lib
            .get::<ItemFinishFn>(b"redox_mod_on_item_finish_using\0")
            .ok()
            .map(|s| *s)
    };

    // Resolve block interaction callbacks
    type BlockUseFn = unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32, i32) -> i32;
    let on_block_use_without_item_fn = unsafe {
        boxed_lib
            .get::<BlockUseFn>(b"redox_mod_on_block_use_without_item\0")
            .ok()
            .map(|s| *s)
    };

    type BlockStepFn = unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32);
    let on_block_step_on_fn = unsafe {
        boxed_lib
            .get::<BlockStepFn>(b"redox_mod_on_block_step_on\0")
            .ok()
            .map(|s| *s)
    };

    type BlockDestroyFn = unsafe extern "C" fn(redox_mc_abi::types::FfiStr, i32, i32, i32);
    let on_block_destroy_fn = unsafe {
        boxed_lib
            .get::<BlockDestroyFn>(b"redox_mod_on_block_destroy\0")
            .ok()
            .map(|s| *s)
    };

    // Resolve typed network channel callbacks (Task 6.3).
    let dispatch_network_payload_fn = unsafe {
        boxed_lib
            .get::<redox_mc_abi::types::NetworkDispatchFn>(b"redox_mod_dispatch_network_payload\0")
            .ok()
            .map(|s| *s)
    };
    let drain_network_outbox_fn = unsafe {
        boxed_lib
            .get::<redox_mc_abi::types::NetworkOutboxDrainFn>(b"redox_mod_drain_network_outbox\0")
            .ok()
            .map(|s| *s)
    };
    let set_running_side_fn = unsafe {
        boxed_lib
            .get::<unsafe extern "C" fn(u32) -> redox_mc_abi::types::FfiStatus>(
                b"redox_mod_set_running_side\0",
            )
            .ok()
            .map(|s| *s)
    };
    let set_network_side_fn = unsafe {
        boxed_lib
            .get::<unsafe extern "C" fn(u32) -> redox_mc_abi::types::FfiStatus>(
                b"redox_mod_set_network_side\0",
            )
            .ok()
            .map(|s| *s)
    };

    Ok(LoadedMod {
        manifest: candidate.manifest.clone(),
        binary_path: cached_path,
        _library: boxed_lib,
        on_bootstrap_fn,
        on_register_content_fn,
        on_server_tick_fn,
        on_server_chat_fn,
        on_item_use_fn,
        on_item_use_on_fn,
        on_item_finish_using_fn,
        on_block_use_without_item_fn,
        on_block_step_on_fn,
        on_block_destroy_fn,
        dispatch_network_payload_fn,
        drain_network_outbox_fn,
        set_running_side_fn,
        set_network_side_fn,
    })
}

impl Default for ModManager {
    fn default() -> Self {
        Self::new()
    }
}

unsafe extern "C" fn default_loader_log(
    level: redox_mc_abi::types::FfiLogLevel,
    target: redox_mc_abi::types::FfiStr,
    msg: redox_mc_abi::types::FfiStr,
) {
    let t_str = unsafe { target.as_str().unwrap_or("unknown") };
    let m_str = unsafe { msg.as_str().unwrap_or("") };
    println!("[{}] [{:?}] {}", t_str, level, m_str);
}

unsafe extern "C" fn default_register_event_listener(
    _event_id: u32,
    _callback: unsafe extern "C" fn(*const ()),
    _user_data: *mut (),
) -> redox_mc_abi::types::FfiStatus {
    redox_mc_abi::types::FfiStatus::Success
}

// Global ModManager instance
pub(crate) static MOD_MANAGER: Mutex<Option<ModManager>> = Mutex::new(None);

/// Side on which this JVM is running, detected from the game classloader.
static RUNNING_SIDE: OnceLock<ModSide> = OnceLock::new();

pub fn set_running_side(side: ModSide) {
    let _ = RUNNING_SIDE.set(side);
}

/// Returns the detected running side (defaults to server before detection).
pub fn running_side() -> ModSide {
    RUNNING_SIDE.get().copied().unwrap_or(ModSide::Server)
}

/// Notifies every loaded mod which side owns outbound messages queued by the
/// next callback on this thread.
pub fn notify_network_side(side: ModSide) {
    let snapshot = dispatcher::snapshot();
    let callbacks: Vec<_> = snapshot
        .iter()
        .filter_map(|loaded| {
            loaded
                .set_network_side_fn
                .map(|cb| (loaded.manifest.id.clone(), cb))
        })
        .collect();

    let abi_side = crate::network::side_to_abi(side);
    for (id, callback) in callbacks {
        let status = unsafe { callback(abi_side) };
        if let Err(safety_err) = crate::safety::check_ffi_status(&id, status) {
            eprintln!(
                "[redox-mc-loader] ERROR: Mod '{}' failed to set network side: {}",
                id, safety_err
            );
        }
    }
}

/// Notifies every loaded mod of the process-wide running side.
pub fn notify_running_side(side: ModSide) {
    let snapshot = dispatcher::snapshot();
    let callbacks: Vec<_> = snapshot
        .iter()
        .filter_map(|loaded| {
            loaded
                .set_running_side_fn
                .map(|cb| (loaded.manifest.id.clone(), cb))
        })
        .collect();

    let abi_side = crate::network::side_to_abi(side);
    for (id, callback) in callbacks {
        let status = unsafe { callback(abi_side) };
        if let Err(safety_err) = crate::safety::check_ffi_status(&id, status) {
            eprintln!(
                "[redox-mc-loader] ERROR: Mod '{}' failed to set running side: {}",
                id, safety_err
            );
        }
    }
}

/// Dispatches an inbound mod channel payload to loader-local handlers and to
/// every loaded mod in deterministic order.
///
/// # Safety
/// `data` is only borrowed for the duration of the call; mods must copy anything
/// they need to retain.
pub unsafe fn dispatch_network_payload(
    channel: &str,
    data: &[u8],
    sender_name: &str,
    sender_uuid: &str,
    side: ModSide,
) {
    notify_network_side(side);
    crate::network::dispatch_local_handlers(channel, data);

    let snapshot = dispatcher::snapshot();
    let callbacks: Vec<(String, redox_mc_abi::types::NetworkDispatchFn)> = snapshot
        .iter()
        .filter_map(|loaded| {
            loaded
                .dispatch_network_payload_fn
                .map(|cb| (loaded.manifest.id.clone(), cb))
        })
        .collect();

    let ffi_channel = redox_mc_abi::types::FfiStr::from_str(channel);
    let ffi_data = redox_mc_abi::types::FfiSlice::from_slice(data);
    let ffi_sender_name = redox_mc_abi::types::FfiStr::from_str(sender_name);
    let ffi_sender_uuid = redox_mc_abi::types::FfiStr::from_str(sender_uuid);
    let ffi_side = crate::network::side_to_abi(side);

    for (id, cb) in callbacks {
        let status = unsafe {
            cb(
                ffi_channel,
                ffi_data,
                ffi_sender_name,
                ffi_sender_uuid,
                ffi_side,
            )
        };
        if let Err(safety_err) = crate::safety::check_ffi_status(&id, status) {
            eprintln!(
                "[redox-mc-loader] ERROR: Mod '{}' network dispatch failed: {}",
                id, safety_err
            );
        }
    }
}

/// Returns the outbox drain callbacks for all loaded mods in deterministic order.
pub fn network_drain_callbacks() -> Vec<(String, redox_mc_abi::types::NetworkOutboxDrainFn)> {
    let snapshot = dispatcher::snapshot();
    snapshot
        .iter()
        .filter_map(|loaded| {
            loaded
                .drain_network_outbox_fn
                .map(|cb| (loaded.manifest.id.clone(), cb))
        })
        .collect()
}

pub fn get_or_init_manager() -> &'static Mutex<Option<ModManager>> {
    &MOD_MANAGER
}

/// Collects all registered item definitions from loaded mods in deterministic order.
pub fn collect_registered_items() -> Vec<crate::registry::RegisteredItemEntry> {
    let guard = MOD_MANAGER.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(manager) = guard.as_ref() {
        manager.collect_registered_items()
    } else {
        Vec::new()
    }
}

/// Collects all registered block definitions from loaded mods in deterministic order.
pub fn collect_registered_blocks() -> Vec<crate::registry::RegisteredBlockEntry> {
    let guard = MOD_MANAGER.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(manager) = guard.as_ref() {
        manager.collect_registered_blocks()
    } else {
        Vec::new()
    }
}

/// Collects and registers all virtual resources from loaded mods in deterministic order.
pub fn collect_registered_resources() {
    let guard = MOD_MANAGER.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(manager) = guard.as_ref() {
        manager.collect_registered_resources();
    }
}

/// Called from RedoxBridge.onBootstrapEntry().
///
/// `running_side` is detected from the game classloader before mods are loaded.
pub fn on_bootstrap_entry(running_side: ModSide) {
    set_running_side(running_side);
    println!(
        "[redox-mc-loader] Running side detected as: {:?}",
        running_side
    );

    {
        let mut guard = MOD_MANAGER.lock().unwrap_or_else(|p| p.into_inner());
        let manager = guard.get_or_insert_with(ModManager::new);

        if !manager.initialized {
            let mods_dir = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("mods");

            let loader_ver =
                Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 1, 0));

            if let Err(err) = manager.load_mods(&mods_dir, running_side, &loader_ver) {
                eprintln!(
                    "[redox-mc-loader] ERROR: Failed during mod loading: {}",
                    err
                );
            }
        }
    }

    // Mods are loaded; publish the process-wide side before any callback runs.
    notify_running_side(running_side);
    dispatch_bootstrap();
}

/// Dispatches the Minecraft bootstrap hook to the current callback snapshot.
///
/// The snapshot is loaded through `ArcSwap` and is therefore safe to use while
/// another thread hot-reloads the mods.
pub fn dispatch_bootstrap() {
    let snapshot = dispatcher::snapshot();
    if snapshot.is_empty() {
        return;
    }

    println!(
        "[redox-mc-loader] Dispatching Bootstrap.bootStrap() hook to {} loaded mod(s)...",
        snapshot.len()
    );

    for loaded in snapshot.iter() {
        if let Some(on_bootstrap) = loaded.on_bootstrap_fn {
            let status = unsafe { on_bootstrap() };
            if let Err(safety_err) = crate::safety::check_ffi_status(&loaded.manifest.id, status) {
                eprintln!(
                    "[redox-mc-loader] ERROR: Mod '{}' on_bootstrap failed: {}",
                    loaded.manifest.id, safety_err
                );
            }
        }
    }
}

/// Called from RedoxBridge.onRegisterContent()
pub fn on_register_content() {
    let snapshot = dispatcher::snapshot();
    if snapshot.is_empty() {
        return;
    }

    println!(
        "[redox-mc-loader] Dispatching BuiltInRegistries.freeze() hook to {} loaded mod(s)...",
        snapshot.len()
    );

    for loaded in snapshot.iter() {
        if let Some(on_register) = loaded.on_register_content_fn {
            let status = unsafe { on_register() };
            if let Err(safety_err) = crate::safety::check_ffi_status(&loaded.manifest.id, status) {
                eprintln!(
                    "[redox-mc-loader] ERROR: Mod '{}' on_register_content failed: {}",
                    loaded.manifest.id, safety_err
                );
            }
        }
    }
}

/// Called from RedoxBridge.onServerTick(long tickCount)
pub fn on_server_tick(tick_count: i64) {
    notify_network_side(ModSide::Server);

    let snapshot = dispatcher::snapshot();
    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_server_tick_fn {
            let status = unsafe { cb(tick_count) };
            if let Err(safety_err) = crate::safety::check_ffi_status(&loaded.manifest.id, status) {
                eprintln!(
                    "[redox-mc-loader] ERROR: Mod '{}' on_server_tick failed: {}",
                    loaded.manifest.id, safety_err
                );
            }
        }
    }
}

/// Called from RedoxBridge.onServerChat(String playerName, String message)
/// Returns true if allowed, false if cancelled by any loaded mod.
pub fn on_server_chat(player_name: &str, message: &str) -> bool {
    notify_network_side(ModSide::Server);

    let snapshot = dispatcher::snapshot();
    let ffi_player = redox_mc_abi::types::FfiStr::from_str(player_name);
    let ffi_msg = redox_mc_abi::types::FfiStr::from_str(message);

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_server_chat_fn {
            let res = unsafe { cb(ffi_player, ffi_msg) };
            match crate::safety::check_ffi_result(&loaded.manifest.id, res) {
                Ok(allow) => {
                    if !allow {
                        return false;
                    }
                }
                Err(safety_err) => {
                    eprintln!(
                        "[redox-mc-loader] ERROR: Mod '{}' on_server_chat failed: {}",
                        loaded.manifest.id, safety_err
                    );
                }
            }
        }
    }
    true
}

/// Dispatches item use to loaded mods.
/// Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
pub fn on_item_use(item_id: &str, hand: i32) -> i32 {
    notify_network_side(ModSide::Server);
    let ffi_id = redox_mc_abi::types::FfiStr::from_str(item_id);
    let snapshot = dispatcher::snapshot();

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_item_use_fn {
            let res = unsafe { cb(ffi_id, hand) };
            if res != 0 {
                return res;
            }
        }
    }
    0
}

/// Dispatches item useOn to loaded mods.
/// Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
pub fn on_item_use_on(item_id: &str, x: i32, y: i32, z: i32, face: i32, hand: i32) -> i32 {
    notify_network_side(ModSide::Server);
    let ffi_id = redox_mc_abi::types::FfiStr::from_str(item_id);
    let snapshot = dispatcher::snapshot();

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_item_use_on_fn {
            let res = unsafe { cb(ffi_id, x, y, z, face, hand) };
            if res != 0 {
                return res;
            }
        }
    }
    0
}

/// Dispatches item finish using to loaded mods.
pub fn on_item_finish_using(item_id: &str) {
    notify_network_side(ModSide::Server);
    let ffi_id = redox_mc_abi::types::FfiStr::from_str(item_id);
    let snapshot = dispatcher::snapshot();

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_item_finish_using_fn {
            unsafe { cb(ffi_id) };
        }
    }
}

/// Dispatches block useWithoutItem to loaded mods.
/// Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
pub fn on_block_use_without_item(block_id: &str, x: i32, y: i32, z: i32, direction: i32) -> i32 {
    notify_network_side(ModSide::Server);
    let ffi_id = redox_mc_abi::types::FfiStr::from_str(block_id);
    let snapshot = dispatcher::snapshot();

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_block_use_without_item_fn {
            let res = unsafe { cb(ffi_id, x, y, z, direction) };
            if res != 0 {
                return res;
            }
        }
    }
    0
}

/// Dispatches block stepOn to loaded mods.
pub fn on_block_step_on(block_id: &str, x: i32, y: i32, z: i32) {
    notify_network_side(ModSide::Server);
    let ffi_id = redox_mc_abi::types::FfiStr::from_str(block_id);
    let snapshot = dispatcher::snapshot();

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_block_step_on_fn {
            unsafe { cb(ffi_id, x, y, z) };
        }
    }
}

/// Dispatches block destroy to loaded mods.
pub fn on_block_destroy(block_id: &str, x: i32, y: i32, z: i32) {
    notify_network_side(ModSide::Server);
    let ffi_id = redox_mc_abi::types::FfiStr::from_str(block_id);
    let snapshot = dispatcher::snapshot();

    for loaded in snapshot.iter() {
        if let Some(cb) = loaded.on_block_destroy_fn {
            unsafe { cb(ffi_id, x, y, z) };
        }
    }
}

/// Returns the list of loaded mods as `(mod_id, version)` pairs.
pub fn get_loaded_mods_info() -> Vec<(String, String)> {
    dispatcher::snapshot()
        .iter()
        .map(|m| (m.manifest.id.clone(), m.manifest.version.to_string()))
        .collect()
}

/// Returns the mods that the remote side must also have installed.
///
/// Only `side = "both"` mods participate in the handshake. Server-only mods are
/// not required on clients, and client-only mods are not required on servers.
pub fn get_handshake_mods_info() -> Vec<(String, String)> {
    dispatcher::snapshot()
        .iter()
        .filter(|m| m.manifest.side == ModSide::Both)
        .map(|m| (m.manifest.id.clone(), m.manifest.version.to_string()))
        .collect()
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
