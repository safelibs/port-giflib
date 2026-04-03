#include <stddef.h>

#include "gif_hash.h"
#include "gif_lib.h"

#define ASSERT_SIZE(type, expected) _Static_assert(sizeof(type) == (expected), #type " size")
#define ASSERT_OFFSET(type, field, expected) \
	_Static_assert(offsetof(type, field) == (expected), #type "." #field " offset")

ASSERT_SIZE(GifColorType, 3);

ASSERT_SIZE(ColorMapObject, 24);
ASSERT_OFFSET(ColorMapObject, ColorCount, 0);
ASSERT_OFFSET(ColorMapObject, BitsPerPixel, 4);
ASSERT_OFFSET(ColorMapObject, SortFlag, 8);
ASSERT_OFFSET(ColorMapObject, Colors, 16);

ASSERT_SIZE(GifImageDesc, 32);
ASSERT_OFFSET(GifImageDesc, Left, 0);
ASSERT_OFFSET(GifImageDesc, Top, 4);
ASSERT_OFFSET(GifImageDesc, Width, 8);
ASSERT_OFFSET(GifImageDesc, Height, 12);
ASSERT_OFFSET(GifImageDesc, Interlace, 16);
ASSERT_OFFSET(GifImageDesc, ColorMap, 24);

ASSERT_SIZE(ExtensionBlock, 24);
ASSERT_OFFSET(ExtensionBlock, ByteCount, 0);
ASSERT_OFFSET(ExtensionBlock, Bytes, 8);
ASSERT_OFFSET(ExtensionBlock, Function, 16);

ASSERT_SIZE(SavedImage, 56);
ASSERT_OFFSET(SavedImage, ImageDesc, 0);
ASSERT_OFFSET(SavedImage, RasterBits, 32);
ASSERT_OFFSET(SavedImage, ExtensionBlockCount, 40);
ASSERT_OFFSET(SavedImage, ExtensionBlocks, 48);

ASSERT_SIZE(GifFileType, 120);
ASSERT_OFFSET(GifFileType, SWidth, 0);
ASSERT_OFFSET(GifFileType, SHeight, 4);
ASSERT_OFFSET(GifFileType, SColorResolution, 8);
ASSERT_OFFSET(GifFileType, SBackGroundColor, 12);
ASSERT_OFFSET(GifFileType, AspectByte, 16);
ASSERT_OFFSET(GifFileType, SColorMap, 24);
ASSERT_OFFSET(GifFileType, ImageCount, 32);
ASSERT_OFFSET(GifFileType, Image, 40);
ASSERT_OFFSET(GifFileType, SavedImages, 72);
ASSERT_OFFSET(GifFileType, ExtensionBlockCount, 80);
ASSERT_OFFSET(GifFileType, ExtensionBlocks, 88);
ASSERT_OFFSET(GifFileType, Error, 96);
ASSERT_OFFSET(GifFileType, UserData, 104);
ASSERT_OFFSET(GifFileType, Private, 112);

ASSERT_SIZE(GraphicsControlBlock, 16);
ASSERT_OFFSET(GraphicsControlBlock, DisposalMode, 0);
ASSERT_OFFSET(GraphicsControlBlock, UserInputFlag, 4);
ASSERT_OFFSET(GraphicsControlBlock, DelayTime, 8);
ASSERT_OFFSET(GraphicsControlBlock, TransparentColor, 12);

ASSERT_SIZE(GifHashTableType, 32768);

int main(void) {
	return 0;
}
