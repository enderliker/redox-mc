use jvmti_bindings::sys::jni;
use std::ffi::{CStr, CString};
use std::ptr;

/// Converts a JNI jstring to an owned Rust String.
pub(super) unsafe fn jstring_to_string(env: *mut jni::JNIEnv, j_str: jni::jstring) -> String {
    if j_str.is_null() {
        return String::new();
    }
    let chars = ((**env).GetStringUTFChars)(env, j_str, ptr::null_mut());
    if chars.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(chars).to_string_lossy().to_string();
    ((**env).ReleaseStringUTFChars)(env, j_str, chars);
    s
}

/// Converts a Rust byte slice into a JNI jbyteArray.
pub(super) unsafe fn bytes_to_jbyte_array(env: *mut jni::JNIEnv, data: &[u8]) -> jni::jbyteArray {
    let arr = ((**env).NewByteArray)(env, data.len() as jni::jsize);
    if arr.is_null() {
        return ptr::null_mut();
    }
    ((**env).SetByteArrayRegion)(
        env,
        arr,
        0,
        data.len() as jni::jsize,
        data.as_ptr() as *const jni::jbyte,
    );
    arr
}

/// Converts a slice of Rust Strings into a JNI jobjectArray of java/lang/String.
pub(super) unsafe fn string_vec_to_jobject_array(
    env: *mut jni::JNIEnv,
    strings: &[String],
) -> jni::jobjectArray {
    let string_class_name = CString::new("java/lang/String").unwrap();
    let string_class = ((**env).FindClass)(env, string_class_name.as_ptr());
    if string_class.is_null() {
        return ptr::null_mut();
    }

    let arr = ((**env).NewObjectArray)(
        env,
        strings.len() as jni::jsize,
        string_class,
        ptr::null_mut(),
    );
    if arr.is_null() {
        ((**env).DeleteLocalRef)(env, string_class as jni::jobject);
        return ptr::null_mut();
    }

    for (i, s) in strings.iter().enumerate() {
        let c_str = CString::new(s.as_str()).unwrap_or_default();
        let j_str = ((**env).NewStringUTF)(env, c_str.as_ptr());
        if !j_str.is_null() {
            ((**env).SetObjectArrayElement)(env, arr, i as jni::jsize, j_str as jni::jobject);
            ((**env).DeleteLocalRef)(env, j_str as jni::jobject);
        }
    }

    ((**env).DeleteLocalRef)(env, string_class as jni::jobject);
    arr
}

pub(super) unsafe fn clear_pending_exception(env: *mut jni::JNIEnv) {
    if !env.is_null() && ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
        ((**env).ExceptionClear)(env);
    }
}

pub(super) unsafe fn delete_local_ref(env: *mut jni::JNIEnv, reference: jni::jobject) {
    if !reference.is_null() {
        ((**env).DeleteLocalRef)(env, reference);
    }
}

/// Converts a JNI jobjectArray of java/lang/String to a Rust Vec<String>.
pub(super) unsafe fn jobject_array_to_string_vec(
    env: *mut jni::JNIEnv,
    arr: jni::jobjectArray,
) -> Vec<String> {
    if arr.is_null() {
        return Vec::new();
    }
    let len = ((**env).GetArrayLength)(env, arr as jni::jarray) as usize;
    let mut vec = Vec::with_capacity(len);
    for i in 0..len {
        let elem = ((**env).GetObjectArrayElement)(env, arr, i as jni::jsize) as jni::jstring;
        if !elem.is_null() {
            vec.push(jstring_to_string(env, elem));
            ((**env).DeleteLocalRef)(env, elem as jni::jobject);
        } else {
            vec.push(String::new());
        }
    }
    vec
}

/// Converts a Rust `String` into a JNI string, returning null if the string
/// contains an interior NUL byte.
pub(super) unsafe fn string_to_jstring(env: *mut jni::JNIEnv, value: String) -> jni::jstring {
    let c_value = CString::new(value).unwrap_or_default();
    ((**env).NewStringUTF)(env, c_value.as_ptr())
}
