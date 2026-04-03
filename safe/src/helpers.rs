#![allow(non_snake_case)]

use core::cmp::max;
use core::ptr;

use crate::bootstrap::catch_panic_or;
use crate::ffi::{
    ColorMapObject, ExtensionBlock, GifColorType, GifFileType, GifImageDesc, GifPixelType,
    SavedImage, GIF_ERROR, GIF_OK,
};
use crate::memory::{alloc_array, alloc_struct, c_free, c_malloc, calloc_array, realloc_array};

fn gif_bit_size_impl(n: i32) -> i32 {
    let mut i = 1;
    while i <= 8 {
        if (1 << i) >= n {
            break;
        }
        i += 1;
    }
    i
}

fn positive_usize(value: i32) -> Option<usize> {
    usize::try_from(value).ok().filter(|count| *count > 0)
}

fn image_pixel_count(desc: &GifImageDesc) -> Option<usize> {
    positive_usize(desc.Height)?.checked_mul(positive_usize(desc.Width)?)
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn free_saved_image_contents(sp: *mut SavedImage) {
    if sp.is_null() {
        return;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        if !(*sp).ImageDesc.ColorMap.is_null() {
            GifFreeMapObject((*sp).ImageDesc.ColorMap);
            (*sp).ImageDesc.ColorMap = ptr::null_mut();
        }
        if !(*sp).RasterBits.is_null() {
            c_free((*sp).RasterBits);
            (*sp).RasterBits = ptr::null_mut();
        }
        GifFreeExtensions(&mut (*sp).ExtensionBlockCount, &mut (*sp).ExtensionBlocks);
    }
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn deep_copy_extension_blocks(
    dst: *mut SavedImage,
    src: *const SavedImage,
) -> Result<(), ()> {
    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let src_blocks = unsafe { (*src).ExtensionBlocks };
    if src_blocks.is_null() {
        return Ok(());
    }

    // SAFETY: The surrounding checks ensure these raw giflib pointers are valid for this access.
    let count = match usize::try_from(unsafe { (*src).ExtensionBlockCount }) {
        Ok(count) => count,
        Err(_) => return Err(()),
    };
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    let dst_blocks = unsafe { alloc_array::<ExtensionBlock>(count) };
    if dst_blocks.is_null() {
        return Err(());
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*dst).ExtensionBlocks = dst_blocks;
        for index in 0..count {
            let src_block = src_blocks.add(index);
            let dst_block = dst_blocks.add(index);
            (*dst_block).ByteCount = (*src_block).ByteCount;
            (*dst_block).Function = (*src_block).Function;
            (*dst_block).Bytes = ptr::null_mut();

            let byte_count = match usize::try_from((*src_block).ByteCount) {
                Ok(byte_count) => byte_count,
                Err(_) => return Err(()),
            };
            let bytes: *mut u8 = c_malloc(byte_count).cast();
            if bytes.is_null() {
                return Err(());
            }
            if byte_count > 0 {
                if (*src_block).Bytes.is_null() {
                    return Err(());
                }
                ptr::copy_nonoverlapping((*src_block).Bytes, bytes, byte_count);
            }
            (*dst_block).Bytes = bytes;
        }
    }

    Ok(())
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn make_saved_image_impl(
    GifFile: *mut GifFileType,
    CopyFrom: *const SavedImage,
) -> *mut SavedImage {
    if GifFile.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        let gif = &mut *GifFile;
        if gif.SavedImages.is_null() {
            gif.SavedImages = alloc_struct::<SavedImage>();
        } else {
            let newSavedImages = realloc_array::<SavedImage>(
                gif.SavedImages,
                match usize::try_from(gif.ImageCount.saturating_add(1)) {
                    Ok(count) => count,
                    Err(_) => return ptr::null_mut(),
                },
            );
            if newSavedImages.is_null() {
                return ptr::null_mut();
            }
            gif.SavedImages = newSavedImages;
        }
        if gif.SavedImages.is_null() {
            return ptr::null_mut();
        }

        let sp = gif
            .SavedImages
            .add(usize::try_from(gif.ImageCount).unwrap_or(0));
        gif.ImageCount += 1;

        if CopyFrom.is_null() {
            ptr::write_bytes(sp, 0, 1);
            return sp;
        }

        ptr::write_bytes(sp, 0, 1);
        (*sp).ImageDesc = (*CopyFrom).ImageDesc;
        (*sp).ImageDesc.ColorMap = ptr::null_mut();
        (*sp).RasterBits = ptr::null_mut();
        (*sp).ExtensionBlockCount = (*CopyFrom).ExtensionBlockCount;
        (*sp).ExtensionBlocks = ptr::null_mut();

        if !(*CopyFrom).ImageDesc.ColorMap.is_null() {
            let map = GifMakeMapObject(
                (*(*CopyFrom).ImageDesc.ColorMap).ColorCount,
                (*(*CopyFrom).ImageDesc.ColorMap).Colors,
            );
            if map.is_null() {
                FreeLastSavedImage(GifFile);
                return ptr::null_mut();
            }
            (*sp).ImageDesc.ColorMap = map;
        }

        let pixel_count = match image_pixel_count(&(*CopyFrom).ImageDesc) {
            Some(pixel_count) => pixel_count,
            None => {
                FreeLastSavedImage(GifFile);
                return ptr::null_mut();
            }
        };
        let raster_bits = alloc_array::<GifPixelType>(pixel_count);
        if raster_bits.is_null() || (*CopyFrom).RasterBits.is_null() {
            FreeLastSavedImage(GifFile);
            return ptr::null_mut();
        }
        ptr::copy_nonoverlapping((*CopyFrom).RasterBits, raster_bits, pixel_count);
        (*sp).RasterBits = raster_bits;

        if deep_copy_extension_blocks(sp, CopyFrom).is_err() {
            FreeLastSavedImage(GifFile);
            return ptr::null_mut();
        }

        sp
    }
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifBitSize(n: i32) -> i32 {
    catch_panic_or(0, || gif_bit_size_impl(n))
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifMakeMapObject(
    ColorCount: i32,
    ColorMap: *const GifColorType,
) -> *mut ColorMapObject {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(ptr::null_mut(), || unsafe {
        if ColorCount != (1 << gif_bit_size_impl(ColorCount)) {
            return ptr::null_mut();
        }

        let Object = alloc_struct::<ColorMapObject>();
        if Object.is_null() {
            return ptr::null_mut();
        }

        let count = match usize::try_from(ColorCount) {
            Ok(count) => count,
            Err(_) => {
                c_free(Object);
                return ptr::null_mut();
            }
        };
        let Colors = calloc_array::<GifColorType>(count);
        if Colors.is_null() {
            c_free(Object);
            return ptr::null_mut();
        }

        (*Object).ColorCount = ColorCount;
        (*Object).BitsPerPixel = gif_bit_size_impl(ColorCount);
        (*Object).SortFlag.set(false);
        (*Object).Colors = Colors;

        if !ColorMap.is_null() {
            ptr::copy_nonoverlapping(ColorMap, Colors, count);
        }

        Object
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifFreeMapObject(Object: *mut ColorMapObject) {
    catch_panic_or((), || unsafe {
        if Object.is_null() {
            return;
        }
        c_free((*Object).Colors);
        c_free(Object);
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifUnionColorMap(
    ColorIn1: *const ColorMapObject,
    ColorIn2: *const ColorMapObject,
    ColorTransIn2: *mut GifPixelType,
) -> *mut ColorMapObject {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(ptr::null_mut(), || unsafe {
        if ColorIn1.is_null() || ColorIn2.is_null() || ColorTransIn2.is_null() {
            return ptr::null_mut();
        }

        let color_union = GifMakeMapObject(
            max((*ColorIn1).ColorCount, (*ColorIn2).ColorCount) * 2,
            ptr::null(),
        );
        if color_union.is_null() {
            return ptr::null_mut();
        }

        let color_in1_count = match usize::try_from((*ColorIn1).ColorCount) {
            Ok(count) => count,
            Err(_) => {
                GifFreeMapObject(color_union);
                return ptr::null_mut();
            }
        };
        let color_in2_count = match usize::try_from((*ColorIn2).ColorCount) {
            Ok(count) => count,
            Err(_) => {
                GifFreeMapObject(color_union);
                return ptr::null_mut();
            }
        };

        for i in 0..color_in1_count {
            *(*color_union).Colors.add(i) = *(*ColorIn1).Colors.add(i);
        }
        let mut crnt_slot = color_in1_count;
        while crnt_slot > 0 {
            let color = *(*ColorIn1).Colors.add(crnt_slot - 1);
            if color.Red != 0 || color.Green != 0 || color.Blue != 0 {
                break;
            }
            crnt_slot -= 1;
        }

        for i in 0..color_in2_count {
            if crnt_slot > 256 {
                break;
            }

            let mut j = 0usize;
            while j < color_in1_count {
                let left = *(*ColorIn1).Colors.add(j);
                let right = *(*ColorIn2).Colors.add(i);
                if left.Red == right.Red && left.Green == right.Green && left.Blue == right.Blue {
                    break;
                }
                j += 1;
            }

            if j < color_in1_count {
                *ColorTransIn2.add(i) = j as GifPixelType;
            } else {
                *(*color_union).Colors.add(crnt_slot) = *(*ColorIn2).Colors.add(i);
                *ColorTransIn2.add(i) = crnt_slot as GifPixelType;
                crnt_slot += 1;
            }
        }

        if crnt_slot > 256 {
            GifFreeMapObject(color_union);
            return ptr::null_mut();
        }

        let new_gif_bit_size = gif_bit_size_impl(crnt_slot as i32);
        let round_up_to = 1usize << usize::try_from(new_gif_bit_size).unwrap_or(0);

        if round_up_to != usize::try_from((*color_union).ColorCount).unwrap_or(0) {
            for j in crnt_slot..round_up_to {
                let color = (*color_union).Colors.add(j);
                (*color).Red = 0;
                (*color).Green = 0;
                (*color).Blue = 0;
            }

            if round_up_to < usize::try_from((*color_union).ColorCount).unwrap_or(0) {
                let new_map = realloc_array::<GifColorType>((*color_union).Colors, round_up_to);
                if new_map.is_null() {
                    GifFreeMapObject(color_union);
                    return ptr::null_mut();
                }
                (*color_union).Colors = new_map;
            }
        }

        (*color_union).ColorCount = round_up_to as i32;
        (*color_union).BitsPerPixel = new_gif_bit_size;
        color_union
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifApplyTranslation(
    Image: *mut SavedImage,
    Translation: *const GifPixelType,
) {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or((), || unsafe {
        if Image.is_null() || Translation.is_null() || (*Image).RasterBits.is_null() {
            return;
        }
        let raster_size = match image_pixel_count(&(*Image).ImageDesc) {
            Some(raster_size) => raster_size,
            None => return,
        };
        for i in 0..raster_size {
            let pixel = *(*Image).RasterBits.add(i);
            *(*Image).RasterBits.add(i) = *Translation.add(pixel as usize);
        }
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifAddExtensionBlock(
    ExtensionBlockCount: *mut i32,
    ExtensionBlocks: *mut *mut ExtensionBlock,
    Function: i32,
    Len: u32,
    ExtData: *mut u8,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(GIF_ERROR, || unsafe {
        if ExtensionBlockCount.is_null() || ExtensionBlocks.is_null() {
            return GIF_ERROR;
        }
        let current_count = match usize::try_from(*ExtensionBlockCount) {
            Ok(count) => count,
            Err(_) => return GIF_ERROR,
        };
        let byte_count = match i32::try_from(Len) {
            Ok(byte_count) => byte_count,
            Err(_) => return GIF_ERROR,
        };

        if (*ExtensionBlocks).is_null() {
            *ExtensionBlocks = alloc_struct::<ExtensionBlock>();
        } else {
            let ep_new = realloc_array::<ExtensionBlock>(*ExtensionBlocks, current_count + 1);
            if ep_new.is_null() {
                return GIF_ERROR;
            }
            *ExtensionBlocks = ep_new;
        }

        if (*ExtensionBlocks).is_null() {
            return GIF_ERROR;
        }

        let ep = (*ExtensionBlocks).add(current_count);
        *ExtensionBlockCount += 1;

        (*ep).Function = Function;
        (*ep).ByteCount = byte_count;
        (*ep).Bytes = c_malloc(usize::try_from(byte_count).unwrap_or(0)).cast();
        if (*ep).Bytes.is_null() {
            return GIF_ERROR;
        }

        if !ExtData.is_null() {
            ptr::copy_nonoverlapping(
                ExtData,
                (*ep).Bytes,
                usize::try_from(byte_count).unwrap_or(0),
            );
        }

        GIF_OK
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifFreeExtensions(
    ExtensionBlockCount: *mut i32,
    ExtensionBlocks: *mut *mut ExtensionBlock,
) {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or((), || unsafe {
        if ExtensionBlockCount.is_null()
            || ExtensionBlocks.is_null()
            || (*ExtensionBlocks).is_null()
        {
            return;
        }
        let count = usize::try_from(*ExtensionBlockCount).unwrap_or(0);
        for index in 0..count {
            c_free((*(*ExtensionBlocks).add(index)).Bytes);
        }
        c_free(*ExtensionBlocks);
        *ExtensionBlocks = ptr::null_mut();
        *ExtensionBlockCount = 0;
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn FreeLastSavedImage(GifFile: *mut GifFileType) {
    catch_panic_or((), || unsafe {
        if GifFile.is_null() || (*GifFile).SavedImages.is_null() || (*GifFile).ImageCount <= 0 {
            return;
        }
        (*GifFile).ImageCount -= 1;
        let sp = (*GifFile)
            .SavedImages
            .add(usize::try_from((*GifFile).ImageCount).unwrap_or(0));
        free_saved_image_contents(sp);
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifMakeSavedImage(
    GifFile: *mut GifFileType,
    CopyFrom: *const SavedImage,
) -> *mut SavedImage {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(ptr::null_mut(), || unsafe {
        make_saved_image_impl(GifFile, CopyFrom)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifFreeSavedImages(GifFile: *mut GifFileType) {
    catch_panic_or((), || unsafe {
        if GifFile.is_null() || (*GifFile).SavedImages.is_null() {
            return;
        }
        let image_count = usize::try_from((*GifFile).ImageCount).unwrap_or(0);
        for index in 0..image_count {
            free_saved_image_contents((*GifFile).SavedImages.add(index));
        }
        c_free((*GifFile).SavedImages);
        (*GifFile).SavedImages = ptr::null_mut();
    })
}
