//! # Redox-MC ABI
//!
//! Binary Interface specification, C-compatible memory representations,
//! vtables, and symmetric deallocation contracts for the Redox-MC native mod loader.
//!
//! ## Core Guarantees
//!
//! 1. **Binary Stability**: All structures that cross dynamic library boundaries
//!    implement `#[repr(C)]` or fixed-width integer representations (`#[repr(u8)]`, `#[repr(i32)]`).
//! 2. **Symmetric Deallocation**: Any memory allocated in the loader must be freed
//!    by the loader; any memory allocated in a mod must be freed by that mod.
//! 3. **Panic Isolation**: Panics must not cross `extern "C"` boundaries; errors are
//!    communicated via `FfiStatus` or `FfiResult<T>`.

pub mod memory;
pub mod types;
pub mod vtable;

pub use memory::{
    free_local_buffer, free_local_string, redox_loader_free_buffer, redox_loader_free_string,
    redox_mod_free_buffer, redox_mod_free_string,
};
pub use types::{
    AbiVersion, FfiBuffer, FfiLogLevel, FfiModSide, FfiResult, FfiSlice, FfiSliceMut, FfiStatus,
    FfiStr, FfiString,
};
pub use vtable::{LoaderContext, LoaderVTable, ModInitFn, ModVTable};

/// Canonical ABI version of this release.
pub const REDOX_ABI_VERSION: AbiVersion = AbiVersion::new(1, 0, 0);

/// Convenience prelude module.
pub mod prelude {
    pub use crate::memory::*;
    pub use crate::types::*;
    pub use crate::vtable::*;
    pub use crate::REDOX_ABI_VERSION;
}
