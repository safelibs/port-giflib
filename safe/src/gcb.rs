#![allow(non_snake_case)]

use core::mem::size_of;

use crate::bootstrap::catch_panic_or;
use crate::ffi::{
    ExtensionBlock, GifByteType, GifFileType, GraphicsControlBlock, GIF_ERROR, GIF_OK,
    GRAPHICS_EXT_FUNC_CODE, NO_TRANSPARENT_COLOR,
};
use crate::helpers::GifAddExtensionBlock;

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn gcb_to_extension_impl(
    GCB: *const GraphicsControlBlock,
    GifExtension: *mut GifByteType,
) -> usize {
    if GCB.is_null() || GifExtension.is_null() {
        return 0;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        *GifExtension.add(0) = 0;
        *GifExtension.add(0) |= if (*GCB).TransparentColor == NO_TRANSPARENT_COLOR {
            0x00
        } else {
            0x01
        };
        *GifExtension.add(0) |= if (*GCB).UserInputFlag.get() {
            0x02
        } else {
            0x00
        };
        *GifExtension.add(0) |= (((*GCB).DisposalMode & 0x07) << 2) as u8;
        *GifExtension.add(1) = ((*GCB).DelayTime & 0xff) as u8;
        *GifExtension.add(2) = (((*GCB).DelayTime >> 8) & 0xff) as u8;
        *GifExtension.add(3) = (*GCB).TransparentColor as u8;
    }

    4
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn extension_to_gcb_impl(
    GifExtensionLength: usize,
    GifExtension: *const GifByteType,
    GCB: *mut GraphicsControlBlock,
) -> i32 {
    if GifExtensionLength != 4 || GifExtension.is_null() || GCB.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        (*GCB).DisposalMode = i32::from((*GifExtension.add(0) >> 2) & 0x07);
        (*GCB).UserInputFlag.set((*GifExtension.add(0) & 0x02) != 0);
        (*GCB).DelayTime = i32::from(*GifExtension.add(1)) | (i32::from(*GifExtension.add(2)) << 8);
        (*GCB).TransparentColor = if (*GifExtension.add(0) & 0x01) != 0 {
            i32::from(*GifExtension.add(3))
        } else {
            NO_TRANSPARENT_COLOR
        };
    }

    GIF_OK
}

// SAFETY: This helper is only called while the surrounding giflib raw-pointer invariants hold.
unsafe fn saved_extension_to_gcb_impl(
    GifFile: *mut GifFileType,
    ImageIndex: i32,
    GCB: *mut GraphicsControlBlock,
) -> i32 {
    if GifFile.is_null() || GCB.is_null() {
        return GIF_ERROR;
    }

    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    unsafe {
        if ImageIndex < 0
            || ImageIndex > (*GifFile).ImageCount - 1
            || (*GifFile).SavedImages.is_null()
        {
            return GIF_ERROR;
        }

        (*GCB).DisposalMode = 0;
        (*GCB).UserInputFlag.set(false);
        (*GCB).DelayTime = 0;
        (*GCB).TransparentColor = NO_TRANSPARENT_COLOR;

        let saved = (*GifFile).SavedImages.add(ImageIndex as usize);
        let extension_count = usize::try_from((*saved).ExtensionBlockCount).unwrap_or(0);
        for index in 0..extension_count {
            let extension: *mut ExtensionBlock = (*saved).ExtensionBlocks.add(index);
            if (*extension).Function == GRAPHICS_EXT_FUNC_CODE {
                return extension_to_gcb_impl(
                    usize::try_from((*extension).ByteCount).unwrap_or(0),
                    (*extension).Bytes,
                    GCB,
                );
            }
        }
    }

    GIF_ERROR
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifGCBToExtension(
    GCB: *const GraphicsControlBlock,
    GifExtension: *mut GifByteType,
) -> usize {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(0, || unsafe { gcb_to_extension_impl(GCB, GifExtension) })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn EGifGCBToSavedExtension(
    GCB: *const GraphicsControlBlock,
    GifFile: *mut GifFileType,
    ImageIndex: i32,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(GIF_ERROR, || unsafe {
        let saved_images = if GifFile.is_null() {
            return GIF_ERROR;
        } else {
            (*GifFile).SavedImages
        };

        if ImageIndex < 0 || ImageIndex > (*GifFile).ImageCount - 1 || saved_images.is_null() {
            return GIF_ERROR;
        }

        let saved = &mut *saved_images.add(ImageIndex as usize);
        if !saved.ExtensionBlocks.is_null() {
            let extension_count = usize::try_from(saved.ExtensionBlockCount).unwrap_or(0);
            for index in 0..extension_count {
                let extension: *mut ExtensionBlock = saved.ExtensionBlocks.add(index);
                if (*extension).Function == GRAPHICS_EXT_FUNC_CODE {
                    if (*extension).Bytes.is_null() {
                        return GIF_ERROR;
                    }
                    let _ = gcb_to_extension_impl(GCB, (*extension).Bytes);
                    return GIF_OK;
                }
            }
        }

        let mut buffer = [0u8; size_of::<GraphicsControlBlock>()];
        let len = gcb_to_extension_impl(GCB, buffer.as_mut_ptr());
        if len == 0 {
            return GIF_ERROR;
        }

        GifAddExtensionBlock(
            &mut saved.ExtensionBlockCount,
            &mut saved.ExtensionBlocks,
            GRAPHICS_EXT_FUNC_CODE,
            len as u32,
            buffer.as_mut_ptr(),
        )
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifExtensionToGCB(
    GifExtensionLength: usize,
    GifExtension: *const GifByteType,
    GCB: *mut GraphicsControlBlock,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(GIF_ERROR, || unsafe {
        extension_to_gcb_impl(GifExtensionLength, GifExtension, GCB)
    })
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn DGifSavedExtensionToGCB(
    GifFile: *mut GifFileType,
    ImageIndex: i32,
    GCB: *mut GraphicsControlBlock,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(GIF_ERROR, || unsafe {
        saved_extension_to_gcb_impl(GifFile, ImageIndex, GCB)
    })
}
