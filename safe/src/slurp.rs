#![allow(non_snake_case)]

use core::mem::size_of;
use core::ptr;

use crate::bootstrap::catch_gif_error_or;
use crate::decode::{
    get_extension_impl, get_extension_next_impl, get_image_desc_impl, get_line_impl,
    get_record_type_impl, set_error,
};
use crate::ffi::{
    GifByteType, GifFileType, SavedImage, D_GIF_ERR_IMAGE_DEFECT, D_GIF_ERR_NO_IMAG_DSCR,
    EXTENSION_RECORD_TYPE, GIF_ERROR, GIF_OK, IMAGE_DESC_RECORD_TYPE, TERMINATE_RECORD_TYPE,
};
use crate::helpers::{FreeLastSavedImage, GifAddExtensionBlock};
use crate::memory::{alloc_array, realloc_array};

const INTERLACED_OFFSET: [i32; 4] = [0, 4, 2, 1];
const INTERLACED_JUMPS: [i32; 4] = [8, 8, 4, 2];

fn checked_positive_usize(value: i32) -> Option<usize> {
    usize::try_from(value).ok().filter(|count| *count > 0)
}

fn checked_image_size(width: i32, height: i32) -> Option<usize> {
    checked_positive_usize(width)?.checked_mul(checked_positive_usize(height)?)
}

fn checked_raster_offset(row: i32, width: i32) -> Option<usize> {
    usize::try_from(row)
        .ok()?
        .checked_mul(usize::try_from(width).ok()?)
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn current_saved_image_impl(GifFile: *mut GifFileType) -> *mut SavedImage {
    if GifFile.is_null() || unsafe { (*GifFile).SavedImages.is_null() } {
        unsafe {
            set_error(GifFile, D_GIF_ERR_IMAGE_DEFECT);
        }
        return ptr::null_mut();
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let image_count = match unsafe { usize::try_from((*GifFile).ImageCount) } {
        Ok(image_count) if image_count > 0 => image_count,
        _ => {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                set_error(GifFile, D_GIF_ERR_IMAGE_DEFECT);
            }
            return ptr::null_mut();
        }
    };

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    unsafe { (*GifFile).SavedImages.add(image_count - 1) }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn decrease_image_counter_impl(GifFile: *mut GifFileType) {
    if GifFile.is_null()
        || unsafe { (*GifFile).SavedImages.is_null() }
// SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
        || unsafe { (*GifFile).ImageCount } <= 0
    {
        return;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        FreeLastSavedImage(GifFile);
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let new_count = unsafe { usize::try_from((*GifFile).ImageCount).unwrap_or(0) };
    if new_count > 0 {
        let corrected = unsafe { realloc_array((*GifFile).SavedImages, new_count) };
        if !corrected.is_null() {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            unsafe {
                (*GifFile).SavedImages = corrected;
            }
        }
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn slurp_impl(GifFile: *mut GifFileType) -> i32 {
    if GifFile.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GifFile).ExtensionBlocks = ptr::null_mut();
        (*GifFile).ExtensionBlockCount = 0;
    }

    loop {
        let mut record_type = 0;
        // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
        if unsafe { get_record_type_impl(GifFile, &mut record_type) } == GIF_ERROR {
            return GIF_ERROR;
        }

        match record_type {
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            IMAGE_DESC_RECORD_TYPE => unsafe {
                if get_image_desc_impl(GifFile) == GIF_ERROR {
                    return GIF_ERROR;
                }

                let saved = current_saved_image_impl(GifFile);
                if saved.is_null() {
                    return GIF_ERROR;
                }
                let width = (*saved).ImageDesc.Width;
                let height = (*saved).ImageDesc.Height;
                let image_size = match checked_image_size(width, height) {
                    Some(image_size) => image_size,
                    None => {
                        decrease_image_counter_impl(GifFile);
                        return GIF_ERROR;
                    }
                };

                let image_size_i32 = match i32::try_from(image_size) {
                    Ok(image_size_i32) => image_size_i32,
                    Err(_) => {
                        decrease_image_counter_impl(GifFile);
                        return GIF_ERROR;
                    }
                };

                if image_size > usize::MAX / size_of::<GifByteType>() {
                    decrease_image_counter_impl(GifFile);
                    return GIF_ERROR;
                }

                (*saved).RasterBits = alloc_array(image_size);
                if (*saved).RasterBits.is_null() {
                    decrease_image_counter_impl(GifFile);
                    return GIF_ERROR;
                }

                if (*saved).ImageDesc.Interlace.get() {
                    for pass in 0..INTERLACED_OFFSET.len() {
                        let mut row = INTERLACED_OFFSET[pass];
                        while row < height {
                            let offset = match checked_raster_offset(row, width) {
                                Some(offset) => offset,
                                None => {
                                    decrease_image_counter_impl(GifFile);
                                    return GIF_ERROR;
                                }
                            };
                            if get_line_impl(GifFile, (*saved).RasterBits.add(offset), width)
                                == GIF_ERROR
                            {
                                decrease_image_counter_impl(GifFile);
                                return GIF_ERROR;
                            }
                            row += INTERLACED_JUMPS[pass];
                        }
                    }
                } else if get_line_impl(GifFile, (*saved).RasterBits, image_size_i32) == GIF_ERROR {
                    decrease_image_counter_impl(GifFile);
                    return GIF_ERROR;
                }

                if !(*GifFile).ExtensionBlocks.is_null() {
                    (*saved).ExtensionBlocks = (*GifFile).ExtensionBlocks;
                    (*saved).ExtensionBlockCount = (*GifFile).ExtensionBlockCount;
                    (*GifFile).ExtensionBlocks = ptr::null_mut();
                    (*GifFile).ExtensionBlockCount = 0;
                }
            },
            // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
            EXTENSION_RECORD_TYPE => unsafe {
                let mut ext_function = 0;
                let mut ext_data = ptr::null_mut();

                if get_extension_impl(GifFile, &mut ext_function, &mut ext_data) == GIF_ERROR {
                    return GIF_ERROR;
                }

                if !ext_data.is_null()
                    && GifAddExtensionBlock(
                        &mut (*GifFile).ExtensionBlockCount,
                        &mut (*GifFile).ExtensionBlocks,
                        ext_function,
                        u32::from(*ext_data),
                        ext_data.add(1),
                    ) == GIF_ERROR
                {
                    return GIF_ERROR;
                }

                loop {
                    if get_extension_next_impl(GifFile, &mut ext_data) == GIF_ERROR {
                        return GIF_ERROR;
                    }
                    if ext_data.is_null() {
                        break;
                    }

                    if GifAddExtensionBlock(
                        &mut (*GifFile).ExtensionBlockCount,
                        &mut (*GifFile).ExtensionBlocks,
                        0,
                        u32::from(*ext_data),
                        ext_data.add(1),
                    ) == GIF_ERROR
                    {
                        return GIF_ERROR;
                    }
                }
            },
            TERMINATE_RECORD_TYPE => break,
            _ => {}
        }
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    if unsafe { (*GifFile).ImageCount } == 0 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_NO_IMAG_DSCR);
        }
        return GIF_ERROR;
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifDecreaseImageCounter(GifFile: *mut GifFileType) {
    catch_gif_error_or((), GifFile, D_GIF_ERR_IMAGE_DEFECT, || unsafe {
        decrease_image_counter_impl(GifFile);
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifSlurp(GifFile: *mut GifFileType) -> i32 {
    catch_gif_error_or(GIF_ERROR, GifFile, D_GIF_ERR_IMAGE_DEFECT, || unsafe {
        slurp_impl(GifFile)
    })
}
