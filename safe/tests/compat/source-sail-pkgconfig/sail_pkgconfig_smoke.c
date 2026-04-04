#include <stdio.h>

#include "gif_lib.h"

/*
 * Source-build reproducer for sail's install-surface expectations: the
 * pkg-config aliases must compile a public-header translation unit and link it
 * against libgif without any private headers.
 */

int main(void) {
	int bits = GifBitSize(3);

	printf("bitsize=%d\n", bits);
	return bits == 2 ? 0 : 1;
}
