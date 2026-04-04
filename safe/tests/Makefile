# Regression-test suite for the giflib public API using the safe build outputs.
#
# The authoritative harness source, fixtures, and comparison oracles live under
# original/tests and original/pic. This makefile consumes them in place.

.SUFFIXES: .gif .rgb

CC ?= cc
CPPFLAGS ?=
CFLAGS ?= -O2
LDLIBS ?= -lm

ORIGINAL_INCLUDEDIR ?= ../../original
ORIGINAL_TESTS_DIR ?= ../../original/tests
ORIGINAL_PIC_DIR ?= ../../original/pic
LIBGIF_INCLUDEDIR ?= ../include
LIBGIF_LIBDIR ?= ../target/release
DEB_HOST_MULTIARCH ?= $(shell dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || gcc -print-multiarch)
LIBGIF_RPATH = $(abspath $(LIBGIF_LIBDIR))
LIBGIF_PC_TEMPLATE ?= ../debian/pkgconfig/libgif7.pc.in
LIBGIF_PC_VERSION ?= $(shell dpkg-parsechangelog -l ../debian/changelog -SVersion 2>/dev/null | sed 's/+.*//')

PICS = $(ORIGINAL_PIC_DIR)
GIFS := $(sort $(wildcard $(ORIGINAL_PIC_DIR)/*.gif))

SAFE_HEADER = $(LIBGIF_INCLUDEDIR)/gif_lib.h
LIBGIF_STATIC = $(LIBGIF_LIBDIR)/libgif.a
LIBGIF_SHARED = $(LIBGIF_LIBDIR)/libgif.so
APITEST = ./public_api_regress
APITEST_SRC = $(ORIGINAL_TESTS_DIR)/public_api_regress.c
APITEST_CFLAGS = $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra -I$(LIBGIF_INCLUDEDIR)
LINK_COMPAT_CFLAGS = $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra -I$(ORIGINAL_INCLUDEDIR)
LINK_COMPAT_OBJ = ./public_api_regress.link-compat.o
LINK_COMPAT_STATIC = ./public_api_regress.link-compat-static
LINK_COMPAT_SHARED = ./public_api_regress.link-compat-shared
INTERNAL_EXPORT_SMOKE = ./internal_exports_smoke
MALFORMED_DIR = ./malformed
MALFORMED_FIXTURES := $(sort $(wildcard $(MALFORMED_DIR)/*.gif))
MALFORMED_BASELINE = $(MALFORMED_DIR)/original-baseline.txt
MALFORMED_OBSERVE = ./malformed_observe
MALFORMED_CAPTURE = ./capture_malformed_baseline.sh
COMPAT_DIR = ./compat
COMPAT_CFLAGS = $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra -I$(LIBGIF_INCLUDEDIR)
COMPAT_RUNTIME_EMPTY_COMMENT = $(COMPAT_DIR)/sf-103-empty-comment/slurp_empty_comment
COMPAT_RUNTIME_NO_IMAGE_DESC = $(COMPAT_DIR)/sf-64-no-image-desc/no_image_desc
COMPAT_RUNTIME_METADATA = $(COMPAT_DIR)/runtime-metadata/slurp_metadata
COMPAT_SOURCE_GDAL_SHARED = $(COMPAT_DIR)/runtime-metadata/slurp_metadata.gdal-shared
COMPAT_SOURCE_EXACTIMAGE = $(COMPAT_DIR)/source-exactimage-memory-roundtrip/exactimage_memory_roundtrip
COMPAT_SOURCE_IMLIB2 = $(COMPAT_DIR)/source-imlib2-sequential/imlib2_sequential_decode
COMPAT_SOURCE_SAIL_SYSROOT = $(COMPAT_DIR)/source-sail-pkgconfig/sysroot
COMPAT_SOURCE_SAIL_PKGCONFIG_DIR = $(COMPAT_SOURCE_SAIL_SYSROOT)/usr/lib/$(DEB_HOST_MULTIARCH)/pkgconfig
COMPAT_SOURCE_SAIL_PC = $(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR)/libgif7.pc
COMPAT_SOURCE_SAIL = $(COMPAT_DIR)/source-sail-pkgconfig/sail_pkgconfig_smoke
COMPAT_BINS = \
	$(COMPAT_RUNTIME_EMPTY_COMMENT) \
	$(COMPAT_RUNTIME_NO_IMAGE_DESC) \
	$(COMPAT_RUNTIME_METADATA) \
	$(COMPAT_SOURCE_GDAL_SHARED) \
	$(COMPAT_SOURCE_EXACTIMAGE) \
	$(COMPAT_SOURCE_IMLIB2) \
	$(COMPAT_SOURCE_SAIL)
COMPAT_REGRESS_TARGETS := \
	compat-runtime-sf-103-empty-comment-regress \
	compat-runtime-sf-64-no-image-desc-regress \
	compat-runtime-metadata-regress \
	compat-source-gdal-shared-link-regress \
	compat-source-exactimage-roundtrip-regress \
	compat-source-sail-pkgconfig-regress \
	compat-source-libwebp-static-link-regress \
	compat-source-imlib2-sequential-regress

.PHONY: test clean safe-header-regress link-compat-regress internal-export-regress \
	legacy-regress fileio-regress alloc-regress render-regress gif2rgb-regress \
	gifbuild-regress gifclrmp-regress gifecho-regress giffilter-regress \
	giffix-regress gifsponge-regress drawing-regress giftext-regress \
	giftool-regress gifwedge-regress malformed-regress malformed-baseline-regress \
	compat-regress $(COMPAT_REGRESS_TARGETS)

test: $(APITEST) \
	render-regress \
	gifbuild-regress \
	gifclrmp-regress \
	legacy-regress \
	fileio-regress \
	alloc-regress \
	gifecho-regress \
	giffilter-regress \
	giffix-regress \
	gifsponge-regress \
	drawing-regress \
	giftext-regress \
	giftool-regress \
	gifwedge-regress \
	malformed-regress \
	malformed-baseline-regress \
	compat-regress
	@echo "No output is good news"

$(APITEST): $(APITEST_SRC) $(LIBGIF_STATIC) $(SAFE_HEADER)
	$(CC) $(APITEST_CFLAGS) $(APITEST_SRC) $(LIBGIF_STATIC) $(LDLIBS) -o $@

safe-header-regress:
	$(RM) $(APITEST)
	$(CC) $(APITEST_CFLAGS) $(APITEST_SRC) $(LIBGIF_STATIC) $(LDLIBS) -o $(APITEST)
	@echo "public api: Checking safe header compatibility."
	@$(APITEST) legacy | diff -u $(ORIGINAL_TESTS_DIR)/legacy.summary -
	@$(APITEST) fileio | diff -u $(ORIGINAL_TESTS_DIR)/fileio.summary -
	@$(APITEST) alloc | diff -u $(ORIGINAL_TESTS_DIR)/alloc.summary -

render-regress: $(APITEST)
	@for test in $(GIFS); \
	do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		if echo "Testing RGB rendering of $${test}" >&2; \
		$(APITEST) render $${test} > $@.$${stem}.regress; \
		then cmp $(ORIGINAL_TESTS_DIR)/$${stem}.rgb $@.$${stem}.regress; \
		else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

gif2rgb-regress: $(APITEST)
	@echo "gif2rgb: Checking idempotency through public quantization APIs."
	@$(APITEST) rgb-to-gif 3 100 100 <$(ORIGINAL_TESTS_DIR)/gifgrid.rgb | \
		$(APITEST) icon - | diff -u $(ORIGINAL_TESTS_DIR)/gifgrid.ico -

gifbuild-regress: $(APITEST)
	@echo "gifbuild: Checking structural dump of treescap.gif via public API."
	@$(APITEST) icon <$(PICS)/treescap.gif | diff -u $(ORIGINAL_TESTS_DIR)/treescap.ico -
	@echo "gifbuild: Checking slurp/spew roundtrip on an animation."
	@$(APITEST) highlevel-copy $(PICS)/fire.gif | $(APITEST) dump - | \
		diff -u $(ORIGINAL_TESTS_DIR)/fire.dmp -

gifclrmp-regress: $(APITEST)
	@for test in $(GIFS); \
	do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		if echo "gifclrmap: Checking colormap of $${test}" >&2; \
		$(APITEST) map $${test} > $@.$${stem}.regress; \
		then diff -u $(ORIGINAL_TESTS_DIR)/$${stem}.map $@.$${stem}.regress; \
		else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

legacy-regress: $(APITEST)
	@echo "public api: Checking sequential legacy helpers."
	@$(APITEST) legacy | diff -u $(ORIGINAL_TESTS_DIR)/legacy.summary -

fileio-regress: $(APITEST)
	@echo "public api: Checking file name and handle wrappers."
	@$(APITEST) fileio | diff -u $(ORIGINAL_TESTS_DIR)/fileio.summary -

alloc-regress: $(APITEST)
	@echo "public api: Checking allocation, translation, and GCB helpers."
	@$(APITEST) alloc | diff -u $(ORIGINAL_TESTS_DIR)/alloc.summary -

gifecho-regress: $(APITEST)
	@echo "gifecho: Testing public text rendering behavior"
	@$(APITEST) generate foobar | $(APITEST) icon - | diff -u $(ORIGINAL_TESTS_DIR)/foobar.ico -

drawing-regress: $(APITEST)
	@echo "public api: Checking drawing helpers."
	@$(APITEST) generate drawing | $(APITEST) icon - | diff -u $(ORIGINAL_TESTS_DIR)/drawing.ico -

giffilter-regress: $(APITEST)
	@for test in $(GIFS); \
	do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		if echo "giffilter: Testing low-level copy of $${test}" >&2; \
		$(APITEST) lowlevel-copy $${test} | $(APITEST) render - > $@.$${stem}.regress; \
		then cmp $(ORIGINAL_TESTS_DIR)/$${stem}.rgb $@.$${stem}.regress; \
		else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

giffix-regress: $(APITEST)
	@echo "giffix: Testing giffix behavior"
	@head --bytes=-20 <$(PICS)/treescap.gif | $(APITEST) repair - | \
		$(APITEST) icon - | diff -u $(ORIGINAL_TESTS_DIR)/giffixed.ico -

gifsponge-regress: $(APITEST)
	@for test in $(GIFS); \
	do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		if echo "gifsponge: Testing slurp/spew copy of $${test}" >&2; \
		$(APITEST) highlevel-copy $${test} | $(APITEST) render - > $@.$${stem}.regress; \
		then cmp $(ORIGINAL_TESTS_DIR)/$${stem}.rgb $@.$${stem}.regress; \
		else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

giftext-regress: $(APITEST)
	@for test in $(GIFS); \
	do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		if echo "giftext: Checking text dump of $${test}" >&2; \
		$(APITEST) dump < $${test} > $@.$${stem}.regress; \
		then diff -u $(ORIGINAL_TESTS_DIR)/$${stem}.dmp $@.$${stem}.regress; \
		else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

giftool-regress: $(APITEST)
	@echo "giftool: Checking that expensive copy via the public API is faithful."
	@$(APITEST) highlevel-copy $(PICS)/gifgrid.gif | $(APITEST) render - | \
		cmp - $(ORIGINAL_TESTS_DIR)/gifgrid.rgb
	@echo "giftool: Checking that it de-interlaces correctly."
	@$(APITEST) interlace off $(PICS)/treescap-interlaced.gif | $(APITEST) render - | \
		cmp - $(ORIGINAL_TESTS_DIR)/treescap.rgb
	@echo "giftool: Checking that it interlaces correctly."
	@$(APITEST) interlace on $(PICS)/treescap.gif | $(APITEST) render - | \
		cmp - $(ORIGINAL_TESTS_DIR)/treescap-interlaced.rgb

gifwedge-regress: $(APITEST)
	@echo "gifwedge: Checking wedge generation through public APIs."
	@$(APITEST) generate wedge | cmp - $(ORIGINAL_TESTS_DIR)/wedge.gif

malformed-regress: $(APITEST)
	@echo "public api: Checking malformed input rejection."
	@for test in $(MALFORMED_FIXTURES); \
	do \
		echo "malformed: rejecting $${test}" >&2; \
		$(APITEST) malformed $${test}; \
	done

$(MALFORMED_OBSERVE): ./malformed_observe.c $(LIBGIF_STATIC) $(SAFE_HEADER)
	$(CC) $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra -I$(LIBGIF_INCLUDEDIR) \
		$< $(LIBGIF_STATIC) $(LDLIBS) -o $@

malformed-baseline-regress: $(MALFORMED_OBSERVE) $(MALFORMED_BASELINE) $(MALFORMED_CAPTURE)
	@echo "public api: Checking malformed observation baseline."
	@$(MALFORMED_CAPTURE) $(MALFORMED_OBSERVE) $(MALFORMED_DIR) > $@.regress
	@diff -u $(MALFORMED_BASELINE) $@.regress
	@rm -f $@.regress

compat-regress: $(COMPAT_REGRESS_TARGETS)
ifeq ($(strip $(COMPAT_REGRESS_TARGETS)),)
	@echo "compat: no issue-specific local reproducers registered."
else
	@echo "compat: completed $(words $(COMPAT_REGRESS_TARGETS)) registered issue-specific reproducers."
endif

link-compat-regress:
	$(RM) $(LINK_COMPAT_OBJ) $(LINK_COMPAT_STATIC) $(LINK_COMPAT_SHARED)
	$(CC) $(LINK_COMPAT_CFLAGS) -c $(APITEST_SRC) -o $(LINK_COMPAT_OBJ)
	$(CC) $(LINK_COMPAT_OBJ) $(LIBGIF_STATIC) $(LDLIBS) -o $(LINK_COMPAT_STATIC)
	$(CC) $(LINK_COMPAT_OBJ) -L$(LIBGIF_LIBDIR) -Wl,-rpath,$(LIBGIF_LIBDIR) -lgif $(LDLIBS) -o $(LINK_COMPAT_SHARED)
	@for bin in $(LINK_COMPAT_STATIC) $(LINK_COMPAT_SHARED); \
	do \
		echo "link compatibility: legacy via $${bin}" >&2; \
		$${bin} legacy | diff -u $(ORIGINAL_TESTS_DIR)/legacy.summary -; \
		echo "link compatibility: alloc via $${bin}" >&2; \
		$${bin} alloc | diff -u $(ORIGINAL_TESTS_DIR)/alloc.summary -; \
		echo "link compatibility: render via $${bin}" >&2; \
		$${bin} render $(PICS)/welcome2.gif | cmp - $(ORIGINAL_TESTS_DIR)/welcome2.rgb; \
		echo "link compatibility: malformed via $${bin}" >&2; \
		head --bytes=-1 <$(PICS)/treescap.gif | $${bin} malformed -; \
		echo "link compatibility: highlevel-copy via $${bin}" >&2; \
		$${bin} highlevel-copy $(PICS)/fire.gif | $${bin} dump - | \
			diff -u $(ORIGINAL_TESTS_DIR)/fire.dmp -; \
	done

$(INTERNAL_EXPORT_SMOKE): ./internal_exports_smoke.c $(LIBGIF_STATIC) $(SAFE_HEADER)
	$(CC) $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra \
		-I$(LIBGIF_INCLUDEDIR) -I$(ORIGINAL_INCLUDEDIR) $< $(LIBGIF_STATIC) $(LDLIBS) -o $@

internal-export-regress: $(INTERNAL_EXPORT_SMOKE)
	@$(INTERNAL_EXPORT_SMOKE)

$(COMPAT_RUNTIME_EMPTY_COMMENT): $(COMPAT_DIR)/sf-103-empty-comment/slurp_empty_comment.c $(LIBGIF_STATIC) $(SAFE_HEADER)
	$(CC) $(COMPAT_CFLAGS) $< $(LIBGIF_STATIC) $(LDLIBS) -o $@

compat-runtime-sf-103-empty-comment-regress: $(COMPAT_RUNTIME_EMPTY_COMMENT)
	@echo "compat(runtime): empty comment slurp for giftext/webp/tracker/libextractor/camlimages/gdal consumers."
	@$(COMPAT_RUNTIME_EMPTY_COMMENT)

$(COMPAT_RUNTIME_NO_IMAGE_DESC): $(COMPAT_DIR)/sf-64-no-image-desc/no_image_desc.c $(LIBGIF_STATIC) $(SAFE_HEADER)
	$(CC) $(COMPAT_CFLAGS) $< $(LIBGIF_STATIC) $(LDLIBS) -o $@

compat-runtime-sf-64-no-image-desc-regress: $(COMPAT_RUNTIME_NO_IMAGE_DESC)
	@echo "compat(runtime): no-image-descriptor rejection for fbi/mtpaint/tracker/libextractor/camlimages/gdal."
	@$(COMPAT_RUNTIME_NO_IMAGE_DESC)

$(COMPAT_RUNTIME_METADATA): $(COMPAT_DIR)/runtime-metadata/slurp_metadata.c $(LIBGIF_STATIC) $(SAFE_HEADER)
	$(CC) $(COMPAT_CFLAGS) $< $(LIBGIF_STATIC) $(LDLIBS) -o $@

compat-runtime-metadata-regress: $(COMPAT_RUNTIME_METADATA)
	@echo "compat(runtime): metadata/frame-count slurp smoke for gif2webp/tracker/libextractor/camlimages/gdal."
	@$(COMPAT_RUNTIME_METADATA) $(PICS)/fire.gif 30 60 33 1
	@$(COMPAT_RUNTIME_METADATA) $(PICS)/treescap-interlaced.gif 40 40 1 1
	@$(COMPAT_RUNTIME_METADATA) $(PICS)/welcome2.gif 290 48 6 0

$(COMPAT_SOURCE_GDAL_SHARED): $(COMPAT_DIR)/runtime-metadata/slurp_metadata.c $(LIBGIF_SHARED) $(SAFE_HEADER)
	$(CC) $(COMPAT_CFLAGS) $< -L$(LIBGIF_RPATH) -Wl,-rpath,$(LIBGIF_RPATH) -lgif $(LDLIBS) -o $@

compat-source-gdal-shared-link-regress: $(COMPAT_SOURCE_GDAL_SHARED)
	@echo "compat(source): gdal-style shared-link slurp smoke."
	@$(COMPAT_SOURCE_GDAL_SHARED) $(PICS)/treescap-interlaced.gif 40 40 1 1
	@$(COMPAT_SOURCE_GDAL_SHARED) $(PICS)/welcome2.gif 290 48 6 0

$(COMPAT_SOURCE_EXACTIMAGE): $(COMPAT_DIR)/source-exactimage-memory-roundtrip/exactimage_memory_roundtrip.c $(LIBGIF_SHARED) $(SAFE_HEADER)
	$(CC) $(COMPAT_CFLAGS) $< -L$(LIBGIF_RPATH) -Wl,-rpath,$(LIBGIF_RPATH) -lgif $(LDLIBS) -o $@

compat-source-exactimage-roundtrip-regress: $(COMPAT_SOURCE_EXACTIMAGE)
	@echo "compat(source): exactimage-style public encode/decode roundtrip."
	@$(COMPAT_SOURCE_EXACTIMAGE)

$(COMPAT_SOURCE_SAIL_PC): $(LIBGIF_PC_TEMPLATE) $(LIBGIF_SHARED) $(SAFE_HEADER)
	mkdir -p $(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) $(COMPAT_SOURCE_SAIL_SYSROOT)/usr/include $(COMPAT_SOURCE_SAIL_SYSROOT)/usr/lib/$(DEB_HOST_MULTIARCH)
	ln -sf $(abspath $(SAFE_HEADER)) $(COMPAT_SOURCE_SAIL_SYSROOT)/usr/include/gif_lib.h
	ln -sf $(abspath $(LIBGIF_SHARED)) $(COMPAT_SOURCE_SAIL_SYSROOT)/usr/lib/$(DEB_HOST_MULTIARCH)/libgif.so
	sed -e "s/@DEB_TARGET_MULTIARCH@/$(DEB_HOST_MULTIARCH)/" \
		-e "s/@VERSION@/$(LIBGIF_PC_VERSION)/" \
		$< > $@
	ln -sf libgif7.pc $(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR)/libgif.pc

$(COMPAT_SOURCE_SAIL): $(COMPAT_DIR)/source-sail-pkgconfig/sail_pkgconfig_smoke.c $(COMPAT_SOURCE_SAIL_PC)
	env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR=$(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) \
		PKG_CONFIG_SYSROOT_DIR=$(COMPAT_SOURCE_SAIL_SYSROOT) \
		$(CC) $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra $< -o $@ \
		$$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR=$(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) \
			PKG_CONFIG_SYSROOT_DIR=$(COMPAT_SOURCE_SAIL_SYSROOT) \
			pkg-config --cflags --libs libgif) \
		-Wl,-rpath,$(LIBGIF_RPATH)

compat-source-sail-pkgconfig-regress: $(COMPAT_SOURCE_SAIL)
	@echo "compat(source): sail-style pkg-config alias and shared link smoke."
	@env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR=$(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) PKG_CONFIG_SYSROOT_DIR= pkg-config --exists libgif7
	@env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR=$(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) PKG_CONFIG_SYSROOT_DIR= pkg-config --exists libgif
	@test "$$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR=$(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) PKG_CONFIG_SYSROOT_DIR= pkg-config --variable=libdir libgif7)" = "/usr/lib/$(DEB_HOST_MULTIARCH)"
	@test "$$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR=$(COMPAT_SOURCE_SAIL_PKGCONFIG_DIR) PKG_CONFIG_SYSROOT_DIR= pkg-config --variable=includedir libgif7)" = "/usr/include"
	@$(COMPAT_SOURCE_SAIL)

compat-source-libwebp-static-link-regress: $(COMPAT_RUNTIME_METADATA)
	@echo "compat(source): libwebp-style static-link slurp smoke."
	@$(COMPAT_RUNTIME_METADATA) $(PICS)/fire.gif 30 60 33 1

$(COMPAT_SOURCE_IMLIB2): $(COMPAT_DIR)/source-imlib2-sequential/imlib2_sequential_decode.c $(LIBGIF_SHARED) $(SAFE_HEADER)
	$(CC) $(COMPAT_CFLAGS) $< -L$(LIBGIF_RPATH) -Wl,-rpath,$(LIBGIF_RPATH) -lgif $(LDLIBS) -o $@

compat-source-imlib2-sequential-regress: $(COMPAT_SOURCE_IMLIB2)
	@echo "compat(source): imlib2-style sequential decode smoke."
	@$(COMPAT_SOURCE_IMLIB2) $(PICS)/welcome2.gif 290 48 6

clean:
	rm -f $(APITEST) $(LINK_COMPAT_OBJ) $(LINK_COMPAT_STATIC) $(LINK_COMPAT_SHARED) \
		$(INTERNAL_EXPORT_SMOKE) $(MALFORMED_OBSERVE) $(COMPAT_BINS) \
		*.actual *.expected *.regress
	rm -rf $(COMPAT_SOURCE_SAIL_SYSROOT)
