//! C-compatible ABI data types for Redox-MC.
//!
//! All types in this module implement `#[repr(C)]` or `#[repr(u8)]`/`#[repr(i32)]`
//! ensuring a stable binary layout across dynamic library (cdylib) boundaries.

use std::fmt;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

/// ABI specification version.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl AbiVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Checks if this ABI version is backwards-compatible with `required`.
    /// Major versions must match; this version's minor must be >= required.minor.
    pub const fn is_compatible_with(&self, required: &AbiVersion) -> bool {
        self.major == required.major && self.minor >= required.minor
    }
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Item behavior flag: custom right-click in air interaction (`use`).
pub const ITEM_FLAG_HAS_USE: u32 = 1 << 0;
/// Item behavior flag: custom right-click on block interaction (`useOn`).
pub const ITEM_FLAG_HAS_USE_ON: u32 = 1 << 1;
/// Item behavior flag: custom finish consuming/using interaction.
pub const ITEM_FLAG_HAS_FINISH_USING: u32 = 1 << 2;

/// Block behavior flag: custom right-click without held item interaction (`useWithoutItem`).
pub const BLOCK_FLAG_HAS_USE_WITHOUT_ITEM: u32 = 1 << 0;
/// Block behavior flag: custom entity step on interaction (`stepOn`).
pub const BLOCK_FLAG_HAS_STEP_ON: u32 = 1 << 1;
/// Block behavior flag: custom destroy interaction (`destroy`).
pub const BLOCK_FLAG_HAS_DESTROY: u32 = 1 << 2;

/// Standard status code returned by FFI calls.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FfiStatus {
    Success = 0,
    GenericError = -1,
    NullPointer = -2,
    InvalidArgument = -3,
    BufferTooSmall = -4,
    Panic = -5,
    VersionMismatch = -6,
    Unsupported = -7,
    NotFound = -8,
}

impl FfiStatus {
    #[inline]
    pub const fn is_success(&self) -> bool {
        matches!(self, FfiStatus::Success)
    }
}

/// A result wrapper containing a status and an optional/default value.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiResult<T> {
    pub status: FfiStatus,
    pub value: T,
}

impl<T> FfiResult<T> {
    #[inline]
    pub const fn ok(value: T) -> Self {
        Self {
            status: FfiStatus::Success,
            value,
        }
    }

    #[inline]
    pub const fn err(status: FfiStatus, fallback: T) -> Self {
        Self {
            status,
            value: fallback,
        }
    }

    #[inline]
    pub const fn is_ok(&self) -> bool {
        self.status.is_success()
    }

    #[inline]
    pub fn into_result(self) -> Result<T, FfiStatus> {
        if self.status.is_success() {
            Ok(self.value)
        } else {
            Err(self.status)
        }
    }
}

/// Borrowed UTF-8 string slice across FFI boundaries.
///
/// Layout: pointer (8 bytes) + length (8 bytes) on 64-bit targets.
/// Does not own or deallocate memory.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiStr {
    pub ptr: *const u8,
    pub len: usize,
}

fn invalid_utf8_error() -> Utf8Error {
    let mut b = [0u8; 1];
    b[0] = 0xFF;
    std::str::from_utf8(&b).unwrap_err()
}

impl FfiStr {
    pub const EMPTY: Self = Self {
        ptr: std::ptr::null(),
        len: 0,
    };

    /// Creates an `FfiStr` from a Rust `&str`.
    #[inline]
    pub const fn from_str(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// Converts this `FfiStr` into a Rust `&str` reference.
    ///
    /// # Safety
    /// The caller must ensure that `ptr` points to valid memory for `len` bytes
    /// and that the lifetime matches the referenced data.
    #[inline]
    pub unsafe fn as_str(&self) -> Result<&str, Utf8Error> {
        if self.len == 0 {
            return Ok("");
        }
        if self.ptr.is_null() {
            return Err(invalid_utf8_error());
        }
        let bytes = std::slice::from_raw_parts(self.ptr, self.len);
        std::str::from_utf8(bytes)
    }

    /// Returns the underlying byte slice.
    ///
    /// # Safety
    /// Same safety requirements as `as_str`.
    #[inline]
    pub unsafe fn as_bytes(&self) -> &[u8] {
        if self.len == 0 || self.ptr.is_null() {
            &[]
        } else {
            std::slice::from_raw_parts(self.ptr, self.len)
        }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0 || self.ptr.is_null()
    }
}

impl fmt::Debug for FfiStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = unsafe { self.as_str().unwrap_or("<invalid utf-8>") };
        write!(f, "FfiStr({:?})", text)
    }
}

