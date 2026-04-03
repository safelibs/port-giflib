#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(non_snake_case)]

use core::mem::{offset_of, size_of};

pub type GifPixelType = u8;
pub type GifRowType = *mut GifPixelType;
pub type GifByteType = u8;
pub type GifPrefixType = u32;
pub type GifWord = i32;
pub type GifRecordType = i32;

pub const GIF_ERROR: i32 = 0;
pub const GIF_OK: i32 = 1;

pub const UNDEFINED_RECORD_TYPE: GifRecordType = 0;
pub const SCREEN_DESC_RECORD_TYPE: GifRecordType = 1;
pub const IMAGE_DESC_RECORD_TYPE: GifRecordType = 2;
pub const EXTENSION_RECORD_TYPE: GifRecordType = 3;
pub const TERMINATE_RECORD_TYPE: GifRecordType = 4;

pub const CONTINUE_EXT_FUNC_CODE: i32 = 0x00;
pub const COMMENT_EXT_FUNC_CODE: i32 = 0xfe;
pub const GRAPHICS_EXT_FUNC_CODE: i32 = 0xf9;
pub const PLAINTEXT_EXT_FUNC_CODE: i32 = 0x01;
pub const APPLICATION_EXT_FUNC_CODE: i32 = 0xff;

pub const DISPOSAL_UNSPECIFIED: i32 = 0;
pub const DISPOSE_DO_NOT: i32 = 1;
pub const DISPOSE_BACKGROUND: i32 = 2;
pub const DISPOSE_PREVIOUS: i32 = 3;
pub const NO_TRANSPARENT_COLOR: i32 = -1;

pub const E_GIF_ERR_OPEN_FAILED: i32 = 1;
pub const E_GIF_ERR_WRITE_FAILED: i32 = 2;
pub const E_GIF_ERR_HAS_SCRN_DSCR: i32 = 3;
pub const E_GIF_ERR_HAS_IMAG_DSCR: i32 = 4;
pub const E_GIF_ERR_NO_COLOR_MAP: i32 = 5;
pub const E_GIF_ERR_DATA_TOO_BIG: i32 = 6;
pub const E_GIF_ERR_NOT_ENOUGH_MEM: i32 = 7;
pub const E_GIF_ERR_DISK_IS_FULL: i32 = 8;
pub const E_GIF_ERR_CLOSE_FAILED: i32 = 9;
pub const E_GIF_ERR_NOT_WRITEABLE: i32 = 10;

pub const D_GIF_ERR_OPEN_FAILED: i32 = 101;
pub const D_GIF_ERR_READ_FAILED: i32 = 102;
pub const D_GIF_ERR_NOT_GIF_FILE: i32 = 103;
pub const D_GIF_ERR_NO_SCRN_DSCR: i32 = 104;
pub const D_GIF_ERR_NO_IMAG_DSCR: i32 = 105;
pub const D_GIF_ERR_NO_COLOR_MAP: i32 = 106;
pub const D_GIF_ERR_WRONG_RECORD: i32 = 107;
pub const D_GIF_ERR_DATA_TOO_BIG: i32 = 108;
pub const D_GIF_ERR_NOT_ENOUGH_MEM: i32 = 109;
pub const D_GIF_ERR_CLOSE_FAILED: i32 = 110;
pub const D_GIF_ERR_NOT_READABLE: i32 = 111;
pub const D_GIF_ERR_IMAGE_DEFECT: i32 = 112;
pub const D_GIF_ERR_EOF_TOO_SOON: i32 = 113;

pub const GIF_FONT_WIDTH: usize = 8;
pub const GIF_FONT_HEIGHT: usize = 8;

pub const HT_SIZE: usize = 8192;
pub const HT_KEY_MASK: u32 = 0x1FFF;
pub const HT_KEY_NUM_BITS: u32 = 13;
pub const HT_MAX_KEY: u32 = 8191;
pub const HT_MAX_CODE: u32 = 4095;
pub const HT_EMPTY_KEY: u32 = 0xFFFFF;

#[repr(transparent)]
#[derive(Copy, Clone, Default)]
pub struct GifBool(pub u8);

impl GifBool {
    pub const fn new(value: bool) -> Self {
        Self(value as u8)
    }

    pub const fn get(self) -> bool {
        self.0 != 0
    }

