use std::panic::{catch_unwind, AssertUnwindSafe};

#[cfg(target_os = "linux")]
#[link(name = "gif_legacy", kind = "static", modifiers = "+whole-archive")]
unsafe extern "C" {
    fn DGifOpen();
    fn EGifOpen();
    fn GifMakeMapObject();
    fn GifErrorString();
    fn GifDrawText8x8();
    fn _InitHashTable();
    fn openbsd_reallocarray();
    fn GifQuantizeBuffer();
}

#[cfg(not(target_os = "linux"))]
#[link(name = "gif_legacy", kind = "static")]
unsafe extern "C" {
    fn DGifOpen();
}

#[used]
static LINK_DGIF_OPEN: unsafe extern "C" fn() = DGifOpen;
#[used]
static LINK_EGIF_OPEN: unsafe extern "C" fn() = EGifOpen;
#[used]
static LINK_GIF_MAKE_MAP_OBJECT: unsafe extern "C" fn() = GifMakeMapObject;
#[used]
static LINK_GIF_ERROR_STRING: unsafe extern "C" fn() = GifErrorString;
#[used]
static LINK_GIF_DRAW_TEXT_8X8: unsafe extern "C" fn() = GifDrawText8x8;
#[used]
static LINK_INIT_HASH_TABLE: unsafe extern "C" fn() = _InitHashTable;
#[used]
static LINK_OPENBSD_REALLOCARRAY: unsafe extern "C" fn() = openbsd_reallocarray;
#[used]
static LINK_GIF_QUANTIZE_BUFFER: unsafe extern "C" fn() = GifQuantizeBuffer;

pub(crate) const LEGACY_BACKEND_ENABLED: bool = true;

#[allow(dead_code)]
pub(crate) fn catch_panic_or<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}
