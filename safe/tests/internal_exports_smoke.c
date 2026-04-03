#include <assert.h>
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "gif_hash.h"
#include "gif_lib.h"

void DGifDecreaseImageCounter(GifFileType *GifFile);
void FreeLastSavedImage(GifFileType *GifFile);
void *openbsd_reallocarray(void *optr, size_t nmemb, size_t size);

static void smoke_hash_helpers(void) {
	GifHashTableType *table = _InitHashTable();

	assert(table != NULL);
	assert(_ExistsHashTable(table, 0x12345U) == -1);
	_InsertHashTable(table, 0x12345U, 77);
	assert(_ExistsHashTable(table, 0x12345U) == 77);
	_ClearHashTable(table);
	assert(_ExistsHashTable(table, 0x12345U) == -1);
	free(table);
}

static void smoke_free_last_saved_image(void) {
	GifColorType colors[2] = {{0, 0, 0}, {255, 255, 255}};
	GifFileType gif = {0};
	SavedImage *image;
	static unsigned char ext_data[] = {'o', 'k'};

	gif.ImageCount = 1;
	gif.SavedImages = calloc(1, sizeof(*gif.SavedImages));
	assert(gif.SavedImages != NULL);

	image = &gif.SavedImages[0];
	image->ImageDesc.ColorMap = GifMakeMapObject(2, colors);
	assert(image->ImageDesc.ColorMap != NULL);
	image->RasterBits = malloc(4);
	assert(image->RasterBits != NULL);
	memset(image->RasterBits, 7, 4);
	assert(GifAddExtensionBlock(&image->ExtensionBlockCount,
	                           &image->ExtensionBlocks,
	                           COMMENT_EXT_FUNC_CODE,
	                           sizeof(ext_data),
	                           ext_data) == GIF_OK);

	FreeLastSavedImage(&gif);
	assert(gif.ImageCount == 0);
	free(gif.SavedImages);
}

static void smoke_dgif_decrease_image_counter(void) {
	GifFileType gif = {0};

	gif.ImageCount = 2;
	gif.SavedImages = calloc((size_t)gif.ImageCount, sizeof(*gif.SavedImages));
	assert(gif.SavedImages != NULL);
	gif.SavedImages[1].RasterBits = malloc(8);
	assert(gif.SavedImages[1].RasterBits != NULL);
	memset(gif.SavedImages[1].RasterBits, 1, 8);

	DGifDecreaseImageCounter(&gif);
	assert(gif.ImageCount == 1);
	free(gif.SavedImages);
}

static void smoke_openbsd_reallocarray(void) {
	int *values;

	errno = 0;
	values = openbsd_reallocarray(NULL, 4, sizeof(*values));
	assert(values != NULL);
	for (size_t i = 0; i < 4; i++) {
		values[i] = (int)(i * 3);
	}
	free(values);

	errno = 0;
	assert(openbsd_reallocarray(NULL, ((size_t)-1 / 2) + 1, 2) == NULL);
	assert(errno == ENOMEM);
}

int main(void) {
	smoke_hash_helpers();
	smoke_free_last_saved_image();
	smoke_dgif_decrease_image_counter();
	smoke_openbsd_reallocarray();
	return 0;
}
