#![allow(non_snake_case)]

use core::ffi::{c_char, c_void};
use core::ptr;
use core::slice;

use std::ffi::CStr;

use crate::bootstrap::{catch_error_or, catch_gif_and_error_or, catch_gif_error_or};
use crate::ffi::{
    ColorMapObject, ExtensionBlock, GifFileType, GifPixelType, OutputFunc, SavedImage,
    APPLICATION_EXT_FUNC_CODE, COMMENT_EXT_FUNC_CODE, CONTINUE_EXT_FUNC_CODE,
    E_GIF_ERR_CLOSE_FAILED, E_GIF_ERR_DATA_TOO_BIG, E_GIF_ERR_DISK_IS_FULL,
    E_GIF_ERR_HAS_IMAG_DSCR, E_GIF_ERR_HAS_SCRN_DSCR, E_GIF_ERR_NOT_ENOUGH_MEM,
    E_GIF_ERR_NOT_WRITEABLE, E_GIF_ERR_NO_COLOR_MAP, E_GIF_ERR_OPEN_FAILED, E_GIF_ERR_WRITE_FAILED,
    GIF_ERROR, GIF_OK, GRAPHICS_EXT_FUNC_CODE, PLAINTEXT_EXT_FUNC_CODE,
};
use crate::hash::{_ClearHashTable, _ExistsHashTable, _InitHashTable, _InsertHashTable};
use crate::helpers::{GifFreeMapObject, GifMakeMapObject};
use crate::io::{
    close_fd, fclose_output, fdopen_write, internal_write, open_output_file, write_exact,
};
use crate::memory::c_free;
use crate::state::{
    alloc_encoder_state, alloc_gif_file, encoder_state, free_encoder_state, free_gif_file,
    EncoderState, DESCRIPTOR_INTRODUCER, EXTENSION_INTRODUCER, FILE_STATE_IMAGE, FILE_STATE_SCREEN,
    FILE_STATE_WRITE, FIRST_CODE, FLUSH_OUTPUT, LZ_MAX_CODE, TERMINATOR_INTRODUCER,
};

const GIF87_STAMP: &[u8; 7] = b"GIF87a\0";
const GIF89_STAMP: &[u8; 7] = b"GIF89a\0";
const GIF87_WRITE_STAMP: &[u8; 6] = b"GIF87a";
const GIF89_WRITE_STAMP: &[u8; 6] = b"GIF89a";

const CODE_MASK: [GifPixelType; 9] = [0x00, 0x01, 0x03, 0x07, 0x0f, 0x1f, 0x3f, 0x7f, 0xff];
const INTERLACED_OFFSET: [i32; 4] = [0, 4, 2, 1];
const INTERLACED_JUMPS: [i32; 4] = [8, 8, 4, 2];

fn catch_encode_or<T>(fallback: T, GifFile: *mut GifFileType, f: impl FnOnce() -> T) -> T {
    catch_gif_error_or(fallback, GifFile, E_GIF_ERR_NOT_ENOUGH_MEM, f)
}

fn catch_encode_open_or<T>(fallback: T, Error: *mut i32, f: impl FnOnce() -> T) -> T {
    catch_error_or(fallback, Error, E_GIF_ERR_NOT_ENOUGH_MEM, f)
}

