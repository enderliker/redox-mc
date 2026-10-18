//! Microbenchmark runners measuring JNI boundary crossing overhead.

use super::jvm_host::JvmHost;
use super::stats::{compute_stats, BenchmarkStats};
use jvmti_bindings::sys::jni;
use std::ffi::CString;
use std::hint::black_box;
use std::ptr;
use std::time::Instant;

/// Benchmark execution parameters.
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub warmup_iterations: usize,
    pub batch_size: usize,
    pub num_batches: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 50_000,
            batch_size: 1_000,
            num_batches: 200,
        }
    }
}

// Bytecode for net.redoxmc.bench.BenchmarkTarget:
// public class BenchmarkTarget {
//     public static void noop() {}
//     public static long add(long a, long b) { return a + b; }
//     public int value = 42;
//     public int getValue() { return value; }
// }
const BENCHMARK_TARGET_BYTES: &[u8] = &[
    202, 254, 186, 190, 0, 0, 0, 69, 0, 22, 10, 0, 2, 0, 3, 7, 0, 4, 12, 0, 5, 0, 6, 1, 0, 16, 106,
    97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 1, 0, 6, 60, 105, 110, 105,
    116, 62, 1, 0, 3, 40, 41, 86, 9, 0, 8, 0, 9, 7, 0, 10, 12, 0, 11, 0, 12, 1, 0, 33, 110, 101,
    116, 47, 114, 101, 100, 111, 120, 109, 99, 47, 98, 101, 110, 99, 104, 47, 66, 101, 110, 99,
    104, 109, 97, 114, 103, 101, 116, 1, 0, 5, 118, 97, 108, 117, 101, 1, 0, 1, 73, 1, 0, 4, 67,
    111, 100, 101, 1, 0, 15, 76, 105, 110, 101, 78, 117, 109, 98, 101, 114, 84, 97, 98, 108, 101,
    1, 0, 4, 110, 111, 111, 112, 1, 0, 3, 97, 100, 100, 1, 0, 5, 40, 74, 74, 41, 74, 1, 0, 8, 103,
    101, 116, 86, 97, 108, 117, 101, 1, 0, 3, 40, 41, 73, 1, 0, 10, 83, 111, 117, 114, 99, 101, 70,
    105, 108, 101, 1, 0, 20, 66, 101, 110, 99, 104, 109, 97, 114, 103, 101, 116, 46, 106, 97, 118,
    97, 0, 33, 0, 8, 0, 2, 0, 0, 0, 1, 0, 1, 0, 11, 0, 12, 0, 0, 0, 4, 0, 1, 0, 5, 0, 6, 0, 1, 0,
    13, 0, 0, 0, 39, 0, 2, 0, 1, 0, 0, 0, 11, 42, 183, 0, 1, 42, 16, 42, 181, 0, 7, 177, 0, 0, 0,
    1, 0, 14, 0, 0, 0, 10, 0, 2, 0, 0, 0, 2, 0, 4, 0, 5, 0, 9, 0, 15, 0, 6, 0, 1, 0, 13, 0, 0, 0,
    25, 0, 0, 0, 0, 0, 0, 0, 1, 177, 0, 0, 0, 1, 0, 14, 0, 0, 0, 6, 0, 1, 0, 0, 0, 3, 0, 9, 0, 16,
    0, 17, 0, 1, 0, 13, 0, 0, 0, 28, 0, 4, 0, 4, 0, 0, 0, 4, 30, 32, 97, 173, 0, 0, 0, 1, 0, 14, 0,
    0, 0, 6, 0, 1, 0, 0, 0, 4, 0, 1, 0, 18, 0, 19, 0, 1, 0, 13, 0, 0, 0, 29, 0, 1, 0, 1, 0, 0, 0,
    5, 42, 180, 0, 7, 172, 0, 0, 0, 1, 0, 14, 0, 0, 0, 6, 0, 1, 0, 0, 0, 6, 0, 1, 0, 20, 0, 0, 0,
    2, 0, 21,
];

