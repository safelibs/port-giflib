#include <stdio.h>
#include <stdlib.h>

#include "gif_lib.h"

/*
 * Source-build reproducer for Imlib2's loader path, which uses the sequential
 * decode APIs rather than DGifSlurp.
 */

static void fail_with_code(const char *path, const char *step, int error_code) {
	fprintf(stderr, "%s: %s: %s\n", path, step, GifErrorString(error_code));
	exit(EXIT_FAILURE);
}

static void skip_extension_blocks(GifFileType *gif, const char *path) {
	GifByteType *extension = NULL;
	int ext_code = 0;

	if (DGifGetExtension(gif, &ext_code, &extension) == GIF_ERROR) {
		fail_with_code(path, "DGifGetExtension", gif->Error);
	}
	while (extension != NULL) {
		if (DGifGetExtensionNext(gif, &extension) == GIF_ERROR) {
			fail_with_code(path, "DGifGetExtensionNext", gif->Error);
		}
	}
}

int main(int argc, char **argv) {
	GifFileType *gif;
	GifRecordType record_type;
	int expected_width;
	int expected_height;
	int expected_images;
	int open_error = 0;
	int close_error = 0;
	int image_count = 0;

	if (argc != 5) {
		fprintf(stderr, "usage: %s <gif> <width> <height> <images>\n",
		        argv[0]);
		return EXIT_FAILURE;
	}

	expected_width = atoi(argv[2]);
	expected_height = atoi(argv[3]);
	expected_images = atoi(argv[4]);

	gif = DGifOpenFileName(argv[1], &open_error);
	if (gif == NULL) {
		fprintf(stderr, "%s: DGifOpenFileName failed: %s\n", argv[1],
		        GifErrorString(open_error));
		return EXIT_FAILURE;
	}
	if (gif->SWidth != expected_width || gif->SHeight != expected_height) {
		(void)DGifCloseFile(gif, NULL);
		fprintf(stderr, "%s: unexpected logical screen %dx%d\n", argv[1],
		        gif->SWidth, gif->SHeight);
		return EXIT_FAILURE;
	}

	for (;;) {
		GifPixelType *row;
		int row_index;

		if (DGifGetRecordType(gif, &record_type) == GIF_ERROR) {
			fail_with_code(argv[1], "DGifGetRecordType", gif->Error);
		}
		if (record_type == TERMINATE_RECORD_TYPE) {
			break;
		}
		if (record_type == EXTENSION_RECORD_TYPE) {
			skip_extension_blocks(gif, argv[1]);
			continue;
		}
		if (record_type != IMAGE_DESC_RECORD_TYPE) {
			(void)DGifCloseFile(gif, NULL);
			fprintf(stderr, "%s: unexpected record type %d\n", argv[1],
			        record_type);
			return EXIT_FAILURE;
		}
		if (DGifGetImageDesc(gif) == GIF_ERROR) {
			fail_with_code(argv[1], "DGifGetImageDesc", gif->Error);
		}
		row = (GifPixelType *)malloc((size_t)gif->Image.Width);
		if (row == NULL) {
			(void)DGifCloseFile(gif, NULL);
			fputs("out of memory\n", stderr);
			return EXIT_FAILURE;
		}
		for (row_index = 0; row_index < gif->Image.Height; row_index++) {
			if (DGifGetLine(gif, row, gif->Image.Width) == GIF_ERROR) {
				free(row);
				fail_with_code(argv[1], "DGifGetLine", gif->Error);
			}
		}
		free(row);
		image_count++;
	}

	if (image_count != expected_images) {
		(void)DGifCloseFile(gif, NULL);
		fprintf(stderr, "%s: expected %d images, got %d\n", argv[1],
		        expected_images, image_count);
		return EXIT_FAILURE;
	}
	if (DGifCloseFile(gif, &close_error) == GIF_ERROR) {
		fprintf(stderr, "%s: DGifCloseFile failed: %s\n", argv[1],
		        GifErrorString(close_error));
		return EXIT_FAILURE;
	}

	printf("%s %dx%d images=%d\n", argv[1], expected_width, expected_height,
	       expected_images);
	return EXIT_SUCCESS;
}
