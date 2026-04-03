#![allow(non_snake_case)]

use core::ffi::c_char;
use core::ptr;

use crate::bootstrap::catch_panic_or;
use crate::ffi::{
    D_GIF_ERR_CLOSE_FAILED, D_GIF_ERR_DATA_TOO_BIG, D_GIF_ERR_EOF_TOO_SOON, D_GIF_ERR_IMAGE_DEFECT,
    D_GIF_ERR_NOT_ENOUGH_MEM, D_GIF_ERR_NOT_GIF_FILE, D_GIF_ERR_NOT_READABLE,
    D_GIF_ERR_NO_COLOR_MAP, D_GIF_ERR_NO_IMAG_DSCR, D_GIF_ERR_NO_SCRN_DSCR, D_GIF_ERR_OPEN_FAILED,
    D_GIF_ERR_READ_FAILED, D_GIF_ERR_WRONG_RECORD, E_GIF_ERR_CLOSE_FAILED, E_GIF_ERR_DATA_TOO_BIG,
    E_GIF_ERR_DISK_IS_FULL, E_GIF_ERR_HAS_IMAG_DSCR, E_GIF_ERR_HAS_SCRN_DSCR,
    E_GIF_ERR_NOT_ENOUGH_MEM, E_GIF_ERR_NOT_WRITEABLE, E_GIF_ERR_NO_COLOR_MAP,
    E_GIF_ERR_OPEN_FAILED, E_GIF_ERR_WRITE_FAILED,
};

const FAILED_TO_OPEN_GIVEN_FILE: &[u8] = b"Failed to open given file\0";
const FAILED_TO_WRITE_TO_GIVEN_FILE: &[u8] = b"Failed to write to given file\0";
const SCREEN_DESCRIPTOR_HAS_ALREADY_BEEN_SET: &[u8] = b"Screen descriptor has already been set\0";
const IMAGE_DESCRIPTOR_IS_STILL_ACTIVE: &[u8] = b"Image descriptor is still active\0";
const NEITHER_GLOBAL_NOR_LOCAL_COLOR_MAP: &[u8] = b"Neither global nor local color map\0";
const NUMBER_OF_PIXELS_BIGGER_THAN_WIDTH_HEIGHT: &[u8] =
    b"Number of pixels bigger than width * height\0";
const FAILED_TO_ALLOCATE_REQUIRED_MEMORY: &[u8] = b"Failed to allocate required memory\0";
const WRITE_FAILED_DISK_FULL: &[u8] = b"Write failed (disk full?)\0";
const FAILED_TO_CLOSE_GIVEN_FILE: &[u8] = b"Failed to close given file\0";
const GIVEN_FILE_WAS_NOT_OPENED_FOR_WRITE: &[u8] = b"Given file was not opened for write\0";
const FAILED_TO_READ_FROM_GIVEN_FILE: &[u8] = b"Failed to read from given file\0";
const DATA_IS_NOT_IN_GIF_FORMAT: &[u8] = b"Data is not in GIF format\0";
const NO_SCREEN_DESCRIPTOR_DETECTED: &[u8] = b"No screen descriptor detected\0";
const NO_IMAGE_DESCRIPTOR_DETECTED: &[u8] = b"No Image Descriptor detected\0";
const WRONG_RECORD_TYPE_DETECTED: &[u8] = b"Wrong record type detected\0";
const GIVEN_FILE_WAS_NOT_OPENED_FOR_READ: &[u8] = b"Given file was not opened for read\0";
const IMAGE_IS_DEFECTIVE_DECODING_ABORTED: &[u8] = b"Image is defective, decoding aborted\0";
const IMAGE_EOF_DETECTED_BEFORE_IMAGE_COMPLETE: &[u8] =
    b"Image EOF detected before image complete\0";

const fn c_string(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr().cast()
}

fn gif_error_string_impl(ErrorCode: i32) -> *const c_char {
    match ErrorCode {
        E_GIF_ERR_OPEN_FAILED | D_GIF_ERR_OPEN_FAILED => c_string(FAILED_TO_OPEN_GIVEN_FILE),
        E_GIF_ERR_WRITE_FAILED => c_string(FAILED_TO_WRITE_TO_GIVEN_FILE),
        E_GIF_ERR_HAS_SCRN_DSCR => c_string(SCREEN_DESCRIPTOR_HAS_ALREADY_BEEN_SET),
        E_GIF_ERR_HAS_IMAG_DSCR => c_string(IMAGE_DESCRIPTOR_IS_STILL_ACTIVE),
        E_GIF_ERR_NO_COLOR_MAP | D_GIF_ERR_NO_COLOR_MAP => {
            c_string(NEITHER_GLOBAL_NOR_LOCAL_COLOR_MAP)
        }
        E_GIF_ERR_DATA_TOO_BIG | D_GIF_ERR_DATA_TOO_BIG => {
            c_string(NUMBER_OF_PIXELS_BIGGER_THAN_WIDTH_HEIGHT)
        }
        E_GIF_ERR_NOT_ENOUGH_MEM | D_GIF_ERR_NOT_ENOUGH_MEM => {
            c_string(FAILED_TO_ALLOCATE_REQUIRED_MEMORY)
        }
        E_GIF_ERR_DISK_IS_FULL => c_string(WRITE_FAILED_DISK_FULL),
        E_GIF_ERR_CLOSE_FAILED | D_GIF_ERR_CLOSE_FAILED => c_string(FAILED_TO_CLOSE_GIVEN_FILE),
        E_GIF_ERR_NOT_WRITEABLE => c_string(GIVEN_FILE_WAS_NOT_OPENED_FOR_WRITE),
        D_GIF_ERR_READ_FAILED => c_string(FAILED_TO_READ_FROM_GIVEN_FILE),
        D_GIF_ERR_NOT_GIF_FILE => c_string(DATA_IS_NOT_IN_GIF_FORMAT),
        D_GIF_ERR_NO_SCRN_DSCR => c_string(NO_SCREEN_DESCRIPTOR_DETECTED),
        D_GIF_ERR_NO_IMAG_DSCR => c_string(NO_IMAGE_DESCRIPTOR_DETECTED),
        D_GIF_ERR_WRONG_RECORD => c_string(WRONG_RECORD_TYPE_DETECTED),
        D_GIF_ERR_NOT_READABLE => c_string(GIVEN_FILE_WAS_NOT_OPENED_FOR_READ),
        D_GIF_ERR_IMAGE_DEFECT => c_string(IMAGE_IS_DEFECTIVE_DECODING_ABORTED),
        D_GIF_ERR_EOF_TOO_SOON => c_string(IMAGE_EOF_DETECTED_BEFORE_IMAGE_COMPLETE),
        _ => ptr::null(),
    }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifErrorString(ErrorCode: i32) -> *const c_char {
    catch_panic_or(ptr::null(), || gif_error_string_impl(ErrorCode))
}
