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
// Loader / Mod Deallocation Helpers (Passed via VTable function pointers)
// -----------------------------------------------------------------------------

/// Deallocates an `FfiString` using the caller binary's local allocator.
///
/// # Safety
/// The caller must ensure `s` was created by the caller binary's allocator.
pub unsafe extern "C" fn redox_loader_free_string(s: FfiString) {
    free_local_string(s);
}

/// Deallocates an `FfiBuffer` using the caller binary's local allocator.
///
/// # Safety
/// The caller must ensure `b` was created by the caller binary's allocator.
pub unsafe extern "C" fn redox_loader_free_buffer(b: FfiBuffer) {
    free_local_buffer(b);
}

/// Deallocates an `FfiString` using a mod's local allocator.
///
/// # Safety
/// The caller must ensure `s` was created by the mod's allocator.
pub unsafe extern "C" fn redox_mod_free_string(s: FfiString) {
    free_local_string(s);
}

/// Deallocates an `FfiBuffer` using a mod's local allocator.
///
/// # Safety
/// The caller must ensure `b` was created by the mod's allocator.
pub unsafe extern "C" fn redox_mod_free_buffer(b: FfiBuffer) {
    free_local_buffer(b);
}
