use super::strings::{
    bytes_to_jbyte_array, clear_pending_exception, delete_local_ref, jstring_to_string,
};
use crate::class_injector;
use jvmti_bindings::sys::jni;
use std::ffi::{c_void, CString};
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

/// JNI native: net.redoxmc.bridge.RedoxBridge.onModCustomPayload(String namespace, String path, byte[] data)
///
/// Legacy compatibility entry point that does not carry sender identity.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayload(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_namespace: jni::jstring,
    j_path: jni::jstring,
    j_data: jni::jbyteArray,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if env.is_null() || j_namespace.is_null() || j_path.is_null() || j_data.is_null() {
            return;
        }

        let namespace = jstring_to_string(env, j_namespace);
        let path = jstring_to_string(env, j_path);
        let channel = format!("{}:{}", namespace, path);

        let length = ((**env).GetArrayLength)(env, j_data) as usize;
        let elements = ((**env).GetByteArrayElements)(env, j_data, ptr::null_mut());
        if !elements.is_null() {
            let slice: &[u8] = std::slice::from_raw_parts(elements as *const u8, length);
            let side = crate::mod_loader::running_side();
            crate::mod_loader::dispatch_network_payload(&channel, slice, "", "", side);
            ((**env).ReleaseByteArrayElements)(env, j_data, elements, jni::JNI_ABORT);
            drain_mod_network_outboxes(env, side);
        }
    }));
}

/// JNI native: net.redoxmc.bridge.RedoxBridge.onModCustomPayloadFrom(
///     String channel, byte[] data, String senderName, String senderUuid, int side)
///
/// Dispatches an inbound mod payload with full sender identity and side context.
///
/// # Safety
/// Must be invoked by the JVM with a valid JNI environment pointer.
#[no_mangle]
pub unsafe extern "system" fn Java_net_redoxmc_bridge_RedoxBridge_onModCustomPayloadFrom(
    env: *mut jni::JNIEnv,
    _class: jni::jclass,
    j_channel: jni::jstring,
    j_data: jni::jbyteArray,
    j_sender_name: jni::jstring,
    j_sender_uuid: jni::jstring,
    j_side: jni::jint,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if env.is_null() || j_channel.is_null() || j_data.is_null() {
            return;
        }

        let channel = jstring_to_string(env, j_channel);
        let sender_name = jstring_to_string(env, j_sender_name);
        let sender_uuid = jstring_to_string(env, j_sender_uuid);
        let side = crate::network::side_from_abi(j_side as u32)
            .unwrap_or_else(crate::mod_loader::running_side);

        let length = ((**env).GetArrayLength)(env, j_data) as usize;
        let elements = ((**env).GetByteArrayElements)(env, j_data, ptr::null_mut());
        if elements.is_null() {
            if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
                ((**env).ExceptionClear)(env);
            }
            return;
        }

        {
            let slice: &[u8] = std::slice::from_raw_parts(elements as *const u8, length);
            crate::mod_loader::dispatch_network_payload(
                &channel,
                slice,
                &sender_name,
                &sender_uuid,
                side,
            );
        }
        ((**env).ReleaseByteArrayElements)(env, j_data, elements, jni::JNI_ABORT);

        // Low-latency reply path: flush messages queued by the receive callback.
        drain_mod_network_outboxes(env, side);
    }));
}

/// Cached method ID for `ModNetworkBridge.sendModPayload(...)`.
static SEND_MOD_PAYLOAD_METHOD: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

struct NetworkDrainContext {
    env: *mut jni::JNIEnv,
    class: jni::jclass,
    method: jni::jmethodID,
    origin_side: crate::manifest::ModSide,
    sent: usize,
    failed: usize,
}

unsafe fn get_send_mod_payload_method(env: *mut jni::JNIEnv, class: jni::jclass) -> jni::jmethodID {
    let cached = SEND_MOD_PAYLOAD_METHOD.load(Ordering::Acquire);
    if !cached.is_null() {
        return cached as jni::jmethodID;
    }

    let name = CString::new("sendModPayload").unwrap();
    let sig =
        CString::new("(Ljava/lang/String;ILjava/lang/String;Ljava/lang/String;[BI)Z").unwrap();
    let method = ((**env).GetStaticMethodID)(env, class, name.as_ptr(), sig.as_ptr());
    if method.is_null() {
        clear_pending_exception(env);
        return ptr::null_mut();
    }

    match SEND_MOD_PAYLOAD_METHOD.compare_exchange(
        ptr::null_mut(),
        method,
        Ordering::Release,
        Ordering::Relaxed,
    ) {
        Ok(_) => method,
        Err(existing) => existing as jni::jmethodID,
    }
}

