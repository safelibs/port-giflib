#![allow(non_snake_case)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;

use libc::{calloc, free, malloc, realloc};

use crate::bootstrap::catch_panic_or;

pub const MUL_NO_OVERFLOW: usize = 1usize << (usize::BITS / 2);

pub unsafe fn alloc_struct<T>() -> *mut T {
    unsafe { malloc(size_of::<T>()).cast() }
}

pub unsafe fn calloc_array<T>(count: usize) -> *mut T {
    unsafe { calloc(count, size_of::<T>()).cast() }
}

pub unsafe fn alloc_array<T>(count: usize) -> *mut T {
    unsafe { openbsd_reallocarray_impl(ptr::null_mut(), count, size_of::<T>()).cast() }
}

pub unsafe fn realloc_array<T>(ptr: *mut T, count: usize) -> *mut T {
    unsafe { openbsd_reallocarray_impl(ptr.cast(), count, size_of::<T>()).cast() }
}

pub unsafe fn c_malloc(size: usize) -> *mut c_void {
    unsafe { malloc(size) }
}

pub unsafe fn c_free<T>(ptr: *mut T) {
    unsafe { free(ptr.cast()) };
}

pub fn reallocarray_overflow(nmemb: usize, size: usize) -> bool {
    (nmemb >= MUL_NO_OVERFLOW || size >= MUL_NO_OVERFLOW) && nmemb > 0 && usize::MAX / nmemb < size
}

pub unsafe fn set_errno_enomem() {
    unsafe {
        *libc::__errno_location() = libc::ENOMEM;
    }
}

pub unsafe fn openbsd_reallocarray_impl(
    optr: *mut c_void,
    nmemb: usize,
    size: usize,
) -> *mut c_void {
    if reallocarray_overflow(nmemb, size) {
        unsafe { set_errno_enomem() };
        return ptr::null_mut();
    }
    if size == 0 || nmemb == 0 {
        return ptr::null_mut();
    }
    unsafe { realloc(optr, size * nmemb) }
}

#[no_mangle]
pub unsafe extern "C" fn openbsd_reallocarray(
    optr: *mut c_void,
    nmemb: usize,
    size: usize,
) -> *mut c_void {
    catch_panic_or(ptr::null_mut(), || unsafe {
        openbsd_reallocarray_impl(optr, nmemb, size)
    })
}
