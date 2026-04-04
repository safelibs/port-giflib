#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>

#include "gif_lib.h"

/*
 * Runtime-dependent compatibility reproducer for SF bug #103.
 * Downstream readers such as giftext, gif2webp, tracker-extract,
 * libextractor, CamlImages, and GDAL rely on DGifSlurp accepting
 * a valid GIF89 comment extension even when the comment is empty.
 */

typedef struct {
	GifByteType *data;
	size_t len;
	size_t cap;
} Buffer;

typedef struct {
	const GifByteType *data;
	size_t len;
	size_t pos;
} MemoryReader;

static void buffer_append(Buffer *buffer, const GifByteType *data, size_t len) {
	size_t needed = buffer->len + len;

	if (needed > buffer->cap) {
		size_t new_cap = buffer->cap == 0 ? 64 : buffer->cap;
		GifByteType *new_data;

		while (new_cap < needed) {
			new_cap *= 2;
		}
		new_data = (GifByteType *)realloc(buffer->data, new_cap);
		if (new_data == NULL) {
			fputs("out of memory\n", stderr);
			exit(EXIT_FAILURE);
		}
		buffer->data = new_data;
		buffer->cap = new_cap;
	}

	memcpy(buffer->data + buffer->len, data, len);
	buffer->len += len;
}

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

static int memory_write(GifFileType *gif, const GifByteType *src, int len) {
	Buffer *buffer = (Buffer *)gif->UserData;

	buffer_append(buffer, src, (size_t)len);
	return len;
}

static void fail(const char *message) {
	fputs(message, stderr);
	fputc('\n', stderr);
	exit(EXIT_FAILURE);
}

static bool contains_empty_comment_extension(const GifByteType *data, size_t len) {
	size_t i;

	for (i = 0; i + 2 < len; i++) {
		if (data[i] == 0x21 && data[i + 1] == 0xfe && data[i + 2] == 0x00) {
			return true;
		}
	}

	return false;
}

int main(void) {
	static const GifColorType colors[2] = {
	    {0, 0, 0},
	    {255, 255, 255},
	};
	Buffer encoded = {0};
	MemoryReader reader;
	ColorMapObject *color_map;
	GifFileType *writer;
	GifFileType *gif;
	GifPixelType pixel = 1;
	int open_error = 0;
	int close_error = 0;

	writer = EGifOpen(&encoded, memory_write, &open_error);
	if (writer == NULL) {
		fprintf(stderr, "EGifOpen failed: %s\n", GifErrorString(open_error));
		return EXIT_FAILURE;
	}
	color_map = GifMakeMapObject(2, colors);
	if (color_map == NULL) {
		(void)EGifCloseFile(writer, NULL);
		fail("GifMakeMapObject failed");
	}
	if (EGifPutScreenDesc(writer, 1, 1, 1, 0, color_map) == GIF_ERROR) {
		GifFreeMapObject(color_map);
		fprintf(stderr, "EGifPutScreenDesc failed: %s\n",
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		free(encoded.data);
		return EXIT_FAILURE;
	}
	GifFreeMapObject(color_map);
	if (EGifPutComment(writer, "") == GIF_ERROR) {
		fprintf(stderr, "EGifPutComment failed: %s\n",
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		free(encoded.data);
		return EXIT_FAILURE;
	}
	if (EGifPutImageDesc(writer, 0, 0, 1, 1, false, NULL) == GIF_ERROR) {
		fprintf(stderr, "EGifPutImageDesc failed: %s\n",
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		free(encoded.data);
		return EXIT_FAILURE;
	}
	if (EGifPutLine(writer, &pixel, 1) == GIF_ERROR) {
		fprintf(stderr, "EGifPutLine failed: %s\n",
		        GifErrorString(writer->Error));
		(void)EGifCloseFile(writer, NULL);
		free(encoded.data);
		return EXIT_FAILURE;
	}
	if (EGifCloseFile(writer, &close_error) == GIF_ERROR) {
		fprintf(stderr, "EGifCloseFile failed: %s\n",
		        GifErrorString(close_error));
		free(encoded.data);
		return EXIT_FAILURE;
	}
	if (!contains_empty_comment_extension(encoded.data, encoded.len)) {
		free(encoded.data);
		fail("EGifPutComment did not emit an empty GIF89 comment extension");
	}

	reader.data = encoded.data;
	reader.len = encoded.len;
	reader.pos = 0;
	gif = DGifOpen(&reader, memory_read, &open_error);
	if (gif == NULL) {
		free(encoded.data);
		fail("DGifOpen rejected a valid GIF89 empty-comment fixture");
	}
	if (DGifSlurp(gif) == GIF_ERROR) {
		fprintf(stderr, "DGifSlurp failed: %s\n",
		        GifErrorString(gif->Error));
		(void)DGifCloseFile(gif, NULL);
		free(encoded.data);
		return EXIT_FAILURE;
	}
	if (gif->SWidth != 1 || gif->SHeight != 1 || gif->ImageCount != 1) {
		free(encoded.data);
		fail("DGifSlurp returned unexpected screen metrics");
	}
	if (gif->SavedImages == NULL ||
	    gif->SavedImages[0].ImageDesc.Width != 1 ||
	    gif->SavedImages[0].ImageDesc.Height != 1) {
		free(encoded.data);
		fail("DGifSlurp did not preserve the single image descriptor");
	}
	if (gif->ExtensionBlockCount != 0 ||
	    gif->SavedImages[0].ExtensionBlockCount != 0) {
		free(encoded.data);
		fail("empty comment should not leave non-empty extension payloads");
	}
	if (DGifCloseFile(gif, &close_error) == GIF_ERROR) {
		fprintf(stderr, "DGifCloseFile failed: %s\n",
		        GifErrorString(close_error));
		free(encoded.data);
		return EXIT_FAILURE;
	}
	free(encoded.data);

	puts("sf-103-empty-comment ok");
	return EXIT_SUCCESS;
}
