#define _GNU_SOURCE

#include <ctype.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "gif_lib.h"

#define ARRAY_LEN(a) (sizeof(a) / sizeof((a)[0]))
#define KEY_LETTERS                                                           \
	"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO"                 \
	"PQRSTUVWXYZ!\"#$%&'()*+,-./:<=>?@[\\]^_`{|}~"
#define PRINTABLES ((int)(sizeof(KEY_LETTERS) - 1))
#define MAKE_PRINTABLE(c) (isprint(c) ? (c) : ' ')

typedef struct {
	unsigned char *data;
	size_t len;
	size_t cap;
} Buffer;

typedef struct {
	const unsigned char *data;
	size_t len;
	size_t pos;
} MemoryReader;

typedef struct {
	int current_place;
	long offset;
	char hex_form[49];
	char ascii_form[17];
} BlockPrinterState;

static void usage(void);
static void fatal(const char *fmt, ...);
static const char *gif_error_string_or_default(int error_code);
static void *xmalloc(size_t size);
static void *xcalloc(size_t count, size_t size);
static void *xrealloc(void *ptr, size_t size);
static FILE *xopen_memstream(char **data, size_t *len);
static void buffer_append(Buffer *buf, const void *data, size_t len);
static void buffer_free(Buffer *buf);
static Buffer read_stream(FILE *stream);
static Buffer read_input(const char *path);
static void write_stdout(const Buffer *buf);
static ColorMapObject *make_monochrome_map(void);
static void encode_monochrome_image(GifFileType *gif, int width, int height,
                                    const GifPixelType *pixels,
                                    bool force_gif89, const char *comment,
                                    bool use_pixels);
static void print_saved_image_pixels(FILE *out, const SavedImage *image);
static int memory_read(GifFileType *gif, GifByteType *dst, int len);
static int memory_write(GifFileType *gif, const GifByteType *src, int len);
static void free_screen_buffer(GifRowType *screen, int height);
static Buffer render_rgb(const Buffer *input);
static Buffer dump_map(const Buffer *input);
static void print_ext_block(FILE *out, BlockPrinterState *state,
                            const GifByteType *extension, bool reset);
static Buffer dump_text(const Buffer *input, const char *label);
static void visible_dump_buffer(FILE *out, const GifByteType *buf, int len);
static void dump_extensions(FILE *out, GifFileType *gif, int extension_count,
                            ExtensionBlock *extension_blocks);
static Buffer dump_icon(const Buffer *input);
static Buffer lowlevel_copy(const Buffer *input);
static Buffer repair_truncated_gif(const Buffer *input);
static void copy_extension_blocks(int source_count,
                                  const ExtensionBlock *source_blocks,
                                  int *target_count,
                                  ExtensionBlock **target_blocks);
static Buffer highlevel_copy(const Buffer *input, bool set_interlace,
                             bool interlace_value);
static Buffer rgb_to_gif(const Buffer *input, int exp_num_colors,
                         int width, int height);
static Buffer summarize_legacy_api_coverage(void);
static Buffer summarize_file_api_coverage(void);
static Buffer summarize_alloc_api_coverage(void);
static Buffer generate_foobar(void);
static Buffer generate_drawing(void);
static Buffer generate_wedge(void);
static bool malformed_input_is_rejected(const Buffer *input);
static void draw_text_line_with_public_font(SavedImage *image,
                                            const char *text,
                                            int foreground_index);

static void usage(void) {
	fprintf(stderr,
	        "usage: public_api_regress "
	        "{render|map|dump|icon|lowlevel-copy|repair|highlevel-copy|"
	        "interlace|rgb-to-gif|legacy|fileio|alloc|generate|"
	        "malformed} [args]\n");
	exit(EXIT_FAILURE);
}

static void fatal(const char *fmt, ...) {
	va_list ap;

	va_start(ap, fmt);
	(void)vfprintf(stderr, fmt, ap);
	va_end(ap);
	fputc('\n', stderr);
	exit(EXIT_FAILURE);
}

static const char *gif_error_string_or_default(int error_code) {
	const char *message = GifErrorString(error_code);

	return message != NULL ? message : "unknown GIF error";
}

static void *xmalloc(size_t size) {
	void *ptr = malloc(size);

	if (ptr == NULL) {
		fatal("out of memory");
	}

	return ptr;
}

static void *xcalloc(size_t count, size_t size) {
	void *ptr = calloc(count, size);

	if (ptr == NULL) {
		fatal("out of memory");
	}

	return ptr;
}

static void *xrealloc(void *ptr, size_t size) {
	void *new_ptr = realloc(ptr, size);

	if (new_ptr == NULL) {
		fatal("out of memory");
	}

	return new_ptr;
}

static FILE *xopen_memstream(char **data, size_t *len) {
	FILE *stream = open_memstream(data, len);

	if (stream == NULL) {
		fatal("open_memstream failed");
	}

	return stream;
}

static void buffer_append(Buffer *buf, const void *data, size_t len) {
	size_t needed;

	if (len == 0) {
		return;
	}

	needed = buf->len + len;
	if (needed > buf->cap) {
		size_t new_cap = buf->cap == 0 ? 4096 : buf->cap;

		while (new_cap < needed) {
			new_cap *= 2;
		}
		buf->data = xrealloc(buf->data, new_cap);
		buf->cap = new_cap;
	}

	memcpy(buf->data + buf->len, data, len);
	buf->len += len;
}

static void buffer_free(Buffer *buf) {
	free(buf->data);
	buf->data = NULL;
	buf->len = 0;
	buf->cap = 0;
}

static Buffer read_stream(FILE *stream) {
	Buffer buf = {0};
	unsigned char chunk[4096];
	size_t nread;

	while ((nread = fread(chunk, 1, sizeof(chunk), stream)) > 0) {
		buffer_append(&buf, chunk, nread);
	}

	if (ferror(stream)) {
		buffer_free(&buf);
		fatal("failed to read input");
	}

	return buf;
}

static Buffer read_input(const char *path) {
	Buffer buf;
	FILE *stream;

	if (path == NULL || strcmp(path, "-") == 0) {
		return read_stream(stdin);
	}

	stream = fopen(path, "rb");
	if (stream == NULL) {
		fatal("failed to open %s", path);
	}
	buf = read_stream(stream);
	if (fclose(stream) != 0) {
		buffer_free(&buf);
		fatal("failed to close %s", path);
	}

	return buf;
}

static void write_stdout(const Buffer *buf) {
	if (buf->len == 0) {
		return;
	}

	if (fwrite(buf->data, 1, buf->len, stdout) != buf->len) {
		fatal("failed to write output");
	}
}

static ColorMapObject *make_monochrome_map(void) {
	ColorMapObject *color_map = GifMakeMapObject(2, NULL);

	if (color_map == NULL) {
		fatal("GifMakeMapObject failed");
	}

	color_map->Colors[0].Red = 0;
	color_map->Colors[0].Green = 0;
	color_map->Colors[0].Blue = 0;
	color_map->Colors[1].Red = 255;
	color_map->Colors[1].Green = 255;
	color_map->Colors[1].Blue = 255;

	return color_map;
}

static void encode_monochrome_image(GifFileType *gif, int width, int height,
                                    const GifPixelType *pixels,
                                    bool force_gif89, const char *comment,
                                    bool use_pixels) {
	ColorMapObject *color_map = make_monochrome_map();
	int i;

	if (force_gif89) {
		EGifSetGifVersion(gif, true);
	}
	if (EGifPutScreenDesc(gif, width, height, 1, 0, color_map) ==
	    GIF_ERROR) {
		GifFreeMapObject(color_map);
		fatal("EGifPutScreenDesc failed: %s",
		      gif_error_string_or_default(gif->Error));
	}
	if (comment != NULL &&
	    EGifPutComment(gif, comment) == GIF_ERROR) {
		GifFreeMapObject(color_map);
		fatal("EGifPutComment failed: %s",
		      gif_error_string_or_default(gif->Error));
	}
	if (EGifPutImageDesc(gif, 0, 0, width, height, false, NULL) ==
	    GIF_ERROR) {
		GifFreeMapObject(color_map);
		fatal("EGifPutImageDesc failed: %s",
		      gif_error_string_or_default(gif->Error));
	}

	if (use_pixels) {
		for (i = 0; i < width * height; i++) {
			if (EGifPutPixel(gif, pixels[i]) == GIF_ERROR) {
				GifFreeMapObject(color_map);
				fatal("EGifPutPixel failed: %s",
				      gif_error_string_or_default(gif->Error));
			}
		}
	} else {
		GifPixelType *row = xmalloc((size_t)width * sizeof(GifPixelType));

		for (i = 0; i < height; i++) {
			memcpy(row, pixels + i * width,
			       (size_t)width * sizeof(GifPixelType));
			if (EGifPutLine(gif, row, width) == GIF_ERROR) {
				free(row);
				GifFreeMapObject(color_map);
				fatal("EGifPutLine failed: %s",
				      gif_error_string_or_default(gif->Error));
			}
		}
		free(row);
	}

	GifFreeMapObject(color_map);
}

static void print_saved_image_pixels(FILE *out, const SavedImage *image) {
	int count = image->ImageDesc.Width * image->ImageDesc.Height;
	int i;

	for (i = 0; i < count; i++) {
		(void)fprintf(out, "%d", image->RasterBits[i]);
	}
}

static int memory_read(GifFileType *gif, GifByteType *dst, int len) {
	MemoryReader *reader = gif->UserData;
	size_t remaining = reader->len - reader->pos;
	size_t wanted = (size_t)len;

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
	Buffer *out = gif->UserData;

	buffer_append(out, src, (size_t)len);
	return len;
}

static void free_screen_buffer(GifRowType *screen, int height) {
	int i;

	if (screen == NULL) {
		return;
	}

	for (i = 0; i < height; i++) {
		free(screen[i]);
	}
	free(screen);
}

