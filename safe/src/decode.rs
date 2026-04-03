#![allow(non_snake_case)]

use core::ffi::{c_char, c_void};
use core::ptr;

use crate::bootstrap::{catch_error_or, catch_gif_and_error_or, catch_gif_error_or};
use crate::ffi::{
    GifByteType, GifFileType, GifPixelType, GifPrefixType, GifRecordType, SavedImage,
    D_GIF_ERR_CLOSE_FAILED, D_GIF_ERR_DATA_TOO_BIG, D_GIF_ERR_EOF_TOO_SOON, D_GIF_ERR_IMAGE_DEFECT,
    D_GIF_ERR_NOT_ENOUGH_MEM, D_GIF_ERR_NOT_GIF_FILE, D_GIF_ERR_NOT_READABLE,
    D_GIF_ERR_NO_SCRN_DSCR, D_GIF_ERR_OPEN_FAILED, D_GIF_ERR_READ_FAILED, D_GIF_ERR_WRONG_RECORD,
    EXTENSION_RECORD_TYPE, GIF_ERROR, GIF_OK, IMAGE_DESC_RECORD_TYPE, TERMINATE_RECORD_TYPE,
    UNDEFINED_RECORD_TYPE,
};
use crate::helpers::{GifFreeExtensions, GifFreeMapObject, GifFreeSavedImages, GifMakeMapObject};
use crate::io::{close_fd, fclose_input, fdopen_read, internal_read, open_input_file};
use crate::memory::{alloc_struct, realloc_array};
use crate::state::{
    alloc_decoder_state, alloc_gif_file, decoder_state, free_decoder_state, free_gif_file,
    DecoderState, DESCRIPTOR_INTRODUCER, EXTENSION_INTRODUCER, FILE_STATE_READ, LZ_BITS,
    LZ_MAX_CODE, NO_SUCH_CODE, TERMINATOR_INTRODUCER,
};

const GIF_STAMP_LEN: usize = 6;
const GIF_VERSION_POS: usize = 3;
const GIF87_STAMP: &[u8; 7] = b"GIF87a\0";
const GIF89_STAMP: &[u8; 7] = b"GIF89a\0";
const GIF_PREFIX: &[u8; GIF_VERSION_POS] = b"GIF";
const CODE_MASKS: [u16; 13] = [
    0x0000, 0x0001, 0x0003, 0x0007, 0x000f, 0x001f, 0x003f, 0x007f, 0x00ff, 0x01ff, 0x03ff, 0x07ff,
    0x0fff,
];

fn catch_decode_or<T>(fallback: T, GifFile: *mut GifFileType, f: impl FnOnce() -> T) -> T {
    catch_gif_error_or(fallback, GifFile, D_GIF_ERR_NOT_ENOUGH_MEM, f)
}

fn catch_decode_open_or<T>(fallback: T, Error: *mut i32, f: impl FnOnce() -> T) -> T {
    catch_error_or(fallback, Error, D_GIF_ERR_NOT_ENOUGH_MEM, f)
}