fn catch_encode_close_or<T>(
    fallback: T,
    GifFile: *mut GifFileType,
    ErrorCode: *mut i32,
    f: impl FnOnce() -> T,
) -> T {
    catch_gif_and_error_or(fallback, GifFile, ErrorCode, E_GIF_ERR_CLOSE_FAILED, f)
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn set_error(GifFile: *mut GifFileType, Error: i32) {
    if !GifFile.is_null() {
        unsafe {
            (*GifFile).Error = Error;
        }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn require_encoder(GifFile: *mut GifFileType) -> *mut EncoderState {
    unsafe { encoder_state(GifFile) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn require_writeable(GifFile: *mut GifFileType) -> *mut EncoderState {
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() || unsafe { (*state).file_state & FILE_STATE_WRITE } == 0 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            set_error(GifFile, E_GIF_ERR_NOT_WRITEABLE);
        }
        return ptr::null_mut();
    }

    state
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn clone_color_map(ColorMap: *const ColorMapObject) -> *mut ColorMapObject {
    if ColorMap.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    unsafe { GifMakeMapObject((*ColorMap).ColorCount, (*ColorMap).Colors) }
}

fn requires_gif89(Function: i32) -> bool {
    matches!(
        Function,
        COMMENT_EXT_FUNC_CODE
            | GRAPHICS_EXT_FUNC_CODE
            | PLAINTEXT_EXT_FUNC_CODE
            | APPLICATION_EXT_FUNC_CODE
    )
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn gif_version_bytes(GifFile: *mut GifFileType) -> &'static [u8; 6] {
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() {
        return GIF87_WRITE_STAMP;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let image_count = if GifFile.is_null() || unsafe { (*GifFile).ImageCount } <= 0 {
        0
    } else {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        usize::try_from(unsafe { (*GifFile).ImageCount }).unwrap_or(0)
    };

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if !GifFile.is_null() && !unsafe { (*GifFile).SavedImages }.is_null() {
        for image_index in 0..image_count {
            let saved = unsafe { &*(*GifFile).SavedImages.add(image_index) };
            if saved.ExtensionBlocks.is_null() {
                continue;
            }
            let extension_count = usize::try_from(saved.ExtensionBlockCount).unwrap_or(0);
            for extension_index in 0..extension_count {
                // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
                let function = unsafe { (*saved.ExtensionBlocks.add(extension_index)).Function };
                if requires_gif89(function) {
                    unsafe {
                        (*state).gif89 = true;
                    }
                }
            }
        }
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if !GifFile.is_null() && !unsafe { (*GifFile).ExtensionBlocks }.is_null() {
        let extension_count =
            usize::try_from(unsafe { (*GifFile).ExtensionBlockCount }).unwrap_or(0);
        for extension_index in 0..extension_count {
            // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
            let function = unsafe { (*(*GifFile).ExtensionBlocks.add(extension_index)).Function };
            if requires_gif89(function) {
                unsafe {
                    (*state).gif89 = true;
                }
            }
        }
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).gif89 } {
        GIF89_WRITE_STAMP
    } else {
        GIF87_WRITE_STAMP
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_word_impl(Word: i32, GifFile: *mut GifFileType) -> i32 {
    let bytes = [(Word & 0xff) as u8, ((Word >> 8) & 0xff) as u8];
    if unsafe { write_exact(GifFile, bytes.as_ptr(), bytes.len()) } {
        GIF_OK
    } else {
        GIF_ERROR
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn buffered_output_impl(Buf: &mut [u8; 256], GifFile: *mut GifFileType, c: i32) -> i32 {
    if c == FLUSH_OUTPUT {
        if Buf[0] != 0
// SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            && unsafe { internal_write(GifFile, Buf.as_ptr(), usize::from(Buf[0]) + 1) }
                != usize::from(Buf[0]) + 1
        {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
            }
            return GIF_ERROR;
        }

        Buf[0] = 0;
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe { internal_write(GifFile, Buf.as_ptr(), 1) } != 1 {
            unsafe {
                set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
            }
            return GIF_ERROR;
        }
    } else {
        if Buf[0] == 255 {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            if unsafe { internal_write(GifFile, Buf.as_ptr(), usize::from(Buf[0]) + 1) }
                != usize::from(Buf[0]) + 1
            {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
                }
                return GIF_ERROR;
            }
            Buf[0] = 0;
        }

        Buf[0] = Buf[0].wrapping_add(1);
        Buf[usize::from(Buf[0])] = c as u8;
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn compress_output_impl(GifFile: *mut GifFileType, Code: i32) -> i32 {
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    let mut retval = GIF_OK;
    if Code == FLUSH_OUTPUT {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        while unsafe { (*state).current_shift_state } > 0 {
            if unsafe {
                buffered_output_impl(
                    &mut (*state).output_buffer,
                    GifFile,
                    ((*state).current_shift_dword & 0xff) as i32,
                )
            } == GIF_ERROR
            {
                retval = GIF_ERROR;
            }
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*state).current_shift_dword >>= 8;
                (*state).current_shift_state -= 8;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*state).current_shift_state = 0;
        }
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        if unsafe { buffered_output_impl(&mut (*state).output_buffer, GifFile, FLUSH_OUTPUT) }
            == GIF_ERROR
        {
            retval = GIF_ERROR;
        }
    } else {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*state).current_shift_dword |= (Code as u64) << (*state).current_shift_state;
            (*state).current_shift_state += (*state).running_bits;
        }
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        while unsafe { (*state).current_shift_state } >= 8 {
            if unsafe {
                buffered_output_impl(
                    &mut (*state).output_buffer,
                    GifFile,
                    ((*state).current_shift_dword & 0xff) as i32,
                )
            } == GIF_ERROR
            {
                retval = GIF_ERROR;
            }
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*state).current_shift_dword >>= 8;
                (*state).current_shift_state -= 8;
            }
        }
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).running_code >= (*state).max_code1 } && Code <= 4095 {
        unsafe {
            (*state).running_bits += 1;
            (*state).max_code1 = 1 << (*state).running_bits;
        }
    }

    retval
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn setup_compress_impl(GifFile: *mut GifFileType) -> i32 {
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let bits_per_pixel = if unsafe { !(*GifFile).Image.ColorMap.is_null() } {
        unsafe { (*(*GifFile).Image.ColorMap).BitsPerPixel }
    } else if unsafe { !(*GifFile).SColorMap.is_null() } {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*(*GifFile).SColorMap).BitsPerPixel }
    } else {
        unsafe {
            set_error(GifFile, E_GIF_ERR_NO_COLOR_MAP);
        }
        return GIF_ERROR;
    };

    let bits_per_pixel = bits_per_pixel.max(2);
    let code_size = [bits_per_pixel as u8];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { internal_write(GifFile, code_size.as_ptr(), 1) };

    unsafe {
        (*state).output_buffer[0] = 0;
        (*state).bits_per_pixel = bits_per_pixel;
        (*state).clear_code = 1 << bits_per_pixel;
        (*state).eof_code = (*state).clear_code + 1;
        (*state).running_code = (*state).eof_code + 1;
        (*state).running_bits = bits_per_pixel + 1;
        (*state).max_code1 = 1 << (*state).running_bits;
        (*state).current_code = FIRST_CODE;
        (*state).current_shift_state = 0;
        (*state).current_shift_dword = 0;
        _ClearHashTable((*state).hash_table);
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { compress_output_impl(GifFile, (*state).clear_code) } == GIF_ERROR {
        unsafe {
            set_error(GifFile, E_GIF_ERR_DISK_IS_FULL);
        }
        return GIF_ERROR;
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn compress_line_impl(
    GifFile: *mut GifFileType,
    Line: *const GifPixelType,
    LineLen: i32,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() || Line.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let hash_table = unsafe { (*state).hash_table };
    let mut i = 0;
    let mut current_code = if unsafe { (*state).current_code == FIRST_CODE } {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let pixel = unsafe { *Line };
        i = 1;
        i32::from(pixel)
    } else {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*state).current_code }
    };

    while i < LineLen {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let pixel = unsafe { *Line.add(i as usize) };
        i += 1;
        let new_key = ((current_code as u32) << 8) + u32::from(pixel);
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let new_code = unsafe { _ExistsHashTable(hash_table, new_key) };
        if new_code >= 0 {
            current_code = new_code;
        } else {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            if unsafe { compress_output_impl(GifFile, current_code) } == GIF_ERROR {
                unsafe {
                    set_error(GifFile, E_GIF_ERR_DISK_IS_FULL);
                }
                return GIF_ERROR;
            }
            current_code = i32::from(pixel);

            // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
            if unsafe { (*state).running_code } >= LZ_MAX_CODE {
                if unsafe { compress_output_impl(GifFile, (*state).clear_code) } == GIF_ERROR {
                    unsafe {
                        set_error(GifFile, E_GIF_ERR_DISK_IS_FULL);
                    }
                    return GIF_ERROR;
                }
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    (*state).running_code = (*state).eof_code + 1;
                    (*state).running_bits = (*state).bits_per_pixel + 1;
                    (*state).max_code1 = 1 << (*state).running_bits;
                    _ClearHashTable(hash_table);
                }
            } else {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    _InsertHashTable(hash_table, new_key, (*state).running_code);
                    (*state).running_code += 1;
                }
            }
        }
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).current_code = current_code;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).pixel_count } == 0 {
        if unsafe { compress_output_impl(GifFile, current_code) } == GIF_ERROR {
            unsafe {
                set_error(GifFile, E_GIF_ERR_DISK_IS_FULL);
            }
            return GIF_ERROR;
        }
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        if unsafe { compress_output_impl(GifFile, (*state).eof_code) } == GIF_ERROR {
            unsafe {
                set_error(GifFile, E_GIF_ERR_DISK_IS_FULL);
            }
            return GIF_ERROR;
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe { compress_output_impl(GifFile, FLUSH_OUTPUT) } == GIF_ERROR {
            unsafe {
                set_error(GifFile, E_GIF_ERR_DISK_IS_FULL);
            }
            return GIF_ERROR;
        }
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn write_extensions_impl(
    GifFileOut: *mut GifFileType,
    ExtensionBlocks: *mut ExtensionBlock,
    ExtensionBlockCount: i32,
) -> i32 {
    if ExtensionBlocks.is_null() {
        return GIF_OK;
    }

    let extension_count = usize::try_from(ExtensionBlockCount).unwrap_or(0);
    for index in 0..extension_count {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let extension = unsafe { &*ExtensionBlocks.add(index) };
        if extension.Function != CONTINUE_EXT_FUNC_CODE
            && unsafe { put_extension_leader_impl(GifFileOut, extension.Function) } == GIF_ERROR
        {
            return GIF_ERROR;
        }

        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe {
            put_extension_block_impl(GifFileOut, extension.ByteCount, extension.Bytes.cast())
        } == GIF_ERROR
        {
            return GIF_ERROR;
        }

        if (index + 1 == extension_count
// SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
            || unsafe { (*ExtensionBlocks.add(index + 1)).Function } != CONTINUE_EXT_FUNC_CODE)
            && unsafe { put_extension_trailer_impl(GifFileOut) } == GIF_ERROR
        {
            return GIF_ERROR;
        }
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn open_common_impl(Error: *mut i32, set_gif_alloc_error: bool) -> *mut GifFileType {
    let gif_file = unsafe { alloc_gif_file() };
    if gif_file.is_null() {
        if set_gif_alloc_error && !Error.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                *Error = E_GIF_ERR_NOT_ENOUGH_MEM;
            }
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { alloc_encoder_state() };
    if state.is_null() {
        unsafe {
            free_gif_file(gif_file);
        }
        if !Error.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                *Error = E_GIF_ERR_NOT_ENOUGH_MEM;
            }
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).hash_table = _InitHashTable();
    }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).hash_table.is_null() } {
        unsafe {
            free_encoder_state(state);
            free_gif_file(gif_file);
        }
        if !Error.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                *Error = E_GIF_ERR_NOT_ENOUGH_MEM;
            }
        }
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*gif_file).Private = state.cast::<c_void>();
        (*gif_file).Error = 0;
    }

    gif_file
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_screen_desc_impl(
    GifFile: *mut GifFileType,
    Width: i32,
    Height: i32,
    ColorRes: i32,
    BackGround: i32,
    ColorMap: *const ColorMapObject,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() {
        unsafe {
            set_error(GifFile, E_GIF_ERR_NOT_WRITEABLE);
        }
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).file_state & FILE_STATE_SCREEN } != 0 {
        unsafe {
            set_error(GifFile, E_GIF_ERR_HAS_SCRN_DSCR);
        }
        return GIF_ERROR;
    }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).file_state & FILE_STATE_WRITE } == 0 {
        unsafe {
            set_error(GifFile, E_GIF_ERR_NOT_WRITEABLE);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let version = unsafe { gif_version_bytes(GifFile) };
    if unsafe { internal_write(GifFile, version.as_ptr(), version.len()) } != version.len() {
        unsafe {
            set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GifFile).SColorMap = ptr::null_mut();
        (*GifFile).SWidth = Width;
        (*GifFile).SHeight = Height;
        (*GifFile).SColorResolution = ColorRes;
        (*GifFile).SBackGroundColor = BackGround;
    }

    if !ColorMap.is_null() {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let cloned = unsafe { clone_color_map(ColorMap) };
        if cloned.is_null() {
            unsafe {
                set_error(GifFile, E_GIF_ERR_NOT_ENOUGH_MEM);
            }
            return GIF_ERROR;
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*GifFile).SColorMap = cloned;
        }
    }

    let mut buf = [0u8; 3];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { put_word_impl(Width, GifFile) };
    let _ = unsafe { put_word_impl(Height, GifFile) };

    buf[0] = (if ColorMap.is_null() { 0x00 } else { 0x80 })
        | (((ColorRes - 1) << 4) as u8)
        | if ColorMap.is_null() {
            0x07
        } else {
            // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
            unsafe { ((*ColorMap).BitsPerPixel - 1) as u8 }
        };
    if !ColorMap.is_null() && unsafe { (*ColorMap).SortFlag.get() } {
        buf[0] |= 0x08;
    }
    buf[1] = BackGround as u8;
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    buf[2] = unsafe { (*GifFile).AspectByte };
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), buf.len()) };

    if !ColorMap.is_null() {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let color_count = usize::try_from(unsafe { (*ColorMap).ColorCount }).unwrap_or(0);
        for index in 0..color_count {
            let color = unsafe { *(*ColorMap).Colors.add(index) };
            buf[0] = color.Red;
            buf[1] = color.Green;
            buf[2] = color.Blue;
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            if unsafe { internal_write(GifFile, buf.as_ptr(), buf.len()) } != buf.len() {
                unsafe {
                    set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
                }
                return GIF_ERROR;
            }
        }
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).file_state |= FILE_STATE_SCREEN;
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_image_desc_impl(
    GifFile: *mut GifFileType,
    Left: i32,
    Top: i32,
    Width: i32,
    Height: i32,
    Interlace: bool,
    ColorMap: *const ColorMapObject,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_writeable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).file_state & FILE_STATE_IMAGE } != 0
        && unsafe { (*state).pixel_count } > 0xffff0000
    {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            set_error(GifFile, E_GIF_ERR_HAS_IMAG_DSCR);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GifFile).Image.Left = Left;
        (*GifFile).Image.Top = Top;
        (*GifFile).Image.Width = Width;
        (*GifFile).Image.Height = Height;
        (*GifFile).Image.Interlace.set(Interlace);
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*GifFile).Image.ColorMap } != ColorMap.cast_mut() {
        if !ColorMap.is_null() {
            if unsafe { !(*GifFile).Image.ColorMap.is_null() } {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                unsafe {
                    GifFreeMapObject((*GifFile).Image.ColorMap);
                    (*GifFile).Image.ColorMap = ptr::null_mut();
                }
            }

            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            let cloned = unsafe { clone_color_map(ColorMap) };
            if cloned.is_null() {
                unsafe {
                    set_error(GifFile, E_GIF_ERR_NOT_ENOUGH_MEM);
                }
                return GIF_ERROR;
            }
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*GifFile).Image.ColorMap = cloned;
            }
        } else {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*GifFile).Image.ColorMap = ptr::null_mut();
            }
        }
    }

    let mut buf = [0u8; 3];
    buf[0] = DESCRIPTOR_INTRODUCER;
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), 1) };
    let _ = unsafe { put_word_impl(Left, GifFile) };
    let _ = unsafe { put_word_impl(Top, GifFile) };
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { put_word_impl(Width, GifFile) };
    let _ = unsafe { put_word_impl(Height, GifFile) };
    buf[0] = (if ColorMap.is_null() { 0x00 } else { 0x80 })
        | (if Interlace { 0x40 } else { 0x00 })
        | if ColorMap.is_null() {
            0
        } else {
            // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
            unsafe { ((*ColorMap).BitsPerPixel - 1) as u8 }
        };
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), 1) };

    if !ColorMap.is_null() {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let color_count = usize::try_from(unsafe { (*ColorMap).ColorCount }).unwrap_or(0);
        for index in 0..color_count {
            let color = unsafe { *(*ColorMap).Colors.add(index) };
            buf[0] = color.Red;
            buf[1] = color.Green;
            buf[2] = color.Blue;
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            if unsafe { internal_write(GifFile, buf.as_ptr(), buf.len()) } != buf.len() {
                unsafe {
                    set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
                }
                return GIF_ERROR;
            }
        }
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*GifFile).SColorMap.is_null() && (*GifFile).Image.ColorMap.is_null() } {
        unsafe {
            set_error(GifFile, E_GIF_ERR_NO_COLOR_MAP);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).file_state |= FILE_STATE_IMAGE;
        (*state).pixel_count = ((Width as i64) * (Height as i64)) as u64;
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { setup_compress_impl(GifFile) };

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_line_impl(GifFile: *mut GifFileType, Line: *mut GifPixelType, LineLen: i32) -> i32 {
    let state = unsafe { require_writeable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    let line_len = if LineLen == 0 {
        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        unsafe { (*GifFile).Image.Width }
    } else {
        LineLen
    };
    let line_len_unsigned = line_len as u32 as u64;
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).pixel_count } < line_len_unsigned {
        unsafe {
            set_error(GifFile, E_GIF_ERR_DATA_TOO_BIG);
        }
        return GIF_ERROR;
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).pixel_count -= line_len_unsigned;
    }

    if Line.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let mask = CODE_MASK[usize::try_from(unsafe { (*state).bits_per_pixel }).unwrap_or(0)];
    let line_len = usize::try_from(line_len).unwrap_or(0);
    if mask != 0xff {
        // SAFETY: The caller guarantees the raw buffer is valid for the requested element count.
        let pixels = unsafe { slice::from_raw_parts_mut(Line, line_len) };
        for pixel in pixels {
            *pixel &= mask;
        }
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { compress_line_impl(GifFile, Line, line_len as i32) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_pixel_impl(GifFile: *mut GifFileType, Pixel: GifPixelType) -> i32 {
    let state = unsafe { require_writeable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).pixel_count } == 0 {
        unsafe {
            set_error(GifFile, E_GIF_ERR_DATA_TOO_BIG);
        }
        return GIF_ERROR;
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*state).pixel_count -= 1;
    }

    let mut pixel =
// SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        Pixel & CODE_MASK[usize::try_from(unsafe { (*state).bits_per_pixel }).unwrap_or(0)];
    unsafe { compress_line_impl(GifFile, &mut pixel, 1) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_extension_leader_impl(GifFile: *mut GifFileType, ExtCode: i32) -> i32 {
    if unsafe { require_writeable(GifFile) }.is_null() {
        return GIF_ERROR;
    }

    let buf = [EXTENSION_INTRODUCER, ExtCode as u8];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), buf.len()) };
    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_extension_block_impl(
    GifFile: *mut GifFileType,
    ExtLen: i32,
    Extension: *const c_void,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { require_writeable(GifFile) }.is_null() {
        return GIF_ERROR;
    }

    let len = usize::try_from(ExtLen).unwrap_or(0);
    if len > 0 && Extension.is_null() {
        return GIF_ERROR;
    }

    let buf = [ExtLen as u8];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), 1) };
    let _ = unsafe { internal_write(GifFile, Extension.cast(), len) };
    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_extension_trailer_impl(GifFile: *mut GifFileType) -> i32 {
    if unsafe { require_writeable(GifFile) }.is_null() {
        return GIF_ERROR;
    }

    let buf = [0u8];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), 1) };
    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_extension_impl(
    GifFile: *mut GifFileType,
    ExtCode: i32,
    ExtLen: i32,
    Extension: *const c_void,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { require_writeable(GifFile) }.is_null() {
        return GIF_ERROR;
    }

    let len = usize::try_from(ExtLen).unwrap_or(0);
    if len > 0 && Extension.is_null() {
        return GIF_ERROR;
    }

    if ExtCode == 0 {
        let buf = [ExtLen as u8];
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let _ = unsafe { internal_write(GifFile, buf.as_ptr(), 1) };
    } else {
        let buf = [EXTENSION_INTRODUCER, ExtCode as u8, ExtLen as u8];
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let _ = unsafe { internal_write(GifFile, buf.as_ptr(), buf.len()) };
    }
    let _ = unsafe { internal_write(GifFile, Extension.cast(), len) };
    let trailer = [0u8];
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let _ = unsafe { internal_write(GifFile, trailer.as_ptr(), 1) };
    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_comment_impl(GifFile: *mut GifFileType, Comment: *const c_char) -> i32 {
    if Comment.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: giflib string parameters are required to be valid NUL-terminated C strings.
    let bytes = unsafe { CStr::from_ptr(Comment).to_bytes() };
    if bytes.len() <= 255 {
        return unsafe {
            put_extension_impl(
                GifFile,
                COMMENT_EXT_FUNC_CODE,
                bytes.len() as i32,
                bytes.as_ptr().cast(),
            )
        };
    }

    let mut remaining = bytes.len();
    let mut cursor = bytes.as_ptr();
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe { put_extension_leader_impl(GifFile, COMMENT_EXT_FUNC_CODE) } == GIF_ERROR {
        return GIF_ERROR;
    }

    while remaining > 255 {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe { put_extension_block_impl(GifFile, 255, cursor.cast()) } == GIF_ERROR {
            return GIF_ERROR;
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            cursor = cursor.add(255);
        }
        remaining -= 255;
    }

    if remaining > 0
// SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        && unsafe { put_extension_block_impl(GifFile, remaining as i32, cursor.cast()) }
            == GIF_ERROR
    {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { put_extension_trailer_impl(GifFile) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn put_code_next_impl(GifFile: *mut GifFileType, CodeBlock: *const u8) -> i32 {
    let state = unsafe { require_writeable(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }

    if !CodeBlock.is_null() {
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        let block_len = usize::from(unsafe { *CodeBlock }) + 1;
        if unsafe { internal_write(GifFile, CodeBlock, block_len) } != block_len {
            unsafe {
                set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
            }
            return GIF_ERROR;
        }
    } else {
        let buf = [0u8];
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe { internal_write(GifFile, buf.as_ptr(), 1) } != 1 {
            unsafe {
                set_error(GifFile, E_GIF_ERR_WRITE_FAILED);
            }
            return GIF_ERROR;
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            (*state).pixel_count = 0;
        }
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn close_file_impl(GifFile: *mut GifFileType, ErrorCode: *mut i32) -> i32 {
    if GifFile.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let state = unsafe { require_encoder(GifFile) };
    if state.is_null() {
        return GIF_ERROR;
    }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*state).file_state & FILE_STATE_WRITE } == 0 {
        if !ErrorCode.is_null() {
            unsafe {
                *ErrorCode = E_GIF_ERR_NOT_WRITEABLE;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            free_gif_file(GifFile);
        }
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let file = unsafe { (*state).file };
    let buf = [TERMINATOR_INTRODUCER];
    let _ = unsafe { internal_write(GifFile, buf.as_ptr(), 1) };

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { !(*GifFile).Image.ColorMap.is_null() } {
        unsafe {
            GifFreeMapObject((*GifFile).Image.ColorMap);
            (*GifFile).Image.ColorMap = ptr::null_mut();
        }
    }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { !(*GifFile).SColorMap.is_null() } {
        unsafe {
            GifFreeMapObject((*GifFile).SColorMap);
            (*GifFile).SColorMap = ptr::null_mut();
        }
    }
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { !(*state).hash_table.is_null() } {
        unsafe {
            c_free((*state).hash_table);
            (*state).hash_table = ptr::null_mut();
        }
    }
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        free_encoder_state(state);
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if !file.is_null() && unsafe { fclose_output(file) } != 0 {
        if !ErrorCode.is_null() {
            unsafe {
                *ErrorCode = E_GIF_ERR_CLOSE_FAILED;
            }
        }
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        unsafe {
            free_gif_file(GifFile);
        }
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
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

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn spew_impl(GifFileOut: *mut GifFileType) -> i32 {
    if GifFileOut.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe {
        put_screen_desc_impl(
            GifFileOut,
            (*GifFileOut).SWidth,
            (*GifFileOut).SHeight,
            (*GifFileOut).SColorResolution,
            (*GifFileOut).SBackGroundColor,
            (*GifFileOut).SColorMap,
        )
    } == GIF_ERROR
    {
        return GIF_ERROR;
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let image_count = usize::try_from(unsafe { (*GifFileOut).ImageCount }).unwrap_or(0);
    for image_index in 0..image_count {
        if unsafe { (*GifFileOut).SavedImages.is_null() } {
            break;
        }

        // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        let saved: &SavedImage = unsafe { &*(*GifFileOut).SavedImages.add(image_index) };
        let saved_height = saved.ImageDesc.Height;
        let saved_width = saved.ImageDesc.Width;

        if saved.RasterBits.is_null() {
            continue;
        }

        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe {
            write_extensions_impl(GifFileOut, saved.ExtensionBlocks, saved.ExtensionBlockCount)
        } == GIF_ERROR
        {
            return GIF_ERROR;
        }

        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe {
            put_image_desc_impl(
                GifFileOut,
                saved.ImageDesc.Left,
                saved.ImageDesc.Top,
                saved_width,
                saved_height,
                saved.ImageDesc.Interlace.get(),
                saved.ImageDesc.ColorMap,
            )
        } == GIF_ERROR
        {
            return GIF_ERROR;
        }

        let width = usize::try_from(saved_width).unwrap_or(0);
        let height = usize::try_from(saved_height).unwrap_or(0);
        if saved.ImageDesc.Interlace.get() {
            for pass in 0..4 {
                let mut row = usize::try_from(INTERLACED_OFFSET[pass]).unwrap_or(0);
                let jump = usize::try_from(INTERLACED_JUMPS[pass]).unwrap_or(1);
                while row < height {
                    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                    if unsafe {
                        put_line_impl(GifFileOut, saved.RasterBits.add(row * width), saved_width)
                    } == GIF_ERROR
                    {
                        return GIF_ERROR;
                    }
                    row += jump;
                }
            }
        } else {
            for row in 0..height {
                // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
                if unsafe {
                    put_line_impl(GifFileOut, saved.RasterBits.add(row * width), saved_width)
                } == GIF_ERROR
                {
                    return GIF_ERROR;
                }
            }
        }
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    if unsafe {
        write_extensions_impl(
            GifFileOut,
            (*GifFileOut).ExtensionBlocks,
            (*GifFileOut).ExtensionBlockCount,
        )
    } == GIF_ERROR
    {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe { close_file_impl(GifFileOut, ptr::null_mut()) }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifOpenFileName(
    FileName: *const c_char,
    TestExistence: bool,
    Error: *mut i32,
) -> *mut GifFileType {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_open_or(ptr::null_mut(), Error, || unsafe {
        let file_handle = open_output_file(FileName, TestExistence);
        if file_handle == -1 {
            if !Error.is_null() {
                *Error = E_GIF_ERR_OPEN_FAILED;
            }
            return ptr::null_mut();
        }

        let gif_file = EGifOpenFileHandle(file_handle, Error);
        if gif_file.is_null() {
            close_fd(file_handle);
        }
        gif_file
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifOpenFileHandle(FileHandle: i32, Error: *mut i32) -> *mut GifFileType {
    catch_encode_open_or(ptr::null_mut(), Error, || unsafe {
        let gif_file = open_common_impl(Error, false);
        if gif_file.is_null() {
            return ptr::null_mut();
        }

        let state = require_encoder(gif_file);
        if state.is_null() {
            free_gif_file(gif_file);
            return ptr::null_mut();
        }

        (*state).file_handle = FileHandle;
        (*state).file = fdopen_write(FileHandle);
        (*state).file_state = FILE_STATE_WRITE;
        (*state).gif89 = false;
        (*state).write_func = None;
        (*gif_file).UserData = ptr::null_mut();
        (*gif_file).Error = 0;

        gif_file
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifOpen(
    userData: *mut c_void,
    writeFunc: OutputFunc,
    Error: *mut i32,
) -> *mut GifFileType {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_open_or(ptr::null_mut(), Error, || unsafe {
        let gif_file = open_common_impl(Error, true);
        if gif_file.is_null() {
            return ptr::null_mut();
        }

        let state = require_encoder(gif_file);
        if state.is_null() {
            free_gif_file(gif_file);
            return ptr::null_mut();
        }

        (*state).file_handle = 0;
        (*state).file = ptr::null_mut();
        (*state).file_state = FILE_STATE_WRITE;
        (*state).write_func = writeFunc;
        (*state).gif89 = false;
        (*gif_file).UserData = userData;
        (*gif_file).Error = 0;

        gif_file
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifGetGifVersion(GifFile: *mut GifFileType) -> *const c_char {
    catch_encode_or(GIF87_STAMP.as_ptr().cast(), GifFile, || unsafe {
        if gif_version_bytes(GifFile) == GIF89_WRITE_STAMP {
            GIF89_STAMP.as_ptr().cast()
        } else {
            GIF87_STAMP.as_ptr().cast()
        }
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifSetGifVersion(GifFile: *mut GifFileType, gif89: bool) {
    catch_encode_or((), GifFile, || unsafe {
        let state = require_encoder(GifFile);
        if !state.is_null() {
            (*state).gif89 = gif89;
        }
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutScreenDesc(
    GifFile: *mut GifFileType,
    Width: i32,
    Height: i32,
    ColorRes: i32,
    BackGround: i32,
    ColorMap: *const ColorMapObject,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_screen_desc_impl(GifFile, Width, Height, ColorRes, BackGround, ColorMap)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutImageDesc(
    GifFile: *mut GifFileType,
    Left: i32,
    Top: i32,
    Width: i32,
    Height: i32,
    Interlace: bool,
    ColorMap: *const ColorMapObject,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_image_desc_impl(GifFile, Left, Top, Width, Height, Interlace, ColorMap)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutLine(
    GifFile: *mut GifFileType,
    Line: *mut GifPixelType,
    LineLen: i32,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_line_impl(GifFile, Line, LineLen)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutPixel(GifFile: *mut GifFileType, Pixel: GifPixelType) -> i32 {
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_pixel_impl(GifFile, Pixel)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutComment(GifFile: *mut GifFileType, Comment: *const c_char) -> i32 {
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_comment_impl(GifFile, Comment)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutExtensionLeader(GifFile: *mut GifFileType, ExtCode: i32) -> i32 {
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_extension_leader_impl(GifFile, ExtCode)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutExtensionBlock(
    GifFile: *mut GifFileType,
    ExtLen: i32,
    Extension: *const c_void,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_extension_block_impl(GifFile, ExtLen, Extension)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutExtensionTrailer(GifFile: *mut GifFileType) -> i32 {
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_extension_trailer_impl(GifFile)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutExtension(
    GifFile: *mut GifFileType,
    ExtCode: i32,
    ExtLen: i32,
    Extension: *const c_void,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_extension_impl(GifFile, ExtCode, ExtLen, Extension)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutCode(
    GifFile: *mut GifFileType,
    _CodeSize: i32,
    CodeBlock: *const u8,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_code_next_impl(GifFile, CodeBlock)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifPutCodeNext(GifFile: *mut GifFileType, CodeBlock: *const u8) -> i32 {
    catch_encode_or(GIF_ERROR, GifFile, || unsafe {
        put_code_next_impl(GifFile, CodeBlock)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifCloseFile(GifFile: *mut GifFileType, ErrorCode: *mut i32) -> i32 {
    catch_encode_close_or(GIF_ERROR, GifFile, ErrorCode, || unsafe {
        close_file_impl(GifFile, ErrorCode)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifSpew(GifFileOut: *mut GifFileType) -> i32 {
    catch_encode_or(GIF_ERROR, GifFileOut, || unsafe { spew_impl(GifFileOut) })
}
