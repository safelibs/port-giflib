use core::ffi::c_void;
use core::ptr;

use libc::FILE;

use crate::ffi::{GifFileType, GifHashTableType, OutputFunc};
use crate::memory::{alloc_struct, c_free};

pub(crate) const EXTENSION_INTRODUCER: u8 = 0x21;
pub(crate) const DESCRIPTOR_INTRODUCER: u8 = 0x2c;
pub(crate) const TERMINATOR_INTRODUCER: u8 = 0x3b;

pub(crate) const LZ_MAX_CODE: i32 = 4095;
pub(crate) const FLUSH_OUTPUT: i32 = 4096;
pub(crate) const FIRST_CODE: i32 = 4097;

pub(crate) const FILE_STATE_WRITE: i32 = 0x01;
pub(crate) const FILE_STATE_SCREEN: i32 = 0x02;
pub(crate) const FILE_STATE_IMAGE: i32 = 0x04;

const ENCODER_STATE_TAG: [u8; 8] = *b"EGIFRS03";

#[repr(C)]
pub(crate) struct EncoderState {
    tag: [u8; 8],
    pub(crate) file_state: i32,
    pub(crate) file_handle: i32,
    pub(crate) bits_per_pixel: i32,
    pub(crate) clear_code: i32,
    pub(crate) eof_code: i32,
    pub(crate) running_code: i32,
    pub(crate) running_bits: i32,
    pub(crate) max_code1: i32,
    pub(crate) current_code: i32,
    pub(crate) current_shift_state: i32,
    pub(crate) current_shift_dword: u64,
    pub(crate) pixel_count: u64,
    pub(crate) file: *mut FILE,
    pub(crate) write_func: OutputFunc,
    pub(crate) output_buffer: [u8; 256],
    pub(crate) hash_table: *mut GifHashTableType,
    pub(crate) gif89: bool,
}

pub(crate) unsafe fn alloc_gif_file() -> *mut GifFileType {
    let gif_file = unsafe { alloc_struct::<GifFileType>() };
    if gif_file.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::write_bytes(gif_file, 0, 1);
    }
    gif_file
}

pub(crate) unsafe fn free_gif_file(gif_file: *mut GifFileType) {
    unsafe {
        c_free(gif_file);
    }
}

pub(crate) unsafe fn alloc_encoder_state() -> *mut EncoderState {
    let state = unsafe { alloc_struct::<EncoderState>() };
    if state.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        ptr::write_bytes(state, 0, 1);
        (*state).tag = ENCODER_STATE_TAG;
        (*state).current_code = FIRST_CODE;
    }

    state
}

pub(crate) unsafe fn free_encoder_state(state: *mut EncoderState) {
    unsafe {
        c_free(state);
    }
}

pub(crate) unsafe fn encoder_state_from_private(private: *mut c_void) -> *mut EncoderState {
    if private.is_null() {
        return ptr::null_mut();
    }

    let state = private.cast::<EncoderState>();
    if unsafe { (*state).tag } != ENCODER_STATE_TAG {
        return ptr::null_mut();
    }

    state
}

pub(crate) unsafe fn encoder_state(gif_file: *mut GifFileType) -> *mut EncoderState {
    if gif_file.is_null() {
        return ptr::null_mut();
    }

    unsafe { encoder_state_from_private((*gif_file).Private) }
}
