#include <stdio.h>
#include <stdlib.h>

#include "gif_lib.h"

/*
 * Metadata/frame-count reproducer reused by runtime consumers plus the
 * source-build gdal and libwebp local link-mode smokes. These consumers only
 * need DGifSlurp to report correct dimensions, frame counts, and interlace
 * state for representative public decode workloads.
 */

static void fail(const char *path, const char *message) {
	fprintf(stderr, "%s: %s\n", path, message);
	exit(EXIT_FAILURE);
}

int main(int argc, char **argv) {
	GifFileType *gif;
	int expected_width;
	int expected_height;
	int expected_images;
	int expected_interlace;
	int open_error = 0;
	int close_error = 0;

	if (argc != 6) {
		fprintf(stderr,
		        "usage: %s <gif> <width> <height> <images> <interlace>\n",
		        argv[0]);
		return EXIT_FAILURE;
	}

	expected_width = atoi(argv[2]);
	expected_height = atoi(argv[3]);
	expected_images = atoi(argv[4]);
	expected_interlace = atoi(argv[5]);

	gif = DGifOpenFileName(argv[1], &open_error);
	if (gif == NULL) {
		fprintf(stderr, "%s: DGifOpenFileName failed: %s\n", argv[1],
		        GifErrorString(open_error));
		return EXIT_FAILURE;
	}
	if (DGifSlurp(gif) == GIF_ERROR) {
		fprintf(stderr, "%s: DGifSlurp failed: %s\n", argv[1],
		        GifErrorString(gif->Error));
		(void)DGifCloseFile(gif, NULL);
		return EXIT_FAILURE;
	}
	if (gif->SWidth != expected_width || gif->SHeight != expected_height) {
		(void)DGifCloseFile(gif, NULL);
		fail(argv[1], "unexpected screen dimensions");
	}
	if (gif->ImageCount != expected_images || gif->SavedImages == NULL) {
		(void)DGifCloseFile(gif, NULL);
		fail(argv[1], "unexpected image count");
	}
	if ((gif->SavedImages[0].ImageDesc.Interlace ? 1 : 0) !=
	    expected_interlace) {
		(void)DGifCloseFile(gif, NULL);
		fail(argv[1], "unexpected first-frame interlace flag");
	}
	if (DGifCloseFile(gif, &close_error) == GIF_ERROR) {
		fprintf(stderr, "%s: DGifCloseFile failed: %s\n", argv[1],
		        GifErrorString(close_error));
		return EXIT_FAILURE;
	}

	printf("%s %dx%d images=%d interlace=%d\n", argv[1], expected_width,
	       expected_height, expected_images, expected_interlace);
	return EXIT_SUCCESS;
}
