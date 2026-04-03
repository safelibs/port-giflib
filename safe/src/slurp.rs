#![allow(non_snake_case)]

use core::mem::size_of;
use core::ptr;

use crate::bootstrap::catch_panic_or;
use crate::decode::{
    get_extension_impl, get_extension_next_impl, get_image_desc_impl, get_line_impl,
    get_record_type_impl, set_error,
};
use crate::ffi::{
    GifByteType, GifFileType, D_GIF_ERR_NO_IMAG_DSCR, EXTENSION_RECORD_TYPE, GIF_ERROR, GIF_OK,
    IMAGE_DESC_RECORD_TYPE, TERMINATE_RECORD_TYPE,
};
use crate::helpers::{FreeLastSavedImage, GifAddExtensionBlock};
use crate::memory::{alloc_array, realloc_array};

const INTERLACED_OFFSET: [i32; 4] = [0, 4, 2, 1];
const INTERLACED_JUMPS: [i32; 4] = [8, 8, 4, 2];

unsafe fn decrease_image_counter_impl(GifFile: *mut GifFileType) {
    if GifFile.is_null()
        || unsafe { (*GifFile).SavedImages.is_null() }
        || unsafe { (*GifFile).ImageCount } <= 0
    {
        return;
    }

    unsafe {
        FreeLastSavedImage(GifFile);
    }

    let new_count = unsafe { usize::try_from((*GifFile).ImageCount).unwrap_or(0) };
    if new_count > 0 {
        let corrected = unsafe { realloc_array((*GifFile).SavedImages, new_count) };
        if !corrected.is_null() {
            unsafe {
                (*GifFile).SavedImages = corrected;
            }
        }
    }
}

unsafe fn slurp_impl(GifFile: *mut GifFileType) -> i32 {
    if GifFile.is_null() {
        return GIF_ERROR;
    }

    unsafe {
        (*GifFile).ExtensionBlocks = ptr::null_mut();
        (*GifFile).ExtensionBlockCount = 0;
    }

    loop {
        let mut record_type = 0;
        if unsafe { get_record_type_impl(GifFile, &mut record_type) } == GIF_ERROR {
            return GIF_ERROR;
        }

        match record_type {
            IMAGE_DESC_RECORD_TYPE => unsafe {
                if get_image_desc_impl(GifFile) == GIF_ERROR {
                    return GIF_ERROR;
                }

                let saved = (*GifFile)
                    .SavedImages
                    .add((*GifFile).ImageCount as usize - 1);
                let width = (*saved).ImageDesc.Width;
                let height = (*saved).ImageDesc.Height;
                if width <= 0 || height <= 0 || width > i32::MAX / height {
                    decrease_image_counter_impl(GifFile);
                    return GIF_ERROR;
                }

                let image_size = match usize::try_from(width) {
                    Ok(width) => match usize::try_from(height) {
                        Ok(height) => match width.checked_mul(height) {
                            Some(size) => size,
                            None => {
                                decrease_image_counter_impl(GifFile);
                                return GIF_ERROR;
                            }
                        },
                        Err(_) => {
                            decrease_image_counter_impl(GifFile);
                            return GIF_ERROR;
                        }
                    },
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
                            if get_line_impl(
                                GifFile,
                                (*saved).RasterBits.add((row as usize) * (width as usize)),
                                width,
                            ) == GIF_ERROR
                            {
                                decrease_image_counter_impl(GifFile);
                                return GIF_ERROR;
                            }
                            row += INTERLACED_JUMPS[pass];
                        }
                    }
                } else if get_line_impl(GifFile, (*saved).RasterBits, image_size as i32)
                    == GIF_ERROR
                {
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

    if unsafe { (*GifFile).ImageCount } == 0 {
        unsafe {
            set_error(GifFile, D_GIF_ERR_NO_IMAG_DSCR);
        }
        return GIF_ERROR;
    }

    GIF_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DGifDecreaseImageCounter(GifFile: *mut GifFileType) {
    catch_panic_or((), || unsafe {
        decrease_image_counter_impl(GifFile);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DGifSlurp(GifFile: *mut GifFileType) -> i32 {
    catch_panic_or(GIF_ERROR, || unsafe { slurp_impl(GifFile) })
}
