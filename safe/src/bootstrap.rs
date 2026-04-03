use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::ffi::{GifFileType, InputFunc, OutputFunc};

#[cfg(target_os = "linux")]
#[link(name = "gif_legacy", kind = "static", modifiers = "+whole-archive")]
unsafe extern "C" {
    fn DGifOpen(
        user_ptr: *mut core::ffi::c_void,
        read_func: InputFunc,
        error: *mut i32,
    ) -> *mut GifFileType;
    fn EGifOpen(
        user_ptr: *mut core::ffi::c_void,
        write_func: OutputFunc,
        error: *mut i32,
    ) -> *mut GifFileType;
}

#[cfg(not(target_os = "linux"))]
#[link(name = "gif_legacy", kind = "static")]
unsafe extern "C" {
    fn DGifOpen(
        user_ptr: *mut core::ffi::c_void,
        read_func: InputFunc,
        error: *mut i32,
    ) -> *mut GifFileType;
    fn EGifOpen(
        user_ptr: *mut core::ffi::c_void,
        write_func: OutputFunc,
        error: *mut i32,
    ) -> *mut GifFileType;
}

#[used]
static LINK_DGIF_OPEN: unsafe extern "C" fn(
    *mut core::ffi::c_void,
    InputFunc,
    *mut i32,
) -> *mut GifFileType = DGifOpen;
#[used]
static LINK_EGIF_OPEN: unsafe extern "C" fn(
    *mut core::ffi::c_void,
    OutputFunc,
    *mut i32,
) -> *mut GifFileType = EGifOpen;

pub(crate) const LEGACY_BACKEND_ENABLED: bool = true;

#[allow(dead_code)]
pub(crate) fn catch_panic_or<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}