    pub fn set(&mut self, value: bool) {
        self.0 = u8::from(value);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct GifColorType {
    pub Red: GifByteType,
    pub Green: GifByteType,
    pub Blue: GifByteType,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct ColorMapObject {
    pub ColorCount: i32,
    pub BitsPerPixel: i32,
    pub SortFlag: GifBool,
    pub _padding0: [u8; 7],
    pub Colors: *mut GifColorType,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct GifImageDesc {
    pub Left: GifWord,
    pub Top: GifWord,
    pub Width: GifWord,
    pub Height: GifWord,
    pub Interlace: GifBool,
    pub _padding0: [u8; 7],
    pub ColorMap: *mut ColorMapObject,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct ExtensionBlock {
    pub ByteCount: i32,
    pub Bytes: *mut GifByteType,
    pub Function: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SavedImage {
    pub ImageDesc: GifImageDesc,
    pub RasterBits: *mut GifByteType,
    pub ExtensionBlockCount: i32,
    pub ExtensionBlocks: *mut ExtensionBlock,
}

#[repr(C)]
#[derive(Default)]
pub struct GifFileType {
    pub SWidth: GifWord,
    pub SHeight: GifWord,
    pub SColorResolution: GifWord,
    pub SBackGroundColor: GifWord,
    pub AspectByte: GifByteType,
    pub _padding0: [u8; 7],
    pub SColorMap: *mut ColorMapObject,
    pub ImageCount: i32,
    pub _padding1: [u8; 4],
    pub Image: GifImageDesc,
    pub SavedImages: *mut SavedImage,
    pub ExtensionBlockCount: i32,
    pub _padding2: [u8; 4],
    pub ExtensionBlocks: *mut ExtensionBlock,
    pub Error: i32,
    pub _padding3: [u8; 4],
    pub UserData: *mut core::ffi::c_void,
    pub Private: *mut core::ffi::c_void,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct GraphicsControlBlock {
    pub DisposalMode: i32,
    pub UserInputFlag: GifBool,
    pub _padding0: [u8; 3],
    pub DelayTime: i32,
    pub TransparentColor: i32,
}

#[repr(C)]
pub struct GifHashTableType {
    pub HTable: [u32; HT_SIZE],
}

pub type InputFunc = Option<unsafe extern "C" fn(*mut GifFileType, *mut GifByteType, i32) -> i32>;
pub type OutputFunc =
    Option<unsafe extern "C" fn(*mut GifFileType, *const GifByteType, i32) -> i32>;

const _: () = {
    assert!(size_of::<GifColorType>() == 3);
    assert!(size_of::<ColorMapObject>() == 24);
    assert!(offset_of!(ColorMapObject, ColorCount) == 0);
    assert!(offset_of!(ColorMapObject, BitsPerPixel) == 4);
    assert!(offset_of!(ColorMapObject, SortFlag) == 8);
    assert!(offset_of!(ColorMapObject, Colors) == 16);

    assert!(size_of::<GifImageDesc>() == 32);
    assert!(offset_of!(GifImageDesc, Left) == 0);
    assert!(offset_of!(GifImageDesc, Top) == 4);
    assert!(offset_of!(GifImageDesc, Width) == 8);
    assert!(offset_of!(GifImageDesc, Height) == 12);
    assert!(offset_of!(GifImageDesc, Interlace) == 16);
    assert!(offset_of!(GifImageDesc, ColorMap) == 24);

    assert!(size_of::<ExtensionBlock>() == 24);
    assert!(offset_of!(ExtensionBlock, ByteCount) == 0);
    assert!(offset_of!(ExtensionBlock, Bytes) == 8);
    assert!(offset_of!(ExtensionBlock, Function) == 16);

    assert!(size_of::<SavedImage>() == 56);
    assert!(offset_of!(SavedImage, ImageDesc) == 0);
    assert!(offset_of!(SavedImage, RasterBits) == 32);
    assert!(offset_of!(SavedImage, ExtensionBlockCount) == 40);
    assert!(offset_of!(SavedImage, ExtensionBlocks) == 48);

    assert!(size_of::<GifFileType>() == 120);
    assert!(offset_of!(GifFileType, SWidth) == 0);
    assert!(offset_of!(GifFileType, SHeight) == 4);
    assert!(offset_of!(GifFileType, SColorResolution) == 8);
    assert!(offset_of!(GifFileType, SBackGroundColor) == 12);
    assert!(offset_of!(GifFileType, AspectByte) == 16);
    assert!(offset_of!(GifFileType, SColorMap) == 24);
    assert!(offset_of!(GifFileType, ImageCount) == 32);
    assert!(offset_of!(GifFileType, Image) == 40);
    assert!(offset_of!(GifFileType, SavedImages) == 72);
    assert!(offset_of!(GifFileType, ExtensionBlockCount) == 80);
    assert!(offset_of!(GifFileType, ExtensionBlocks) == 88);
    assert!(offset_of!(GifFileType, Error) == 96);
    assert!(offset_of!(GifFileType, UserData) == 104);
    assert!(offset_of!(GifFileType, Private) == 112);

    assert!(size_of::<GraphicsControlBlock>() == 16);
    assert!(offset_of!(GraphicsControlBlock, DisposalMode) == 0);
    assert!(offset_of!(GraphicsControlBlock, UserInputFlag) == 4);
    assert!(offset_of!(GraphicsControlBlock, DelayTime) == 8);
    assert!(offset_of!(GraphicsControlBlock, TransparentColor) == 12);

    assert!(size_of::<GifHashTableType>() == 32768);
};
