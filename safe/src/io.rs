use core::ffi::c_char;

use libc::{mode_t, FILE};

use crate::ffi::GifFileType;
use crate::state::{decoder_state, encoder_state};

const WRITE_MODE: &[u8] = b"wb\0";
const READ_MODE: &[u8] = b"rb\0";

#[cfg(windows)]
unsafe fn set_binary_mode(file_handle: i32) {
    unsafe {
        let _ = libc::_setmode(file_handle, libc::O_BINARY);
    }
}

#[cfg(not(windows))]
unsafe fn set_binary_mode(_file_handle: i32) {}

pub(crate) unsafe fn open_input_file(file_name: *const c_char) -> i32 {
    if file_name.is_null() {
        return -1;
    }

    unsafe { libc::open(file_name, libc::O_RDONLY) }
}

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

    unsafe { libc::open(file_name, flags, mode) }
}

pub(crate) unsafe fn close_fd(file_handle: i32) {
    unsafe {
        let _ = libc::close(file_handle);
    }
}

pub(crate) unsafe fn fdopen_read(file_handle: i32) -> *mut FILE {
    unsafe {
        set_binary_mode(file_handle);
        libc::fdopen(file_handle, READ_MODE.as_ptr().cast())
    }
}

pub(crate) unsafe fn fdopen_write(file_handle: i32) -> *mut FILE {
    unsafe {
        set_binary_mode(file_handle);
        libc::fdopen(file_handle, WRITE_MODE.as_ptr().cast())
    }
}

pub(crate) unsafe fn fclose_input(file: *mut FILE) -> i32 {
    if file.is_null() {
        return 0;
    }

    unsafe { libc::fclose(file) }
}

pub(crate) unsafe fn fclose_output(file: *mut FILE) -> i32 {
    if file.is_null() {
        return 0;
    }

    unsafe { libc::fclose(file) }
}

pub(crate) unsafe fn internal_read(
    gif_file: *mut GifFileType,
    buffer: *mut u8,
    len: usize,
) -> usize {
    let state = unsafe { decoder_state(gif_file) };
    if state.is_null() || buffer.is_null() || len == 0 {
        return 0;
    }

    if let Some(read_func) = unsafe { (*state).read_func } {
        let len = match i32::try_from(len) {
            Ok(len) => len,
            Err(_) => return 0,
        };
        let read = unsafe { read_func(gif_file, buffer, len) };
        if read < 0 {
            0
        } else {
            read as usize
        }
    } else if unsafe { (*state).file.is_null() } {
        0
    } else {
        unsafe { libc::fread(buffer.cast(), 1, len, (*state).file) }
    }
}

pub(crate) unsafe fn internal_write(
    gif_file: *mut GifFileType,
    buffer: *const u8,
    len: usize,
) -> usize {
    let state = unsafe { encoder_state(gif_file) };
    if state.is_null() || len == 0 {
        return 0;
    }

    if let Some(write_func) = unsafe { (*state).write_func } {
        let len = match i32::try_from(len) {
            Ok(len) => len,
            Err(_) => return 0,
        };
        let written = unsafe { write_func(gif_file, buffer, len) };
        if written < 0 {
            0
        } else {
            written as usize
        }
    } else if unsafe { (*state).file.is_null() } {
        0
    } else {
        unsafe { libc::fwrite(buffer.cast(), 1, len, (*state).file) }
    }
}

pub(crate) unsafe fn write_exact(
    gif_file: *mut GifFileType,
    buffer: *const u8,
    len: usize,
) -> bool {
    unsafe { internal_write(gif_file, buffer, len) == len }
}
