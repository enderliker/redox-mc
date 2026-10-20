//! In-memory virtual resource registry and default metadata provider.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

/// Key identifying an in-memory virtual resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceKey {
    /// "data" or "assets"
    pub pack_type: String,
    /// e.g. "testmod"
    pub namespace: String,
    /// e.g. "recipes/ruby.json"
    pub path: String,
}

static VIRTUAL_RESOURCES: OnceLock<Mutex<HashMap<ResourceKey, Vec<u8>>>> = OnceLock::new();
static VIRTUAL_ROOT_RESOURCES: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

fn virtual_resources() -> &'static Mutex<HashMap<ResourceKey, Vec<u8>>> {
    VIRTUAL_RESOURCES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn virtual_root_resources() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    VIRTUAL_ROOT_RESOURCES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registers an in-memory virtual resource.
pub fn register_virtual_resource(
    pack_type: impl Into<String>,
    namespace: impl Into<String>,
    path: impl Into<String>,
    data: Vec<u8>,
) {
    let key = ResourceKey {
        pack_type: pack_type.into(),
        namespace: namespace.into(),
        path: path.into(),
    };
    virtual_resources().lock().unwrap().insert(key, data);
}

/// Registers an in-memory root pack resource (e.g. custom "pack.mcmeta").
pub fn register_virtual_root_resource(path: impl Into<String>, data: Vec<u8>) {
    virtual_root_resources()
        .lock()
        .unwrap()
        .insert(path.into(), data);
}

/// Retrieves an in-memory virtual resource, if registered.
pub fn get_virtual_resource(pack_type: &str, namespace: &str, path: &str) -> Option<Vec<u8>> {
    let key = ResourceKey {
        pack_type: pack_type.to_string(),
        namespace: namespace.to_string(),
        path: path.to_string(),
    };
    virtual_resources().lock().unwrap().get(&key).cloned()
}

/// Retrieves a root resource. If "pack.mcmeta" is requested and not registered,
/// generates and returns default pack metadata supporting formats 97 (client) and 121 (server data).
pub fn get_virtual_root_resource(path: &str) -> Option<Vec<u8>> {
    if let Some(data) = virtual_root_resources().lock().unwrap().get(path) {
        return Some(data.clone());
    }

    if path == "pack.mcmeta" {
        return Some(
            br#"{"pack":{"description":"Redox Virtual Mod Resources","min_format":97,"max_format":121}}"#
                .to_vec(),
        );
    }

    None
}

/// Lists all relative resource paths under `directory` for `(pack_type, namespace)`.
pub fn list_virtual_resources(pack_type: &str, namespace: &str, directory: &str) -> Vec<String> {
    let reg = virtual_resources().lock().unwrap();
    let dir_prefix = if directory.is_empty() || directory.ends_with('/') {
        directory.to_string()
    } else {
        format!("{}/", directory)
    };

    let mut paths = Vec::new();
    for key in reg.keys() {
        if key.pack_type == pack_type
            && key.namespace == namespace
            && (directory.is_empty() || key.path.starts_with(&dir_prefix) || key.path == directory)
        {
            paths.push(key.path.clone());
        }
    }
    paths.sort();
    paths
}

/// Returns all distinct namespaces registered for `pack_type`.
pub fn get_virtual_namespaces(pack_type: &str) -> Vec<String> {
    let reg = virtual_resources().lock().unwrap();
    let mut set = HashSet::new();
    for key in reg.keys() {
        if key.pack_type == pack_type {
            set.insert(key.namespace.clone());
        }
    }
    let mut list: Vec<String> = set.into_iter().collect();
    list.sort();
    list
}

/// Clears all virtual resources (used for test isolation).
pub fn clear_virtual_resources() {
    virtual_resources().lock().unwrap().clear();
    virtual_root_resources().lock().unwrap().clear();
}
