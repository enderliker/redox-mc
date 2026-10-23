use super::strings::{bytes_to_jbyte_array, jstring_to_string, string_vec_to_jobject_array};
use jvmti_bindings::sys::jni;
use std::ffi::CString;
use std::ptr;

/// JNI native: net.redoxmc.bridge.RedoxBridge.getVirtualResource(String type, String namespace, String path)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getVirtualResource(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_type: jni::jstring,
    j_namespace: jni::jstring,
    j_path: jni::jstring,
) -> jni::jbyteArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_type.is_null() || j_namespace.is_null() || j_path.is_null() {
            return ptr::null_mut();
        }

        let type_str = jstring_to_string(env, j_type);
        let ns_str = jstring_to_string(env, j_namespace);
        let path_str = jstring_to_string(env, j_path);

        if let Some(data) =
            crate::registry::resource::get_virtual_resource(&type_str, &ns_str, &path_str)
        {
            bytes_to_jbyte_array(env, &data)
        } else {
            ptr::null_mut()
        }
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.listVirtualResources(String type, String namespace, String directory)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_listVirtualResources(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_type: jni::jstring,
    j_namespace: jni::jstring,
    j_dir: jni::jstring,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_type.is_null() || j_namespace.is_null() || j_dir.is_null() {
            return ptr::null_mut();
        }

        let type_str = jstring_to_string(env, j_type);
        let ns_str = jstring_to_string(env, j_namespace);
        let dir_str = jstring_to_string(env, j_dir);

        let list = crate::registry::resource::list_virtual_resources(&type_str, &ns_str, &dir_str);
        string_vec_to_jobject_array(env, &list)
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getVirtualNamespaces(String type)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getVirtualNamespaces(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_type: jni::jstring,
) -> jni::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_type.is_null() {
            return ptr::null_mut();
        }

        let type_str = jstring_to_string(env, j_type);
        let list = crate::registry::resource::get_virtual_namespaces(&type_str);
        string_vec_to_jobject_array(env, &list)
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getVirtualRootResource(String path)
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getVirtualRootResource(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_path: jni::jstring,
) -> jni::jbyteArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if j_path.is_null() {
            return ptr::null_mut();
        }

        let path_str = jstring_to_string(env, j_path);
        if let Some(data) = crate::registry::resource::get_virtual_root_resource(&path_str) {
            bytes_to_jbyte_array(env, &data)
        } else {
            ptr::null_mut()
        }
    }));

    result.unwrap_or(ptr::null_mut())
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.getRegistryHash()
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_getRegistryHash(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
) -> jni::jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let hash = crate::registry::get_registry_hash().unwrap_or_default();
        let c_str = CString::new(hash).unwrap_or_default();
        ((**env).NewStringUTF)(env, c_str.as_ptr())
    }));
    result.unwrap_or(ptr::null_mut())
}