fn catch_decode_close_or<T>(
    fallback: T,
    GifFile: *mut GifFileType,
    ErrorCode: *mut i32,
    f: impl FnOnce() -> T,
) -> T {
    catch_gif_and_error_or(fallback, GifFile, ErrorCode, D_GIF_ERR_CLOSE_FAILED, f)
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn set_error(GifFile: *mut GifFileType, Error: i32) {
    if !GifFile.is_null() {
        unsafe {
            (*GifFile).Error = Error;
        }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn require_decoder(GifFile: *mut GifFileType) -> *mut DecoderState {
    unsafe { decoder_state(GifFile) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn require_readable(GifFile: *mut GifFileType) -> *mut DecoderState {
    let state = unsafe { require_decoder(GifFile) };
    if state.is_null() || unsafe { (*state).file_state & FILE_STATE_READ } == 0 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            set_error(GifFile, D_GIF_ERR_NOT_READABLE);
        }
        return ptr::null_mut();
    }

    state
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn free_gif_contents(GifFile: *mut GifFileType) {
    if GifFile.is_null() {
        return;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        if !(*GifFile).Image.ColorMap.is_null() {
            GifFreeMapObject((*GifFile).Image.ColorMap);
            (*GifFile).Image.ColorMap = ptr::null_mut();
        }

        if !(*GifFile).SColorMap.is_null() {
            GifFreeMapObject((*GifFile).SColorMap);
            (*GifFile).SColorMap = ptr::null_mut();
        }

        if !(*GifFile).SavedImages.is_null() {
            GifFreeSavedImages(GifFile);
            (*GifFile).SavedImages = ptr::null_mut();
        }

        GifFreeExtensions(
            &mut (*GifFile).ExtensionBlockCount,
            &mut (*GifFile).ExtensionBlocks,
        );
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn destroy_decoder(GifFile: *mut GifFileType) {
    if GifFile.is_null() {
        return;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { decoder_state(GifFile) };
    let file = if state.is_null() {
        ptr::null_mut()
    } else {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*state).file }
    };
    let file_handle = if state.is_null() {
        -1
    } else {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*state).file_handle }
    };
    let use_callback = if state.is_null() {
        false
    } else {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*state).read_func.is_some() }
    };

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        free_gif_contents(GifFile);
    }

    if !file.is_null() {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let _ = unsafe { fclose_input(file) };
    } else if !use_callback && file_handle >= 0 {
        unsafe {
            close_fd(file_handle);
        }
    }

    if !state.is_null() {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            free_decoder_state(state);
        }
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        free_gif_file(GifFile);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn open_common_impl(Error: *mut i32) -> *mut GifFileType {
    let gif_file = unsafe { alloc_gif_file() };
    if gif_file.is_null() {
        if !Error.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                *Error = D_GIF_ERR_NOT_ENOUGH_MEM;
            }
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { alloc_decoder_state() };
    if state.is_null() {
        unsafe {
            free_gif_file(gif_file);
        }
        if !Error.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                *Error = D_GIF_ERR_NOT_ENOUGH_MEM;
            }
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*gif_file).Private = state.cast::<c_void>();
        (*gif_file).SavedImages = ptr::null_mut();
        (*gif_file).SColorMap = ptr::null_mut();
        (*gif_file).UserData = ptr::null_mut();
        (*gif_file).Error = 0;
    }

    gif_file
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn finish_open_impl(
    GifFile: *mut GifFileType,
    Error: *mut i32,
    screen_desc_error: Option<i32>,
) -> *mut GifFileType {
    let mut buf = [0u8; GIF_STAMP_LEN + 1];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, buf.as_mut_ptr(), GIF_STAMP_LEN) } != GIF_STAMP_LEN {
        if !Error.is_null() {
            unsafe {
                *Error = D_GIF_ERR_READ_FAILED;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            destroy_decoder(GifFile);
        }
        return ptr::null_mut();
    }

    if buf[..GIF_VERSION_POS] != GIF_PREFIX[..] {
        if !Error.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                *Error = D_GIF_ERR_NOT_GIF_FILE;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            destroy_decoder(GifFile);
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { get_screen_desc_impl(GifFile) } == GIF_ERROR {
        if let Some(error) = screen_desc_error {
            if !Error.is_null() {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    *Error = error;
                }
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            destroy_decoder(GifFile);
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_decoder(GifFile) };
    if !state.is_null() {
        unsafe {
            (*state).gif89 = buf[GIF_VERSION_POS + 1] == b'9';
        }
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GifFile).Error = 0;
    }

    GifFile
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn get_word_impl(GifFile: *mut GifFileType, Word: *mut i32) -> i32 {
    if Word.is_null() {
        return GIF_ERROR;
    }

    let mut bytes = [0u8; 2];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, bytes.as_mut_ptr(), bytes.len()) } != bytes.len() {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        *Word = i32::from(bytes[0]) | (i32::from(bytes[1]) << 8);
    }
    GIF_OK
}