impl PartialEq for FfiStr {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        if self.ptr == other.ptr {
            return true;
        }
        unsafe { self.as_bytes() == other.as_bytes() }
    }
}

impl Eq for FfiStr {}

/// Owned heap-allocated UTF-8 string across FFI boundaries.
///
/// Layout: pointer (8 bytes) + length (8 bytes) + capacity (8 bytes) on 64-bit targets.
///
/// # Strict Memory Ownership
/// Memory allocated by the loader must be freed with `redox_loader_free_string`.
/// Memory allocated by a mod must be freed with `redox_mod_free_string`.
#[repr(C)]
pub struct FfiString {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl FfiString {
    pub const NULL: Self = Self {
        ptr: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };

    /// Creates an `FfiString` by taking ownership of a Rust `String`.
    pub fn from_string(mut s: String) -> Self {
        let ptr = s.as_mut_ptr();
        let len = s.len();
        let capacity = s.capacity();
        std::mem::forget(s);
        Self { ptr, len, capacity }
    }

    /// Reconstructs a Rust `String` from this `FfiString`.
    ///
    /// # Safety
    /// Must only be called by the same allocator / binary that created this string.
    pub unsafe fn into_string(self) -> Result<String, FromUtf8Error> {
        if self.ptr.is_null() || self.capacity == 0 {
            return Ok(String::new());
        }
        let vec = Vec::from_raw_parts(self.ptr, self.len, self.capacity);
        String::from_utf8(vec)
    }

    /// Borrows this string as an `FfiStr`.
    #[inline]
    pub fn as_ffi_str(&self) -> FfiStr {
        FfiStr {
            ptr: self.ptr,
            len: self.len,
        }
    }

    /// Borrows this string as a Rust `&str`.
    ///
    /// # Safety
    /// Valid while this `FfiString` is alive and not modified.
    #[inline]
    pub unsafe fn as_str(&self) -> Result<&str, Utf8Error> {
        if self.len == 0 {
            return Ok("");
        }
        if self.ptr.is_null() {
            return Err(invalid_utf8_error());
        }
        let bytes = std::slice::from_raw_parts(self.ptr, self.len);
        std::str::from_utf8(bytes)
    }

    #[inline]
    pub const fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl fmt::Debug for FfiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = unsafe { self.as_str().unwrap_or("<invalid utf-8>") };
        write!(f, "FfiString({:?})", text)
    }
}

/// Generic borrowed slice across FFI boundaries.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiSlice<T> {
    pub ptr: *const T,
    pub len: usize,
}

impl<T> FfiSlice<T> {
    pub const EMPTY: Self = Self {
        ptr: std::ptr::null(),
        len: 0,
    };

    #[inline]
    pub const fn from_slice(s: &[T]) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// # Safety
    /// `ptr` must be valid for `len` elements of type `T`.
    #[inline]
    pub unsafe fn as_slice(&self) -> &[T] {
        if self.len == 0 || self.ptr.is_null() {
            &[]
        } else {
            std::slice::from_raw_parts(self.ptr, self.len)
        }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0 || self.ptr.is_null()
    }
}

/// Generic mutable borrowed slice across FFI boundaries.
#[repr(C)]
pub struct FfiSliceMut<T> {
    pub ptr: *mut T,
    pub len: usize,
}

impl<T> FfiSliceMut<T> {
    pub const EMPTY: Self = Self {
        ptr: std::ptr::null_mut(),
        len: 0,
    };

    #[inline]
    pub fn from_slice_mut(s: &mut [T]) -> Self {
        Self {
            ptr: s.as_mut_ptr(),
            len: s.len(),
        }
    }

    /// # Safety
    /// `ptr` must be valid for `len` elements of type `T`.
    #[inline]
    pub unsafe fn as_slice_mut(&mut self) -> &mut [T] {
        if self.len == 0 || self.ptr.is_null() {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(self.ptr, self.len)
        }
    }
}

