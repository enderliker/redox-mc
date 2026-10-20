//! Virtual resource and asset registration API for native Redox-MC mods.

use std::sync::{Mutex, OnceLock};

/// An in-memory virtual resource (asset or data pack entry) declared by a mod.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VirtualResource {
    /// "assets" for client resource packs or "data" for server datapacks.
    pub pack_type: String,
    /// Mod namespace (e.g. "testmod").
    pub namespace: String,
    /// Relative path within namespace (e.g. "recipes/ruby.json", "loot_table/blocks/ruby_block.json").
    pub path: String,
    /// Raw bytes of the resource content.
    pub content: Vec<u8>,
}

static REGISTERED_RESOURCES: OnceLock<Mutex<Vec<VirtualResource>>> = OnceLock::new();

fn resources_registry() -> &'static Mutex<Vec<VirtualResource>> {
    REGISTERED_RESOURCES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Registers an in-memory virtual resource with an explicit pack type ("data" or "assets").
pub fn register_resource(
    pack_type: impl Into<String>,
    namespace: impl Into<String>,
    path: impl Into<String>,
    content: impl Into<Vec<u8>>,
) {
    let mut reg = resources_registry().lock().unwrap();
    reg.push(VirtualResource {
        pack_type: pack_type.into(),
        namespace: namespace.into(),
        path: path.into(),
        content: content.into(),
    });
}

/// Convenience helper to register a server data pack resource (`pack_type = "data"`).
pub fn register_data_resource(
    namespace: impl Into<String>,
    path: impl Into<String>,
    content: impl Into<Vec<u8>>,
) {
    register_resource("data", namespace, path, content);
}

/// Convenience helper to register a client asset resource (`pack_type = "assets"`).
pub fn register_asset_resource(
    namespace: impl Into<String>,
    path: impl Into<String>,
    content: impl Into<Vec<u8>>,
) {
    register_resource("assets", namespace, path, content);
}

/// Returns a copy of all currently registered virtual resources.
pub fn get_registered_resources() -> Vec<VirtualResource> {
    resources_registry().lock().unwrap().clone()
}

/// Clears the registered virtual resources registry (primarily for test isolation).
pub fn clear_registered_resources() {
    resources_registry().lock().unwrap().clear();
}

/// C-ABI exported symbol allowing the loader to query the number of registered virtual resources.
#[no_mangle]
pub extern "C" fn redox_mod_get_resource_count() -> usize {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        get_registered_resources().len()
    }))
    .unwrap_or(0)
}

/// C-ABI exported symbol allowing the loader to retrieve a virtual resource at `index`.
///
/// # Safety
/// Output pointers must point to valid, writable memory.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_get_resource_at(
    index: usize,
    out_pack_type: *mut redox_mc_abi::types::FfiStr,
    out_namespace: *mut redox_mc_abi::types::FfiStr,
    out_path: *mut redox_mc_abi::types::FfiStr,
    out_data_ptr: *mut *const u8,
    out_data_len: *mut usize,
) -> redox_mc_abi::types::FfiStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_pack_type.is_null()
            || out_namespace.is_null()
            || out_path.is_null()
            || out_data_ptr.is_null()
            || out_data_len.is_null()
        {
            return redox_mc_abi::types::FfiStatus::NullPointer;
        }

        let reg = resources_registry().lock().unwrap();
        if index >= reg.len() {
            return redox_mc_abi::types::FfiStatus::NotFound;
        }

        let res = &reg[index];
        *out_pack_type = redox_mc_abi::types::FfiStr::from_str(res.pack_type.as_str());
        *out_namespace = redox_mc_abi::types::FfiStr::from_str(res.namespace.as_str());
        *out_path = redox_mc_abi::types::FfiStr::from_str(res.path.as_str());
        *out_data_ptr = res.content.as_ptr();
        *out_data_len = res.content.len();

        redox_mc_abi::types::FfiStatus::Success
    }));

    result.unwrap_or(redox_mc_abi::types::FfiStatus::Panic)
}