fn checked_image_pixel_count_u64(width: i32, height: i32) -> Option<u64> {
    let width = u64::try_from(width).ok()?;
    let height = u64::try_from(height).ok()?;
    width.checked_mul(height)
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn setup_decompress_impl(GifFile: *mut GifFileType) -> i32 {
    let state = unsafe { require_decoder(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    let mut code_size = 0u8;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, &mut code_size, 1) } != 1 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    let bits_per_pixel = i32::from(code_size);
    if bits_per_pixel > 8 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).buf[0] = 0;
        (*state).bits_per_pixel = bits_per_pixel;
        (*state).clear_code = 1 << bits_per_pixel;
        (*state).eof_code = (*state).clear_code + 1;
        (*state).running_code = (*state).eof_code + 1;
        (*state).running_bits = bits_per_pixel + 1;
        (*state).max_code1 = 1 << (*state).running_bits;
        (*state).stack_ptr = 0;
        (*state).last_code = NO_SUCH_CODE;
        (*state).current_shift_state = 0;
        (*state).current_shift_dword = 0;
        for prefix in &mut (*state).prefix {
            *prefix = NO_SUCH_CODE as u32;
        }
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn get_prefix_char_impl(Prefix: *const GifPrefixType, mut Code: i32, ClearCode: i32) -> i32 {
    let mut i = 0;

    while Code > ClearCode && i <= LZ_MAX_CODE {
        if Code > LZ_MAX_CODE {
            return NO_SUCH_CODE;
        }
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        Code = unsafe { *Prefix.add(Code as usize) as i32 };
        i += 1;
    }

    Code
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn buffered_input_impl(
    GifFile: *mut GifFileType,
    Buf: *mut GifByteType,
    NextByte: *mut GifByteType,
) -> i32 {
    if Buf.is_null() || NextByte.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        if *Buf == 0 {
            if internal_read(GifFile, Buf, 1) != 1 {
                set_error(GifFile, D_GIF_ERR_READ_FAILED);
                return GIF_ERROR;
            }
            if *Buf == 0 {
                set_error(GifFile, D_GIF_ERR_IMAGE_DEFECT);
                return GIF_ERROR;
            }
            if internal_read(GifFile, Buf.add(1), usize::from(*Buf)) != usize::from(*Buf) {
                set_error(GifFile, D_GIF_ERR_READ_FAILED);
                return GIF_ERROR;
            }
            *NextByte = *Buf.add(1);
            *Buf.add(1) = 2;
            *Buf = (*Buf).wrapping_sub(1);
        } else {
            *NextByte = *Buf.add((*Buf.add(1)) as usize);
            *Buf.add(1) = (*Buf.add(1)).wrapping_add(1);
            *Buf = (*Buf).wrapping_sub(1);
        }
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn decompress_input_impl(GifFile: *mut GifFileType, Code: *mut i32) -> i32 {
    let state = unsafe { require_decoder(GifFile) };
    if state.is_null() || Code.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).running_bits } > LZ_BITS {
        unsafe {
            set_error(GifFile, D_GIF_ERR_IMAGE_DEFECT);
        }
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    while unsafe { (*state).current_shift_state } < unsafe { (*state).running_bits } {
        let mut next_byte = 0u8;
        if unsafe { buffered_input_impl(GifFile, (*state).buf.as_mut_ptr(), &mut next_byte) }
            == GIF_ERROR
        {
            return GIF_ERROR;
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*state).current_shift_dword |=
                u64::from(next_byte) << ((*state).current_shift_state as u32);
            (*state).current_shift_state += 8;
        }
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let running_bits = unsafe { (*state).running_bits as usize };
    unsafe {
        *Code = ((*state).current_shift_dword & u64::from(CODE_MASKS[running_bits])) as i32;
        (*state).current_shift_dword >>= (*state).running_bits as u32;
        (*state).current_shift_state -= (*state).running_bits;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).running_code } < LZ_MAX_CODE + 2 {
        unsafe {
            (*state).running_code += 1;
            if (*state).running_code > (*state).max_code1 && (*state).running_bits < LZ_BITS {
                (*state).max_code1 <<= 1;
                (*state).running_bits += 1;
            }
        }
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn decompress_line_impl(
    GifFile: *mut GifFileType,
    Line: *mut GifPixelType,
    LineLen: i32,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_decoder(GifFile) };
    if state.is_null() || Line.is_null() || LineLen < 0 {
        return GIF_ERROR;
    }

    let mut i = 0;
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let mut stack_ptr = unsafe { (*state).stack_ptr };
    let mut last_code = unsafe { (*state).last_code };
    let eof_code = unsafe { (*state).eof_code };
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let clear_code = unsafe { (*state).clear_code };

    if stack_ptr > LZ_MAX_CODE {
        return GIF_ERROR;
    }

    if stack_ptr != 0 {
        while stack_ptr != 0 && i < LineLen {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                stack_ptr -= 1;
                *Line.add(i as usize) = (*state).stack[stack_ptr as usize];
            }
            i += 1;
        }
    }

    while i < LineLen {
        let mut current_code = 0;
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe { decompress_input_impl(GifFile, &mut current_code) } == GIF_ERROR {
            return GIF_ERROR;
        }

        if current_code == eof_code {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_EOF_TOO_SOON);
            }
            return GIF_ERROR;
        } else if current_code == clear_code {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                for prefix in &mut (*state).prefix {
                    *prefix = NO_SUCH_CODE as u32;
                }
                (*state).running_code = (*state).eof_code + 1;
                (*state).running_bits = (*state).bits_per_pixel + 1;
                (*state).max_code1 = 1 << (*state).running_bits;
                (*state).last_code = NO_SUCH_CODE;
            }
            last_code = NO_SUCH_CODE;
        } else {
            if current_code < clear_code {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    *Line.add(i as usize) = current_code as u8;
                }
                i += 1;
            } else {
                let mut current_prefix;

                // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
                if unsafe { (*state).prefix[current_code as usize] } == NO_SUCH_CODE as u32 {
                    current_prefix = last_code;

                    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
                    let suffix_value = if current_code == unsafe { (*state).running_code } - 2 {
                        unsafe {
                            get_prefix_char_impl((*state).prefix.as_ptr(), last_code, clear_code)
                        }
                    } else {
                        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                        unsafe {
                            get_prefix_char_impl((*state).prefix.as_ptr(), current_code, clear_code)
                        }
                    };

                    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                    unsafe {
                        (*state).suffix[((*state).running_code - 2) as usize] = suffix_value as u8;
                        (*state).stack[stack_ptr as usize] = suffix_value as u8;
                    }
                    stack_ptr += 1;
                } else {
                    current_prefix = current_code;
                }

                while stack_ptr < LZ_MAX_CODE
                    && current_prefix > clear_code
                    && current_prefix <= LZ_MAX_CODE
                {
                    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                    unsafe {
                        (*state).stack[stack_ptr as usize] =
                            (*state).suffix[current_prefix as usize];
                    }
                    stack_ptr += 1;
                    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
                    current_prefix = unsafe { (*state).prefix[current_prefix as usize] as i32 };
                }

                if stack_ptr >= LZ_MAX_CODE || current_prefix > LZ_MAX_CODE {
                    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                    unsafe {
                        set_error(GifFile, D_GIF_ERR_IMAGE_DEFECT);
                    }
                    return GIF_ERROR;
                }

                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    (*state).stack[stack_ptr as usize] = current_prefix as u8;
                }
                stack_ptr += 1;

                while stack_ptr != 0 && i < LineLen {
                    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                    unsafe {
                        stack_ptr -= 1;
                        *Line.add(i as usize) = (*state).stack[stack_ptr as usize];
                    }
                    i += 1;
                }
            }

            // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
            let running_code_minus_two = unsafe { (*state).running_code } - 2;
            if last_code != NO_SUCH_CODE
                && running_code_minus_two < LZ_MAX_CODE + 1
// SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
                && unsafe { (*state).prefix[running_code_minus_two as usize] }
                    == NO_SUCH_CODE as u32
            {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    (*state).prefix[running_code_minus_two as usize] = last_code as u32;
                    (*state).suffix[running_code_minus_two as usize] = if current_code
                        == running_code_minus_two
                    {
                        get_prefix_char_impl((*state).prefix.as_ptr(), last_code, clear_code) as u8
                    } else {
                        get_prefix_char_impl((*state).prefix.as_ptr(), current_code, clear_code)
                            as u8
                    };
                }
            }

            last_code = current_code;
        }
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).last_code = last_code;
        (*state).stack_ptr = stack_ptr;
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_screen_desc_impl(GifFile: *mut GifFileType) -> i32 {
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { get_word_impl(GifFile, &mut (*GifFile).SWidth) } == GIF_ERROR
        || unsafe { get_word_impl(GifFile, &mut (*GifFile).SHeight) } == GIF_ERROR
    {
        return GIF_ERROR;
    }

    let mut buf = [0u8; 3];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, buf.as_mut_ptr(), buf.len()) } != buf.len() {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
            GifFreeMapObject((*GifFile).SColorMap);
            (*GifFile).SColorMap = ptr::null_mut();
        }
        return GIF_ERROR;
    }

    let bits_per_pixel = i32::from((buf[0] & 0x07) + 1);
    let sort_flag = (buf[0] & 0x08) != 0;

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GifFile).SColorResolution = i32::from((((buf[0] & 0x70) + 1) >> 4) + 1);
        (*GifFile).SBackGroundColor = i32::from(buf[1]);
        (*GifFile).AspectByte = buf[2];
    }

    if (buf[0] & 0x80) != 0 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*GifFile).SColorMap = GifMakeMapObject(1 << bits_per_pixel, ptr::null());
            if (*GifFile).SColorMap.is_null() {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                return GIF_ERROR;
            }
            (*(*GifFile).SColorMap).SortFlag.set(sort_flag);
        }

        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let color_count = unsafe { (*(*GifFile).SColorMap).ColorCount };
        for index in 0..usize::try_from(color_count).unwrap_or(0) {
            if unsafe { internal_read(GifFile, buf.as_mut_ptr(), buf.len()) } != buf.len() {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    GifFreeMapObject((*GifFile).SColorMap);
                    (*GifFile).SColorMap = ptr::null_mut();
                    set_error(GifFile, D_GIF_ERR_READ_FAILED);
                }
                return GIF_ERROR;
            }
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*(*(*GifFile).SColorMap).Colors.add(index)).Red = buf[0];
                (*(*(*GifFile).SColorMap).Colors.add(index)).Green = buf[1];
                (*(*(*GifFile).SColorMap).Colors.add(index)).Blue = buf[2];
            }
        }
    } else {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*GifFile).SColorMap = ptr::null_mut();
        }
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetGifVersion(GifFile: *mut GifFileType) -> *const c_char {
    catch_decode_or(GIF87_STAMP.as_ptr().cast(), GifFile, || unsafe {
        let state = require_decoder(GifFile);
        if state.is_null() || !(*state).gif89 {
            GIF87_STAMP.as_ptr().cast()
        } else {
            GIF89_STAMP.as_ptr().cast()
        }
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_record_type_impl(
    GifFile: *mut GifFileType,
    Type: *mut GifRecordType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() || Type.is_null() {
        return GIF_ERROR;
    }

    let mut buf = 0u8;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, &mut buf, 1) } != 1 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        *Type = match buf {
            DESCRIPTOR_INTRODUCER => IMAGE_DESC_RECORD_TYPE,
            EXTENSION_INTRODUCER => EXTENSION_RECORD_TYPE,
            TERMINATOR_INTRODUCER => TERMINATE_RECORD_TYPE,
            _ => {
                set_error(GifFile, D_GIF_ERR_WRONG_RECORD);
                UNDEFINED_RECORD_TYPE
            }
        };
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { *Type } == UNDEFINED_RECORD_TYPE {
        GIF_ERROR
    } else {
        GIF_OK
    }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetRecordType(
    GifFile: *mut GifFileType,
    Type: *mut GifRecordType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_record_type_impl(GifFile, Type)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_image_header_impl(GifFile: *mut GifFileType) -> i32 {
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { get_word_impl(GifFile, &mut (*GifFile).Image.Left) } == GIF_ERROR
        || unsafe { get_word_impl(GifFile, &mut (*GifFile).Image.Top) } == GIF_ERROR
        || unsafe { get_word_impl(GifFile, &mut (*GifFile).Image.Width) } == GIF_ERROR
// SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        || unsafe { get_word_impl(GifFile, &mut (*GifFile).Image.Height) } == GIF_ERROR
    {
        return GIF_ERROR;
    }

    let mut packed = 0u8;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, &mut packed, 1) } != 1 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
            GifFreeMapObject((*GifFile).Image.ColorMap);
            (*GifFile).Image.ColorMap = ptr::null_mut();
        }
        return GIF_ERROR;
    }

    let bits_per_pixel = i32::from((packed & 0x07) + 1);
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GifFile).Image.Interlace.set((packed & 0x40) != 0);
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        if !(*GifFile).Image.ColorMap.is_null() {
            GifFreeMapObject((*GifFile).Image.ColorMap);
            (*GifFile).Image.ColorMap = ptr::null_mut();
        }
    }

    if (packed & 0x80) != 0 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*GifFile).Image.ColorMap = GifMakeMapObject(1 << bits_per_pixel, ptr::null());
            if (*GifFile).Image.ColorMap.is_null() {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                return GIF_ERROR;
            }
        }

        let mut buf = [0u8; 3];
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let color_count = unsafe { (*(*GifFile).Image.ColorMap).ColorCount };
        for index in 0..usize::try_from(color_count).unwrap_or(0) {
            if unsafe { internal_read(GifFile, buf.as_mut_ptr(), buf.len()) } != buf.len() {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    GifFreeMapObject((*GifFile).Image.ColorMap);
                    set_error(GifFile, D_GIF_ERR_READ_FAILED);
                    (*GifFile).Image.ColorMap = ptr::null_mut();
                }
                return GIF_ERROR;
            }
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*(*(*GifFile).Image.ColorMap).Colors.add(index)).Red = buf[0];
                (*(*(*GifFile).Image.ColorMap).Colors.add(index)).Green = buf[1];
                (*(*(*GifFile).Image.ColorMap).Colors.add(index)).Blue = buf[2];
            }
        }
    }

    let pixel_count = match checked_image_pixel_count_u64(
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*GifFile).Image.Width },
        unsafe { (*GifFile).Image.Height },
    ) {
        Some(pixel_count) => pixel_count,
        None => {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_DATA_TOO_BIG);
            }
            return GIF_ERROR;
        }
    };

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).pixel_count = pixel_count;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { setup_decompress_impl(GifFile) }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetImageHeader(GifFile: *mut GifFileType) -> i32 {
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_image_header_impl(GifFile)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_image_desc_impl(GifFile: *mut GifFileType) -> i32 {
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { get_image_header_impl(GifFile) } == GIF_ERROR {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let next_image_count = match unsafe { (*GifFile).ImageCount.checked_add(1) } {
        Some(count) => count,
        None => {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
            }
            return GIF_ERROR;
        }
    };

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        if !(*GifFile).SavedImages.is_null() {
            let new_count = match usize::try_from(next_image_count) {
                Ok(count) => count,
                Err(_) => {
                    set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                    return GIF_ERROR;
                }
            };
            let new_saved_images = realloc_array::<SavedImage>((*GifFile).SavedImages, new_count);
            if new_saved_images.is_null() {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                return GIF_ERROR;
            }
            (*GifFile).SavedImages = new_saved_images;
        } else {
            (*GifFile).SavedImages = alloc_struct::<SavedImage>();
            if (*GifFile).SavedImages.is_null() {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                return GIF_ERROR;
            }
        }

        let image_index = match usize::try_from((*GifFile).ImageCount) {
            Ok(index) => index,
            Err(_) => {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                return GIF_ERROR;
            }
        };
        let saved = (*GifFile).SavedImages.add(image_index);
        ptr::write_bytes(saved, 0, 1);
        (*saved).ImageDesc = (*GifFile).Image;
        (*saved).ImageDesc.ColorMap = ptr::null_mut();

        if !(*GifFile).Image.ColorMap.is_null() {
            (*saved).ImageDesc.ColorMap = GifMakeMapObject(
                (*(*GifFile).Image.ColorMap).ColorCount,
                (*(*GifFile).Image.ColorMap).Colors,
            );
            if (*saved).ImageDesc.ColorMap.is_null() {
                set_error(GifFile, D_GIF_ERR_NOT_ENOUGH_MEM);
                return GIF_ERROR;
            }
        }

        (*saved).RasterBits = ptr::null_mut();
        (*saved).ExtensionBlockCount = 0;
        (*saved).ExtensionBlocks = ptr::null_mut();
        (*GifFile).ImageCount = next_image_count;
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetImageDesc(GifFile: *mut GifFileType) -> i32 {
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_image_desc_impl(GifFile)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_code_next_impl(
    GifFile: *mut GifFileType,
    CodeBlock: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() || CodeBlock.is_null() {
        return GIF_ERROR;
    }

    let mut buf = 0u8;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, &mut buf, 1) } != 1 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    if buf > 0 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            *CodeBlock = (*state).buf.as_mut_ptr();
            **CodeBlock = buf;
        }
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        if unsafe { internal_read(GifFile, (*CodeBlock).add(1), usize::from(buf)) }
            != usize::from(buf)
        {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_READ_FAILED);
            }
            return GIF_ERROR;
        }
    } else {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            *CodeBlock = ptr::null_mut();
            (*state).buf[0] = 0;
            (*state).pixel_count = 0;
        }
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetCodeNext(
    GifFile: *mut GifFileType,
    CodeBlock: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_code_next_impl(GifFile, CodeBlock)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_line_impl(
    GifFile: *mut GifFileType,
    Line: *mut GifPixelType,
    mut LineLen: i32,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    if Line.is_null() {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            set_error(GifFile, D_GIF_ERR_IMAGE_DEFECT);
        }
        return GIF_ERROR;
    }

    if LineLen == 0 {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        LineLen = unsafe { (*GifFile).Image.Width };
    }

    let line_len = match u64::try_from(LineLen) {
        Ok(line_len) => line_len,
        Err(_) => {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_DATA_TOO_BIG);
            }
            return GIF_ERROR;
        }
    };

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).pixel_count = match (*state).pixel_count.checked_sub(line_len) {
            Some(pixel_count) => pixel_count,
            None => {
                set_error(GifFile, D_GIF_ERR_DATA_TOO_BIG);
                return GIF_ERROR;
            }
        };
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { decompress_line_impl(GifFile, Line, LineLen) } == GIF_OK {
        if unsafe { (*state).pixel_count } == 0 {
            let mut dummy = ptr::null_mut();
            loop {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                if unsafe { get_code_next_impl(GifFile, &mut dummy) } == GIF_ERROR {
                    return GIF_ERROR;
                }
                if dummy.is_null() {
                    break;
                }
            }
        }
        GIF_OK
    } else {
        GIF_ERROR
    }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetLine(
    GifFile: *mut GifFileType,
    Line: *mut GifPixelType,
    LineLen: i32,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_line_impl(GifFile, Line, LineLen)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn get_pixel_impl(GifFile: *mut GifFileType, Pixel: GifPixelType) -> i32 {
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).pixel_count = match (*state).pixel_count.checked_sub(1) {
            Some(pixel_count) => pixel_count,
            None => {
                set_error(GifFile, D_GIF_ERR_DATA_TOO_BIG);
                return GIF_ERROR;
            }
        };
    }

    let mut pixel = Pixel;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { decompress_line_impl(GifFile, &mut pixel, 1) } == GIF_OK {
        if unsafe { (*state).pixel_count } == 0 {
            let mut dummy = ptr::null_mut();
            loop {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                if unsafe { get_code_next_impl(GifFile, &mut dummy) } == GIF_ERROR {
                    return GIF_ERROR;
                }
                if dummy.is_null() {
                    break;
                }
            }
        }
        GIF_OK
    } else {
        GIF_ERROR
    }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetPixel(GifFile: *mut GifFileType, Pixel: GifPixelType) -> i32 {
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_pixel_impl(GifFile, Pixel)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_extension_impl(
    GifFile: *mut GifFileType,
    ExtCode: *mut i32,
    Extension: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() || ExtCode.is_null() || Extension.is_null() {
        return GIF_ERROR;
    }

    let mut buf = 0u8;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, &mut buf, 1) } != 1 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        *ExtCode = i32::from(buf);
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { get_extension_next_impl(GifFile, Extension) }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetExtension(
    GifFile: *mut GifFileType,
    ExtCode: *mut i32,
    Extension: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_extension_impl(GifFile, ExtCode, Extension)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
pub(crate) unsafe fn get_extension_next_impl(
    GifFile: *mut GifFileType,
    Extension: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() || Extension.is_null() {
        return GIF_ERROR;
    }

    let mut buf = 0u8;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { internal_read(GifFile, &mut buf, 1) } != 1 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_READ_FAILED);
        }
        return GIF_ERROR;
    }

    if buf > 0 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            *Extension = (*state).buf.as_mut_ptr();
            **Extension = buf;
        }
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        if unsafe { internal_read(GifFile, (*Extension).add(1), usize::from(buf)) }
            != usize::from(buf)
        {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_READ_FAILED);
            }
            return GIF_ERROR;
        }
    } else {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            *Extension = ptr::null_mut();
        }
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetExtensionNext(
    GifFile: *mut GifFileType,
    Extension: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_extension_next_impl(GifFile, Extension)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn get_code_impl(
    GifFile: *mut GifFileType,
    CodeSize: *mut i32,
    CodeBlock: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() || CodeSize.is_null() || CodeBlock.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        *CodeSize = (*state).bits_per_pixel;
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { get_code_next_impl(GifFile, CodeBlock) }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetCode(
    GifFile: *mut GifFileType,
    CodeSize: *mut i32,
    CodeBlock: *mut *mut GifByteType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_code_impl(GifFile, CodeSize, CodeBlock)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn get_lz_codes_impl(GifFile: *mut GifFileType, Code: *mut i32) -> i32 {
    let state = unsafe { require_readable(GifFile) };
    if state.is_null() || Code.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { decompress_input_impl(GifFile, Code) } == GIF_ERROR {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { *Code } == unsafe { (*state).eof_code } {
        let mut code_block = ptr::null_mut();
        loop {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            if unsafe { get_code_next_impl(GifFile, &mut code_block) } == GIF_ERROR {
                return GIF_ERROR;
            }
            if code_block.is_null() {
                break;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            *Code = -1;
        }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    } else if unsafe { *Code } == unsafe { (*state).clear_code } {
        unsafe {
            (*state).running_code = (*state).eof_code + 1;
            (*state).running_bits = (*state).bits_per_pixel + 1;
            (*state).max_code1 = 1 << (*state).running_bits;
        }
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetLZCodes(GifFile: *mut GifFileType, Code: *mut i32) -> i32 {
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_lz_codes_impl(GifFile, Code)
    })
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn close_file_impl(GifFile: *mut GifFileType, ErrorCode: *mut i32) -> i32 {
    if GifFile.is_null() || unsafe { (*GifFile).Private.is_null() } {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_decoder(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        free_gif_contents(GifFile);
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).file_state & FILE_STATE_READ } == 0 {
        if !ErrorCode.is_null() {
            unsafe {
                *ErrorCode = D_GIF_ERR_NOT_READABLE;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            free_decoder_state(state);
            free_gif_file(GifFile);
        }
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { !(*state).file.is_null() && fclose_input((*state).file) != 0 } {
        if !ErrorCode.is_null() {
            unsafe {
                *ErrorCode = D_GIF_ERR_CLOSE_FAILED;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            free_decoder_state(state);
            free_gif_file(GifFile);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        free_decoder_state(state);
        free_gif_file(GifFile);
    }
    if !ErrorCode.is_null() {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            *ErrorCode = 0;
        }
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifOpenFileName(
    FileName: *const c_char,
    Error: *mut i32,
) -> *mut GifFileType {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_open_or(ptr::null_mut(), Error, || unsafe {
        let file_handle = open_input_file(FileName);
        if file_handle == -1 {
            if !Error.is_null() {
                *Error = D_GIF_ERR_OPEN_FAILED;
            }
            return ptr::null_mut();
        }

        DGifOpenFileHandle(file_handle, Error)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifOpenFileHandle(FileHandle: i32, Error: *mut i32) -> *mut GifFileType {
    catch_decode_open_or(ptr::null_mut(), Error, || unsafe {
        let gif_file = open_common_impl(Error);
        if gif_file.is_null() {
            close_fd(FileHandle);
            return ptr::null_mut();
        }

        let state = require_decoder(gif_file);
        if state.is_null() {
            destroy_decoder(gif_file);
            close_fd(FileHandle);
            return ptr::null_mut();
        }

        (*state).file_handle = FileHandle;
        (*state).file = fdopen_read(FileHandle);
        (*state).file_state = FILE_STATE_READ;
        (*state).read_func = None;
        (*state).gif89 = false;
        (*gif_file).UserData = ptr::null_mut();

        if (*state).file.is_null() {
            if !Error.is_null() {
                *Error = D_GIF_ERR_OPEN_FAILED;
            }
            destroy_decoder(gif_file);
            return ptr::null_mut();
        }

        finish_open_impl(gif_file, Error, None)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifOpen(
    userData: *mut c_void,
    readFunc: crate::ffi::InputFunc,
    Error: *mut i32,
) -> *mut GifFileType {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_decode_open_or(ptr::null_mut(), Error, || unsafe {
        let gif_file = open_common_impl(Error);
        if gif_file.is_null() {
            return ptr::null_mut();
        }

        let state = require_decoder(gif_file);
        if state.is_null() {
            destroy_decoder(gif_file);
            return ptr::null_mut();
        }

        (*state).file_handle = -1;
        (*state).file = ptr::null_mut();
        (*state).file_state = FILE_STATE_READ;
        (*state).read_func = readFunc;
        (*state).gif89 = false;
        (*gif_file).UserData = userData;

        finish_open_impl(gif_file, Error, Some(D_GIF_ERR_NO_SCRN_DSCR))
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifGetScreenDesc(GifFile: *mut GifFileType) -> i32 {
    catch_decode_or(GIF_ERROR, GifFile, || unsafe {
        get_screen_desc_impl(GifFile)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifCloseFile(GifFile: *mut GifFileType, ErrorCode: *mut i32) -> i32 {
    catch_decode_close_or(GIF_ERROR, GifFile, ErrorCode, || unsafe {
        close_file_impl(GifFile, ErrorCode)
    })
}