/// Owned heap-allocated byte buffer across FFI boundaries.
///
/// # Strict Memory Ownership
/// Memory allocated by the loader must be freed with `redox_loader_free_buffer`.
/// Memory allocated by a mod must be freed with `redox_mod_free_buffer`.
#[repr(C)]
pub struct FfiBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl FfiBuffer {
    pub const NULL: Self = Self {
        ptr: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };

    /// Creates an `FfiBuffer` by taking ownership of a `Vec<u8>`.
    pub fn from_vec(mut v: Vec<u8>) -> Self {
        let ptr = v.as_mut_ptr();
        let len = v.len();
        let capacity = v.capacity();
        std::mem::forget(v);
        Self { ptr, len, capacity }
    }

    /// Reconstructs a `Vec<u8>` from this `FfiBuffer`.
    ///
    /// # Safety
    /// Must only be called by the same allocator / binary that created this buffer.
    pub unsafe fn into_vec(self) -> Vec<u8> {
        if self.ptr.is_null() || self.capacity == 0 {
            return Vec::new();
        }
        Vec::from_raw_parts(self.ptr, self.len, self.capacity)
    }

    /// Borrows this buffer as a byte slice.
    ///
    /// # Safety
    /// Valid while this `FfiBuffer` is alive and not modified.
    #[inline]
    pub unsafe fn as_bytes(&self) -> &[u8] {
        if self.len == 0 || self.ptr.is_null() {
            &[]
        } else {
            std::slice::from_raw_parts(self.ptr, self.len)
        }
    }

    /// Borrows this buffer as a mutable byte slice.
    ///
    /// # Safety
    /// Valid while this `FfiBuffer` is alive and not accessed concurrently.
    #[inline]
    pub unsafe fn as_bytes_mut(&mut self) -> &mut [u8] {
        if self.len == 0 || self.ptr.is_null() {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(self.ptr, self.len)
        }
    }

    #[inline]
    pub const fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl fmt::Debug for FfiBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FfiBuffer(ptr: {:?}, len: {}, cap: {})",
            self.ptr, self.len, self.capacity
        )
    }
}

/// Target side / environment where code is running.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FfiModSide {
    Both = 0,
    Client = 1,
    Server = 2,
}

/// ABI marker: the network callback is running on the dedicated/integrated server.
pub const NETWORK_SIDE_SERVER: u32 = 0;
/// ABI marker: the network callback is running on the client.
pub const NETWORK_SIDE_CLIENT: u32 = 1;

/// Outbound target: broadcast to every peer of the current side.
pub const NETWORK_TARGET_BROADCAST: u32 = 0;
/// Outbound target: a specific player, addressed by name and/or UUID.
pub const NETWORK_TARGET_PLAYER: u32 = 1;
/// Outbound target: the remote server (valid from the client side).
pub const NETWORK_TARGET_SERVER: u32 = 2;

/// Signature of the mod-exported function that receives an inbound mod payload.
///
/// Parameters:
/// 1. Full channel identifier (`namespace:path`).
/// 2. Borrowed payload bytes (valid only for the duration of the call).
/// 3. Sender name (`"server"` for client-side inbound messages).
/// 4. Sender UUID (empty for client-side inbound messages).
/// 5. One of [`NETWORK_SIDE_SERVER`] / [`NETWORK_SIDE_CLIENT`].
pub type NetworkDispatchFn =
    unsafe extern "C" fn(FfiStr, FfiSlice<u8>, FfiStr, FfiStr, u32) -> FfiStatus;

/// Signature of the visitor invoked by a mod when the loader drains its outbox.
///
/// Parameters:
/// 1. Opaque loader-provided context pointer.
/// 2. Full channel identifier (`namespace:path`).
/// 3. One of the `NETWORK_TARGET_*` constants.
/// 4. Target player name (empty for broadcast/server targets).
/// 5. Target player UUID (empty for broadcast/server targets).
/// 6. Borrowed payload bytes (valid only for the duration of the call).
pub type NetworkOutboundVisitor = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    FfiStr,
    u32,
    FfiStr,
    FfiStr,
    FfiSlice<u8>,
) -> FfiStatus;

/// Signature of the mod-exported function that drains queued outbound messages.
///
/// The mod must invoke `visitor` once per queued message and may free the
/// message memory immediately after the visitor returns.
pub type NetworkOutboxDrainFn =
    unsafe extern "C" fn(NetworkOutboundVisitor, *mut std::ffi::c_void) -> FfiStatus;

/// Log level severity for ABI logger callbacks.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FfiLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}
