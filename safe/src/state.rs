use core::ffi::c_void;
use core::ptr;

use libc::FILE;

use crate::ffi::{GifFileType, GifHashTableType, InputFunc, OutputFunc};
use crate::memory::{alloc_struct, c_free};

pub(crate) const EXTENSION_INTRODUCER: u8 = 0x21;
pub(crate) const DESCRIPTOR_INTRODUCER: u8 = 0x2c;
pub(crate) const TERMINATOR_INTRODUCER: u8 = 0x3b;

pub(crate) const LZ_MAX_CODE: i32 = 4095;
pub(crate) const LZ_MAX_CODE_U: usize = LZ_MAX_CODE as usize;
pub(crate) const LZ_BITS: i32 = 12;

pub(crate) const FLUSH_OUTPUT: i32 = 4096;
pub(crate) const FIRST_CODE: i32 = 4097;
pub(crate) const NO_SUCH_CODE: i32 = 4098;

pub(crate) const FILE_STATE_WRITE: i32 = 0x01;
pub(crate) const FILE_STATE_SCREEN: i32 = 0x02;
pub(crate) const FILE_STATE_IMAGE: i32 = 0x04;
pub(crate) const FILE_STATE_READ: i32 = 0x08;

const ENCODER_STATE_TAG: [u8; 8] = *b"EGIFRS04";
const DECODER_STATE_TAG: [u8; 8] = *b"DGIFRS04";

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

#[repr(C)]
pub(crate) struct DecoderState {
    tag: [u8; 8],
    pub(crate) file_state: i32,
    pub(crate) file_handle: i32,
    pub(crate) bits_per_pixel: i32,
    pub(crate) clear_code: i32,
    pub(crate) eof_code: i32,
    pub(crate) running_code: i32,
    pub(crate) running_bits: i32,
    pub(crate) max_code1: i32,
    pub(crate) last_code: i32,
    pub(crate) stack_ptr: i32,
    pub(crate) current_shift_state: i32,
    pub(crate) current_shift_dword: u64,
    pub(crate) pixel_count: u64,
    pub(crate) file: *mut FILE,
    pub(crate) read_func: InputFunc,
    pub(crate) buf: [u8; 256],
    pub(crate) stack: [u8; LZ_MAX_CODE_U],
    pub(crate) suffix: [u8; LZ_MAX_CODE_U + 1],
    pub(crate) prefix: [u32; LZ_MAX_CODE_U + 1],
    pub(crate) gif89: bool,
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn alloc_gif_file() -> *mut GifFileType {
    let gif_file = unsafe { alloc_struct::<GifFileType>() };
    if gif_file.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        ptr::write_bytes(gif_file, 0, 1);
    }
    gif_file
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn free_gif_file(gif_file: *mut GifFileType) {
    unsafe {
        c_free(gif_file);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn alloc_encoder_state() -> *mut EncoderState {
    let state = unsafe { alloc_struct::<EncoderState>() };
    if state.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        ptr::write_bytes(state, 0, 1);
        (*state).tag = ENCODER_STATE_TAG;
        (*state).file_handle = -1;
        (*state).current_code = FIRST_CODE;
    }

    state
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn free_encoder_state(state: *mut EncoderState) {
    unsafe {
        c_free(state);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn alloc_decoder_state() -> *mut DecoderState {
    let state = unsafe { alloc_struct::<DecoderState>() };
    if state.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        ptr::write_bytes(state, 0, 1);
        (*state).tag = DECODER_STATE_TAG;
        (*state).file_handle = -1;
        (*state).last_code = NO_SUCH_CODE;
    }

    state
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn free_decoder_state(state: *mut DecoderState) {
    unsafe {
        c_free(state);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn encoder_state_from_private(private: *mut c_void) -> *mut EncoderState {
    if private.is_null() {
        return ptr::null_mut();
    }

    let state = private.cast::<EncoderState>();
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).tag } != ENCODER_STATE_TAG {
        return ptr::null_mut();
    }

    state
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn decoder_state_from_private(private: *mut c_void) -> *mut DecoderState {
    if private.is_null() {
        return ptr::null_mut();
    }

    let state = private.cast::<DecoderState>();
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).tag } != DECODER_STATE_TAG {
        return ptr::null_mut();
    }

    state
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn encoder_state(gif_file: *mut GifFileType) -> *mut EncoderState {
    if gif_file.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    unsafe { encoder_state_from_private((*gif_file).Private) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn decoder_state(gif_file: *mut GifFileType) -> *mut DecoderState {
    if gif_file.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    unsafe { decoder_state_from_private((*gif_file).Private) }
}
