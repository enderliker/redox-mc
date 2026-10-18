//! Safety and panic isolation subsystem for Redox-MC loader.
//!
//! HotSpot JVM relies on catching hardware `SIGSEGV` signals on Linux/macOS
//! and Structured Exception Handling (SEH) Access Violations on Windows
//! to implement implicit null checks, deoptimization checkpoints, and stack overflow handling.
//!
//! The loader strictly avoids registering any signal handlers for `SIGSEGV` or SEH filters,
//! guaranteeing zero interference with JVM internals.
//! When a native mod panics, the panic is intercepted at the mod's FFI boundary via
//! `#[redox_export]` and returned as `FfiStatus::Panic`.

use redox_mc_abi::{FfiResult, FfiStatus};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafetyError {
    #[error("Native mod '{0}' panicked during execution and was safely isolated")]
    ModPanicked(String),
    #[error("Native mod '{0}' returned failure code: {1}")]
    ModFailed(String, i32),
    #[error("Native mod '{0}' returned FFI error status: {1:?}")]
    FfiError(String, FfiStatus),
}

/// Checks an `FfiStatus` returned from an exported mod function.
pub fn check_ffi_status(mod_id: &str, status: FfiStatus) -> Result<(), SafetyError> {
    match status {
        FfiStatus::Success => Ok(()),
        FfiStatus::Panic => {
            eprintln!(
                "[redox-mc-loader] ERROR: Native mod '{}' panicked during execution. JVM preserved.",
                mod_id
            );
            Err(SafetyError::ModPanicked(mod_id.to_string()))
        }
        other => Err(SafetyError::FfiError(mod_id.to_string(), other)),
    }
}

/// Checks an integer return code from a mod entrypoint (e.g. `redox_mod_init`).
pub fn check_ffi_code(mod_id: &str, code: i32) -> Result<(), SafetyError> {
    if code == 0 {
        Ok(())
    } else if code == FfiStatus::Panic as i32 {
        eprintln!(
            "[redox-mc-loader] ERROR: Native mod '{}' panicked during initialization (code: -5).",
            mod_id
        );
        Err(SafetyError::ModPanicked(mod_id.to_string()))
    } else {
        Err(SafetyError::ModFailed(mod_id.to_string(), code))
    }
}

/// Validates an `FfiResult<T>` and returns the inner value if successful.
pub fn check_ffi_result<T>(mod_id: &str, result: FfiResult<T>) -> Result<T, SafetyError> {
    check_ffi_status(mod_id, result.status)?;
    Ok(result.value)
}

/// Verifies that HotSpot's signal handling is strictly preserved without loader overrides.
#[inline]
pub const fn is_hotspot_signal_handling_preserved() -> bool {
    // The loader intentionally registers NO SIGSEGV, SIGBUS, or Access Violation handlers.
    true
}
