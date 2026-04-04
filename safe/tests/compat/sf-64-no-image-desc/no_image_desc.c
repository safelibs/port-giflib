#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gif_lib.h"

/*
 * Runtime-dependent compatibility reproducer for the no-image-descriptor
 * malformed-input path. fbi and mtpaint use this validity split to decide
 * whether to keep native GIF handling, while tracker-extract, libextractor,
 * CamlImages, and GDAL depend on DGifSlurp rejecting the file cleanly.
 */

typedef struct {
	const GifByteType *data;
	size_t len;
	size_t pos;
} MemoryReader;

static int memory_read(GifFileType *gif, GifByteType *dst, int len) {
	MemoryReader *reader = (MemoryReader *)gif->UserData;
	size_t wanted = (size_t)len;
	size_t remaining = reader->len - reader->pos;

	if (wanted > remaining) {
		wanted = remaining;
	}
	if (wanted > 0) {
		memcpy(dst, reader->data + reader->pos, wanted);
		reader->pos += wanted;
	}

	return (int)wanted;
}

static void fail(const char *message) {
	fputs(message, stderr);
	fputc('\n', stderr);
	exit(EXIT_FAILURE);
}

int main(void) {
	static const GifByteType gif_data[] = {
	    'G', 'I', 'F', '8', '9', 'a',
	    0x01, 0x00, 0x01, 0x00,
	    0x80, 0x00, 0x00,
	    0x00, 0x00, 0x00,
	    0xff, 0xff, 0xff,
	    0x3b,
	};
	static const char expected_error[] = "No Image Descriptor detected";
	MemoryReader reader = {gif_data, sizeof(gif_data), 0};
	GifFileType *gif;
	const char *error_text;
	int open_error = 0;
	int close_error = 0;

	gif = DGifOpen(&reader, memory_read, &open_error);
	if (gif == NULL) {
		fail("DGifOpen unexpectedly rejected the malformed no-image fixture");
	}
	if (DGifSlurp(gif) != GIF_ERROR) {
		(void)DGifCloseFile(gif, NULL);
		fail("DGifSlurp accepted a GIF without any image descriptor");
	}
	if (gif->Error != D_GIF_ERR_NO_IMAG_DSCR) {
		fprintf(stderr, "unexpected DGifSlurp error code: %d\n", gif->Error);
		(void)DGifCloseFile(gif, NULL);
		return EXIT_FAILURE;
	}
	error_text = GifErrorString(gif->Error);
	if (error_text == NULL || strcmp(error_text, expected_error) != 0) {
		fail("GifErrorString did not report the no-image-descriptor failure");
	}
	if (gif->ImageCount != 0) {
		(void)DGifCloseFile(gif, NULL);
		fail("DGifSlurp should not manufacture SavedImages for malformed input");
	}
	if (DGifCloseFile(gif, &close_error) == GIF_ERROR) {
		fprintf(stderr, "DGifCloseFile failed after malformed decode: %s\n",
		        GifErrorString(close_error));
		return EXIT_FAILURE;
	}

	puts("sf-64-no-image-desc ok");
	return EXIT_SUCCESS;
}
