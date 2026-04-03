use core::ffi::c_char;

use libc::{mode_t, FILE};

use crate::ffi::GifFileType;
use crate::state::{decoder_state, encoder_state};

const WRITE_MODE: &[u8] = b"wb\0";
const READ_MODE: &[u8] = b"rb\0";

#[cfg(windows)]
// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn set_binary_mode(file_handle: i32) {
    unsafe {
        let _ = libc::_setmode(file_handle, libc::O_BINARY);
    }
}

#[cfg(not(windows))]
// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn set_binary_mode(_file_handle: i32) {}

pub(crate) unsafe fn open_input_file(file_name: *const c_char) -> i32 {
    if file_name.is_null() {
        return -1;
    }

    // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
    unsafe { libc::open(file_name, libc::O_RDONLY) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn open_output_file(file_name: *const c_char, test_existence: bool) -> i32 {
    if file_name.is_null() {
        return -1;
    }

    let flags = if test_existence {
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL
    } else {
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
    };
    let mode: mode_t = libc::S_IRUSR | libc::S_IWUSR;

    // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
    unsafe { libc::open(file_name, flags, mode) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn close_fd(file_handle: i32) {
    unsafe {
        let _ = libc::close(file_handle);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn fdopen_read(file_handle: i32) -> *mut FILE {
    unsafe {
        set_binary_mode(file_handle);
        libc::fdopen(file_handle, READ_MODE.as_ptr().cast())
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn fdopen_write(file_handle: i32) -> *mut FILE {
    unsafe {
        set_binary_mode(file_handle);
        libc::fdopen(file_handle, WRITE_MODE.as_ptr().cast())
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn fclose_input(file: *mut FILE) -> i32 {
    if file.is_null() {
        return 0;
    }

    // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
    unsafe { libc::fclose(file) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn fclose_output(file: *mut FILE) -> i32 {
    if file.is_null() {
        return 0;
    }

    // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
    unsafe { libc::fclose(file) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn internal_read(
    gif_file: *mut GifFileType,
    buffer: *mut u8,
    len: usize,
) -> usize {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { decoder_state(gif_file) };
    if state.is_null() || buffer.is_null() || len == 0 {
        return 0;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if let Some(read_func) = unsafe { (*state).read_func } {
        let len = match i32::try_from(len) {
            Ok(len) => len,
            Err(_) => return 0,
        };
        // SAFETY: The stored callback comes from the C caller and is invoked with the original giflib ABI.
        let read = unsafe { read_func(gif_file, buffer, len) };
        if read < 0 {
            0
        } else {
            read as usize
        }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    } else if unsafe { (*state).file.is_null() } {
        0
    } else {
        // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
        unsafe { libc::fread(buffer.cast(), 1, len, (*state).file) }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn internal_write(
    gif_file: *mut GifFileType,
    buffer: *const u8,
    len: usize,
) -> usize {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { encoder_state(gif_file) };
    if state.is_null() || len == 0 {
        return 0;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if let Some(write_func) = unsafe { (*state).write_func } {
        let len = match i32::try_from(len) {
            Ok(len) => len,
            Err(_) => return 0,
        };
        // SAFETY: The stored callback comes from the C caller and is invoked with the original giflib ABI.
        let written = unsafe { write_func(gif_file, buffer, len) };
        if written < 0 {
            0
        } else {
            written as usize
        }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    } else if unsafe { (*state).file.is_null() } {
        0
    } else {
        // SAFETY: This forwards validated pointers and sizes to the matching libc routine.
        unsafe { libc::fwrite(buffer.cast(), 1, len, (*state).file) }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn write_exact(
    gif_file: *mut GifFileType,
    buffer: *const u8,
    len: usize,
) -> bool {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { internal_write(gif_file, buffer, len) == len }
}
