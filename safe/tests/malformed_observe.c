#include <stdio.h>
#include <string.h>

#include "gif_lib.h"

static const char *fixture_basename(const char *path) {
	const char *slash = strrchr(path, '/');

	return slash != NULL ? slash + 1 : path;
}

int main(int argc, char **argv) {
	int i;

	if (argc < 2) {
		fprintf(stderr, "usage: %s fixture.gif...\n", argv[0]);
		return 64;
	}

	for (i = 1; i < argc; i++) {
		const char *path = argv[i];
		const char *basename = fixture_basename(path);
		GifFileType *gif;
		int open_error = 0;

		gif = DGifOpenFileName(path, &open_error);
		if (gif == NULL) {
			printf("%s\t0\t%d\tNA\tNA\tNA\tNA\n", basename,
			       open_error);
			continue;
		}

		{
			int slurp_rc = DGifSlurp(gif);
			int gif_error_after_slurp = gif->Error;
			int close_error = 0;
			int close_rc = DGifCloseFile(gif, &close_error);

			printf("%s\t1\t%d\t%d\t%d\t%d\t%d\n", basename,
			       open_error, slurp_rc, gif_error_after_slurp,
			       close_rc, close_error);
		}
	}

	return 0;
}
