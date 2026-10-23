use super::strings::{
    jobject_array_to_string_vec, jstring_to_string, string_to_jstring, string_vec_to_jobject_array,
};
use jvmti_bindings::sys::jni;
use std::ptr;

/// JNI native: net.redoxmc.bridge.RedoxBridge.getLoadedModIds()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getLoadedModIds(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mods = crate::mod_loader::get_handshake_mods_info();
        let ids: Vec<String> = mods.into_iter().map(|(id, _)| id).collect();
        string_vec_to_jobject_array(env, &ids)
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getLoadedModVersions()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getLoadedModVersions(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mods = crate::mod_loader::get_handshake_mods_info();
        let versions: Vec<String> = mods.into_iter().map(|(_, ver)| ver).collect();
        string_vec_to_jobject_array(env, &versions)
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getHandshakeManifest()
///
/// Returns a single consistent snapshot shaped as `[hash, id0, version0, ...]`.
/// The mod list is filtered to `side = "both"` mods, which are the only ones the
/// remote side must also have installed.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getHandshakeManifest(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let hash = crate::registry::get_registry_hash().unwrap_or_default();
        let mods = crate::mod_loader::get_handshake_mods_info();
        let mut manifest = Vec::with_capacity(1 + mods.len() * 2);
        manifest.push(hash);
        for (id, version) in mods {
            manifest.push(id);
            manifest.push(version);
        }
        string_vec_to_jobject_array(env, &manifest)
    }));
    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.validateClientHandshake(...)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_validateClientHandshake(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_client_hash: jni::jstring,
    j_client_mod_ids: jni::jobjectArray,
    j_client_mod_versions: jni::jobjectArray,
) -> jni::jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let client_hash = jstring_to_string(env, j_client_hash);
        let client_mod_ids = jobject_array_to_string_vec(env, j_client_mod_ids);
        let client_mod_versions = jobject_array_to_string_vec(env, j_client_mod_versions);
        if client_mod_ids.len() != client_mod_versions.len() {
            return string_to_jstring(
                env,
                "Malformed client handshake: mod id and version arrays differ in length."
                    .to_string(),
            );
        }

        let client_mods: Vec<crate::network::handshake::HandshakeModInfo> = client_mod_ids
            .into_iter()
            .zip(client_mod_versions)
            .map(|(id, version)| crate::network::handshake::HandshakeModInfo { id, version })
            .collect();

        let server_hash = crate::registry::get_registry_hash().unwrap_or_default();
        let server_mods: Vec<crate::network::handshake::HandshakeModInfo> =
            crate::mod_loader::get_handshake_mods_info()
                .into_iter()
                .map(|(id, version)| crate::network::handshake::HandshakeModInfo { id, version })
                .collect();

        match crate::network::handshake::validate_client_handshake(
            &server_hash,
            &server_mods,
            &client_hash,
            &client_mods,
        ) {
            Ok(()) => ptr::null_mut(),
            Err(reason) => string_to_jstring(env, reason),
        }
    }));
    result.unwrap_or(ptr::null_mut())
}
