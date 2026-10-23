use super::events::*;
use super::handshake::*;
use super::network::*;
use super::registry::*;
use jvmti_bindings::sys::jni;
use std::ffi::CString;

/// Explicitly registers native methods for net.redoxmc.bridge.RedoxBridge using JNI RegisterNatives.
///
/// # Safety
/// `env` must be a valid pointer to a live JNIEnv for the calling thread.
pub unsafe fn register_bridge_natives(env: *mut jni::JNIEnv) -> Result<(), String> {
    let class_name = CString::new("net/redoxmc/bridge/RedoxBridge").unwrap();
    let bridge_class = ((**env).FindClass)(env, class_name.as_ptr());
    if bridge_class.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        return Err("Could not find class net/redoxmc/bridge/RedoxBridge".into());
    }

    let m_init = CString::new("init").unwrap();
    let sig_void = CString::new("()V").unwrap();

    let m_boot = CString::new("onBootstrapEntry").unwrap();
    let m_reg = CString::new("onRegisterContent").unwrap();
    let m_frozen = CString::new("onRegistriesFrozen").unwrap();

    let m_captured = CString::new("isClassLoaderCaptured").unwrap();
    let sig_bool = CString::new("()Z").unwrap();

    let m_get_cl = CString::new("getCapturedClassLoader").unwrap();
    let sig_get_cl = CString::new("()Ljava/lang/ClassLoader;").unwrap();

    let m_test_def = CString::new("testDefineClass").unwrap();
    let sig_test_def = CString::new("(Ljava/lang/String;[B)Z").unwrap();

    let m_tick = CString::new("onServerTick").unwrap();
    let sig_tick = CString::new("(J)V").unwrap();

    let m_chat = CString::new("onServerChat").unwrap();
    let sig_chat = CString::new("(Ljava/lang/String;Ljava/lang/String;)Z").unwrap();

    let m_item_use = CString::new("onItemUse").unwrap();
    let sig_item_use = CString::new("(Ljava/lang/String;I)I").unwrap();

    let m_item_use_on = CString::new("onItemUseOn").unwrap();
    let sig_item_use_on = CString::new("(Ljava/lang/String;IIIII)I").unwrap();

    let m_item_finish = CString::new("onItemFinishUsing").unwrap();
    let sig_item_finish = CString::new("(Ljava/lang/String;)V").unwrap();

    let m_block_use = CString::new("onBlockUseWithoutItem").unwrap();
    let sig_block_use = CString::new("(Ljava/lang/String;IIII)I").unwrap();

    let m_block_step = CString::new("onBlockStepOn").unwrap();
    let sig_block_step = CString::new("(Ljava/lang/String;III)V").unwrap();

    let m_block_destroy = CString::new("onBlockDestroy").unwrap();
    let sig_block_destroy = CString::new("(Ljava/lang/String;III)V").unwrap();

    let m_get_vresource = CString::new("getVirtualResource").unwrap();
    let sig_get_vresource =
        CString::new("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)[B").unwrap();

    let m_list_vresources = CString::new("listVirtualResources").unwrap();
    let sig_list_vresources =
        CString::new("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)[Ljava/lang/String;")
            .unwrap();

    let m_get_vnamespaces = CString::new("getVirtualNamespaces").unwrap();
    let sig_get_vnamespaces = CString::new("(Ljava/lang/String;)[Ljava/lang/String;").unwrap();

    let m_get_vroot = CString::new("getVirtualRootResource").unwrap();
    let sig_get_vroot = CString::new("(Ljava/lang/String;)[B").unwrap();

    let m_mod_payload = CString::new("onModCustomPayload").unwrap();
    let sig_mod_payload = CString::new("(Ljava/lang/String;Ljava/lang/String;[B)V").unwrap();

    let m_mod_payload_from = CString::new("onModCustomPayloadFrom").unwrap();
    let sig_mod_payload_from =
        CString::new("(Ljava/lang/String;[BLjava/lang/String;Ljava/lang/String;I)V").unwrap();

    let m_client_tick = CString::new("onClientTick").unwrap();

    let m_get_hash = CString::new("getRegistryHash").unwrap();
    let sig_get_hash = CString::new("()Ljava/lang/String;").unwrap();

    let m_get_mod_ids = CString::new("getLoadedModIds").unwrap();
    let sig_get_mod_ids = CString::new("()[Ljava/lang/String;").unwrap();

    let m_get_mod_versions = CString::new("getLoadedModVersions").unwrap();
    let sig_get_mod_versions = CString::new("()[Ljava/lang/String;").unwrap();

    let m_get_manifest = CString::new("getHandshakeManifest").unwrap();
    let sig_get_manifest = CString::new("()[Ljava/lang/String;").unwrap();

    let m_val_handshake = CString::new("validateClientHandshake").unwrap();
    let sig_val_handshake = CString::new(
        "(Ljava/lang/String;[Ljava/lang/String;[Ljava/lang/String;)Ljava/lang/String;",
    )
    .unwrap();

    let methods = [
        jni::JNINativeMethod {
            name: m_init.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_init as *mut _,
        },
        jni::JNINativeMethod {
            name: m_boot.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBootstrapEntry as *mut _,
        },
        jni::JNINativeMethod {
            name: m_reg.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onRegisterContent as *mut _,
        },
        jni::JNINativeMethod {
            name: m_frozen.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onRegistriesFrozen as *mut _,
        },
        jni::JNINativeMethod {
            name: m_tick.as_ptr() as *mut _,
            signature: sig_tick.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onServerTick as *mut _,
        },
        jni::JNINativeMethod {
            name: m_chat.as_ptr() as *mut _,
            signature: sig_chat.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onServerChat as *mut _,
        },
        jni::JNINativeMethod {
            name: m_item_use.as_ptr() as *mut _,
            signature: sig_item_use.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onItemUse as *mut _,
        },
        jni::JNINativeMethod {
            name: m_item_use_on.as_ptr() as *mut _,
            signature: sig_item_use_on.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onItemUseOn as *mut _,
        },
        jni::JNINativeMethod {
            name: m_item_finish.as_ptr() as *mut _,
            signature: sig_item_finish.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onItemFinishUsing as *mut _,
        },
        jni::JNINativeMethod {
            name: m_block_use.as_ptr() as *mut _,
            signature: sig_block_use.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBlockUseWithoutItem as *mut _,
        },
        jni::JNINativeMethod {
            name: m_block_step.as_ptr() as *mut _,
            signature: sig_block_step.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBlockStepOn as *mut _,
        },
        jni::JNINativeMethod {
            name: m_block_destroy.as_ptr() as *mut _,
            signature: sig_block_destroy.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onBlockDestroy as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_vresource.as_ptr() as *mut _,
            signature: sig_get_vresource.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getVirtualResource as *mut _,
        },
        jni::JNINativeMethod {
            name: m_list_vresources.as_ptr() as *mut _,
            signature: sig_list_vresources.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_listVirtualResources as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_vnamespaces.as_ptr() as *mut _,
            signature: sig_get_vnamespaces.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getVirtualNamespaces as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_vroot.as_ptr() as *mut _,
            signature: sig_get_vroot.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getVirtualRootResource as *mut _,
        },
        jni::JNINativeMethod {
            name: m_mod_payload.as_ptr() as *mut _,
            signature: sig_mod_payload.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayload as *mut _,
        },
        jni::JNINativeMethod {
            name: m_mod_payload_from.as_ptr() as *mut _,
            signature: sig_mod_payload_from.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayloadFrom as *mut _,
        },
        jni::JNINativeMethod {
            name: m_client_tick.as_ptr() as *mut _,
            signature: sig_void.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_onClientTick as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_hash.as_ptr() as *mut _,
            signature: sig_get_hash.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getRegistryHash as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_mod_ids.as_ptr() as *mut _,
            signature: sig_get_mod_ids.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getLoadedModIds as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_mod_versions.as_ptr() as *mut _,
            signature: sig_get_mod_versions.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getLoadedModVersions as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_manifest.as_ptr() as *mut _,
            signature: sig_get_manifest.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getHandshakeManifest as *mut _,
        },
        jni::JNINativeMethod {
            name: m_val_handshake.as_ptr() as *mut _,
            signature: sig_val_handshake.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_validateClientHandshake as *mut _,
        },
        jni::JNINativeMethod {
            name: m_captured.as_ptr() as *mut _,
            signature: sig_bool.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_isClassLoaderCaptured as *mut _,
        },
        jni::JNINativeMethod {
            name: m_get_cl.as_ptr() as *mut _,
            signature: sig_get_cl.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_getCapturedClassLoader as *mut _,
        },
        jni::JNINativeMethod {
            name: m_test_def.as_ptr() as *mut _,
            signature: sig_test_def.as_ptr() as *mut _,
            fnPtr: Java_net_redoxmc_bridge_RedoxBridge_testDefineClass as *mut _,
        },
    ];

    let res = ((**env).RegisterNatives)(
        env,
        bridge_class,
        methods.as_ptr(),
        methods.len() as jni::jint,
    );

    if res != jni::JNI_OK {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionDescribe)(env);
            ((**env).ExceptionClear)(env);
        }
        return Err(format!("RegisterNatives failed with code {}", res));
    }

    println!("[redox-mc-loader] Successfully bound native methods to net.redoxmc.bridge.RedoxBridge via RegisterNatives.");
    Ok(())
}
