#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "gif_lib.h"

/*
 * Source-build reproducer for ExactImage, which relies on the public file-name
 * wrappers plus the encode/decode entry points without any private headers.
 */

static void fail(const char *path, const char *message) {
	fprintf(stderr, "%s: %s\n", path, message);
	exit(EXIT_FAILURE);
}

int main(void) {
	GifPixelType pixels[2] = {0, 1};
	char path[] = "/tmp/exactimage-roundtrip-XXXXXX";
	ColorMapObject *color_map;
	GifFileType *writer;
	GifFileType *gif;
	int fd;
	int open_error = 0;
	int close_error = 0;

	fd = mkstemp(path);
	if (fd < 0) {
		perror("mkstemp");
		return EXIT_FAILURE;
	}
	if (close(fd) != 0) {
		perror("close");
		unlink(path);
		return EXIT_FAILURE;
	}

	writer = EGifOpenFileName(path, false, &open_error);
	if (writer == NULL) {
		fprintf(stderr, "%s: EGifOpenFileName failed: %s\n", path,
		        GifErrorString(open_error));
		unlink(path);
		return EXIT_FAILURE;
	}

	color_map = GifMakeMapObject(2, NULL);
	if (color_map == NULL) {
		(void)EGifCloseFile(writer, NULL);
		unlink(path);
		fail(path, "GifMakeMapObject failed");
	}
	color_map->Colors[0].Red = 0;
	color_map->Colors[0].Green = 0;
	color_map->Colors[0].Blue = 0;
	color_map->Colors[1].Red = 255;
	color_map->Colors[1].Green = 255;
	color_map->Colors[1].Blue = 255;

	if (EGifPutScreenDesc(writer, 2, 1, 1, 0, color_map) == GIF_ERROR) {
		GifFreeMapObject(color_map);
		fprintf(stderr, "%s: EGifPutScreenDesc failed: %s\n", path,
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		unlink(path);
		return EXIT_FAILURE;
	}
	if (EGifPutImageDesc(writer, 0, 0, 2, 1, false, NULL) == GIF_ERROR) {
		GifFreeMapObject(color_map);
		fprintf(stderr, "%s: EGifPutImageDesc failed: %s\n", path,
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		unlink(path);
		return EXIT_FAILURE;
	}
	if (EGifPutLine(writer, pixels, 2) == GIF_ERROR) {
		GifFreeMapObject(color_map);
		fprintf(stderr, "%s: EGifPutLine failed: %s\n", path,
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		unlink(path);
		return EXIT_FAILURE;
	}
	GifFreeMapObject(color_map);

	if (EGifCloseFile(writer, &close_error) == GIF_ERROR) {
		fprintf(stderr, "%s: EGifCloseFile failed: %s\n", path,
		        GifErrorString(close_error));
		unlink(path);
		return EXIT_FAILURE;
	}

	gif = DGifOpenFileName(path, &open_error);
	if (gif == NULL) {
		fprintf(stderr, "%s: DGifOpenFileName failed: %s\n", path,
		        GifErrorString(open_error));
		unlink(path);
		return EXIT_FAILURE;
	}
	if (DGifSlurp(gif) == GIF_ERROR) {
		fprintf(stderr, "%s: DGifSlurp failed: %s\n", path,
		        GifErrorString(gif->Error));
		(void)DGifCloseFile(gif, NULL);
		unlink(path);
		return EXIT_FAILURE;
	}
	if (gif->SWidth != 2 || gif->SHeight != 1 || gif->ImageCount != 1 ||
	    gif->SavedImages == NULL) {
		(void)DGifCloseFile(gif, NULL);
		unlink(path);
		fail(path, "unexpected roundtrip screen metrics");
	}
	if (gif->SavedImages[0].ImageDesc.Width != 2 ||
	    gif->SavedImages[0].ImageDesc.Height != 1 ||
	    gif->SavedImages[0].RasterBits == NULL ||
	    gif->SavedImages[0].RasterBits[0] != 0 ||
	    gif->SavedImages[0].RasterBits[1] != 1) {
		(void)DGifCloseFile(gif, NULL);
		unlink(path);
		fail(path, "unexpected roundtrip raster payload");
	}
	if (DGifCloseFile(gif, &close_error) == GIF_ERROR) {
		fprintf(stderr, "%s: DGifCloseFile failed: %s\n", path,
		        GifErrorString(close_error));
		unlink(path);
		return EXIT_FAILURE;
	}
	if (unlink(path) != 0) {
		perror("unlink");
		return EXIT_FAILURE;
	}

	puts("exactimage-memory-roundtrip ok");
	return EXIT_SUCCESS;
}
