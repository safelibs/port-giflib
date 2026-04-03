#![allow(non_snake_case)]

use core::mem::size_of;

use crate::bootstrap::catch_panic_or;
use crate::ffi::{
    ExtensionBlock, GifByteType, GifFileType, GraphicsControlBlock, GIF_ERROR, GIF_OK,
    GRAPHICS_EXT_FUNC_CODE, NO_TRANSPARENT_COLOR,
};
use crate::helpers::GifAddExtensionBlock;

unsafe fn gcb_to_extension_impl(
    GCB: *const GraphicsControlBlock,
    GifExtension: *mut GifByteType,
) -> usize {
    if GCB.is_null() || GifExtension.is_null() {
        return 0;
    }

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn EGifGCBToExtension(
    GCB: *const GraphicsControlBlock,
    GifExtension: *mut GifByteType,
) -> usize {
    catch_panic_or(0, || unsafe { gcb_to_extension_impl(GCB, GifExtension) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn EGifGCBToSavedExtension(
    GCB: *const GraphicsControlBlock,
    GifFile: *mut GifFileType,
    ImageIndex: i32,
) -> i32 {
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
