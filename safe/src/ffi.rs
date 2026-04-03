#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(non_snake_case)]

use core::mem::size_of;
use core::ptr;

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
#[derive(Copy, Clone)]
pub struct ColorMapObject {
    pub ColorCount: i32,
    pub BitsPerPixel: i32,
    pub SortFlag: GifBool,
    pub _padding0: [u8; 7],
    pub Colors: *mut GifColorType,
}

#[repr(C)]
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
pub struct ExtensionBlock {
    pub ByteCount: i32,
    pub Bytes: *mut GifByteType,
    pub Function: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SavedImage {
    pub ImageDesc: GifImageDesc,
    pub RasterBits: *mut GifByteType,
    pub ExtensionBlockCount: i32,
    pub ExtensionBlocks: *mut ExtensionBlock,
}

#[repr(C)]
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

impl Default for ColorMapObject {
    fn default() -> Self {
        Self {
            ColorCount: 0,
            BitsPerPixel: 0,
            SortFlag: GifBool::default(),
            _padding0: [0; 7],
            Colors: ptr::null_mut(),
        }
    }
}

impl Default for GifImageDesc {
    fn default() -> Self {
        Self {
            Left: 0,
            Top: 0,
            Width: 0,
            Height: 0,
            Interlace: GifBool::default(),
            _padding0: [0; 7],
            ColorMap: ptr::null_mut(),
        }
    }
}

impl Default for ExtensionBlock {
    fn default() -> Self {
        Self {
            ByteCount: 0,
            Bytes: ptr::null_mut(),
            Function: 0,
        }
    }
}

impl Default for SavedImage {
    fn default() -> Self {
        Self {
            ImageDesc: GifImageDesc::default(),
            RasterBits: ptr::null_mut(),
            ExtensionBlockCount: 0,
            ExtensionBlocks: ptr::null_mut(),
        }
    }
}

impl Default for GifFileType {
    fn default() -> Self {
        Self {
            SWidth: 0,
            SHeight: 0,
            SColorResolution: 0,
            SBackGroundColor: 0,
            AspectByte: 0,
            _padding0: [0; 7],
            SColorMap: ptr::null_mut(),
            ImageCount: 0,
            _padding1: [0; 4],
            Image: GifImageDesc::default(),
            SavedImages: ptr::null_mut(),
            ExtensionBlockCount: 0,
            _padding2: [0; 4],
            ExtensionBlocks: ptr::null_mut(),
            Error: 0,
            _padding3: [0; 4],
            UserData: ptr::null_mut(),
            Private: ptr::null_mut(),
        }
    }
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

// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub type InputFunc = Option<unsafe extern "C" fn(*mut GifFileType, *mut GifByteType, i32) -> i32>;
pub type OutputFunc =
    Option<unsafe extern "C" fn(*mut GifFileType, *const GifByteType, i32) -> i32>;

const _: () = {
    assert!(size_of::<GifColorType>() == 3);
    assert!(size_of::<ColorMapObject>() == 24);
    assert!(size_of::<GifImageDesc>() == 32);
    assert!(size_of::<ExtensionBlock>() == 24);
    assert!(size_of::<SavedImage>() == 56);
    assert!(size_of::<GifFileType>() == 120);
    assert!(size_of::<GraphicsControlBlock>() == 16);
    assert!(size_of::<GifHashTableType>() == 32768);
};
