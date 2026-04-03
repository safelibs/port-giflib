#![allow(non_snake_case)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;

use libc::{calloc, free, malloc, realloc};

use crate::bootstrap::catch_panic_or_else;

pub const MUL_NO_OVERFLOW: usize = 1usize << (usize::BITS / 2);

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn alloc_struct<T>() -> *mut T {
    unsafe { malloc(size_of::<T>()).cast() }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn calloc_array<T>(count: usize) -> *mut T {
    unsafe { calloc(count, size_of::<T>()).cast() }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn alloc_array<T>(count: usize) -> *mut T {
    unsafe { openbsd_reallocarray_impl(ptr::null_mut(), count, size_of::<T>()).cast() }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn realloc_array<T>(ptr: *mut T, count: usize) -> *mut T {
    unsafe { openbsd_reallocarray_impl(ptr.cast(), count, size_of::<T>()).cast() }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn c_malloc(size: usize) -> *mut c_void {
    unsafe { malloc(size) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn c_free<T>(ptr: *mut T) {
    unsafe { free(ptr.cast()) };
}

pub fn reallocarray_overflow(nmemb: usize, size: usize) -> bool {
    (nmemb >= MUL_NO_OVERFLOW || size >= MUL_NO_OVERFLOW) && nmemb > 0 && usize::MAX / nmemb < size
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn set_errno_enomem() {
    unsafe {
        *libc::__errno_location() = libc::ENOMEM;
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub unsafe fn openbsd_reallocarray_impl(
    optr: *mut c_void,
    nmemb: usize,
    size: usize,
) -> *mut c_void {
    if reallocarray_overflow(nmemb, size) {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe { set_errno_enomem() };
        return ptr::null_mut();
    }
    if size == 0 || nmemb == 0 {
        return ptr::null_mut();
    }
    // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
    unsafe { realloc(optr, size * nmemb) }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn openbsd_reallocarray(
    optr: *mut c_void,
    nmemb: usize,
    size: usize,
) -> *mut c_void {
    catch_panic_or_else(
        ptr::null_mut(),
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        || unsafe {
            set_errno_enomem();
        },
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        || unsafe { openbsd_reallocarray_impl(optr, nmemb, size) },
    )
}
