//! Symmetric Memory Ownership and Deallocation Contracts for Redox-MC.
//!
//! # The Golden Rule of FFI Memory Ownership
//!
//! Dynamic libraries (cdylibs) on modern operating systems may have separate heap allocators
//! (e.g., jemalloc, mimalloc, glibc ptmalloc, Windows msvcrt).
//! **Memory allocated in one dynamic binary MUST be deallocated in that same dynamic binary.**
//!
//! Violating this rule results in undefined behavior (heap corruption, segfaults, double free).
//!
//! ## Conventions
//!
//! 1. **Borrowed data (`FfiStr`, `FfiSlice`, `FfiSliceMut`)**:
//!    - No transfer of ownership.
//!    - Valid only for the duration of the FFI function call.
//!    - Neither side deallocates borrowed memory.
//!
//! 2. **Loader-allocated owned data (`FfiString`, `FfiBuffer`)**:
//!    - Allocated by `redox-mc-loader`.
//!    - When the mod is done with it, the mod MUST call `redox_loader_free_*` (or `LoaderVTable::free_*`).
//!
//! 3. **Mod-allocated owned data (`FfiString`, `FfiBuffer`)**:
//!    - Allocated by the mod binary.
//!    - When the loader is done with it, the loader MUST call `redox_mod_free_*` (or `ModVTable::free_*`).

use crate::types::{FfiBuffer, FfiString};

/// Frees an `FfiString` allocated in the current binary's heap.
///
/// # Safety
/// `s` must have been created by `FfiString::from_string` within the **same** binary's allocator.
#[inline]
pub unsafe fn free_local_string(s: FfiString) {
    if !s.ptr.is_null() && s.capacity > 0 {
        drop(Vec::from_raw_parts(s.ptr, s.len, s.capacity));
    }
}

/// Frees an `FfiBuffer` allocated in the current binary's heap.
///
/// # Safety
/// `b` must have been created by `FfiBuffer::from_vec` within the **same** binary's allocator.
#[inline]
pub unsafe fn free_local_buffer(b: FfiBuffer) {
    if !b.ptr.is_null() && b.capacity > 0 {
        drop(Vec::from_raw_parts(b.ptr, b.len, b.capacity));
    }
}

// -----------------------------------------------------------------------------
// Explicit Loader Deallocation Exports
// -----------------------------------------------------------------------------

/// Deallocates an `FfiString` allocated by the Redox loader.
///
/// Must be exported by `redox-mc-loader`.
///
/// # Safety
/// The caller must ensure `s` was created by `redox-mc-loader`.
#[no_mangle]
pub unsafe extern "C" fn redox_loader_free_string(s: FfiString) {
    free_local_string(s);
}

/// Deallocates an `FfiBuffer` allocated by the Redox loader.
///
/// Must be exported by `redox-mc-loader`.
///
/// # Safety
/// The caller must ensure `b` was created by `redox-mc-loader`.
#[no_mangle]
pub unsafe extern "C" fn redox_loader_free_buffer(b: FfiBuffer) {
    free_local_buffer(b);
}

// -----------------------------------------------------------------------------
// Explicit Mod Deallocation Exports
// -----------------------------------------------------------------------------

/// Deallocates an `FfiString` allocated by a mod.
///
/// Must be implemented/exported by each mod binary (or through `redox-mc-api`).
///
/// # Safety
/// The caller must ensure `s` was created by the calling mod's allocator.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_free_string(s: FfiString) {
    free_local_string(s);
}

/// Deallocates an `FfiBuffer` allocated by a mod.
///
/// Must be implemented/exported by each mod binary (or through `redox-mc-api`).
///
/// # Safety
/// The caller must ensure `b` was created by the calling mod's allocator.
#[no_mangle]
pub unsafe extern "C" fn redox_mod_free_buffer(b: FfiBuffer) {
    free_local_buffer(b);
}