unsafe fn get_or_define_target_class(env: *mut jni::JNIEnv) -> Option<jni::jclass> {
    let name = CString::new("net/redoxmc/bench/BenchmarkTarget").unwrap();
    let existing = ((**env).FindClass)(env, name.as_ptr());
    if !existing.is_null() {
        return Some(existing);
    }
    if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
        ((**env).ExceptionClear)(env);
    }

    let cl_class_name = CString::new("java/lang/ClassLoader").unwrap();
    let cl_class = ((**env).FindClass)(env, cl_class_name.as_ptr());
    let sys_loader = if !cl_class.is_null() {
        let get_sys_name = CString::new("getSystemClassLoader").unwrap();
        let get_sys_sig = CString::new("()Ljava/lang/ClassLoader;").unwrap();
        let get_sys_mid =
            ((**env).GetStaticMethodID)(env, cl_class, get_sys_name.as_ptr(), get_sys_sig.as_ptr());
        if !get_sys_mid.is_null() {
            ((**env).CallStaticObjectMethod)(env, cl_class, get_sys_mid)
        } else {
            ptr::null_mut()
        }
    } else {
        ptr::null_mut()
    };

    let defined = ((**env).DefineClass)(
        env,
        name.as_ptr(),
        sys_loader,
        BENCHMARK_TARGET_BYTES.as_ptr() as *const jni::jbyte,
        BENCHMARK_TARGET_BYTES.len() as jni::jsize,
    );
    if defined.is_null() {
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
        None
    } else {
        Some(defined)
    }
}

/// Runs the 4 canonical JNI microbenchmarks and returns their statistical profiles.
pub fn run_benchmarks(
    host: &JvmHost,
    config: &BenchmarkConfig,
) -> Result<Vec<BenchmarkStats>, String> {
    unsafe {
        let env = host.env();
        let target_class = get_or_define_target_class(env);

        let results = vec![
            // 1. No-arg call
            bench_no_arg_call(env, target_class, config)?,
            // 2. Primitive arguments
            bench_primitive_args(env, target_class, config)?,
            // 3. Small strings (allocation, length inquiry, local ref free)
            bench_small_strings(env, config)?,
            // 4. Complex object lifecycle (StringBuilder init, append, query, free)
            bench_complex_object(env, config)?,
        ];

        Ok(results)
    }
}

/// 1. No-arg call: BenchmarkTarget.noop() [or System.nanoTime() fallback]
unsafe fn bench_no_arg_call(
    env: *mut jni::JNIEnv,
    target_class: Option<jni::jclass>,
    config: &BenchmarkConfig,
) -> Result<BenchmarkStats, String> {
    if let Some(target_class) = target_class {
        let method_name = CString::new("noop").unwrap();
        let method_sig = CString::new("()V").unwrap();
        let method_id = ((**env).GetStaticMethodID)(
            env,
            target_class,
            method_name.as_ptr(),
            method_sig.as_ptr(),
        );

        if !method_id.is_null() {
            // Warmup
            for _ in 0..config.warmup_iterations {
                ((**env).CallStaticVoidMethod)(env, target_class, method_id);
            }

            // Timed batches
            let mut samples = Vec::with_capacity(config.num_batches);
            for _ in 0..config.num_batches {
                let start = Instant::now();
                for _ in 0..config.batch_size {
                    ((**env).CallStaticVoidMethod)(env, target_class, method_id);
                }
                let elapsed = start.elapsed();
                let ns_per_op = elapsed.as_nanos() as f64 / config.batch_size as f64;
                samples.push(ns_per_op);
            }

            let total = config.batch_size * config.num_batches;
            return Ok(compute_stats(
                "1. No-arg Call (BenchmarkTarget.noop)",
                samples,
                total,
            ));
        }
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
    }

    // Fallback: System.nanoTime()
    let class_name = CString::new("java/lang/System").unwrap();
    let class = ((**env).FindClass)(env, class_name.as_ptr());
    if class.is_null() {
        return Err("Could not find class java/lang/System".into());
    }

    let method_name = CString::new("nanoTime").unwrap();
    let method_sig = CString::new("()J").unwrap();
    let method_id =
        ((**env).GetStaticMethodID)(env, class, method_name.as_ptr(), method_sig.as_ptr());
    if method_id.is_null() {
        return Err("Could not find static method nanoTime".into());
    }

    for _ in 0..config.warmup_iterations {
        let res = ((**env).CallStaticLongMethod)(env, class, method_id);
        black_box(res);
    }

    let mut samples = Vec::with_capacity(config.num_batches);
    for _ in 0..config.num_batches {
        let start = Instant::now();
        for _ in 0..config.batch_size {
            let res = ((**env).CallStaticLongMethod)(env, class, method_id);
            black_box(res);
        }
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / config.batch_size as f64;
        samples.push(ns_per_op);
    }

    let total = config.batch_size * config.num_batches;
    Ok(compute_stats(
        "1. No-arg Call (System.nanoTime)",
        samples,
        total,
    ))
}

