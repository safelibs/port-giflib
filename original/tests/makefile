# Regression-test suite for the giflib public API.
#
# The original upstream tests exercised helper utilities. For the Rust port we
# only want tests that go through the documented library surface in gif_lib.h,
# so the default regressions below use a test-only helper linked directly
# against libgif.

.SUFFIXES: .gif .rgb

export LD_LIBRARY_PATH=$(CURDIR)/..

CC ?= cc
CPPFLAGS ?=
CFLAGS ?= -O2

PICS = ../pic
APITEST = ./public_api_regress
APITEST_SRC = public_api_regress.c
APITEST_CFLAGS = $(CPPFLAGS) $(CFLAGS) -std=gnu99 -Wall -Wextra -I..

GIFS := $(shell ls ../pic/*.gif)

# This is what to do by default.
test: public_api_regress \
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
	gifwedge-regress
	@echo "No output is good news"

rebuild: public_api_regress \
	render-rebuild \
	gif2rgb-rebuild \
	gifbuild-rebuild \
	gifclrmp-rebuild \
	legacy-rebuild \
	fileio-rebuild \
	alloc-rebuild \
	gifecho-rebuild \
	drawing-rebuild \
	giftext-rebuild \
	gifwedge-rebuild

public_api_regress: $(APITEST_SRC) ../libgif.a ../gif_lib.h
	$(CC) $(APITEST_CFLAGS) $(APITEST_SRC) ../libgif.a -o $@

# Test decompression and rendering by unpacking images with the sequential
# decoder and comparing the resulting RGB triples to the fixture files.
render-regress: public_api_regress
	@for test in $(GIFS); \
	do \
	    stem=`basename $${test} | sed -e "s/.gif$$//"`; \
	    if echo "Testing RGB rendering of $${test}" >&2; \
	    $(APITEST) render $${test} > $@.$${stem}.regress; \
	    then cmp $${stem}.rgb $@.$${stem}.regress; \
	    else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

render-rebuild: public_api_regress
	@for test in $(GIFS); do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		echo "Remaking $${stem}.rgb"; \
		$(APITEST) render $${test} > $${stem}.rgb; \
	done

gif2rgb-regress: public_api_regress
	@echo "gif2rgb: Checking idempotency through public quantization APIs."
	@$(APITEST) rgb-to-gif 3 100 100 <gifgrid.rgb | $(APITEST) icon - | diff -u gifgrid.ico -

gif2rgb-rebuild: public_api_regress
	@echo "Rebuilding gif2rgb checkfile."
	@$(APITEST) rgb-to-gif 3 100 100 <gifgrid.rgb | $(APITEST) icon - >gifgrid.ico

gifbuild-regress: public_api_regress
	@echo "gifbuild: Checking structural dump of treescap.gif via public API."
	@$(APITEST) icon <$(PICS)/treescap.gif | diff -u treescap.ico -
	@echo "gifbuild: Checking slurp/spew roundtrip on an animation."
	@$(APITEST) highlevel-copy $(PICS)/fire.gif | $(APITEST) dump - | diff -u fire.dmp -

gifbuild-rebuild: public_api_regress
	@echo "Rebuilding gifbuild-compatible icon fixtures."
	@$(APITEST) icon <$(PICS)/treescap.gif >treescap.ico

gifclrmp-regress: public_api_regress
	@for test in $(GIFS); \
	do \
	    stem=`basename $${test} | sed -e "s/.gif$$//"`; \
	    if echo "gifclrmap: Checking colormap of $${test}" >&2; \
	    $(APITEST) map $${test} > $@.$${stem}.regress; \
	    then diff -u $${stem}.map $@.$${stem}.regress; \
	    else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

gifclrmp-rebuild: public_api_regress
	@for test in $(GIFS); do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		echo "Remaking $${stem}.map"; \
		$(APITEST) map $${test} >$${stem}.map; \
	done

legacy-regress: public_api_regress
	@echo "public api: Checking sequential legacy helpers."
	@$(APITEST) legacy | diff -u legacy.summary -

legacy-rebuild: public_api_regress
	@echo "Rebuilding legacy API fixture."
	@$(APITEST) legacy >legacy.summary

fileio-regress: public_api_regress
	@echo "public api: Checking file name and handle wrappers."
	@$(APITEST) fileio | diff -u fileio.summary -

fileio-rebuild: public_api_regress
	@echo "Rebuilding file I/O fixture."
	@$(APITEST) fileio >fileio.summary

alloc-regress: public_api_regress
	@echo "public api: Checking allocation, translation, and GCB helpers."
	@$(APITEST) alloc | diff -u alloc.summary -

alloc-rebuild: public_api_regress
	@echo "Rebuilding allocation helper fixture."
	@$(APITEST) alloc >alloc.summary

gifecho-regress: public_api_regress
	@echo "gifecho: Testing public text rendering behavior"
	@$(APITEST) generate foobar | $(APITEST) icon - | diff -u foobar.ico -

gifecho-rebuild: public_api_regress
	@echo "Rebuilding gifecho fixture."
	@$(APITEST) generate foobar | $(APITEST) icon - >foobar.ico

drawing-regress: public_api_regress
	@echo "public api: Checking drawing helpers."
	@$(APITEST) generate drawing | $(APITEST) icon - | diff -u drawing.ico -

drawing-rebuild: public_api_regress
	@echo "Rebuilding drawing helper fixture."
	@$(APITEST) generate drawing | $(APITEST) icon - >drawing.ico

giffilter-regress: public_api_regress
	@for test in $(GIFS); \
	do \
	    stem=`basename $${test} | sed -e "s/.gif$$//"`; \
	    if echo "giffilter: Testing low-level copy of $${test}" >&2; \
	    $(APITEST) lowlevel-copy $${test} | $(APITEST) render - > $@.$${stem}.regress; \
	    then cmp $${stem}.rgb $@.$${stem}.regress; \
	    else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f $@.*.regress

giffix-regress: public_api_regress
	@echo "giffix: Testing giffix behavior"
	@head --bytes=-20 <$(PICS)/treescap.gif | $(APITEST) repair - | $(APITEST) icon - | diff -u giffixed.ico -

gifsponge-regress: public_api_regress
	@for test in $(GIFS); \
	do \
	    stem=`basename $${test} | sed -e "s/.gif$$//"`; \
	    if echo "gifsponge: Testing slurp/spew copy of $${test}" >&2; \
	    $(APITEST) highlevel-copy $${test} | $(APITEST) render - > $@.$${stem}.regress; \
	    then cmp $${stem}.rgb  $@.$${stem}.regress; \
	    else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f  $@.*.regress

giftext-regress: public_api_regress
	@for test in $(GIFS); \
	do \
	    stem=`basename $${test} | sed -e "s/.gif$$//"`; \
	    if echo "giftext: Checking text dump of $${test}" >&2; \
	    $(APITEST) dump < $${test} > $@.$${stem}.regress; \
	    then diff -u $${stem}.dmp  $@.$${stem}.regress; \
	    else echo "*** Nonzero return status on $${test}!"; exit 1; fi; \
	done
	@rm -f  $@.*.regress

giftext-rebuild: public_api_regress
	@for test in $(GIFS); do \
		stem=`basename $${test} | sed -e "s/.gif$$//"`; \
		echo "Remaking $${stem}.dmp"; \
		$(APITEST) dump < $${test} >$${stem}.dmp; \
	done

giftool-regress: public_api_regress
	@echo "giftool: Checking that expensive copy via the public API is faithful."
	@$(APITEST) highlevel-copy $(PICS)/gifgrid.gif | $(APITEST) render - | cmp - gifgrid.rgb
	@echo "giftool: Checking that it de-interlaces correctly."
	@$(APITEST) interlace off $(PICS)/treescap-interlaced.gif | $(APITEST) render - | cmp - treescap.rgb
	@echo "giftool: Checking that it interlaces correctly."
	@$(APITEST) interlace on $(PICS)/treescap.gif | $(APITEST) render - | cmp - treescap-interlaced.rgb

gifwedge-regress: public_api_regress
	@echo "gifwedge: Checking wedge generation through public APIs."
	@$(APITEST) generate wedge | cmp - wedge.gif

gifwedge-rebuild: public_api_regress
	@echo "Remaking the gifwedge fixture."
	@$(APITEST) generate wedge >wedge.gif

clean:
	rm -f public_api_regress *.actual *.expected *.regress
