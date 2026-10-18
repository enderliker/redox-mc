use redox_mc_abi::prelude::*;
use std::mem::{align_of, offset_of, size_of};

#[test]
#[cfg(target_pointer_width = "64")]
fn test_abi_layout_and_alignment_64bit() {
    // 1. AbiVersion
    assert_eq!(size_of::<AbiVersion>(), 12);
    assert_eq!(align_of::<AbiVersion>(), 4);
    assert_eq!(offset_of!(AbiVersion, major), 0);
    assert_eq!(offset_of!(AbiVersion, minor), 4);
    assert_eq!(offset_of!(AbiVersion, patch), 8);

    // 2. FfiStatus
    assert_eq!(size_of::<FfiStatus>(), 4);
    assert_eq!(align_of::<FfiStatus>(), 4);

    // 3. FfiStr
    assert_eq!(size_of::<FfiStr>(), 16);
    assert_eq!(align_of::<FfiStr>(), 8);
    assert_eq!(offset_of!(FfiStr, ptr), 0);
    assert_eq!(offset_of!(FfiStr, len), 8);

    // 4. FfiString
    assert_eq!(size_of::<FfiString>(), 24);
    assert_eq!(align_of::<FfiString>(), 8);
    assert_eq!(offset_of!(FfiString, ptr), 0);
    assert_eq!(offset_of!(FfiString, len), 8);
    assert_eq!(offset_of!(FfiString, capacity), 16);

    // 5. FfiSlice<u8>
    assert_eq!(size_of::<FfiSlice<u8>>(), 16);
    assert_eq!(align_of::<FfiSlice<u8>>(), 8);
    assert_eq!(offset_of!(FfiSlice<u8>, ptr), 0);
    assert_eq!(offset_of!(FfiSlice<u8>, len), 8);

    // 6. FfiBuffer
    assert_eq!(size_of::<FfiBuffer>(), 24);
    assert_eq!(align_of::<FfiBuffer>(), 8);
    assert_eq!(offset_of!(FfiBuffer, ptr), 0);
    assert_eq!(offset_of!(FfiBuffer, len), 8);
    assert_eq!(offset_of!(FfiBuffer, capacity), 16);

    // 7. FfiModSide & FfiLogLevel
    assert_eq!(size_of::<FfiModSide>(), 1);
    assert_eq!(align_of::<FfiModSide>(), 1);
    assert_eq!(size_of::<FfiLogLevel>(), 1);
    assert_eq!(align_of::<FfiLogLevel>(), 1);

    // 8. LoaderContext
    assert_eq!(size_of::<LoaderContext>(), 48);
    assert_eq!(align_of::<LoaderContext>(), 8);
    assert_eq!(offset_of!(LoaderContext, abi_version), 0);
    assert_eq!(offset_of!(LoaderContext, loader_version_major), 12);
    assert_eq!(offset_of!(LoaderContext, loader_version_minor), 16);
    assert_eq!(offset_of!(LoaderContext, loader_version_patch), 20);
    assert_eq!(offset_of!(LoaderContext, side), 24);
    assert_eq!(offset_of!(LoaderContext, _reserved), 25);
    assert_eq!(offset_of!(LoaderContext, vtable), 32);
    assert_eq!(offset_of!(LoaderContext, loader_handle), 40);

    // 9. LoaderVTable (4 function pointers = 4 * 8 = 32 bytes)
    assert_eq!(size_of::<LoaderVTable>(), 32);
    assert_eq!(align_of::<LoaderVTable>(), 8);
    assert_eq!(offset_of!(LoaderVTable, log), 0);
    assert_eq!(offset_of!(LoaderVTable, register_event_listener), 8);
    assert_eq!(offset_of!(LoaderVTable, free_string), 16);
    assert_eq!(offset_of!(LoaderVTable, free_buffer), 24);

    // 10. ModVTable (6 function pointers = 6 * 8 = 48 bytes)
    assert_eq!(size_of::<ModVTable>(), 48);
    assert_eq!(align_of::<ModVTable>(), 8);
    assert_eq!(offset_of!(ModVTable, on_bootstrap), 0);
    assert_eq!(offset_of!(ModVTable, on_register_content), 8);
    assert_eq!(offset_of!(ModVTable, on_server_tick), 16);
    assert_eq!(offset_of!(ModVTable, on_server_chat), 24);
    assert_eq!(offset_of!(ModVTable, free_string), 32);
    assert_eq!(offset_of!(ModVTable, free_buffer), 40);

    // 11. FfiResult
    assert_eq!(size_of::<FfiResult<u32>>(), 8);
    assert_eq!(align_of::<FfiResult<u32>>(), 4);
    assert_eq!(size_of::<FfiResult<*const ModVTable>>(), 16);
    assert_eq!(align_of::<FfiResult<*const ModVTable>>(), 8);
}

#[test]
fn test_ffi_str_operations() {
    let rust_str = "Redox-MC Modding Platform 26.3";
    let ffi_str = FfiStr::from_str(rust_str);

    assert_eq!(ffi_str.len, rust_str.len());
    assert!(!ffi_str.is_empty());

    let recovered = unsafe { ffi_str.as_str().expect("valid utf-8") };
    assert_eq!(recovered, rust_str);

    let empty = FfiStr::EMPTY;
    assert!(empty.is_empty());
    assert_eq!(unsafe { empty.as_str().unwrap() }, "");
}