static Buffer render_rgb(const Buffer *input) {
	static const int interlaced_offset[] = {0, 4, 2, 1};
	static const int interlaced_jumps[] = {8, 8, 4, 2};
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif;
	GifRowType *screen = NULL;
	ColorMapObject *color_map;
	Buffer out = {0};
	int error_code = 0;
	int i;
	int size;
	GifRecordType record_type;
	GifByteType *extension;
	int ext_code;

	gif = DGifOpen(&reader, memory_read, &error_code);
	if (gif == NULL) {
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (gif->SWidth <= 0 || gif->SHeight <= 0) {
		(void)DGifCloseFile(gif, NULL);
		fatal("image width or height is zero");
	}

	screen = xcalloc((size_t)gif->SHeight, sizeof(*screen));
	size = gif->SWidth * (int)sizeof(GifPixelType);
	screen[0] = xmalloc((size_t)size);
	memset(screen[0], gif->SBackGroundColor, (size_t)size);
	for (i = 1; i < gif->SHeight; i++) {
		screen[i] = xmalloc((size_t)size);
		memcpy(screen[i], screen[0], (size_t)size);
	}

	do {
		if (DGifGetRecordType(gif, &record_type) == GIF_ERROR) {
			error_code = gif->Error;
			free_screen_buffer(screen, gif->SHeight);
			(void)DGifCloseFile(gif, NULL);
			fatal("DGifGetRecordType failed: %s",
			      gif_error_string_or_default(error_code));
		}

		switch (record_type) {
		case IMAGE_DESC_RECORD_TYPE: {
			int row = 0;
			int col = 0;
			int width = 0;
			int height = 0;

			if (DGifGetImageDesc(gif) == GIF_ERROR) {
				error_code = gif->Error;
				free_screen_buffer(screen, gif->SHeight);
				(void)DGifCloseFile(gif, NULL);
				fatal("DGifGetImageDesc failed: %s",
				      gif_error_string_or_default(error_code));
			}

			row = gif->Image.Top;
			col = gif->Image.Left;
			width = gif->Image.Width;
			height = gif->Image.Height;
			if (gif->Image.Left + gif->Image.Width > gif->SWidth ||
			    gif->Image.Top + gif->Image.Height >
			        gif->SHeight) {
				free_screen_buffer(screen, gif->SHeight);
				(void)DGifCloseFile(gif, NULL);
				fatal("image is not confined to screen");
			}

			if (gif->Image.Interlace) {
				int pass;

				for (pass = 0; pass < 4; pass++) {
					int j;

					for (j = row + interlaced_offset[pass];
					     j < row + height;
					     j += interlaced_jumps[pass]) {
						if (DGifGetLine(
						        gif,
						        &screen[j][col],
						        width) ==
						    GIF_ERROR) {
							error_code =
							    gif->Error;
							free_screen_buffer(
							    screen,
							    gif->SHeight);
							(void)DGifCloseFile(
							    gif, NULL);
							fatal("DGifGetLine "
							      "failed: %s",
							      gif_error_string_or_default(
							          error_code));
						}
					}
				}
			} else {
				for (i = 0; i < height; i++) {
					if (DGifGetLine(
					        gif, &screen[row++][col],
					        width) == GIF_ERROR) {
						error_code = gif->Error;
						free_screen_buffer(screen,
						                   gif->SHeight);
						(void)DGifCloseFile(gif,
						                    NULL);
						fatal("DGifGetLine failed: %s",
						      gif_error_string_or_default(
						          error_code));
					}
				}
			}
			break;
		}
		case EXTENSION_RECORD_TYPE:
			if (DGifGetExtension(gif, &ext_code, &extension) ==
			    GIF_ERROR) {
				error_code = gif->Error;
				free_screen_buffer(screen, gif->SHeight);
				(void)DGifCloseFile(gif, NULL);
				fatal("DGifGetExtension failed: %s",
				      gif_error_string_or_default(error_code));
			}
			while (extension != NULL) {
				if (DGifGetExtensionNext(gif, &extension) ==
				    GIF_ERROR) {
					error_code = gif->Error;
					free_screen_buffer(screen,
					                   gif->SHeight);
					(void)DGifCloseFile(gif, NULL);
					fatal("DGifGetExtensionNext failed: "
					      "%s",
					      gif_error_string_or_default(
					          error_code));
				}
			}
			break;
		case TERMINATE_RECORD_TYPE:
			break;
		default:
			break;
		}
	} while (record_type != TERMINATE_RECORD_TYPE);

	color_map = gif->Image.ColorMap != NULL ? gif->Image.ColorMap
	                                        : gif->SColorMap;
	if (color_map == NULL) {
		free_screen_buffer(screen, gif->SHeight);
		(void)DGifCloseFile(gif, NULL);
		fatal("image does not have a colormap");
	}
	if (gif->SBackGroundColor < 0 ||
	    gif->SBackGroundColor >= color_map->ColorCount) {
		free_screen_buffer(screen, gif->SHeight);
		(void)DGifCloseFile(gif, NULL);
		fatal("background color is out of range");
	}

	for (i = 0; i < gif->SHeight; i++) {
		int j;
		unsigned char *row = xmalloc((size_t)gif->SWidth * 3);

		for (j = 0; j < gif->SWidth; j++) {
			GifPixelType pixel = screen[i][j];
			GifColorType color;

			if (pixel >= color_map->ColorCount) {
				free(row);
				free_screen_buffer(screen, gif->SHeight);
				(void)DGifCloseFile(gif, NULL);
				fatal("pixel index %u is out of colormap range",
				      pixel);
			}

			color = color_map->Colors[pixel];
			row[j * 3] = color.Red;
			row[j * 3 + 1] = color.Green;
			row[j * 3 + 2] = color.Blue;
		}
		buffer_append(&out, row, (size_t)gif->SWidth * 3);
		free(row);
	}

	free_screen_buffer(screen, gif->SHeight);
	if (DGifCloseFile(gif, &error_code) == GIF_ERROR) {
		buffer_free(&out);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	return out;
}

static Buffer dump_map(const Buffer *input) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif;
	Buffer out = {0};
	char *text = NULL;
	size_t text_len = 0;
	FILE *stream;
	int error_code = 0;
	int i;

	stream = open_memstream(&text, &text_len);
	if (stream == NULL) {
		fatal("open_memstream failed");
	}

	gif = DGifOpen(&reader, memory_read, &error_code);
	if (gif == NULL) {
		fclose(stream);
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (gif->SColorMap == NULL) {
		(void)DGifCloseFile(gif, NULL);
		fclose(stream);
		fatal("image does not have a global colormap");
	}

	for (i = 0; i < gif->SColorMap->ColorCount; i++) {
		fprintf(stream, "%3d %3d %3d %3d\n", i,
		        gif->SColorMap->Colors[i].Red,
		        gif->SColorMap->Colors[i].Green,
		        gif->SColorMap->Colors[i].Blue);
	}

	if (DGifCloseFile(gif, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (fclose(stream) != 0) {
		free(text);
		fatal("failed to finalize colormap dump");
	}

	out.data = (unsigned char *)text;
	out.len = text_len;
	out.cap = text_len;
	return out;
}

static void print_ext_block(FILE *out, BlockPrinterState *state,
                            const GifByteType *extension, bool reset) {
	static const char hex_digits[] = "0123456789abcdef";
	int i;
	int len;

	if (reset || extension == NULL) {
		if (extension == NULL) {
			if (state->current_place > 0) {
				state->hex_form[state->current_place * 3] = '\0';
				state->ascii_form[state->current_place] = '\0';
				fprintf(out, "\n%05lx: %-49s  %-17s\n",
				        state->offset, state->hex_form,
				        state->ascii_form);
				return;
			}
			fprintf(out, "\n");
		}
		state->current_place = 0;
		state->offset = 0;
	}
	if (extension == NULL) {
		return;
	}

	len = extension[0];
	for (i = 1; i <= len; i++) {
		/* Match giftext's historical truncation exactly so the
		 * regression output stays byte-for-byte compatible.
		 */
		state->hex_form[state->current_place * 3] = ' ';
		state->hex_form[state->current_place * 3 + 1] =
		    hex_digits[(extension[i] >> 4) & 0x0f];
		state->hex_form[state->current_place * 3 + 2] = '\0';
		state->ascii_form[state->current_place] =
		    MAKE_PRINTABLE(extension[i]);
		state->ascii_form[state->current_place + 1] = '\0';
		if (++state->current_place == 16) {
			state->hex_form[state->current_place * 3] = '\0';
			state->ascii_form[state->current_place] = '\0';
			fprintf(out, "\n%05lx: %-49s  %-17s",
			        state->offset, state->hex_form,
			        state->ascii_form);
			state->offset += 16;
			state->current_place = 0;
		}
	}
}

static Buffer dump_text(const Buffer *input, const char *label) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif;
	Buffer out = {0};
	char *text = NULL;
	size_t text_len = 0;
	FILE *stream;
	int error_code = 0;
	int image_num = 1;
	GifRecordType record_type;

	stream = open_memstream(&text, &text_len);
	if (stream == NULL) {
		fatal("open_memstream failed");
	}

	gif = DGifOpen(&reader, memory_read, &error_code);
	if (gif == NULL) {
		fclose(stream);
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	fprintf(stream, "\n%s:\n\n\tScreen Size - Width = %d, Height = %d.\n",
	        label, gif->SWidth, gif->SHeight);
	fprintf(stream,
	        "\tColorResolution = %d, BitsPerPixel = %d, BackGround = %d, "
	        "Aspect = %d.\n",
	        gif->SColorResolution,
	        gif->SColorMap != NULL ? gif->SColorMap->BitsPerPixel : 0,
	        gif->SBackGroundColor, gif->AspectByte);
	fprintf(stream, gif->SColorMap != NULL ? "\tHas Global Color Map.\n\n"
	                                       : "\tNo Global Color Map.\n\n");

	do {
		if (DGifGetRecordType(gif, &record_type) == GIF_ERROR) {
			error_code = gif->Error;
			(void)DGifCloseFile(gif, NULL);
			fclose(stream);
			free(text);
			fatal("DGifGetRecordType failed: %s",
			      gif_error_string_or_default(error_code));
		}

		switch (record_type) {
		case IMAGE_DESC_RECORD_TYPE: {
			int code_size = 0;
			GifByteType *code_block = NULL;

			if (DGifGetImageDesc(gif) == GIF_ERROR) {
				error_code = gif->Error;
				(void)DGifCloseFile(gif, NULL);
				fclose(stream);
				free(text);
				fatal("DGifGetImageDesc failed: %s",
				      gif_error_string_or_default(error_code));
			}

			fprintf(stream,
			        "\nImage #%d:\n\n\tImage Size - Left = %d, Top = %d, "
			        "Width = %d, Height = %d.\n",
			        image_num++, gif->Image.Left, gif->Image.Top,
			        gif->Image.Width, gif->Image.Height);
			fprintf(stream, "\tImage is %s",
			        gif->Image.Interlace ? "Interlaced"
			                             : "Non Interlaced");
			if (gif->Image.ColorMap != NULL) {
				fprintf(stream, ", BitsPerPixel = %d.\n",
				        gif->Image.ColorMap->BitsPerPixel);
			} else {
				fprintf(stream, ".\n");
			}
			fprintf(stream, gif->Image.ColorMap != NULL
			                    ? "\tImage Has Color Map.\n"
			                    : "\tNo Image Color Map.\n");

			if (DGifGetCode(gif, &code_size, &code_block) ==
			    GIF_ERROR) {
				error_code = gif->Error;
				(void)DGifCloseFile(gif, NULL);
				fclose(stream);
				free(text);
				fatal("DGifGetCode failed: %s",
				      gif_error_string_or_default(error_code));
			}
			while (code_block != NULL) {
				if (DGifGetCodeNext(gif, &code_block) ==
				    GIF_ERROR) {
					error_code = gif->Error;
					(void)DGifCloseFile(gif, NULL);
					fclose(stream);
					free(text);
					fatal("DGifGetCodeNext failed: %s",
					      gif_error_string_or_default(
					          error_code));
				}
			}
			break;
		}
		case EXTENSION_RECORD_TYPE: {
			int ext_code = 0;
			GifByteType *extension = NULL;
			BlockPrinterState state = {0};

			if (DGifGetExtension(gif, &ext_code, &extension) ==
			    GIF_ERROR) {
				error_code = gif->Error;
				(void)DGifCloseFile(gif, NULL);
				fclose(stream);
				free(text);
				fatal("DGifGetExtension failed: %s",
				      gif_error_string_or_default(error_code));
			}

			fputc('\n', stream);
			switch (ext_code) {
			case COMMENT_EXT_FUNC_CODE:
				fprintf(stream, "GIF89 comment");
				break;
			case GRAPHICS_EXT_FUNC_CODE:
				fprintf(stream, "GIF89 graphics control");
				break;
			case PLAINTEXT_EXT_FUNC_CODE:
				fprintf(stream, "GIF89 plaintext");
				break;
			case APPLICATION_EXT_FUNC_CODE:
				fprintf(stream, "GIF89 application block");
				break;
			default:
				fprintf(stream,
				        "Extension record of unknown type");
				break;
			}
			fprintf(stream, " (Ext Code = %d [%c]):\n", ext_code,
			        MAKE_PRINTABLE(ext_code));
			print_ext_block(stream, &state, extension, true);

			if (ext_code == GRAPHICS_EXT_FUNC_CODE) {
				GraphicsControlBlock gcb;

				if (extension == NULL) {
					(void)DGifCloseFile(gif, NULL);
					fclose(stream);
					free(text);
					fatal("invalid graphics control block");
				}
				if (DGifExtensionToGCB(extension[0],
				                       extension + 1, &gcb) ==
				    GIF_ERROR) {
					error_code = gif->Error;
					(void)DGifCloseFile(gif, NULL);
					fclose(stream);
					free(text);
					fatal("DGifExtensionToGCB failed: %s",
					      gif_error_string_or_default(
					          error_code));
				}
				fprintf(stream, "\tDisposal Mode: %d\n",
				        gcb.DisposalMode);
				fprintf(stream, "\tUser Input Flag: %d\n",
				        gcb.UserInputFlag);
				fprintf(stream, "\tTransparency on: %s\n",
				        gcb.TransparentColor != -1 ? "yes"
				                                   : "no");
				fprintf(stream, "\tDelayTime: %d\n",
				        gcb.DelayTime);
				fprintf(stream, "\tTransparent Index: %d\n",
				        gcb.TransparentColor);
			}

			for (;;) {
				if (DGifGetExtensionNext(gif, &extension) ==
				    GIF_ERROR) {
					error_code = gif->Error;
					(void)DGifCloseFile(gif, NULL);
					fclose(stream);
					free(text);
					fatal("DGifGetExtensionNext failed: %s",
					      gif_error_string_or_default(
					          error_code));
				}
				if (extension == NULL) {
					break;
				}
				print_ext_block(stream, &state, extension,
				                false);
			}
			break;
		}
		case TERMINATE_RECORD_TYPE:
			break;
		default:
			break;
		}
	} while (record_type != TERMINATE_RECORD_TYPE);

	if (DGifCloseFile(gif, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	fprintf(stream, "\nGIF file terminated normally.\n");
	if (fclose(stream) != 0) {
		free(text);
		fatal("failed to finalize textual dump");
	}

	out.data = (unsigned char *)text;
	out.len = text_len;
	out.cap = text_len;
	return out;
}

static void visible_dump_buffer(FILE *out, const GifByteType *buf, int len) {
	const GifByteType *cp;

	for (cp = buf; cp < buf + len; cp++) {
		if (isprint((int)(*cp)) || *cp == ' ') {
			fputc(*cp, out);
		} else if (*cp == '\n') {
			fputs("\\n", out);
		} else if (*cp == '\r') {
			fputs("\\r", out);
		} else if (*cp == '\b') {
			fputs("\\b", out);
		} else if (*cp < ' ') {
			fputc('\\', out);
			fputc('^', out);
			fputc('@' + *cp, out);
		} else {
			fprintf(out, "\\0x%02x", *cp);
		}
	}
}

static void dump_extensions(FILE *out, GifFileType *gif, int extension_count,
                            ExtensionBlock *extension_blocks) {
	ExtensionBlock *ep;

	for (ep = extension_blocks; ep < extension_blocks + extension_count;
	     ep++) {
		bool last = (ep - extension_blocks ==
		             (extension_count - 1));

		if (ep->Function == COMMENT_EXT_FUNC_CODE) {
			fprintf(out, "comment\n");
			visible_dump_buffer(out, ep->Bytes, ep->ByteCount);
			fputc('\n', out);
			while (!last &&
			       ep[1].Function == CONTINUE_EXT_FUNC_CODE) {
				++ep;
				last = (ep - extension_blocks ==
				        (extension_count - 1));
				visible_dump_buffer(out, ep->Bytes,
				                    ep->ByteCount);
				fputc('\n', out);
			}
			fprintf(out, "end\n\n");
		} else if (ep->Function == PLAINTEXT_EXT_FUNC_CODE) {
			fprintf(out, "plaintext\n");
			visible_dump_buffer(out, ep->Bytes, ep->ByteCount);
			fputc('\n', out);
			while (!last &&
			       ep[1].Function == CONTINUE_EXT_FUNC_CODE) {
				++ep;
				last = (ep - extension_blocks ==
				        (extension_count - 1));
				visible_dump_buffer(out, ep->Bytes,
				                    ep->ByteCount);
				fputc('\n', out);
			}
			fprintf(out, "end\n\n");
		} else if (ep->Function == GRAPHICS_EXT_FUNC_CODE) {
			GraphicsControlBlock gcb;

			fprintf(out, "graphics control\n");
			if (DGifExtensionToGCB(ep->ByteCount, ep->Bytes,
			                       &gcb) == GIF_ERROR) {
				fatal("invalid graphics control block");
			}
			fprintf(out, "\tdisposal mode %d\n",
			        gcb.DisposalMode);
			fprintf(out, "\tuser input flag %s\n",
			        gcb.UserInputFlag ? "on" : "off");
			fprintf(out, "\tdelay %d\n", gcb.DelayTime);
			fprintf(out, "\ttransparent index %d\n",
			        gcb.TransparentColor);
			fprintf(out, "end\n\n");
		} else if (!last &&
		           ep->Function == APPLICATION_EXT_FUNC_CODE &&
		           ep->ByteCount >= 11 &&
		           (ep + 1)->ByteCount >= 3 &&
		           memcmp(ep->Bytes, "NETSCAPE2.0", 11) == 0) {
			unsigned char *params = (++ep)->Bytes;
			unsigned int loop_count =
			    params[1] | (params[2] << 8);

			fprintf(out, "netscape loop %u\n\n", loop_count);
		} else {
			fprintf(out, "extension 0x%02x\n", ep->Function);
			visible_dump_buffer(out, ep->Bytes, ep->ByteCount);
			while (!last &&
			       ep[1].Function == CONTINUE_EXT_FUNC_CODE) {
				++ep;
				last = (ep - extension_blocks ==
				        (extension_count - 1));
				visible_dump_buffer(out, ep->Bytes,
				                    ep->ByteCount);
				fputc('\n', out);
			}
			fprintf(out, "end\n\n");
		}
	}

	(void)gif;
}

static Buffer dump_icon(const Buffer *input) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif;
	Buffer out = {0};
	char *text = NULL;
	size_t text_len = 0;
	FILE *stream;
	int error_code = 0;
	int im;
	int i;
	int j;
	int color_count = 0;

	stream = open_memstream(&text, &text_len);
	if (stream == NULL) {
		fatal("open_memstream failed");
	}

	gif = DGifOpen(&reader, memory_read, &error_code);
	if (gif == NULL) {
		fclose(stream);
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (DGifSlurp(gif) == GIF_ERROR) {
		error_code = gif->Error;
		(void)DGifCloseFile(gif, NULL);
		fclose(stream);
		fatal("DGifSlurp failed: %s",
		      gif_error_string_or_default(error_code));
	}

	fprintf(stream, "screen width %d\nscreen height %d\n", gif->SWidth,
	        gif->SHeight);
	fprintf(stream,
	        "screen colors %d\nscreen background %d\npixel aspect byte %u\n\n",
	        1 << gif->SColorResolution, gif->SBackGroundColor,
	        (unsigned)gif->AspectByte);

	if (gif->SColorMap != NULL) {
		fprintf(stream, "screen map\n");
		fprintf(stream, "\tsort flag %s\n",
		        gif->SColorMap->SortFlag ? "on" : "off");
		for (i = 0; i < gif->SColorMap->ColorCount; i++) {
			if (gif->SColorMap->ColorCount < PRINTABLES) {
				fprintf(stream,
				        "\trgb %03d %03d %03d is %c\n",
				        gif->SColorMap->Colors[i].Red,
				        gif->SColorMap->Colors[i].Green,
				        gif->SColorMap->Colors[i].Blue,
				        KEY_LETTERS[i]);
			} else {
				fprintf(stream, "\trgb %03d %03d %03d\n",
				        gif->SColorMap->Colors[i].Red,
				        gif->SColorMap->Colors[i].Green,
				        gif->SColorMap->Colors[i].Blue);
			}
		}
		fprintf(stream, "end\n\n");
	}

	for (im = 0; im < gif->ImageCount; im++) {
		SavedImage *image = &gif->SavedImages[im];

		dump_extensions(stream, gif, image->ExtensionBlockCount,
		                image->ExtensionBlocks);

		fprintf(stream, "image # %d\nimage left %d\nimage top %d\n",
		        im + 1, image->ImageDesc.Left, image->ImageDesc.Top);
		if (image->ImageDesc.Interlace) {
			fprintf(stream, "image interlaced\n");
		}

		if (image->ImageDesc.ColorMap != NULL) {
			fprintf(stream, "image map\n");
			fprintf(stream, "\tsort flag %s\n",
			        image->ImageDesc.ColorMap->SortFlag
			            ? "on"
			            : "off");
			if (image->ImageDesc.ColorMap->ColorCount <
			    PRINTABLES) {
				for (i = 0;
				     i < image->ImageDesc.ColorMap->ColorCount;
				     i++) {
					fprintf(stream,
					        "\trgb %03d %03d %03d is %c\n",
					        image->ImageDesc.ColorMap
					            ->Colors[i]
					            .Red,
					        image->ImageDesc.ColorMap
					            ->Colors[i]
					            .Green,
					        image->ImageDesc.ColorMap
					            ->Colors[i]
					            .Blue,
					        KEY_LETTERS[i]);
				}
			} else {
				for (i = 0;
				     i < image->ImageDesc.ColorMap->ColorCount;
				     i++) {
					fprintf(stream,
					        "\trgb %03d %03d %03d\n",
					        image->ImageDesc.ColorMap
					            ->Colors[i]
					            .Red,
					        image->ImageDesc.ColorMap
					            ->Colors[i]
					            .Green,
					        image->ImageDesc.ColorMap
					            ->Colors[i]
					            .Blue);
				}
			}
			fprintf(stream, "end\n\n");
		}

		if (image->ImageDesc.ColorMap != NULL) {
			color_count = image->ImageDesc.ColorMap->ColorCount;
		} else if (gif->SColorMap != NULL) {
			color_count = gif->SColorMap->ColorCount;
		}

		if (color_count < PRINTABLES) {
			fprintf(stream, "image bits %d by %d\n",
			        image->ImageDesc.Width,
			        image->ImageDesc.Height);
		} else {
			fprintf(stream, "image bits %d by %d hex\n",
			        image->ImageDesc.Width,
			        image->ImageDesc.Height);
		}
		for (i = 0; i < image->ImageDesc.Height; i++) {
			for (j = 0; j < image->ImageDesc.Width; j++) {
				GifByteType ch = image->RasterBits
				    [i * image->ImageDesc.Width + j];

				if (color_count < PRINTABLES &&
				    ch < PRINTABLES) {
					fputc(KEY_LETTERS[ch], stream);
				} else {
					fprintf(stream, "%02x", ch);
				}
			}
			fputc('\n', stream);
		}
		fputc('\n', stream);
	}

	dump_extensions(stream, gif, gif->ExtensionBlockCount,
	                gif->ExtensionBlocks);
	fprintf(stream,
	        "# The following sets edit modes for GNU EMACS\n"
	        "# Local Variables:\n"
	        "# mode:picture\n"
	        "# truncate-lines:t\n"
	        "# End:\n");

	if (DGifCloseFile(gif, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (fclose(stream) != 0) {
		free(text);
		fatal("failed to finalize icon dump");
	}

	out.data = (unsigned char *)text;
	out.len = text_len;
	out.cap = text_len;
	return out;
}

static Buffer lowlevel_copy(const Buffer *input) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif_in;
	GifFileType *gif_out;
	GifRecordType record_type;
	Buffer out = {0};
	GifByteType *code_block = NULL;
	GifByteType *extension = NULL;
	int error_code = 0;
	int code_size = 0;
	int ext_code = 0;

	gif_in = DGifOpen(&reader, memory_read, &error_code);
	if (gif_in == NULL) {
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	gif_out = EGifOpen(&out, memory_write, &error_code);
	if (gif_out == NULL) {
		(void)DGifCloseFile(gif_in, NULL);
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	if (EGifPutScreenDesc(gif_out, gif_in->SWidth, gif_in->SHeight,
	                      gif_in->SColorResolution,
	                      gif_in->SBackGroundColor,
	                      gif_in->SColorMap) == GIF_ERROR) {
		(void)DGifCloseFile(gif_in, NULL);
		(void)EGifCloseFile(gif_out, NULL);
		buffer_free(&out);
		fatal("EGifPutScreenDesc failed: %s",
		      gif_error_string_or_default(gif_out->Error));
	}

	do {
		if (DGifGetRecordType(gif_in, &record_type) == GIF_ERROR) {
			(void)DGifCloseFile(gif_in, NULL);
			(void)EGifCloseFile(gif_out, NULL);
			buffer_free(&out);
			fatal("DGifGetRecordType failed: %s",
			      gif_error_string_or_default(gif_in->Error));
		}

		switch (record_type) {
		case IMAGE_DESC_RECORD_TYPE:
			if (DGifGetImageDesc(gif_in) == GIF_ERROR) {
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("DGifGetImageDesc failed: %s",
				      gif_error_string_or_default(
				          gif_in->Error));
			}
			if (EGifPutImageDesc(gif_out, gif_in->Image.Left,
			                     gif_in->Image.Top,
			                     gif_in->Image.Width,
			                     gif_in->Image.Height,
			                     gif_in->Image.Interlace,
			                     gif_in->Image.ColorMap) ==
			    GIF_ERROR) {
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("EGifPutImageDesc failed: %s",
				      gif_error_string_or_default(
				          gif_out->Error));
			}
			if (DGifGetCode(gif_in, &code_size, &code_block) ==
			        GIF_ERROR ||
			    EGifPutCode(gif_out, code_size, code_block) ==
			        GIF_ERROR) {
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("low-level code copy failed");
			}
			while (code_block != NULL) {
				if (DGifGetCodeNext(gif_in, &code_block) ==
				        GIF_ERROR ||
				    EGifPutCodeNext(gif_out, code_block) ==
				        GIF_ERROR) {
					(void)DGifCloseFile(gif_in, NULL);
					(void)EGifCloseFile(gif_out, NULL);
					buffer_free(&out);
					fatal("low-level code block copy "
					      "failed");
				}
			}
			break;
		case EXTENSION_RECORD_TYPE:
			if (DGifGetExtension(gif_in, &ext_code, &extension) ==
			        GIF_ERROR ||
			    extension == NULL) {
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("DGifGetExtension failed");
			}
			if (EGifPutExtensionLeader(gif_out, ext_code) ==
			        GIF_ERROR ||
			    EGifPutExtensionBlock(gif_out, extension[0],
			                          extension + 1) ==
			        GIF_ERROR) {
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("EGifPutExtension failed");
			}
			while (extension != NULL) {
				if (DGifGetExtensionNext(gif_in, &extension) ==
				    GIF_ERROR) {
					(void)DGifCloseFile(gif_in, NULL);
					(void)EGifCloseFile(gif_out, NULL);
					buffer_free(&out);
					fatal("DGifGetExtensionNext failed");
				}
				if (extension != NULL) {
					if (EGifPutExtensionBlock(
					        gif_out, extension[0],
					        extension + 1) ==
					    GIF_ERROR) {
						(void)DGifCloseFile(gif_in,
						                    NULL);
						(void)EGifCloseFile(gif_out,
						                    NULL);
						buffer_free(&out);
						fatal("EGifPutExtensionBlock "
						      "failed");
					}
				}
			}
			if (EGifPutExtensionTrailer(gif_out) == GIF_ERROR) {
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("EGifPutExtensionTrailer failed");
			}
			break;
		case TERMINATE_RECORD_TYPE:
			break;
		default:
			break;
		}
	} while (record_type != TERMINATE_RECORD_TYPE);

	if (DGifCloseFile(gif_in, &error_code) == GIF_ERROR) {
		(void)EGifCloseFile(gif_out, NULL);
		buffer_free(&out);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (EGifCloseFile(gif_out, &error_code) == GIF_ERROR) {
		buffer_free(&out);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	return out;
}

static Buffer repair_truncated_gif(const Buffer *input) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif_in;
	GifFileType *gif_out;
	GifRecordType record_type;
	Buffer out = {0};
	GifByteType *extension = NULL;
	GifRowType line_buffer = NULL;
	ColorMapObject *color_map = NULL;
	int error_code = 0;
	int ext_code = 0;
	int darkest_color = 0;
	int color_intensity = 10000;
	int image_num = 0;

	gif_in = DGifOpen(&reader, memory_read, &error_code);
	if (gif_in == NULL) {
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	gif_out = EGifOpen(&out, memory_write, &error_code);
	if (gif_out == NULL) {
		(void)DGifCloseFile(gif_in, NULL);
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	if (EGifPutScreenDesc(gif_out, gif_in->SWidth, gif_in->SHeight,
	                      gif_in->SColorResolution,
	                      gif_in->SBackGroundColor,
	                      gif_in->SColorMap) == GIF_ERROR) {
		(void)DGifCloseFile(gif_in, NULL);
		(void)EGifCloseFile(gif_out, NULL);
		buffer_free(&out);
		fatal("EGifPutScreenDesc failed: %s",
		      gif_error_string_or_default(gif_out->Error));
	}

	line_buffer = xmalloc((size_t)gif_in->SWidth);

	do {
		if (DGifGetRecordType(gif_in, &record_type) == GIF_ERROR) {
			free(line_buffer);
			(void)DGifCloseFile(gif_in, NULL);
			(void)EGifCloseFile(gif_out, NULL);
			buffer_free(&out);
			fatal("DGifGetRecordType failed: %s",
			      gif_error_string_or_default(gif_in->Error));
		}

		switch (record_type) {
		case IMAGE_DESC_RECORD_TYPE: {
			int row;
			int col;
			int width;
			int height;
			int i;
			int j;

			if (DGifGetImageDesc(gif_in) == GIF_ERROR) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("DGifGetImageDesc failed: %s",
				      gif_error_string_or_default(
				          gif_in->Error));
			}
			if (gif_in->Image.Interlace) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("Cannot fix interlaced images.");
			}

			row = gif_in->Image.Top;
			col = gif_in->Image.Left;
			width = gif_in->Image.Width;
			height = gif_in->Image.Height;
			image_num++;
			if (width > gif_in->SWidth) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("Image is wider than total");
			}

			if (EGifPutImageDesc(gif_out, col, row, width, height,
			                     false, gif_in->Image.ColorMap) ==
			    GIF_ERROR) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("EGifPutImageDesc failed: %s",
				      gif_error_string_or_default(
				          gif_out->Error));
			}

			color_map = gif_in->Image.ColorMap != NULL
			                ? gif_in->Image.ColorMap
			                : gif_in->SColorMap;
			darkest_color = 0;
			color_intensity = 10000;
			if (color_map == NULL) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("Image does not have a colormap");
			}
			for (i = 0; i < color_map->ColorCount; i++) {
				j = ((int)color_map->Colors[i].Red) * 30 +
				    ((int)color_map->Colors[i].Green) * 59 +
				    ((int)color_map->Colors[i].Blue) * 11;
				if (j < color_intensity) {
					color_intensity = j;
					darkest_color = i;
				}
			}

			for (i = 0; i < height; i++) {
				if (DGifGetLine(gif_in, line_buffer, width) ==
				    GIF_ERROR) {
					break;
				}
				if (EGifPutLine(gif_out, line_buffer, width) ==
				    GIF_ERROR) {
					free(line_buffer);
					(void)DGifCloseFile(gif_in, NULL);
					(void)EGifCloseFile(gif_out, NULL);
					buffer_free(&out);
					fatal("EGifPutLine failed: %s",
					      gif_error_string_or_default(
					          gif_out->Error));
				}
			}

			if (i < height) {
				for (j = 0; j < width; j++) {
					line_buffer[j] = darkest_color;
				}
				for (; i < height; i++) {
					if (EGifPutLine(gif_out, line_buffer,
					                width) ==
					    GIF_ERROR) {
						free(line_buffer);
						(void)DGifCloseFile(gif_in,
						                    NULL);
						(void)EGifCloseFile(gif_out,
						                    NULL);
						buffer_free(&out);
						fatal("EGifPutLine failed: %s",
						      gif_error_string_or_default(
						          gif_out->Error));
					}
				}
				record_type = TERMINATE_RECORD_TYPE;
			}
			(void)image_num;
			break;
		}
		case EXTENSION_RECORD_TYPE:
			if (DGifGetExtension(gif_in, &ext_code, &extension) ==
			    GIF_ERROR) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("DGifGetExtension failed: %s",
				      gif_error_string_or_default(
				          gif_in->Error));
			}
			if (EGifPutExtensionLeader(gif_out, ext_code) ==
			    GIF_ERROR) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("EGifPutExtensionLeader failed: %s",
				      gif_error_string_or_default(
				          gif_out->Error));
			}
			if (extension != NULL) {
				if (EGifPutExtensionBlock(gif_out,
				                          extension[0],
				                          extension + 1) ==
				    GIF_ERROR) {
					free(line_buffer);
					(void)DGifCloseFile(gif_in, NULL);
					(void)EGifCloseFile(gif_out, NULL);
					buffer_free(&out);
					fatal("EGifPutExtensionBlock failed: "
					      "%s",
					      gif_error_string_or_default(
					          gif_out->Error));
				}
			}
			while (extension != NULL) {
				if (DGifGetExtensionNext(gif_in, &extension) ==
				    GIF_ERROR) {
					free(line_buffer);
					(void)DGifCloseFile(gif_in, NULL);
					(void)EGifCloseFile(gif_out, NULL);
					buffer_free(&out);
					fatal("DGifGetExtensionNext failed: "
					      "%s",
					      gif_error_string_or_default(
					          gif_in->Error));
				}
				if (extension != NULL) {
					if (EGifPutExtensionBlock(
					        gif_out, extension[0],
					        extension + 1) ==
					    GIF_ERROR) {
						free(line_buffer);
						(void)DGifCloseFile(gif_in,
						                    NULL);
						(void)EGifCloseFile(gif_out,
						                    NULL);
						buffer_free(&out);
						fatal("EGifPutExtensionBlock "
						      "failed: %s",
						      gif_error_string_or_default(
						          gif_out->Error));
					}
				}
			}
			if (EGifPutExtensionTrailer(gif_out) == GIF_ERROR) {
				free(line_buffer);
				(void)DGifCloseFile(gif_in, NULL);
				(void)EGifCloseFile(gif_out, NULL);
				buffer_free(&out);
				fatal("EGifPutExtensionTrailer failed: %s",
				      gif_error_string_or_default(
				          gif_out->Error));
			}
			break;
		case TERMINATE_RECORD_TYPE:
			break;
		default:
			break;
		}
	} while (record_type != TERMINATE_RECORD_TYPE);

	free(line_buffer);
	if (DGifCloseFile(gif_in, &error_code) == GIF_ERROR) {
		(void)EGifCloseFile(gif_out, NULL);
		buffer_free(&out);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (EGifCloseFile(gif_out, &error_code) == GIF_ERROR) {
		buffer_free(&out);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	return out;
}

static void copy_extension_blocks(int source_count,
                                  const ExtensionBlock *source_blocks,
                                  int *target_count,
                                  ExtensionBlock **target_blocks) {
	int i;

	for (i = 0; i < source_count; i++) {
		if (GifAddExtensionBlock(target_count, target_blocks,
		                         source_blocks[i].Function,
		                         (unsigned int)source_blocks[i]
		                             .ByteCount,
		                         source_blocks[i].Bytes) ==
		    GIF_ERROR) {
			fatal("GifAddExtensionBlock failed");
		}
	}
}

static Buffer highlevel_copy(const Buffer *input, bool set_interlace,
                             bool interlace_value) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif_in;
	GifFileType *gif_out;
	Buffer out = {0};
	int error_code = 0;
	int i;

	gif_in = DGifOpen(&reader, memory_read, &error_code);
	if (gif_in == NULL) {
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (DGifSlurp(gif_in) == GIF_ERROR) {
		error_code = gif_in->Error;
		(void)DGifCloseFile(gif_in, NULL);
		fatal("DGifSlurp failed: %s",
		      gif_error_string_or_default(error_code));
	}

	gif_out = EGifOpen(&out, memory_write, &error_code);
	if (gif_out == NULL) {
		(void)DGifCloseFile(gif_in, NULL);
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	gif_out->SWidth = gif_in->SWidth;
	gif_out->SHeight = gif_in->SHeight;
	gif_out->SColorResolution = gif_in->SColorResolution;
	gif_out->SBackGroundColor = gif_in->SBackGroundColor;
	gif_out->AspectByte = gif_in->AspectByte;
	if (gif_in->SColorMap != NULL) {
		gif_out->SColorMap = GifMakeMapObject(
		    gif_in->SColorMap->ColorCount,
		    gif_in->SColorMap->Colors);
		if (gif_out->SColorMap == NULL) {
			(void)DGifCloseFile(gif_in, NULL);
			(void)EGifCloseFile(gif_out, NULL);
			buffer_free(&out);
			fatal("GifMakeMapObject failed");
		}
	}

	for (i = 0; i < gif_in->ImageCount; i++) {
		SavedImage *copy =
		    GifMakeSavedImage(gif_out, &gif_in->SavedImages[i]);

		if (copy == NULL) {
			(void)DGifCloseFile(gif_in, NULL);
			(void)EGifCloseFile(gif_out, NULL);
			buffer_free(&out);
			fatal("GifMakeSavedImage failed");
		}
		if (set_interlace) {
			copy->ImageDesc.Interlace = interlace_value;
		}
	}
	copy_extension_blocks(gif_in->ExtensionBlockCount,
	                      gif_in->ExtensionBlocks,
	                      &gif_out->ExtensionBlockCount,
	                      &gif_out->ExtensionBlocks);

	if (EGifSpew(gif_out) == GIF_ERROR) {
		(void)DGifCloseFile(gif_in, NULL);
		buffer_free(&out);
		fatal("EGifSpew failed: %s",
		      gif_error_string_or_default(gif_out->Error));
	}
	if (DGifCloseFile(gif_in, &error_code) == GIF_ERROR) {
		buffer_free(&out);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	return out;
}

static Buffer rgb_to_gif(const Buffer *input, int exp_num_colors,
                         int width, int height) {
	Buffer out = {0};
	GifFileType *gif;
	ColorMapObject *output_color_map = NULL;
	GifByteType *red_buffer = NULL;
	GifByteType *green_buffer = NULL;
	GifByteType *blue_buffer = NULL;
	GifByteType *output_buffer = NULL;
	size_t pixel_count;
	int color_map_size;
	int error_code = 0;
	int i;

	if (exp_num_colors <= 0 || exp_num_colors > 8 || width <= 0 ||
	    height <= 0) {
		fatal("invalid rgb-to-gif dimensions or color count");
	}

	pixel_count = (size_t)width * (size_t)height;
	if (input->len != pixel_count * 3) {
		fatal("expected %zu bytes of RGB input, got %zu",
		      pixel_count * 3, input->len);
	}

	color_map_size = 1 << exp_num_colors;
	red_buffer = xmalloc(pixel_count);
	green_buffer = xmalloc(pixel_count);
	blue_buffer = xmalloc(pixel_count);
	output_buffer = xmalloc(pixel_count);
	output_color_map = GifMakeMapObject(color_map_size, NULL);
	if (output_color_map == NULL) {
		free(red_buffer);
		free(green_buffer);
		free(blue_buffer);
		free(output_buffer);
		fatal("GifMakeMapObject failed");
	}

	for (i = 0; i < width * height; i++) {
		red_buffer[i] = input->data[i * 3];
		green_buffer[i] = input->data[i * 3 + 1];
		blue_buffer[i] = input->data[i * 3 + 2];
	}

	if (GifQuantizeBuffer((unsigned int)width, (unsigned int)height,
	                      &color_map_size, red_buffer, green_buffer,
	                      blue_buffer, output_buffer,
	                      output_color_map->Colors) == GIF_ERROR) {
		free(red_buffer);
		free(green_buffer);
		free(blue_buffer);
		free(output_buffer);
		GifFreeMapObject(output_color_map);
		fatal("GifQuantizeBuffer failed");
	}
	output_color_map->SortFlag = true;

	free(red_buffer);
	free(green_buffer);
	free(blue_buffer);

	gif = EGifOpen(&out, memory_write, &error_code);
	if (gif == NULL) {
		free(output_buffer);
		GifFreeMapObject(output_color_map);
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (EGifPutScreenDesc(gif, width, height, exp_num_colors, 0,
	                      output_color_map) == GIF_ERROR ||
	    EGifPutImageDesc(gif, 0, 0, width, height, false, NULL) ==
	        GIF_ERROR) {
		free(output_buffer);
		GifFreeMapObject(output_color_map);
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("Failed to start GIF output");
	}

	for (i = 0; i < height; i++) {
		if (EGifPutLine(gif, output_buffer + (i * width), width) ==
		    GIF_ERROR) {
			free(output_buffer);
			GifFreeMapObject(output_color_map);
			(void)EGifCloseFile(gif, NULL);
			buffer_free(&out);
			fatal("EGifPutLine failed: %s",
			      gif_error_string_or_default(gif->Error));
		}
	}

	free(output_buffer);
	GifFreeMapObject(output_color_map);
	if (EGifCloseFile(gif, &error_code) == GIF_ERROR) {
		buffer_free(&out);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	return out;
}

static Buffer summarize_legacy_api_coverage(void) {
	static const GifPixelType seq_pixels[] = {0, 1, 1, 0};
	static const GifPixelType lz_pixel[] = {1};
	const char comment_text[] = "seq-public";
	Buffer gif_data = {0};
	Buffer lz_data = {0};
	Buffer out = {0};
	GifFileType *writer;
	GifFileType *reader;
	MemoryReader memory_reader;
	char *text = NULL;
	size_t text_len = 0;
	FILE *stream;
	int error_code = 0;
	GifRecordType record_type;
	GifByteType *extension = NULL;
	int ext_code;
	size_t comment_len = 0;
	char comment[64];
	int pixel_calls = 0;
	int code;
	bool terminated = false;

	writer = EGifOpen(&gif_data, memory_write, &error_code);
	if (writer == NULL) {
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	stream = xopen_memstream(&text, &text_len);
	(void)fprintf(stream, "writer_version %s",
	              EGifGetGifVersion(writer));
	EGifSetGifVersion(writer, true);
	(void)fprintf(stream, " -> %s\n", EGifGetGifVersion(writer));

	encode_monochrome_image(writer, 2, 2, seq_pixels, false, comment_text,
	                        true);
	if (EGifCloseFile(writer, &error_code) == GIF_ERROR) {
		buffer_free(&gif_data);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	memory_reader.data = gif_data.data;
	memory_reader.len = gif_data.len;
	memory_reader.pos = 0;
	reader = DGifOpen(&memory_reader, memory_read, &error_code);
	if (reader == NULL) {
		buffer_free(&gif_data);
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	(void)fprintf(stream, "reader_version %s\n",
	              DGifGetGifVersion(reader));

	if (DGifGetRecordType(reader, &record_type) == GIF_ERROR ||
	    record_type != EXTENSION_RECORD_TYPE) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&gif_data);
		fatal("expected comment extension");
	}
	if (DGifGetExtension(reader, &ext_code, &extension) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&gif_data);
		fatal("DGifGetExtension failed: %s",
		      gif_error_string_or_default(reader->Error));
	}
	if (ext_code != COMMENT_EXT_FUNC_CODE) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&gif_data);
		fatal("expected comment extension, got %d", ext_code);
	}
	while (extension != NULL) {
		if (comment_len + extension[0] >= sizeof(comment)) {
			(void)DGifCloseFile(reader, NULL);
			buffer_free(&gif_data);
			fatal("comment buffer too small");
		}
		memcpy(comment + comment_len, extension + 1, extension[0]);
		comment_len += extension[0];
		if (DGifGetExtensionNext(reader, &extension) == GIF_ERROR) {
			(void)DGifCloseFile(reader, NULL);
			buffer_free(&gif_data);
			fatal("DGifGetExtensionNext failed: %s",
			      gif_error_string_or_default(reader->Error));
		}
	}
	comment[comment_len] = '\0';
	(void)fprintf(stream, "comment %s\n", comment);

	if (DGifGetRecordType(reader, &record_type) == GIF_ERROR ||
	    record_type != IMAGE_DESC_RECORD_TYPE) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&gif_data);
		fatal("expected image descriptor");
	}
	if (DGifGetImageHeader(reader) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&gif_data);
		fatal("DGifGetImageHeader failed: %s",
		      gif_error_string_or_default(reader->Error));
	}
	(void)fprintf(stream, "image %d,%d %dx%d interlace=%d\n",
	              reader->Image.Left, reader->Image.Top,
	              reader->Image.Width, reader->Image.Height,
	              reader->Image.Interlace ? 1 : 0);
	for (pixel_calls = 0; pixel_calls < 4; pixel_calls++) {
		if (DGifGetPixel(reader, 0) == GIF_ERROR) {
			(void)DGifCloseFile(reader, NULL);
			buffer_free(&gif_data);
			fatal("DGifGetPixel failed: %s",
			      gif_error_string_or_default(reader->Error));
		}
	}
	if (DGifGetRecordType(reader, &record_type) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&gif_data);
		fatal("DGifGetRecordType failed after pixels: %s",
		      gif_error_string_or_default(reader->Error));
	}
	terminated = record_type == TERMINATE_RECORD_TYPE;
	(void)fprintf(stream, "pixel_calls %d terminate=%s\n",
	              pixel_calls, terminated ? "yes" : "no");
	if (DGifCloseFile(reader, &error_code) == GIF_ERROR) {
		buffer_free(&gif_data);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	buffer_free(&gif_data);

	writer = EGifOpen(&lz_data, memory_write, &error_code);
	if (writer == NULL) {
		free(text);
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	encode_monochrome_image(writer, 1, 1, lz_pixel, false, NULL, true);
	if (EGifCloseFile(writer, &error_code) == GIF_ERROR) {
		buffer_free(&lz_data);
		free(text);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	memory_reader.data = lz_data.data;
	memory_reader.len = lz_data.len;
	memory_reader.pos = 0;
	reader = DGifOpen(&memory_reader, memory_read, &error_code);
	if (reader == NULL) {
		buffer_free(&lz_data);
		free(text);
		fatal("DGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (DGifGetRecordType(reader, &record_type) == GIF_ERROR ||
	    record_type != IMAGE_DESC_RECORD_TYPE) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&lz_data);
		free(text);
		fatal("expected image descriptor before LZ codes");
	}
	if (DGifGetImageHeader(reader) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&lz_data);
		free(text);
		fatal("DGifGetImageHeader failed: %s",
		      gif_error_string_or_default(reader->Error));
	}

	(void)fprintf(stream, "lz_codes");
	do {
		if (DGifGetLZCodes(reader, &code) == GIF_ERROR) {
			(void)DGifCloseFile(reader, NULL);
			buffer_free(&lz_data);
			free(text);
			fatal("DGifGetLZCodes failed: %s",
			      gif_error_string_or_default(reader->Error));
		}
		(void)fprintf(stream, " %d", code);
	} while (code != -1);
	if (DGifGetRecordType(reader, &record_type) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		buffer_free(&lz_data);
		free(text);
		fatal("DGifGetRecordType failed after LZ codes: %s",
		      gif_error_string_or_default(reader->Error));
	}
	(void)fprintf(stream, " terminate=%s\n",
	              record_type == TERMINATE_RECORD_TYPE ? "yes" : "no");
	if (DGifCloseFile(reader, &error_code) == GIF_ERROR) {
		buffer_free(&lz_data);
		free(text);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	buffer_free(&lz_data);

	if (fclose(stream) != 0) {
		free(text);
		fatal("failed to finalize summary output");
	}
	out.data = (unsigned char *)text;
	out.len = text_len;
	out.cap = text_len;

	return out;
}

static Buffer summarize_file_api_coverage(void) {
	static const GifPixelType name_pixels[] = {0, 1};
	static const GifPixelType handle_pixels[] = {1, 0};
	char name_template[] = "/tmp/giflib-name-XXXXXX";
	char handle_template[] = "/tmp/giflib-handle-XXXXXX";
	GifFileType *writer;
	GifFileType *reader;
	Buffer out = {0};
	char *text = NULL;
	size_t text_len = 0;
	FILE *stream = xopen_memstream(&text, &text_len);
	int error_code = 0;
	int fd;

	fd = mkstemp(name_template);
	if (fd < 0) {
		fclose(stream);
		free(text);
		fatal("mkstemp failed for name test");
	}
	if (close(fd) != 0) {
		fclose(stream);
		free(text);
		(void)unlink(name_template);
		fatal("close failed for name test");
	}

	writer = EGifOpenFileName(name_template, false, &error_code);
	if (writer == NULL) {
		fclose(stream);
		free(text);
		(void)unlink(name_template);
		fatal("EGifOpenFileName failed: %s",
		      gif_error_string_or_default(error_code));
	}
	(void)fprintf(stream, "name writer=%s",
	              EGifGetGifVersion(writer));
	encode_monochrome_image(writer, 2, 1, name_pixels, false, NULL,
	                        false);
	if (EGifCloseFile(writer, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		(void)unlink(name_template);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	reader = DGifOpenFileName(name_template, &error_code);
	if (reader == NULL) {
		fclose(stream);
		free(text);
		(void)unlink(name_template);
		fatal("DGifOpenFileName failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (DGifSlurp(reader) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		fclose(stream);
		free(text);
		(void)unlink(name_template);
		fatal("DGifSlurp failed: %s",
		      gif_error_string_or_default(reader->Error));
	}
	(void)fprintf(stream, " reader=%s size=%dx%d pixels=",
	              DGifGetGifVersion(reader), reader->SWidth,
	              reader->SHeight);
	print_saved_image_pixels(stream, &reader->SavedImages[0]);
	(void)fputc('\n', stream);
	if (DGifCloseFile(reader, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		(void)unlink(name_template);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (unlink(name_template) != 0) {
		fclose(stream);
		free(text);
		fatal("unlink failed for name test");
	}

	fd = mkstemp(handle_template);
	if (fd < 0) {
		fclose(stream);
		free(text);
		fatal("mkstemp failed for handle test");
	}

	writer = EGifOpenFileHandle(fd, &error_code);
	if (writer == NULL) {
		fclose(stream);
		free(text);
		(void)close(fd);
		(void)unlink(handle_template);
		fatal("EGifOpenFileHandle failed: %s",
		      gif_error_string_or_default(error_code));
	}
	EGifSetGifVersion(writer, true);
	(void)fprintf(stream, "handle writer=%s",
	              EGifGetGifVersion(writer));
	encode_monochrome_image(writer, 1, 2, handle_pixels, false, NULL,
	                        false);
	if (EGifCloseFile(writer, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		(void)unlink(handle_template);
		fatal("EGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}

	fd = open(handle_template, O_RDONLY);
	if (fd < 0) {
		fclose(stream);
		free(text);
		(void)unlink(handle_template);
		fatal("open failed for handle test");
	}
	reader = DGifOpenFileHandle(fd, &error_code);
	if (reader == NULL) {
		fclose(stream);
		free(text);
		(void)close(fd);
		(void)unlink(handle_template);
		fatal("DGifOpenFileHandle failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (DGifSlurp(reader) == GIF_ERROR) {
		(void)DGifCloseFile(reader, NULL);
		fclose(stream);
		free(text);
		(void)unlink(handle_template);
		fatal("DGifSlurp failed: %s",
		      gif_error_string_or_default(reader->Error));
	}
	(void)fprintf(stream, " reader=%s size=%dx%d pixels=",
	              DGifGetGifVersion(reader), reader->SWidth,
	              reader->SHeight);
	print_saved_image_pixels(stream, &reader->SavedImages[0]);
	(void)fputc('\n', stream);
	if (DGifCloseFile(reader, &error_code) == GIF_ERROR) {
		fclose(stream);
		free(text);
		(void)unlink(handle_template);
		fatal("DGifCloseFile failed: %s",
		      gif_error_string_or_default(error_code));
	}
	if (unlink(handle_template) != 0) {
		fclose(stream);
		free(text);
		fatal("unlink failed for handle test");
	}

	if (fclose(stream) != 0) {
		free(text);
		fatal("failed to finalize file API summary");
	}
	out.data = (unsigned char *)text;
	out.len = text_len;
	out.cap = text_len;

	return out;
}

static Buffer summarize_alloc_api_coverage(void) {
	static const GifColorType first_colors[] = {
	    {255, 0, 0},
	    {0, 255, 0},
	};
	static const GifColorType second_colors[] = {
	    {0, 255, 0},
	    {0, 0, 255},
	};
	static const GifPixelType initial_raster[] = {0, 1, 1, 0};
	static const unsigned char ext_data[] = {'o', 'k'};
	Buffer out = {0};
	char *text = NULL;
	size_t text_len = 0;
	FILE *stream = xopen_memstream(&text, &text_len);
	ColorMapObject *first = GifMakeMapObject(2, first_colors);
	ColorMapObject *second = GifMakeMapObject(2, second_colors);
	ColorMapObject *combined;
	GifPixelType translation[256] = {0};
	SavedImage translated = {0};
	GraphicsControlBlock gcb = {DISPOSE_PREVIOUS, true, 25, 7};
	GraphicsControlBlock direct = {0};
	GraphicsControlBlock saved = {0};
	GifByteType gcb_bytes[4];
	GifFileType gif = {0};
	ExtensionBlock *extension_blocks = NULL;
	int extension_count = 0;
	int i;

	if (first == NULL || second == NULL) {
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		fclose(stream);
		free(text);
		fatal("GifMakeMapObject failed");
	}

	combined = GifUnionColorMap(first, second, translation);
	if (combined == NULL) {
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		fclose(stream);
		free(text);
		fatal("GifUnionColorMap failed");
	}

	translated.ImageDesc.Width = 4;
	translated.ImageDesc.Height = 1;
	translated.RasterBits = xmalloc(sizeof(initial_raster));
	memcpy(translated.RasterBits, initial_raster, sizeof(initial_raster));
	GifApplyTranslation(&translated, translation);

	(void)fprintf(stream, "bits 2=%d 3=%d 17=%d\n", GifBitSize(2),
	              GifBitSize(3), GifBitSize(17));
	(void)fprintf(stream,
	              "union count=%d bits=%d trans=%d,%d raster=",
	              combined->ColorCount, combined->BitsPerPixel,
	              translation[0], translation[1]);
	for (i = 0; i < translated.ImageDesc.Width; i++) {
		(void)fprintf(stream, "%d", translated.RasterBits[i]);
	}
	(void)fputc('\n', stream);

	if (EGifGCBToExtension(&gcb, gcb_bytes) != 4) {
		free(translated.RasterBits);
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		GifFreeMapObject(combined);
		fclose(stream);
		free(text);
		fatal("EGifGCBToExtension returned an unexpected length");
	}
	if (DGifExtensionToGCB(4, gcb_bytes, &direct) == GIF_ERROR) {
		free(translated.RasterBits);
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		GifFreeMapObject(combined);
		fclose(stream);
		free(text);
		fatal("DGifExtensionToGCB failed");
	}
	if (GifMakeSavedImage(&gif, NULL) == NULL) {
		free(translated.RasterBits);
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		GifFreeMapObject(combined);
		fclose(stream);
		free(text);
		fatal("GifMakeSavedImage failed");
	}
	if (EGifGCBToSavedExtension(&gcb, &gif, 0) == GIF_ERROR) {
		free(translated.RasterBits);
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		GifFreeMapObject(combined);
		GifFreeSavedImages(&gif);
		fclose(stream);
		free(text);
		fatal("EGifGCBToSavedExtension failed");
	}
	if (DGifSavedExtensionToGCB(&gif, 0, &saved) == GIF_ERROR) {
		free(translated.RasterBits);
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		GifFreeMapObject(combined);
		GifFreeSavedImages(&gif);
		fclose(stream);
		free(text);
		fatal("DGifSavedExtensionToGCB failed");
	}
	(void)fprintf(stream,
	              "gcb bytes=%02x%02x%02x%02x direct=%d,%d,%d,%d "
	              "saved=%d,%d,%d,%d count=%d\n",
	              gcb_bytes[0], gcb_bytes[1], gcb_bytes[2],
	              gcb_bytes[3], direct.DisposalMode,
	              direct.UserInputFlag ? 1 : 0, direct.DelayTime,
	              direct.TransparentColor, saved.DisposalMode,
	              saved.UserInputFlag ? 1 : 0, saved.DelayTime,
	              saved.TransparentColor,
	              gif.SavedImages[0].ExtensionBlockCount);

	if (GifAddExtensionBlock(&extension_count, &extension_blocks,
	                         COMMENT_EXT_FUNC_CODE,
	                         (unsigned int)sizeof(ext_data),
	                         (unsigned char *)ext_data) == GIF_ERROR) {
		free(translated.RasterBits);
		GifFreeMapObject(first);
		GifFreeMapObject(second);
		GifFreeMapObject(combined);
		GifFreeSavedImages(&gif);
		fclose(stream);
		free(text);
		fatal("GifAddExtensionBlock failed");
	}
	GifFreeExtensions(&extension_count, &extension_blocks);
	GifFreeSavedImages(&gif);
	(void)fprintf(stream, "free extensions=%d,%s saved_images=%s\n",
	              extension_count,
	              extension_blocks == NULL ? "null" : "set",
	              gif.SavedImages == NULL ? "null" : "set");

	free(translated.RasterBits);
	GifFreeMapObject(first);
	GifFreeMapObject(second);
	GifFreeMapObject(combined);

	if (fclose(stream) != 0) {
		free(text);
		fatal("failed to finalize allocation summary");
	}
	out.data = (unsigned char *)text;
	out.len = text_len;
	out.cap = text_len;

	return out;
}

static Buffer generate_foobar(void) {
	const char text[] = "foobar";
	Buffer out = {0};
	GifFileType *gif;
	SavedImage *image;
	int error_code = 0;

	gif = EGifOpen(&out, memory_write, &error_code);
	if (gif == NULL) {
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	gif->SWidth = GIF_FONT_WIDTH * (int)strlen(text);
	gif->SHeight = GIF_FONT_HEIGHT;
	gif->SColorResolution = 1;
	gif->SBackGroundColor = 0;
	gif->AspectByte = 0;
	gif->SColorMap = GifMakeMapObject(2, NULL);
	if (gif->SColorMap == NULL) {
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("GifMakeMapObject failed");
	}
	gif->SColorMap->Colors[0].Red = 0;
	gif->SColorMap->Colors[0].Green = 0;
	gif->SColorMap->Colors[0].Blue = 0;
	gif->SColorMap->Colors[1].Red = 255;
	gif->SColorMap->Colors[1].Green = 255;
	gif->SColorMap->Colors[1].Blue = 255;

	image = GifMakeSavedImage(gif, NULL);
	if (image == NULL) {
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("GifMakeSavedImage failed");
	}
	image->ImageDesc.Left = 0;
	image->ImageDesc.Top = 0;
	image->ImageDesc.Width = gif->SWidth;
	image->ImageDesc.Height = gif->SHeight;
	image->ImageDesc.Interlace = false;
	image->ImageDesc.ColorMap = NULL;
	image->RasterBits = xcalloc((size_t)gif->SWidth * gif->SHeight,
	                            sizeof(GifByteType));
	draw_text_line_with_public_font(image, text, 1);

	if (EGifSpew(gif) == GIF_ERROR) {
		buffer_free(&out);
		fatal("EGifSpew failed: %s",
		      gif_error_string_or_default(gif->Error));
	}

	return out;
}

static Buffer generate_drawing(void) {
	Buffer out = {0};
	GifFileType *gif;
	SavedImage *image;
	int error_code = 0;

	gif = EGifOpen(&out, memory_write, &error_code);
	if (gif == NULL) {
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	gif->SWidth = 20;
	gif->SHeight = 18;
	gif->SBackGroundColor = 0;
	gif->AspectByte = 0;
	gif->SColorMap = GifMakeMapObject(8, NULL);
	if (gif->SColorMap == NULL) {
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("GifMakeMapObject failed");
	}
	gif->SColorResolution = gif->SColorMap->BitsPerPixel;
	gif->SColorMap->Colors[0] = (GifColorType){0, 0, 0};
	gif->SColorMap->Colors[1] = (GifColorType){255, 255, 255};
	gif->SColorMap->Colors[2] = (GifColorType){255, 0, 0};
	gif->SColorMap->Colors[3] = (GifColorType){0, 255, 0};
	gif->SColorMap->Colors[4] = (GifColorType){0, 0, 255};
	gif->SColorMap->Colors[5] = (GifColorType){255, 255, 0};
	gif->SColorMap->Colors[6] = (GifColorType){0, 255, 255};
	gif->SColorMap->Colors[7] = (GifColorType){255, 0, 255};

	image = GifMakeSavedImage(gif, NULL);
	if (image == NULL) {
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("GifMakeSavedImage failed");
	}
	image->ImageDesc.Left = 0;
	image->ImageDesc.Top = 0;
	image->ImageDesc.Width = gif->SWidth;
	image->ImageDesc.Height = gif->SHeight;
	image->ImageDesc.Interlace = false;
	image->ImageDesc.ColorMap = NULL;
	image->RasterBits = xcalloc((size_t)gif->SWidth * gif->SHeight,
	                            sizeof(GifByteType));

	GifDrawRectangle(image, 1, 1, 5, 3, 1);
	GifDrawBox(image, 7, 1, 5, 3, 2);
	GifDrawText8x8(image, 0, 6, "A", 3);
	GifDrawBoxedText8x8(image, 9, 6, "\tZ", 1, 4, 5);

	if (EGifSpew(gif) == GIF_ERROR) {
		buffer_free(&out);
		fatal("EGifSpew failed: %s",
		      gif_error_string_or_default(gif->Error));
	}

	return out;
}

static void draw_text_line_with_public_font(SavedImage *image,
                                            const char *text,
                                            int foreground_index) {
	int char_index;
	int char_pos_x;
	int len = (int)strlen(text);

	for (char_index = 0, char_pos_x = 0; char_index < len;
	     char_index++, char_pos_x += GIF_FONT_WIDTH) {
		unsigned char c = (unsigned char)text[char_index];
		int row;

		for (row = 0; row < GIF_FONT_HEIGHT; row++) {
			unsigned char byte = GifAsciiTable8x8[c][row];
			unsigned char mask;
			int col;

			for (col = 0, mask = 128; col < GIF_FONT_WIDTH;
			     col++, mask >>= 1) {
				if ((byte & mask) != 0) {
					image->RasterBits[row *
					                      image->ImageDesc.Width +
					                  char_pos_x + col] =
					    (GifByteType)foreground_index;
				}
			}
		}
	}
}

static Buffer generate_wedge(void) {
	const int num_levels = 16;
	const int image_width = 640;
	const int image_height = 350;
	const int level_step = 256 / num_levels;
	Buffer out = {0};
	GifFileType *gif;
	SavedImage *image;
	int error_code = 0;
	int i;
	int j;
	int band;

	gif = EGifOpen(&out, memory_write, &error_code);
	if (gif == NULL) {
		fatal("EGifOpen failed: %s",
		      gif_error_string_or_default(error_code));
	}

	gif->SWidth = image_width;
	gif->SHeight = image_height;
	gif->SColorResolution = 7;
	gif->SBackGroundColor = 0;
	gif->AspectByte = 0;
	gif->SColorMap = GifMakeMapObject(8 * num_levels, NULL);
	if (gif->SColorMap == NULL) {
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("GifMakeMapObject failed");
	}

	for (i = 0; i < 8; i++) {
		for (j = 0; j < num_levels; j++) {
			int level = level_step * j;
			int color_index = i * num_levels + j;

			gif->SColorMap->Colors[color_index].Red =
			    (i == 0 || i == 1 || i == 4 || i == 6) * level;
			gif->SColorMap->Colors[color_index].Green =
			    (i == 0 || i == 2 || i == 4 || i == 5) * level;
			gif->SColorMap->Colors[color_index].Blue =
			    (i == 0 || i == 3 || i == 5 || i == 6) * level;
		}
	}

	image = GifMakeSavedImage(gif, NULL);
	if (image == NULL) {
		(void)EGifCloseFile(gif, NULL);
		buffer_free(&out);
		fatal("GifMakeSavedImage failed");
	}
	image->ImageDesc.Left = 0;
	image->ImageDesc.Top = 0;
	image->ImageDesc.Width = image_width;
	image->ImageDesc.Height = image_height;
	image->ImageDesc.Interlace = false;
	image->ImageDesc.ColorMap = NULL;
	image->RasterBits = xcalloc((size_t)image_width * image_height,
	                            sizeof(GifByteType));

	for (band = 0; band < 7; band++) {
		for (i = 0; i < image_height / 7; i++) {
			int y = band * (image_height / 7) + i;
			int x = 0;

			for (j = 0; j < num_levels; j++) {
				int value = j + num_levels * band;
				int repeat;

				for (repeat = 0; repeat < image_width / num_levels;
				     repeat++) {
					image->RasterBits[y * image_width + x++] =
					    (GifByteType)value;
				}
			}
		}
	}

	if (EGifSpew(gif) == GIF_ERROR) {
		buffer_free(&out);
		fatal("EGifSpew failed: %s",
		      gif_error_string_or_default(gif->Error));
	}

	return out;
}

static bool malformed_input_is_rejected(const Buffer *input) {
	MemoryReader reader = {input->data, input->len, 0};
	GifFileType *gif;
	int error_code = 0;

	gif = DGifOpen(&reader, memory_read, &error_code);
	if (gif == NULL) {
		return true;
	}
	if (DGifSlurp(gif) == GIF_ERROR) {
		(void)DGifCloseFile(gif, NULL);
		return true;
	}
	if (DGifCloseFile(gif, &error_code) == GIF_ERROR) {
		return true;
	}

	return false;
}

int main(int argc, char **argv) {
	Buffer input = {0};
	Buffer output = {0};

	if (argc < 2) {
		usage();
	}

	if (strcmp(argv[1], "render") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = render_rgb(&input);
		write_stdout(&output);
	} else if (strcmp(argv[1], "map") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = dump_map(&input);
		write_stdout(&output);
	} else if (strcmp(argv[1], "dump") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = dump_text(&input, "Stdin");
		write_stdout(&output);
	} else if (strcmp(argv[1], "icon") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = dump_icon(&input);
		write_stdout(&output);
	} else if (strcmp(argv[1], "lowlevel-copy") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = lowlevel_copy(&input);
		write_stdout(&output);
	} else if (strcmp(argv[1], "repair") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = repair_truncated_gif(&input);
		write_stdout(&output);
	} else if (strcmp(argv[1], "highlevel-copy") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		output = highlevel_copy(&input, false, false);
		write_stdout(&output);
	} else if (strcmp(argv[1], "interlace") == 0) {
		bool interlace_value;

		if (argc < 4) {
			usage();
		}
		if (strcmp(argv[2], "on") == 0) {
			interlace_value = true;
		} else if (strcmp(argv[2], "off") == 0) {
			interlace_value = false;
		} else {
			usage();
		}
		input = read_input(argv[3]);
		output = highlevel_copy(&input, true, interlace_value);
		write_stdout(&output);
	} else if (strcmp(argv[1], "rgb-to-gif") == 0) {
		int exp_num_colors;
		int width;
		int height;

		if (argc < 5) {
			usage();
		}
		exp_num_colors = atoi(argv[2]);
		width = atoi(argv[3]);
		height = atoi(argv[4]);
		input = read_input(argc >= 6 ? argv[5] : "-");
		output = rgb_to_gif(&input, exp_num_colors, width, height);
		write_stdout(&output);
	} else if (strcmp(argv[1], "legacy") == 0) {
		output = summarize_legacy_api_coverage();
		write_stdout(&output);
	} else if (strcmp(argv[1], "fileio") == 0) {
		output = summarize_file_api_coverage();
		write_stdout(&output);
	} else if (strcmp(argv[1], "alloc") == 0) {
		output = summarize_alloc_api_coverage();
		write_stdout(&output);
	} else if (strcmp(argv[1], "generate") == 0) {
		if (argc < 3) {
			usage();
		}
		if (strcmp(argv[2], "foobar") == 0) {
			output = generate_foobar();
		} else if (strcmp(argv[2], "drawing") == 0) {
			output = generate_drawing();
		} else if (strcmp(argv[2], "wedge") == 0) {
			output = generate_wedge();
		} else {
			usage();
		}
		write_stdout(&output);
	} else if (strcmp(argv[1], "malformed") == 0) {
		input = read_input(argc >= 3 ? argv[2] : "-");
		if (!malformed_input_is_rejected(&input)) {
			buffer_free(&input);
			fputs("decoder unexpectedly accepted malformed GIF\n",
			      stderr);
			return EXIT_FAILURE;
		}
	} else {
		usage();
	}

	buffer_free(&input);
	buffer_free(&output);
	return EXIT_SUCCESS;
}