/// Drains every loaded mod's outbound network queue and forwards each message
/// to `ModNetworkBridge.sendModPayload(...)`.
///
/// `origin_side` tells the Java bridge whether the message must travel
/// server-to-client or client-to-server (important in integrated servers).
///
/// # Safety
/// `env` must be a valid JNI environment pointer.
pub(crate) unsafe fn drain_mod_network_outboxes(
    env: *mut jni::JNIEnv,
    origin_side: crate::manifest::ModSide,
) {
    if env.is_null() {
        return;
    }

    let class = class_injector::get_mod_network_bridge_class();
    if class.is_null() {
        return;
    }

    let method = get_send_mod_payload_method(env, class);
    if method.is_null() {
        return;
    }

    let callbacks = crate::mod_loader::network_drain_callbacks();
    let abi_side = crate::network::side_to_abi(origin_side);
    for (id, drain) in callbacks {
        let mut context = NetworkDrainContext {
            env,
            class,
            method,
            origin_side,
            sent: 0,
            failed: 0,
        };

        let status = drain(
            abi_side,
            network_outbound_visitor,
            (&mut context as *mut NetworkDrainContext) as *mut c_void,
        );

        if let Err(safety_err) = crate::safety::check_ffi_status(&id, status) {
            eprintln!(
                "[redox-mc-loader] ERROR: Mod '{}' network outbox drain failed: {}",
                id, safety_err
            );
        }
        if context.sent > 0 {
            println!(
                "[redox-mc-loader] Mod '{}' sent {} outbound network message(s) from {:?}.",
                id, context.sent, origin_side
            );
        }
        if context.failed > 0 {
            eprintln!(
                "[redox-mc-loader] Mod '{}' failed to deliver {} outbound network message(s).",
                id, context.failed
            );
        }
    }
}

unsafe extern "C" fn network_outbound_visitor(
    user_data: *mut c_void,
    channel: redox_mc_abi::types::FfiStr,
    target_kind: u32,
    target_name: redox_mc_abi::types::FfiStr,
    target_uuid: redox_mc_abi::types::FfiStr,
    data: redox_mc_abi::types::FfiSlice<u8>,
) -> redox_mc_abi::types::FfiStatus {
    use redox_mc_abi::types::FfiStatus;

    if user_data.is_null() {
        return FfiStatus::NullPointer;
    }
    let context = &mut *(user_data as *mut NetworkDrainContext);

    let Ok(channel) = channel.as_str() else {
        context.failed += 1;
        return FfiStatus::InvalidArgument;
    };
    if channel.is_empty() {
        context.failed += 1;
        return FfiStatus::InvalidArgument;
    }

    let target_name = target_name.as_str().unwrap_or("");
    let target_uuid = target_uuid.as_str().unwrap_or("");
    let data = data.as_slice();

    let channel_c = match CString::new(channel) {
        Ok(value) => value,
        Err(_) => {
            context.failed += 1;
            return FfiStatus::InvalidArgument;
        }
    };
    let target_name_c = CString::new(target_name).unwrap_or_default();
    let target_uuid_c = CString::new(target_uuid).unwrap_or_default();

    let env = context.env;
    let j_channel = ((**env).NewStringUTF)(env, channel_c.as_ptr());
    let j_target_name = ((**env).NewStringUTF)(env, target_name_c.as_ptr());
    let j_target_uuid = ((**env).NewStringUTF)(env, target_uuid_c.as_ptr());
    let j_data = bytes_to_jbyte_array(env, data);

    if j_channel.is_null() || j_target_name.is_null() || j_target_uuid.is_null() || j_data.is_null()
    {
        clear_pending_exception(env);
        delete_local_ref(env, j_channel as jni::jobject);
        delete_local_ref(env, j_target_name as jni::jobject);
        delete_local_ref(env, j_target_uuid as jni::jobject);
        delete_local_ref(env, j_data as jni::jobject);
        context.failed += 1;
        return FfiStatus::GenericError;
    }

    let args = [
        jni::jvalue {
            l: j_channel as jni::jobject,
        },
        jni::jvalue {
            i: target_kind as jni::jint,
        },
        jni::jvalue {
            l: j_target_name as jni::jobject,
        },
        jni::jvalue {
            l: j_target_uuid as jni::jobject,
        },
        jni::jvalue {
            l: j_data as jni::jobject,
        },
        jni::jvalue {
            i: crate::network::side_to_abi(context.origin_side) as jni::jint,
        },
    ];

    let accepted =
        ((**env).CallStaticBooleanMethodA)(env, context.class, context.method, args.as_ptr());

    delete_local_ref(env, j_channel as jni::jobject);
    delete_local_ref(env, j_target_name as jni::jobject);
    delete_local_ref(env, j_target_uuid as jni::jobject);
    delete_local_ref(env, j_data as jni::jobject);

    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
        clear_pending_exception(env);
        context.failed += 1;
        return FfiStatus::Success;
    }

    if accepted == jni::JNI_TRUE {
        context.sent += 1;
    } else {
        context.failed += 1;
    }

    FfiStatus::Success
}