#[test]
fn test_ffi_string_symmetric_ownership() {
    let original = "Hello from Redox Native Mod!".to_string();
    let expected_len = original.len();

    let ffi_string = FfiString::from_string(original);
    assert_eq!(ffi_string.len, expected_len);
    assert!(ffi_string.capacity >= expected_len);
    assert!(!ffi_string.is_null());

    let recovered = unsafe { ffi_string.into_string().expect("valid utf-8") };
    assert_eq!(recovered, "Hello from Redox Native Mod!");

    // Test explicit local deallocation
    let s2 = FfiString::from_string("To be freed locally".to_string());
    unsafe {
        free_local_string(s2);
    }
}

#[test]
fn test_ffi_buffer_operations() {
    let bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x45];
    let len = bytes.len();

    let mut buf = FfiBuffer::from_vec(bytes);
    assert_eq!(buf.len, len);
    assert!(!buf.is_null());

    unsafe {
        let slice_mut = buf.as_bytes_mut();
        slice_mut[4] = 0xDE;
        slice_mut[5] = 0xAD;
    }

    let recovered = unsafe { buf.into_vec() };
    assert_eq!(recovered, vec![0xCA, 0xFE, 0xBA, 0xBE, 0xDE, 0xAD]);

    // Test explicit local deallocation
    let b2 = FfiBuffer::from_vec(vec![1, 2, 3, 4, 5]);
    unsafe {
        free_local_buffer(b2);
    }
}

#[test]
fn test_abi_version_compatibility() {
    let v1_0_0 = AbiVersion::new(1, 0, 0);
    let v1_2_0 = AbiVersion::new(1, 2, 0);
    let v1_2_5 = AbiVersion::new(1, 2, 5);
    let v2_0_0 = AbiVersion::new(2, 0, 0);

    // v1.2.5 running loader supports mod requiring v1.0.0 or v1.2.0
    assert!(v1_2_5.is_compatible_with(&v1_0_0));
    assert!(v1_2_5.is_compatible_with(&v1_2_0));
    assert!(v1_2_5.is_compatible_with(&v1_2_5));

    // v1.0.0 running loader cannot support mod requiring v1.2.0
    assert!(!v1_0_0.is_compatible_with(&v1_2_0));

    // Major version changes are breaking
    assert!(!v2_0_0.is_compatible_with(&v1_0_0));
    assert!(!v1_2_5.is_compatible_with(&v2_0_0));
}

#[test]
fn test_mock_loader_and_mod_handshake() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    // 1. Mock Loader VTable implementation
    static LOGGED_MESSAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    unsafe extern "C" fn mock_log(_level: FfiLogLevel, _target: FfiStr, msg: FfiStr) {
        if let Ok(s) = unsafe { msg.as_str() } {
            LOGGED_MESSAGES.lock().unwrap().push(s.to_string());
        }
    }

    unsafe extern "C" fn mock_register_listener(
        _event_id: u32,
        _cb: unsafe extern "C" fn(*const ()),
        _user_data: *mut (),
    ) -> FfiStatus {
        FfiStatus::Success
    }

    let loader_vtable = LoaderVTable {
        log: mock_log,
        register_event_listener: mock_register_listener,
        free_string: redox_loader_free_string,
        free_buffer: redox_loader_free_buffer,
    };

    let loader_context = LoaderContext {
        abi_version: REDOX_ABI_VERSION,
        loader_version_major: 0,
        loader_version_minor: 1,
        loader_version_patch: 0,
        side: FfiModSide::Server,
        _reserved: [0; 7],
        vtable: &loader_vtable,
        loader_handle: std::ptr::null_mut(),
    };

    // 2. Mock Mod implementation
    static BOOTSTRAP_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn mock_mod_bootstrap() -> FfiStatus {
        BOOTSTRAP_CALLED.store(true, Ordering::SeqCst);
        FfiStatus::Success
    }

    static MOCK_MOD_VTABLE: ModVTable = ModVTable {
        on_bootstrap: Some(mock_mod_bootstrap),
        on_register_content: None,
        on_server_tick: None,
        on_server_chat: None,
        free_string: redox_mod_free_string,
        free_buffer: redox_mod_free_buffer,
    };

    // 3. Mod Init function
    unsafe extern "C" fn mock_mod_init(ctx: *const LoaderContext) -> FfiResult<*const ModVTable> {
        if ctx.is_null() {
            return FfiResult::err(FfiStatus::NullPointer, std::ptr::null());
        }
        let ctx_ref = &*ctx;
        if !ctx_ref.abi_version.is_compatible_with(&REDOX_ABI_VERSION) {
            return FfiResult::err(FfiStatus::VersionMismatch, std::ptr::null());
        }

        // Mod logs a message via loader vtable
        let msg = FfiStr::from_str("TestMod v1.0 initialized!");
        let target = FfiStr::from_str("TestMod");
        ((*ctx_ref.vtable).log)(FfiLogLevel::Info, target, msg);

        FfiResult::ok(&MOCK_MOD_VTABLE as *const _)
    }

    let mod_init: ModInitFn = mock_mod_init;

    // 4. Loader executes mod init
    let init_res = unsafe { mod_init(&loader_context) };
    assert!(init_res.is_ok());

    let returned_vtable = unsafe { &*init_res.value };
    assert!(returned_vtable.on_bootstrap.is_some());

    // Dispatch bootstrap hook
    if let Some(on_boot) = returned_vtable.on_bootstrap {
        let status = unsafe { on_boot() };
        assert_eq!(status, FfiStatus::Success);
    }

    assert!(BOOTSTRAP_CALLED.load(Ordering::SeqCst));
    assert_eq!(
        &*LOGGED_MESSAGES.lock().unwrap(),
        &["TestMod v1.0 initialized!"]
    );
}
