use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::ffi::GifFileType;

pub(crate) fn catch_panic_or<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_panic_or_else(fallback, || {}, f)
}

pub(crate) fn catch_panic_or_else<T>(
    fallback: T,
    on_panic: impl FnOnce(),
    f: impl FnOnce() -> T,
) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            on_panic();
            fallback
        }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn write_error_code(error: *mut i32, code: i32) {
    if !error.is_null() {
        unsafe {
            *error = code;
        }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn write_gif_error(gif_file: *mut GifFileType, code: i32) {
    if !gif_file.is_null() {
        unsafe {
            (*gif_file).Error = code;
        }
    }
}

pub(crate) fn catch_error_or<T>(
    fallback: T,
    error: *mut i32,
    code: i32,
    f: impl FnOnce() -> T,
) -> T {
    catch_panic_or_else(
        fallback,
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        || unsafe {
            write_error_code(error, code);
        },
        f,
    )
}

pub(crate) fn catch_gif_error_or<T>(
    fallback: T,
    gif_file: *mut GifFileType,
    code: i32,
    f: impl FnOnce() -> T,
) -> T {
    catch_panic_or_else(
        fallback,
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        || unsafe {
            write_gif_error(gif_file, code);
        },
        f,
    )
}

pub(crate) fn catch_gif_and_error_or<T>(
    fallback: T,
    gif_file: *mut GifFileType,
    error: *mut i32,
    code: i32,
    f: impl FnOnce() -> T,
) -> T {
    catch_panic_or_else(
        fallback,
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        || unsafe {
            write_gif_error(gif_file, code);
            write_error_code(error, code);
        },
        f,
    )
}