/// 2. Primitive arguments: BenchmarkTarget.add(long, long) [or Math.max fallback]
unsafe fn bench_primitive_args(
    env: *mut jni::JNIEnv,
    target_class: Option<jni::jclass>,
    config: &BenchmarkConfig,
) -> Result<BenchmarkStats, String> {
    let args = [jni::jvalue { j: 42 }, jni::jvalue { j: 100 }];

    if let Some(target_class) = target_class {
        let method_name = CString::new("add").unwrap();
        let method_sig = CString::new("(JJ)J").unwrap();
        let method_id = ((**env).GetStaticMethodID)(
            env,
            target_class,
            method_name.as_ptr(),
            method_sig.as_ptr(),
        );

        if !method_id.is_null() {
            // Warmup
            for _ in 0..config.warmup_iterations {
                let res =
                    ((**env).CallStaticLongMethodA)(env, target_class, method_id, args.as_ptr());
                black_box(res);
            }

            // Timed batches
            let mut samples = Vec::with_capacity(config.num_batches);
            for _ in 0..config.num_batches {
                let start = Instant::now();
                for _ in 0..config.batch_size {
                    let res = ((**env).CallStaticLongMethodA)(
                        env,
                        target_class,
                        method_id,
                        args.as_ptr(),
                    );
                    black_box(res);
                }
                let elapsed = start.elapsed();
                let ns_per_op = elapsed.as_nanos() as f64 / config.batch_size as f64;
                samples.push(ns_per_op);
            }

            let total = config.batch_size * config.num_batches;
            return Ok(compute_stats(
                "2. Primitive Args (BenchmarkTarget.add)",
                samples,
                total,
            ));
        }
        if ((**env).ExceptionCheck)(env) == jni::JNI_TRUE {
            ((**env).ExceptionClear)(env);
        }
    }

    // Fallback: Math.max
    let class_name = CString::new("java/lang/Math").unwrap();
    let class = ((**env).FindClass)(env, class_name.as_ptr());
    if class.is_null() {
        return Err("Could not find class java/lang/Math".into());
    }

    let method_name = CString::new("max").unwrap();
    let method_sig = CString::new("(JJ)J").unwrap();
    let method_id =
        ((**env).GetStaticMethodID)(env, class, method_name.as_ptr(), method_sig.as_ptr());
    if method_id.is_null() {
        return Err("Could not find static method Math.max(long, long)".into());
    }

    for _ in 0..config.warmup_iterations {
        let res = ((**env).CallStaticLongMethodA)(env, class, method_id, args.as_ptr());
        black_box(res);
    }

    let mut samples = Vec::with_capacity(config.num_batches);
    for _ in 0..config.num_batches {
        let start = Instant::now();
        for _ in 0..config.batch_size {
            let res = ((**env).CallStaticLongMethodA)(env, class, method_id, args.as_ptr());
            black_box(res);
        }
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / config.batch_size as f64;
        samples.push(ns_per_op);
    }

    let total = config.batch_size * config.num_batches;
    Ok(compute_stats(
        "2. Primitive Args (Math.max(jlong, jlong))",
        samples,
        total,
    ))
}

