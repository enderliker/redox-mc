use redox_mc_api::abi::{FfiResult, FfiStatus};
use redox_mc_api::redox_export;

#[redox_export]
pub fn export_successful_result(x: u32) -> FfiResult<u32> {
    FfiResult::ok(x * 2)
}

#[redox_export]
pub fn export_panicking_result(_x: u32) -> FfiResult<u32> {
    panic!("error de prueba in panicking_result");
}

#[redox_export]
pub fn export_successful_status() -> FfiStatus {
    FfiStatus::Success
}

#[redox_export]
pub fn export_panicking_status() -> FfiStatus {
    panic!("error de prueba in panicking_status");
}

#[redox_export]
pub fn export_panicking_i32() -> i32 {
    panic!("error de prueba in panicking_i32");
}

#[redox_export]
pub fn export_panicking_void() {
    panic!("error de prueba in panicking_void");
}

#[test]
fn test_successful_calls_return_expected_values() {
    let res = export_successful_result(21);
    assert_eq!(res.status, FfiStatus::Success);
    assert_eq!(res.value, 42);

    let status = export_successful_status();
    assert_eq!(status, FfiStatus::Success);
}

#[test]
fn test_panic_in_result_is_isolated_and_returns_ffi_status_panic() {
    // Calling export_panicking_result must NOT unwind or abort!
    let res = export_panicking_result(10);
    assert_eq!(res.status, FfiStatus::Panic);
}

#[test]
fn test_panic_in_status_is_isolated_and_returns_ffi_status_panic() {
    let status = export_panicking_status();
    assert_eq!(status, FfiStatus::Panic);
}

#[test]
fn test_panic_in_i32_is_isolated_and_returns_panic_code() {
    let code = export_panicking_i32();
    assert_eq!(code, FfiStatus::Panic as i32);
}

#[test]
fn test_panic_in_void_is_isolated_without_unwinding() {
    // Calling export_panicking_void must NOT unwind or crash the process!
    export_panicking_void();
}

redox_mc_api::redox_export_fn! {
    pub fn decl_panicking_result(y: u32) -> redox_mc_api::abi::FfiResult<u32> {
        panic!("declarative macro panic test");
    }
}

redox_mc_api::redox_export_fn! {
    pub fn decl_panicking_status() -> redox_mc_api::abi::FfiStatus {
        panic!("declarative macro status panic test");
    }
}

redox_mc_api::redox_export_fn! {
    pub fn decl_panicking_i32() -> i32 {
        panic!("declarative macro i32 panic test");
    }
}

redox_mc_api::redox_export_fn! {
    pub fn decl_panicking_void() {
        panic!("declarative macro void panic test");
    }
}

#[test]
fn test_declarative_macro_isolates_panics() {
    let res = decl_panicking_result(5);
    assert_eq!(res.status, FfiStatus::Panic);

    let status = decl_panicking_status();
    assert_eq!(status, FfiStatus::Panic);

    let code = decl_panicking_i32();
    assert_eq!(code, FfiStatus::Panic as i32);

    decl_panicking_void();
}
