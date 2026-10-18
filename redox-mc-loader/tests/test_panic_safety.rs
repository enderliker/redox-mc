use redox_mc_abi::{FfiResult, FfiStatus};
use redox_mc_loader::safety::{check_ffi_code, check_ffi_result, check_ffi_status, SafetyError};

#[test]
fn test_check_ffi_status_success() {
    let res = check_ffi_status("test_mod", FfiStatus::Success);
    assert!(res.is_ok());
}

#[test]
fn test_check_ffi_status_panic_detection() {
    let res = check_ffi_status("test_mod", FfiStatus::Panic);
    assert_eq!(res, Err(SafetyError::ModPanicked("test_mod".to_string())));
}

#[test]
fn test_check_ffi_code_panic() {
    let res = check_ffi_code("test_mod", FfiStatus::Panic as i32);
    assert_eq!(res, Err(SafetyError::ModPanicked("test_mod".to_string())));

    let success = check_ffi_code("test_mod", 0);
    assert!(success.is_ok());

    let other_err = check_ffi_code("test_mod", -1);
    assert_eq!(
        other_err,
        Err(SafetyError::ModFailed("test_mod".to_string(), -1))
    );
}

#[test]
fn test_check_ffi_result_panic() {
    let panicking_res: FfiResult<u32> = FfiResult {
        status: FfiStatus::Panic,
        value: 0,
    };
    let checked = check_ffi_result("test_mod", panicking_res);
    assert_eq!(
        checked,
        Err(SafetyError::ModPanicked("test_mod".to_string()))
    );

    let ok_res: FfiResult<u32> = FfiResult::ok(100);
    let checked_ok = check_ffi_result("test_mod", ok_res);
    assert_eq!(checked_ok, Ok(100));
}

#[test]
fn test_no_signal_handlers_registered_by_loader() {
    // Assert that the loader never alters HotSpot's SIGSEGV or Access Violation handling
    assert!(redox_mc_loader::safety::is_hotspot_signal_handling_preserved());
}
