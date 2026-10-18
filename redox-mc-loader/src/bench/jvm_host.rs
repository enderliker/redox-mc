//! JVM lifecycle management and dynamic libjvm loader for microbenchmarks.

use jvmti_bindings::sys::jni;
use libloading::Library;
use std::ffi::{c_void, CString};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;

type JniCreateJavaVmFn = unsafe extern "system" fn(
    pvm: *mut *mut jni::JavaVM,
    penv: *mut *mut c_void,
    args: *mut jni::JavaVMInitArgs,
) -> jni::jint;

type JniGetCreatedJavaVmsFn = unsafe extern "system" fn(
    vm_buf: *mut *mut jni::JavaVM,
    buf_len: jni::jsize,
    n_vms: *mut jni::jsize,
) -> jni::jint;

// Global process-wide JVM lock to serialize benchmark executions and handle JVM single-instance rule
static JVM_PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// Encapsulates an active embedded JVM and JNIEnv.
pub struct JvmHost {
    _lib: Library,
    vm: *mut jni::JavaVM,
    env: *mut jni::JNIEnv,
}

// Safety: JvmHost manages pointers to JVM which can be attached per-thread.
unsafe impl Send for JvmHost {}
unsafe impl Sync for JvmHost {}

impl JvmHost {
    /// Discovers libjvm and initializes or attaches to the JVM.
    pub fn new() -> Result<Self, String> {
        let _guard = JVM_PROCESS_LOCK.lock().unwrap();

        let lib_path = find_jvm_library().ok_or_else(|| {
            "Could not locate libjvm (ensure JAVA_HOME is set or Java 25 is installed)".to_string()
        })?;

        let lib = unsafe {
            Library::new(&lib_path).map_err(|e| format!("Failed to load {:?}: {}", lib_path, e))?
        };

        let mut vm_ptr: *mut jni::JavaVM = ptr::null_mut();
        let mut env_ptr: *mut c_void = ptr::null_mut();

        // 1. Check if a JVM is already instantiated in this process
        unsafe {
            if let Ok(get_vms) = lib.get::<JniGetCreatedJavaVmsFn>(b"JNI_GetCreatedJavaVMs\0") {
                let mut n_vms: jni::jsize = 0;
                let res = get_vms(&mut vm_ptr, 1, &mut n_vms);
                if res == jni::JNI_OK && n_vms > 0 && !vm_ptr.is_null() {
                    // JVM already exists; attach the current thread
                    let attach_res =
                        ((**vm_ptr).AttachCurrentThread)(vm_ptr, &mut env_ptr, ptr::null_mut());
                    if attach_res == jni::JNI_OK && !env_ptr.is_null() {
                        return Ok(Self {
                            _lib: lib,
                            vm: vm_ptr,
                            env: env_ptr as *mut jni::JNIEnv,
                        });
                    }
                }
            }

            // 2. Otherwise create a new JVM instance
            let create_vm = lib
                .get::<JniCreateJavaVmFn>(b"JNI_CreateJavaVM\0")
                .map_err(|e| format!("Failed to resolve JNI_CreateJavaVM: {}", e))?;

            let opt_ms = CString::new("-Xms64m").unwrap();
            let opt_mx = CString::new("-Xmx256m").unwrap();

            let mut options = [
                jni::JavaVMOption {
                    optionString: opt_ms.as_ptr() as *mut _,
                    extraInfo: ptr::null_mut(),
                },
                jni::JavaVMOption {
                    optionString: opt_mx.as_ptr() as *mut _,
                    extraInfo: ptr::null_mut(),
                },
            ];

            let mut args = jni::JavaVMInitArgs {
                version: jni::JNI_VERSION_1_8,
                nOptions: options.len() as jni::jint,
                options: options.as_mut_ptr(),
                ignoreUnrecognized: jni::JNI_FALSE,
            };

            let res = create_vm(&mut vm_ptr, &mut env_ptr, &mut args);
            if res != jni::JNI_OK || vm_ptr.is_null() || env_ptr.is_null() {
                return Err(format!("JNI_CreateJavaVM failed with error code {}", res));
            }

            Ok(Self {
                _lib: lib,
                vm: vm_ptr,
                env: env_ptr as *mut jni::JNIEnv,
            })
        }
    }

    /// Returns the raw JNIEnv pointer for the current thread.
    pub unsafe fn env(&self) -> *mut jni::JNIEnv {
        self.env
    }

    /// Returns the raw JavaVM pointer.
    pub unsafe fn vm(&self) -> *mut jni::JavaVM {
        self.vm
    }
}

/// Locates the platform-specific JVM shared library (`libjvm.so`, `jvm.dll`, `libjvm.dylib`).
pub fn find_jvm_library() -> Option<PathBuf> {
    // 1. Check JAVA_HOME environment variable
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(java_home);
        if let Some(lib) = probe_jvm_in_home(&p) {
            return Some(lib);
        }
    }

    // 2. Check path of `java` binary
    if let Ok(which_out) = std::process::Command::new("which").arg("java").output() {
        if which_out.status.success() {
            let path_str = String::from_utf8_lossy(&which_out.stdout)
                .trim()
                .to_string();
            if let Ok(canonical) = std::fs::canonicalize(path_str) {
                // canonical is /path/to/jdk/bin/java -> parent is /bin, grand-parent is JDK home
                if let Some(bin_dir) = canonical.parent() {
                    if let Some(jdk_home) = bin_dir.parent() {
                        if let Some(lib) = probe_jvm_in_home(jdk_home) {
                            return Some(lib);
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback to common OS paths
    let candidate_paths = [
        // Linux OpenJDK paths
        "/usr/lib/jvm/default-java/lib/server/libjvm.so",
        "/usr/lib/jvm/java-25-openjdk-amd64/lib/server/libjvm.so",
        "/usr/lib/jvm/java-25-openjdk-arm64/lib/server/libjvm.so",
        "/usr/lib/jvm/java-21-openjdk-amd64/lib/server/libjvm.so",
        "/usr/lib/jvm/java-21-openjdk-arm64/lib/server/libjvm.so",
        "/usr/lib/jvm/java-17-openjdk-amd64/lib/server/libjvm.so",
        // macOS standard paths
        "/Library/Java/JavaVirtualMachines/temurin-25.jdk/Contents/Home/lib/server/libjvm.dylib",
        "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/lib/server/libjvm.dylib",
        // Windows standard paths
        r"C:\Program Files\Eclipse Adoptium\jdk-25*\bin\server\jvm.dll",
        r"C:\Program Files\Java\jdk-25*\bin\server\jvm.dll",
    ];

    for candidate in &candidate_paths {
        let p = Path::new(candidate);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // Scan /usr/lib/jvm/* if on linux
    if let Ok(entries) = std::fs::read_dir("/usr/lib/jvm") {
        for entry in entries.flatten() {
            let p = entry.path().join("lib/server/libjvm.so");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

fn probe_jvm_in_home(home: &Path) -> Option<PathBuf> {
    let variants = [
        home.join("lib/server/libjvm.so"),
        home.join("lib/server/libjvm.dylib"),
        home.join("bin/server/jvm.dll"),
        home.join("jre/lib/server/libjvm.so"),
        home.join("jre/bin/server/jvm.dll"),
    ];

    for v in &variants {
        if v.exists() {
            return Some(v.clone());
        }
    }
    None
}
