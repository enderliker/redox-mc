//! Macros and runtime panic isolation helpers.

use redox_mc_abi::{FfiBuffer, FfiResult, FfiStatus, FfiString};
pub use redox_mc_api_macros::redox_export;
use std::any::Any;

/// Trait providing the safe return value when an exported function catches an isolated panic.
pub trait PanicFallback {
    fn panic_fallback() -> Self;
}

impl<T: PanicFallback> PanicFallback for FfiResult<T> {
    #[inline]
    fn panic_fallback() -> Self {
        Self {
            status: FfiStatus::Panic,
            value: T::panic_fallback(),
        }
    }
}

impl PanicFallback for FfiStatus {
    #[inline]
    fn panic_fallback() -> Self {
        FfiStatus::Panic
    }
}

impl PanicFallback for i32 {
    #[inline]
    fn panic_fallback() -> Self {
        FfiStatus::Panic as i32
    }
}

impl PanicFallback for i64 {
    #[inline]
    fn panic_fallback() -> Self {
        FfiStatus::Panic as i64
    }
}

impl PanicFallback for isize {
    #[inline]
    fn panic_fallback() -> Self {
        FfiStatus::Panic as isize
    }
}

macro_rules! impl_panic_fallback_primitive {
    ($($ty:ty),*) => {
        $(
            impl PanicFallback for $ty {
                #[inline]
                fn panic_fallback() -> Self {
                    Default::default()
                }
            }
        )*
    };
}

impl_panic_fallback_primitive!(u8, u16, u32, u64, usize, i8, i16, f32, f64, bool);

impl PanicFallback for () {
    #[inline]
    fn panic_fallback() -> Self {}
}

impl PanicFallback for FfiString {
    #[inline]
    fn panic_fallback() -> Self {
        FfiString::NULL
    }
}

impl PanicFallback for FfiBuffer {
    #[inline]
    fn panic_fallback() -> Self {
        FfiBuffer::NULL
    }
}

impl<T> PanicFallback for *const T {
    #[inline]
    fn panic_fallback() -> Self {
        std::ptr::null()
    }
}

impl<T> PanicFallback for *mut T {
    #[inline]
    fn panic_fallback() -> Self {
        std::ptr::null_mut()
    }
}

/// Centralized panic handler invoked when an exported mod function catches an unwind.
///
/// Formats the panic payload, logs it to stderr without crashing or aborting,
/// and preserves JVM HotSpot execution.
pub fn handle_panic(fn_name: &str, err: Box<dyn Any + Send>) {
    let msg = if let Some(s) = err.downcast_ref::<&'static str>() {
        *s
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.as_str()
    } else {
        "non-string panic payload"
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eprintln!(
            "[redox-mod] ERROR: Panic caught in exported function '{}': {}",
            fn_name, msg
        );
    }));
}

/// Declarative macro alternative to `#[redox_export]` attribute.
#[macro_export]
macro_rules! redox_export_fn {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident ( $($arg:ident : $arg_ty:ty),* ) -> $ret_ty:ty $body:block
    ) => {
        $(#[$meta])*
        #[no_mangle]
        $vis extern "C" fn $name( $($arg : $arg_ty),* ) -> $ret_ty {
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| $body)) {
                Ok(res) => res,
                Err(err) => {
                    $crate::macros::handle_panic(stringify!($name), err);
                    <$ret_ty as $crate::macros::PanicFallback>::panic_fallback()
                }
            }
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident ( $($arg:ident : $arg_ty:ty),* ) $body:block
    ) => {
        $(#[$meta])*
        #[no_mangle]
        $vis extern "C" fn $name( $($arg : $arg_ty),* ) {
            if let Err(err) = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| $body)) {
                $crate::macros::handle_panic(stringify!($name), err);
            }
        }
    };
}