/// 3. Small strings: NewStringUTF + String.length() + DeleteLocalRef
unsafe fn bench_small_strings(
    env: *mut jni::JNIEnv,
    config: &BenchmarkConfig,
) -> Result<BenchmarkStats, String> {
    let class_name = CString::new("java/lang/String").unwrap();
    let class = ((**env).FindClass)(env, class_name.as_ptr());
    if class.is_null() {
        return Err("Could not find class java/lang/String".into());
    }

    let method_name = CString::new("length").unwrap();
    let method_sig = CString::new("()I").unwrap();
    let method_id = ((**env).GetMethodID)(env, class, method_name.as_ptr(), method_sig.as_ptr());
    if method_id.is_null() {
        return Err("Could not find method String.length()".into());
    }

    let sample_text = CString::new("minecraft:diamond_block").unwrap();

    // Warmup
    for _ in 0..config.warmup_iterations {
        let jstr = ((**env).NewStringUTF)(env, sample_text.as_ptr());
        let len = ((**env).CallIntMethod)(env, jstr, method_id);
        black_box(len);
        ((**env).DeleteLocalRef)(env, jstr);
    }

    // Timed batches
    let mut samples = Vec::with_capacity(config.num_batches);
    for _ in 0..config.num_batches {
        let start = Instant::now();
        for _ in 0..config.batch_size {
            let jstr = ((**env).NewStringUTF)(env, sample_text.as_ptr());
            let len = ((**env).CallIntMethod)(env, jstr, method_id);
            black_box(len);
            ((**env).DeleteLocalRef)(env, jstr);
        }
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / config.batch_size as f64;
        samples.push(ns_per_op);
    }

    let total = config.batch_size * config.num_batches;
    Ok(compute_stats(
        "3. Small String (NewStringUTF + length + DeleteLocalRef)",
        samples,
        total,
    ))
}

/// 4. Complex object: StringBuilder new + append + length + DeleteLocalRef
unsafe fn bench_complex_object(
    env: *mut jni::JNIEnv,
    config: &BenchmarkConfig,
) -> Result<BenchmarkStats, String> {
    let class_name = CString::new("java/lang/StringBuilder").unwrap();
    let class = ((**env).FindClass)(env, class_name.as_ptr());
    if class.is_null() {
        return Err("Could not find class java/lang/StringBuilder".into());
    }

    let ctor_name = CString::new("<init>").unwrap();
    let ctor_sig = CString::new("(I)V").unwrap();
    let ctor_id = ((**env).GetMethodID)(env, class, ctor_name.as_ptr(), ctor_sig.as_ptr());
    if ctor_id.is_null() {
        return Err("Could not find StringBuilder constructor (I)V".into());
    }

    let append_name = CString::new("append").unwrap();
    let append_sig = CString::new("(I)Ljava/lang/StringBuilder;").unwrap();
    let append_id = ((**env).GetMethodID)(env, class, append_name.as_ptr(), append_sig.as_ptr());
    if append_id.is_null() {
        return Err("Could not find StringBuilder.append(I)".into());
    }

    let len_name = CString::new("length").unwrap();
    let len_sig = CString::new("()I").unwrap();
    let len_id = ((**env).GetMethodID)(env, class, len_name.as_ptr(), len_sig.as_ptr());
    if len_id.is_null() {
        return Err("Could not find StringBuilder.length()".into());
    }

    let ctor_args = [jni::jvalue { i: 64 }];
    let append_args = [jni::jvalue { i: 1337 }];

    // Warmup
    for _ in 0..config.warmup_iterations {
        let obj = ((**env).NewObjectA)(env, class, ctor_id, ctor_args.as_ptr());
        let _ = ((**env).CallObjectMethodA)(env, obj, append_id, append_args.as_ptr());
        let len = ((**env).CallIntMethod)(env, obj, len_id);
        black_box(len);
        ((**env).DeleteLocalRef)(env, obj);
    }

    // Timed batches
    let mut samples = Vec::with_capacity(config.num_batches);
    for _ in 0..config.num_batches {
        let start = Instant::now();
        for _ in 0..config.batch_size {
            let obj = ((**env).NewObjectA)(env, class, ctor_id, ctor_args.as_ptr());
            let _ = ((**env).CallObjectMethodA)(env, obj, append_id, append_args.as_ptr());
            let len = ((**env).CallIntMethod)(env, obj, len_id);
            black_box(len);
            ((**env).DeleteLocalRef)(env, obj);
        }
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / config.batch_size as f64;
        samples.push(ns_per_op);
    }

    let total = config.batch_size * config.num_batches;
    Ok(compute_stats(
        "4. Complex Object (NewObject + append + length + DeleteLocalRef)",
        samples,
        total,
    ))
}
