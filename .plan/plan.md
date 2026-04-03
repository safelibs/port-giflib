# GIFLIB Rust Port Plan

## 1. Context

`giflib` is a small library, but it has a wide compatibility contract:

- The authoritative source tree is [`original/`](/home/yans/code/safelibs/ported/giflib/original). There is currently no `safe/` Rust crate in the workspace, so the port plan must cover crate bootstrap, ABI export, testing, packaging, and downstream replacement from zero.
- The public C API is defined in [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h). It exposes:
  - public structs: `GifColorType`, `ColorMapObject`, `GifImageDesc`, `ExtensionBlock`, `SavedImage`, `GifFileType`, `GraphicsControlBlock`
  - sequential decode/write APIs: `DGif*` and `EGif*`
  - high-level helpers: `GifMakeMapObject`, `GifFreeMapObject`, `GifUnionColorMap`, `GifApplyTranslation`, `GifAddExtensionBlock`, `GifFreeExtensions`, `GifMakeSavedImage`, `GifFreeSavedImages`
  - quantization and drawing helpers: `GifQuantizeBuffer`, `GifAsciiTable8x8`, `GifDrawText8x8`, `GifDrawBox`, `GifDrawRectangle`, `GifDrawBoxedText8x8`
- The actual ELF export contract is broader than the public header. [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols) and `objdump -T original/libgif.so` show that the replacement library must also export `DGifDecreaseImageCounter`, `FreeLastSavedImage`, `_InitHashTable`, `_ClearHashTable`, `_InsertHashTable`, `_ExistsHashTable`, and `openbsd_reallocarray`, plus the data symbol `GifAsciiTable8x8`.
- The installed header surface is narrower than the ELF export surface. [`original/Makefile`](/home/yans/code/safelibs/ported/giflib/original/Makefile) installs only `gif_lib.h`, and the `libgif-dev` package definition in [`original/debian/libgif-dev.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif-dev.install) must remain library-only. `gif_hash.h` and `gif_lib_private.h` stay as build/test oracles, not installed headers, even though the exported hash-helper symbols must remain link-compatible.
- `objdump -T original/libgif.so` shows the current exported `libgif` symbols are all `Base` symbols with SONAME `libgif.so.7`. There is no custom version script to reproduce; the practical ELF requirements are symbol-name parity, symbol-kind parity for `GifAsciiTable8x8`, and SONAME parity.
- The core library build is defined in [`original/Makefile`](/home/yans/code/safelibs/ported/giflib/original/Makefile). The library sources are `dgif_lib.c`, `egif_lib.c`, `gifalloc.c`, `gif_err.c`, `gif_font.c`, `gif_hash.c`, `openbsd-reallocarray.c`, and `quantize.c`. `libutil` and the CLI utilities are built separately and are not part of the `libgif7` package contract, so they are out of scope for the Rust port except as downstream consumers during validation. The build granularity matters: the upstream makefile compiles one object per `.c` file, so `egif_lib.c` and `dgif_lib.c` are each single export-bearing translation units. Without adding an explicit shim or split source in `safe/`, the bootstrap backend cannot keep `DGifSlurp` from original `dgif_lib.c` while replacing the rest of the `DGif*` exports in Rust.
- The installed package contract comes from [`original/debian/control`](/home/yans/code/safelibs/ported/giflib/original/debian/control), [`original/debian/rules`](/home/yans/code/safelibs/ported/giflib/original/debian/rules), [`original/debian/libgif7.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.install), [`original/debian/libgif-dev.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif-dev.install), [`original/debian/pkgconfig/libgif7.pc.in`](/home/yans/code/safelibs/ported/giflib/original/debian/pkgconfig/libgif7.pc.in), and [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols). The safe port must still ship `libgif7` and `libgif-dev`, install `gif_lib.h`, `libgif.so.7`, `libgif.so`, `libgif.a`, `libgif7.pc`, and `libgif.pc`, and remain a drop-in Ubuntu 24.04 replacement. Because the original Debian rules create `libgif.pc` as an absolute symlink into `/usr/lib/$multiarch/pkgconfig/`, the safe packaging must deliberately tighten that detail: `libgif.pc` must be either a real file or a relative symlink to `libgif7.pc` so extracted-package verification stays self-contained and cannot fall through to host pkg-config state. Because the current Ubuntu package version recorded in [`original/debian/changelog`](/home/yans/code/safelibs/ported/giflib/original/debian/changelog) is `5.2.2-1ubuntu1`, the safe packaging must also use a distinct local version suffix such as `5.2.2-1ubuntu1+safelibs1` rather than reusing the stock version verbatim; downstream verification needs that version distinction to prove it exercised the locally built replacement packages instead of the archive copy. That version form retains a Debian revision, so `safe/debian/source/format` must be a non-native format such as `3.0 (quilt)`; `3.0 (native)` is not acceptable for this package plan.
- The in-repo regression suite is already concentrated on the public library surface. [`original/tests/makefile`](/home/yans/code/safelibs/ported/giflib/original/tests/makefile) drives [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c), and the target-to-API mapping matters for phase-local verification:
  - `fileio-regress` is the direct coverage for `DGifOpenFileName`, `DGifOpenFileHandle`, `EGifOpenFileName`, and `EGifOpenFileHandle`
  - `alloc-regress` is the direct coverage for `EGifGCBToExtension`, `EGifGCBToSavedExtension`, `DGifSavedExtensionToGCB`, `GifAddExtensionBlock`, and other allocation/helper APIs
  - `render-regress`, `gifclrmp-regress`, `giffilter-regress`, and `giftext-regress` cover the sequential decoder/read path
  - `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, and `giffix-regress` cover `DGifSlurp`/`EGifSpew`-driven high-level behavior
  - `gif2rgb-regress`, `gifecho-regress`, `drawing-regress`, and `gifwedge-regress` cover quantization plus font/drawing exports
  - the inherited upstream `test` target does not include `gif2rgb-regress`, so any verifier that claims full regression-matrix coverage must invoke `gif2rgb-regress` explicitly rather than relying on `test` alone
  - malformed-input rejection already exists in the harness as the `public_api_regress malformed` subcommand, but the upstream makefile does not expose a dedicated `malformed-regress` or malformed-baseline target, so `safe/tests/Makefile` must add those explicit targets
- The authoritative regression harness and oracle data already exist in [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c), [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests), and [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic). `safe/tests/` should add only safe-specific drivers and helper programs; it must consume the authoritative harness source, committed summaries/icons/dumps/maps/RGB files, and sample GIFs from `original/` in place rather than duplicating them into `safe/tests/`.
- The downstream replacement harness already exists in [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh) and [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json). It covers both runtime and compile-time dependents, including `giflib-tools`, `webp`, `fbi`, `mtpaint`, `tracker-extract`, `libextractor-plugin-gif`, `libcamlimages-ocaml`, `libgdal34t64`, `gdal`, `exactimage`, `sail`, `libwebp`, and `imlib2`.
- The current downstream harness is still wired to a manual original-library install flow. [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh) currently builds [`original/`](/home/yans/code/safelibs/ported/giflib/original) into `/usr/local`, exports `/usr/local/lib` through `LD_LIBRARY_PATH`, asserts `/usr/local/lib/libgif.so.7` linkage, and falls back to `/usr/local/lib/libgif.a` in one compile-time case. Phase 7 must replace those assumptions with local safe-package build/install steps and package-derived installed-path checks while keeping `original/` only as a fixture/source oracle.
- Security scope is already narrowed in [`relevant_cves.json`](/home/yans/code/safelibs/ported/giflib/relevant_cves.json) to:
  - `CVE-2005-2974`: malformed-input invalid-state/null-dereference class in the decoder
  - `CVE-2019-15133`: divide-by-zero / invalid-dimension handling in `DGifSlurp`
- Debian history in [`original/debian/changelog`](/home/yans/code/safelibs/ported/giflib/original/debian/changelog) and patches in [`original/debian/patches/revert-GifQuantizeBuffer-remove-from-lib.patch`](/home/yans/code/safelibs/ported/giflib/original/debian/patches/revert-GifQuantizeBuffer-remove-from-lib.patch) and [`original/debian/patches/giflib_quantize-header.patch`](/home/yans/code/safelibs/ported/giflib/original/debian/patches/giflib_quantize-header.patch) make `GifQuantizeBuffer` part of the compatibility contract. It must remain in the main `libgif` library and header.
- Performance is explicitly required, even if lower priority than compatibility and safety. The likely hotspots are:
  - `DGifDecompressLine` / `DGifDecompressInput` in [`original/dgif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/dgif_lib.c)
  - `EGifCompressLine` / `EGifCompressOutput` in [`original/egif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/egif_lib.c)
  - `GifQuantizeBuffer` in [`original/quantize.c`](/home/yans/code/safelibs/ported/giflib/original/quantize.c)
  - allocation-heavy high-level helpers in [`original/gifalloc.c`](/home/yans/code/safelibs/ported/giflib/original/gifalloc.c)

Public-ABI layout facts on the current Ubuntu 24.04 x86_64 target, measured from the original headers and relevant to the Rust FFI mirrors:

- `GifColorType`: size 3
- `ColorMapObject`: size 24, offsets `ColorCount=0`, `BitsPerPixel=4`, `SortFlag=8`, `Colors=16`
- `GifImageDesc`: size 32, offsets `Left=0`, `Top=4`, `Width=8`, `Height=12`, `Interlace=16`, `ColorMap=24`
- `ExtensionBlock`: size 24, offsets `ByteCount=0`, `Bytes=8`, `Function=16`
- `SavedImage`: size 56, offsets `ImageDesc=0`, `RasterBits=32`, `ExtensionBlockCount=40`, `ExtensionBlocks=48`
- `GifFileType`: size 120, offsets `SWidth=0`, `SHeight=4`, `SColorResolution=8`, `SBackGroundColor=12`, `AspectByte=16`, `SColorMap=24`, `ImageCount=32`, `Image=40`, `SavedImages=72`, `ExtensionBlockCount=80`, `ExtensionBlocks=88`, `Error=96`, `UserData=104`, `Private=112`
- `GraphicsControlBlock`: size 16, offsets `DisposalMode=0`, `UserInputFlag=4`, `DelayTime=8`, `TransparentColor=12`
- `GifHashTableType`: size 32768

One design point should be explicit up front: `GifFileType.Private` is opaque in the public ABI. The Rust port does not need to reproduce the byte layout of `GifFilePrivateType` from [`original/gif_lib_private.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib_private.h) in the final product. It does need to preserve the observable state-machine behavior that those fields drive.

## 2. Generated Workflow Contract

The generated workflow derived from this plan must obey all of the following rules:

- Linear execution only. Do not use `parallel_groups`.
- Use self-contained inline YAML only. Do not use top-level `include`, and do not use phase-level `prompt_file`, `workflow_file`, `workflow_dir`, `checks`, or any other YAML-source indirection.
- Before the generated workflow begins, this refined `.plan/plan.md` must already be committed in `HEAD` in a planning-only commit, or in a commit whose only tracked changes are under `.plan/`. Treat that git-history requirement as a prerequisite, not as a generated workflow phase, so later checkers can reason linearly from committed history.
- Do not use agent-guided `bounce_targets` lists. Every verifier must use exactly one fixed `bounce_target`.
- Every verifier must be an explicit top-level `check` phase.
- Every verifier must stay in the implement block it verifies and must bounce only to that implement phase.
- Emit the top-level workflow phases in exactly this order: `impl_01_scaffold`, `check_01_scaffold_local`, `impl_02_helpers`, `check_02_helpers`, `impl_03_encode`, `check_03_encode`, `impl_04_decode_core`, `check_04_decode_core`, `impl_05_security_baseline`, `check_05_security_baseline`, `impl_06_performance`, `check_06_performance`, `impl_07_packaging`, `check_07_package_build`, `check_07_downstream`, `impl_08_final_cleanup`, `check_08_final`.
- If a verifier needs to run tests, lint, package builds, `readelf`, `nm`, `objdump`, Docker, or benchmark commands, put the exact commands in the checker instructions. Do not model those commands as separate non-agentic phases.
- Because performance is an explicit requirement, the workflow must contain:
  - one dedicated implementation phase for performance work after the library is functionally Rust-complete
  - one explicit top-level `check` phase for performance that compares the Rust port against the original library on exactly these four `public_api_regress` workloads run against authoritative in-repo fixtures: `render-welcome2` (`render original/pic/welcome2.gif`), `render-treescap-interlaced` (`render original/pic/treescap-interlaced.gif`), `highlevel-copy-fire` (`highlevel-copy original/pic/fire.gif`), and `rgb-to-gif-gifgrid` (`rgb-to-gif 3 100 100 < original/tests/gifgrid.rgb`)
  - the performance gate must use `safe/tests/perf_compare.sh`, run 2 warmup samples plus 7 measured samples of 25 inner-loop invocations per workload for each library, compare medians, print one machine-readable `PERF workload=...` line per workload, and fail if any safe/original median ratio exceeds `2.00`
- Because the inherited `safe/tests` default `test` target mirrors [`original/tests/makefile`](/home/yans/code/safelibs/ported/giflib/original/tests/makefile) and therefore omits `gif2rgb-regress`, any verifier that claims full regression-matrix coverage or that can affect quantization/font/drawing behavior must invoke `gif2rgb-regress` explicitly.
- Any phase after phase 4 that edits `safe/src/decode.rs` must rerun `render-regress`, `gifclrmp-regress`, `giffilter-regress`, and `giftext-regress`, because those are the direct low-level sequential decoder regressions and the malformed/slurp regressions do not replace them.
- Any verifier that claims local source-compatibility coverage must separately prove the installed safe header and the ordinary regression build path, not just ABI layout. Concretely: `cmp -s safe/include/gif_lib.h original/gif_lib.h`, then run `safe-header-regress` from `safe/tests/Makefile` with `LIBGIF_INCLUDEDIR="$PWD/safe/include"` and `ORIGINAL_INCLUDEDIR` pointed at an otherwise empty temporary directory so the ordinary `public_api_regress` build can only find `gif_lib.h` under `LIBGIF_INCLUDEDIR`. `safe-header-regress` must build the same ordinary regression binary used by the other public regression targets and run at least `legacy`, `fileio`, and `alloc`. The only target allowed to compile [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) against `$(ORIGINAL_INCLUDEDIR)/gif_lib.h` is `link-compat-regress`.
- Any verifier that claims package-surface coverage must validate the runtime and development packages separately and must prove each required path individually. At minimum, prove that `libgif7` contains one real versioned `libgif.so.7.*` ELF plus the `libgif.so.7` symlink, and prove that `libgif-dev` contains `gif_lib.h`, `libgif.a`, the `libgif.so` linker entry, `libgif7.pc`, and `libgif.pc`. Also prove that the extracted package include trees contain exactly one installed header file, `gif_lib.h`, and reject `gif_hash.h` or `gif_lib_private.h` recursively anywhere under either extracted package tree. When validating pkg-config metadata from extracted package trees, isolate `pkg-config` from host search paths with `PKG_CONFIG_LIBDIR` plus an empty `PKG_CONFIG_PATH`, and require `libgif.pc` to be either a regular file or a relative symlink that resolves inside the extracted `pkgconfig/` directory. Do not use a single alternation grep that can succeed on only one required path.
- Any verifier that claims package-surface coverage must also compile [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) against the extracted `libgif-dev` header and extracted `libgif.a`, then run at least `legacy` and `alloc` with that packaged binary. Package-file presence alone is not enough.
- Any verifier that claims downstream package-replacement coverage must first prove that [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh) no longer contains `/usr/local` assumptions or original-install helpers such as `build_original_giflib` or `assert_uses_original`, that it makes `safe/` available inside the container at `/work/safe`, and that it defines and uses explicit helpers named `build_safe_packages`, `install_safe_packages`, `resolve_installed_shared_libgif`, `resolve_installed_static_libgif`, `assert_links_to_active_shared_libgif`, and `assert_build_uses_active_giflib`. `build_safe_packages` must build local `libgif7` and `libgif-dev` `.deb`s from `/work/safe`, record the exact built artifact paths in `SAFE_RUNTIME_DEB` and `SAFE_DEV_DEB`, and capture their `Package`/`Version` fields into `SAFE_RUNTIME_PACKAGE`, `SAFE_DEV_PACKAGE`, `SAFE_RUNTIME_VERSION`, and `SAFE_DEV_VERSION` via `dpkg-deb -f`. `install_safe_packages` must install those exact `.deb` paths via `dpkg -i`, assert with `dpkg-query -W` that the active `libgif7` and `libgif-dev` versions equal the recorded built versions, and emit `ACTIVE_RUNTIME_VERSION` and `ACTIVE_DEV_VERSION`. `resolve_installed_shared_libgif` and `resolve_installed_static_libgif` must each take a mandatory label, derive the active runtime/development library paths from package metadata such as `dpkg-query -L` plus `ldconfig`, assert ownership with `dpkg-query -S`, export shell variables `ACTIVE_SHARED_LIBGIF`, `ACTIVE_STATIC_LIBGIF`, `ACTIVE_SHARED_OWNER`, and `ACTIVE_STATIC_OWNER`, and print labeled log lines `ACTIVE_SHARED_LIBGIF[$label]=...`, `ACTIVE_STATIC_LIBGIF[$label]=...`, `ACTIVE_SHARED_OWNER[$label]=...`, and `ACTIVE_STATIC_OWNER[$label]=...`. `assert_links_to_active_shared_libgif` must immediately call `resolve_installed_shared_libgif "$label"` and assert that `ldd` output contains the resolved `ACTIVE_SHARED_LIBGIF`; it is reserved for the runtime labels `giflib-tools-runtime`, `webp-runtime`, `fbi-runtime`, `mtpaint-runtime`, `tracker-extract-runtime`, `libextractor-runtime`, `camlimages-runtime`, and `gdal-runtime`. `assert_build_uses_active_giflib` must immediately call both resolvers with the same label, assert either that `ldd` contains `ACTIVE_SHARED_LIBGIF` or that the recorded build link command for that same build contains `ACTIVE_STATIC_LIBGIF`, and print `LINK_ASSERT_MODE[$label]=shared|static`; it is required for every source-build label `gdal-source`, `exactimage-source`, `sail-source`, `libwebp-source`, and `imlib2-source`. Checkers must inspect both the script contents and the captured `./test-original.sh` log; helper-name greps alone are insufficient.
- Any helper script added under `safe/tests/` must resolve the repository root from its own path before reading fixtures or writing derived outputs. Checkers should not assume those scripts are always invoked from the repository root.
- Consume existing artifacts in place instead of rediscovering or regenerating them:
  - [`original/`](/home/yans/code/safelibs/ported/giflib/original) is the authoritative source snapshot and ABI oracle
  - [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests) and [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic) are the authoritative fixtures/oracles
  - [`original/debian/`](/home/yans/code/safelibs/ported/giflib/original/debian) is the authoritative packaging template set
  - [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json), [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh), and [`relevant_cves.json`](/home/yans/code/safelibs/ported/giflib/relevant_cves.json) are authoritative downstream/security inputs
- Treat tracked files under [`original/`](/home/yans/code/safelibs/ported/giflib/original) as immutable oracle inputs. Local builds may create transient untracked artifacts there, but no implementation phase should commit edits, deletions, or replacements under `original/`.
- Preserve the consume-existing-artifacts contract explicitly in every phase that uses headers, fixtures, Debian metadata, downstream inventory, or CVE notes.
- For the regression suite specifically, `safe/tests/Makefile` must compile [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) and compare against committed oracle files under [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests) and sample GIFs under [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic) in place. Do not create committed duplicate copies of those original harness/oracle files under `safe/tests/`.
- Any verifier that enforces the regression-fixture consume-existing-artifacts contract must search recursively under `safe/tests/`, not just at depth 1. Reject any file or symlink named `public_api_regress.c`, `*.summary`, `*.ico`, `*.dmp`, `*.map`, or `*.rgb` anywhere under `safe/tests/`. Reject any `*.gif` anywhere under `safe/tests/` except the derived malformed fixtures under `safe/tests/malformed/`.
- Do not widen the installed header surface. `gif_lib.h` remains the only installed public header; `gif_hash.h` and `gif_lib_private.h` may be used for tests or ABI oracles but must not be installed into `safe/include` or the Debian packages.
- If new malformed-input fixtures are needed, derive them from existing GIF fixtures already in the workspace and record their provenance, rather than inventing unrelated fresh inputs.
- Until a checker explicitly installs doc-generation prerequisites, baseline-oracle commands should prefer:
  - `make -C original libgif.so libgif.a`
  - `make -C original/tests test`
  instead of `make -C original check`, because the top-level build still routes through docs/manpage generation.
- Every implement prompt in the generated workflow must instruct the agent to commit work to git before yielding.
- Do not modify [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json). The downstream harness phases must consume the existing dependent matrix in place.
- From the first phase that exports Rust-defined C ABI symbols onward, the implementation prompt must require those entry points to catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.
- Any verifier that audits the remaining `unsafe` footprint must do more than print matches: it must fail if any remaining `unsafe` block, `unsafe fn`, or `unsafe impl` under `safe/src/` lacks a nearby `SAFETY:` justification comment.
- Any “absence of bootstrap references” verifier must use success-on-absence logic and narrow scope:
  - use `if rg ...; then exit 1; fi`, not bare `rg`
  - search only library build inputs such as `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/`
  - do not search `safe/tests/` or packaging files, because those are allowed to keep referencing original fixtures, original headers, and the original harness for oracle comparison

## 3. Implementation Phases

### Phase 1

- `Phase Name`: Rust Package Scaffold, ABI Lock-In, And Bootstrap Backend
- `Implement Phase ID`: `impl_01_scaffold`
- `Verification Phases`:
  - `check_01_scaffold_local`
    - type: `check`
    - fixed `bounce_target`: `impl_01_scaffold`
    - purpose: verify that `safe/` exists, emits `libgif.so` and `libgif.a` with the correct SONAME and exported symbol set, preserves the public header, provides a ported regression driver under `safe/tests/` that consumes the authoritative harness/oracles from `original/` in place, and already supports object-link compatibility through a temporary bootstrap backend.
    - commands:
      ```bash
      cargo build --manifest-path safe/Cargo.toml --release
      readelf -d safe/target/release/libgif.so | grep -E 'SONAME.*libgif\.so\.7'
      objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
      test "$(objdump -T safe/target/release/libgif.so | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
      /tmp/giflib-abi-layout
      if find safe/tests \( -type f -o -type l \) \( -name 'public_api_regress.c' -o -name '*.summary' -o -name '*.ico' -o -name '*.dmp' -o -name '*.map' -o -name '*.rgb' \) | grep -q .; then
        echo 'unexpected vendored original harness or oracle files under safe/tests' >&2
        exit 1
      fi
      if find safe/tests \( -type f -o -type l \) -name '*.gif' | grep -q .; then
        echo 'unexpected vendored original sample GIFs under safe/tests' >&2
        exit 1
      fi
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test link-compat-regress internal-export-regress
      ```
- `Preexisting Inputs`:
  - [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h)
  - [`original/Makefile`](/home/yans/code/safelibs/ported/giflib/original/Makefile)
  - [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols)
  - [`original/tests/makefile`](/home/yans/code/safelibs/ported/giflib/original/tests/makefile)
  - [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c)
  - all existing fixtures under [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests) and [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic)
- `New Outputs`:
  - buildable Rust crate under `safe/`
  - `libgif.so` and `libgif.a` emitted by Cargo
  - verbatim installed public header at `safe/include/gif_lib.h`
  - ported regression driver under `safe/tests/` that consumes the authoritative harness source and oracle data from `original/` in place
  - explicit `safe-header-regress` target that proves the ordinary regression binary compiles against `safe/include/gif_lib.h` without falling back to the original header
  - ABI layout probe
  - internal-export smoke probe for non-installed but exported helpers
  - temporary bootstrap backend that still uses the original C core through the Rust build
- `File Changes`:
  - create `safe/Cargo.toml`
  - create `safe/build.rs`
  - create `safe/include/gif_lib.h`
  - create `safe/src/lib.rs`
  - create `safe/src/bootstrap.rs` or equivalent bootstrap-only module
  - create `safe/tests/Makefile`
  - create `safe/tests/abi_layout.c`
  - create `safe/tests/internal_exports_smoke.c`
- `Implementation Details`:
  - Set `[lib] name = "gif"` and `crate-type = ["cdylib", "staticlib"]`.
  - Copy [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h) verbatim into `safe/include/gif_lib.h`. Do not redesign the C header.
  - In `build.rs`, compile the original core library sources into a bootstrap archive such as `libgif_legacy.a` and link it into the Rust outputs with whole-archive semantics. Plain static linking is not enough here because otherwise unreferenced legacy objects could be dropped and the exported ABI surface would be incomplete.
  - Use `build.rs` to set the shared-library SONAME to `libgif.so.7`.
  - Port [`original/tests/makefile`](/home/yans/code/safelibs/ported/giflib/original/tests/makefile) into `safe/tests/Makefile`, but parameterize `ORIGINAL_INCLUDEDIR`, `ORIGINAL_TESTS_DIR`, `ORIGINAL_PIC_DIR`, `LIBGIF_INCLUDEDIR`, and `LIBGIF_LIBDIR` so the same tests can run against a local Cargo build or an installed package while still consuming the authoritative harness/oracle files from `original/` in place.
  - In `safe/tests/Makefile`, compile [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) directly from `$(ORIGINAL_TESTS_DIR)` and point every comparison oracle at files under `$(ORIGINAL_TESTS_DIR)` and `$(ORIGINAL_PIC_DIR)`. The ordinary `public_api_regress` target used by `test`, `render-regress`, `alloc-regress`, and the other normal regression targets must compile with `-I$(LIBGIF_INCLUDEDIR)` and must not use `$(ORIGINAL_INCLUDEDIR)` for `gif_lib.h`. Do not vendor `public_api_regress.c`, `*.summary`, `*.ico`, `*.dmp`, `*.map`, `*.rgb`, or the original sample GIFs into `safe/tests/`.
  - Add a `safe-header-regress` target to `safe/tests/Makefile` that rebuilds that ordinary `public_api_regress` binary against `$(LIBGIF_INCLUDEDIR)` only and runs at least `legacy`, `fileio`, and `alloc`. Checkers will invoke it with `ORIGINAL_INCLUDEDIR` pointed at an empty directory, so it must not rely on the original installed header.
  - Add a `link-compat-regress` target to `safe/tests/Makefile` that compiles [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) exactly once to an object with the original header from [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h), links that object against both the safe static library and the safe shared library, and runs at least `legacy`, `alloc`, `render`, `malformed`, and one saved-image copy case using `highlevel-copy $(ORIGINAL_PIC_DIR)/fire.gif` checked against the existing `fire.dmp` or `fire.rgb` oracle under `$(ORIGINAL_TESTS_DIR)`, so the same precompiled object exercises helper, read, write, slurp, copied-extension, and malformed-input behavior. No other target may compile `public_api_regress.c` against `$(ORIGINAL_INCLUDEDIR)/gif_lib.h`.
  - Add an `internal-export-regress` target backed by `safe/tests/internal_exports_smoke.c` that uses `original/gif_hash.h` and explicit test-only prototypes to exercise `_InitHashTable`, `_ClearHashTable`, `_InsertHashTable`, `_ExistsHashTable`, `FreeLastSavedImage`, `DGifDecreaseImageCounter`, and `openbsd_reallocarray` without turning any private header into an installed interface.
  - `safe/tests/abi_layout.c` should assert the sizes and offsets listed in the Context section for the public structs and `GifHashTableType`. It should include `gif_lib.h` from `safe/include` and `gif_hash.h` from `original/` through an explicit test-only include path; it should not require `GifFilePrivateType` layout parity and must not cause `gif_hash.h` to become an installed header.
- `Verification`:
  - Run the command block in `check_01_scaffold_local`.
  - Confirm that `GifAsciiTable8x8` appears as a `DO Base` exported data symbol in `objdump -T` output.
  - Confirm that `safe/tests/Makefile` consumes [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests) and [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic) in place and that `safe/tests/` does not contain committed duplicate copies of the original harness/oracle files anywhere under the tree.
  - Confirm that the internal-export smoke target links and runs against the bootstrap-backed safe library.

### Phase 2

- `Phase Name`: Port Public Types, Memory Helpers, Error Strings, Font, Hash, And Quantization
- `Implement Phase ID`: `impl_02_helpers`
- `Verification Phases`:
  - `check_02_helpers`
    - type: `check`
    - fixed `bounce_target`: `impl_02_helpers`
    - purpose: verify that the helper/data modules are implemented in Rust, that the public layout still matches C, that precompiled-object link compatibility still holds for the newly replaced helper exports, and that the helper-focused regressions including the `GifMakeSavedImage` source-copy path still pass after removing the corresponding bootstrap C sources.
    - commands:
      ```bash
      cargo build --manifest-path safe/Cargo.toml --release
      objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
      test "$(objdump -T safe/target/release/libgif.so | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
      if rg -n 'gifalloc\.c|gif_err\.c|gif_font\.c|gif_hash\.c|openbsd-reallocarray\.c|quantize\.c' safe/build.rs safe/Cargo.toml safe/src; then
        echo 'unexpected helper/data bootstrap source remains in library build inputs' >&2
        exit 1
      fi
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
      /tmp/giflib-abi-layout
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" alloc-regress legacy-regress fileio-regress gifbuild-regress gif2rgb-regress gifecho-regress drawing-regress gifwedge-regress link-compat-regress internal-export-regress
      ```
- `Preexisting Inputs`:
  - phase 1 scaffold
  - [`original/gifalloc.c`](/home/yans/code/safelibs/ported/giflib/original/gifalloc.c)
  - [`original/gif_err.c`](/home/yans/code/safelibs/ported/giflib/original/gif_err.c)
  - [`original/gif_font.c`](/home/yans/code/safelibs/ported/giflib/original/gif_font.c)
  - [`original/gif_hash.c`](/home/yans/code/safelibs/ported/giflib/original/gif_hash.c)
  - [`original/gif_hash.h`](/home/yans/code/safelibs/ported/giflib/original/gif_hash.h)
  - [`original/openbsd-reallocarray.c`](/home/yans/code/safelibs/ported/giflib/original/openbsd-reallocarray.c)
  - [`original/quantize.c`](/home/yans/code/safelibs/ported/giflib/original/quantize.c)
  - Debian quantization notes in [`original/debian/changelog`](/home/yans/code/safelibs/ported/giflib/original/debian/changelog)
- `New Outputs`:
  - Rust FFI mirrors for all public ABI types plus the non-installed but exported `GifHashTableType`
  - Rust implementations of allocation, extension, map, error, font, hash, `openbsd_reallocarray`, and quantization exports
  - reduced bootstrap backend with helper/data C sources removed
- `File Changes`:
  - create `safe/src/ffi.rs`
  - create `safe/src/memory.rs`
  - create `safe/src/helpers.rs`
  - create `safe/src/error.rs`
  - create `safe/src/draw.rs`
  - create `safe/src/hash.rs`
  - create `safe/src/quantize.rs`
  - update `safe/src/lib.rs`
  - update `safe/build.rs`
- `Implementation Details`:
  - Define `#[repr(C)]` Rust mirrors for the public structs using exact C integer widths. For the `_Bool` fields exposed to C (`SortFlag`, `Interlace`, `UserInputFlag`), avoid directly trusting foreign memory as Rust `bool`; use a layout-compatible byte representation plus accessors/normalization so invalid foreign-written bytes do not create UB.
  - Keep C-visible heap allocations on the C allocator. Memory stored in `ColorMapObject.Colors`, `SavedImage.RasterBits`, `ExtensionBlock.Bytes`, `GifFileType.SavedImages`, and extension arrays must be allocated and freed with `malloc`/`calloc`/`realloc`/`free` semantics.
  - Port `GifBitSize`, `GifMakeMapObject`, `GifFreeMapObject`, `GifUnionColorMap`, `GifApplyTranslation`, `GifAddExtensionBlock`, `GifFreeExtensions`, `GifMakeSavedImage`, `GifFreeSavedImages`, and `FreeLastSavedImage`.
  - Fix the shallow-copy weakness in `GifMakeSavedImage`: the Rust version must deep-copy `ExtensionBlock.Bytes`, not just the `ExtensionBlock` array shell.
  - Keep `gifbuild-regress` phase-local in this helper phase: its `highlevel-copy $(PICS)/fire.gif` path is the direct behavioral gate for `GifMakeSavedImage(gif_out, &gif_in->SavedImages[i])` plus copied extension-block contents, even though `EGifSpew` is still supplied by the bootstrap backend here.
  - Port `GifErrorString` exactly, including returning `NULL` for unknown codes.
  - Port the exported font data and drawing helpers exactly enough to preserve the existing `gifecho`, drawing, and wedge fixtures, and export `GifAsciiTable8x8` as read-only data rather than mutable storage.
  - Port `_InitHashTable`, `_ClearHashTable`, `_InsertHashTable`, `_ExistsHashTable`, and `openbsd_reallocarray`.
  - Preserve `openbsd_reallocarray` overflow and zero-size semantics exactly: overflow returns `NULL` with `errno = ENOMEM`, and any zero `nmemb` or `size` returns `NULL`.
  - Port `GifQuantizeBuffer`, preserving deterministic palette ordering and the Debian-restored ABI contract.
  - Remove `gifalloc.c`, `gif_err.c`, `gif_font.c`, `gif_hash.c`, `openbsd-reallocarray.c`, and `quantize.c` from the bootstrap archive once the Rust exports exist and the helper regressions pass.
- `Verification`:
  - `link-compat-regress` is mandatory in this phase because the helper exports moved here must remain callable from objects compiled earlier against [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h).
  - `gifbuild-regress` is mandatory in this phase because it is the direct `GifMakeSavedImage` source-copy and extension-copy behavioral gate; `alloc-regress` alone only covers the `GifMakeSavedImage(&gif, NULL)` allocation path.
  - `internal-export-regress` is the behavioral gate for the non-installed exported helpers moved in this phase.
  - Symbol diff must still be identical to [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols).

### Phase 3

- `Phase Name`: Port Encoder, Extension Writers, And High-Level Spew
- `Implement Phase ID`: `impl_03_encode`
- `Verification Phases`:
  - `check_03_encode`
    - type: `check`
    - fixed `bounce_target`: `impl_03_encode`
    - purpose: verify that the full write path is now in Rust and remains output-compatible with the existing write-side regressions.
    - commands:
      ```bash
      cargo build --manifest-path safe/Cargo.toml --release
      if rg -n 'egif_lib\.c' safe/build.rs safe/Cargo.toml safe/src; then
        echo 'unexpected encoder bootstrap source remains in library build inputs' >&2
        exit 1
      fi
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" legacy-regress fileio-regress alloc-regress gifbuild-regress gifsponge-regress giftool-regress giffix-regress gif2rgb-regress gifecho-regress drawing-regress gifwedge-regress link-compat-regress
      objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
      ```
- `Preexisting Inputs`:
  - phase 2 helper/data modules
  - [`original/egif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/egif_lib.c)
  - existing ported tests and fixtures under `safe/tests/`
- `New Outputs`:
  - Rust encoder implementation
  - Rust extension/GCB write helpers
  - bootstrap backend with `egif_lib.c` removed
- `File Changes`:
  - create `safe/src/state.rs`
  - create `safe/src/io.rs`
  - create `safe/src/gcb.rs`
  - create `safe/src/encode.rs`
  - update `safe/src/lib.rs`
  - update `safe/build.rs`
- `Implementation Details`:
  - Port `EGifOpenFileName`, `EGifOpenFileHandle`, `EGifOpen`, `EGifGetGifVersion`, `EGifSetGifVersion`, `EGifPutScreenDesc`, `EGifPutImageDesc`, `EGifPutLine`, `EGifPutPixel`, `EGifPutComment`, `EGifPutExtensionLeader`, `EGifPutExtensionBlock`, `EGifPutExtensionTrailer`, `EGifPutExtension`, `EGifGCBToExtension`, `EGifGCBToSavedExtension`, `EGifPutCode`, `EGifPutCodeNext`, `EGifCloseFile`, and `EGifSpew`.
  - Implement an opaque Rust `EncoderState` stored behind `GifFileType.Private`; do not expose internal layout, but preserve the write-side state transitions that the original code drives through `BitsPerPixel`, `ClearCode`, `EOFCode`, `RunningCode`, `RunningBits`, `MaxCode1`, `CrntCode`, `CrntShiftState`, `CrntShiftDWord`, and the output buffer.
  - Preserve the exact interlace pass order and extension-block emission rules from the original encoder.
  - Preserve `EGifOpenFileName` file-creation semantics for `GifTestExistence`, preserve `EGifGetGifVersion` automatic promotion to GIF89 when extensions require it, and keep `EGifSetGifVersion` behavior object-compatible with the current `bool`-based ABI.
  - Preserve file-descriptor ownership semantics for the file-backed write APIs: `EGifOpenFileHandle` must assume ownership of the supplied descriptor via `fdopen(..., "wb")`, `EGifCloseFile` must close that descriptor through `fclose` in file-backed mode, and callback-mode `EGifOpen` handles must skip `fclose` while still freeing all other state.
  - Preserve sequential-API misuse behavior and error codes such as `E_GIF_ERR_HAS_SCRN_DSCR`, `E_GIF_ERR_HAS_IMAG_DSCR`, `E_GIF_ERR_NO_COLOR_MAP`, `E_GIF_ERR_DATA_TOO_BIG`, and `E_GIF_ERR_DISK_IS_FULL`.
  - Treat short callback writes and short `FILE *` writes as the same observable failures the C library reports today rather than silently truncating output.
  - Keep the comment-splitting and graphics-control-block byte layout byte-for-byte compatible.
  - Keep `giffix-regress` phase-local in this encoder phase: the `repair` path in [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) exercises `EGifOpen`, `EGifPutScreenDesc`, and `EGifPutImageDesc`, so encoder regressions there must bounce back to `impl_03_encode` rather than surfacing first in the decoder phase.
  - Remove `egif_lib.c` from the bootstrap archive only after the write-side regressions pass.
- `Verification`:
  - The write-heavy regression targets in `check_03_encode`, especially `fileio-regress`, `alloc-regress`, and `giffix-regress`, are the phase-local gate for the file-wrapper, GCB writer, and repaired-output encoder APIs ported here.
  - Object-link compatibility must still succeed for an object compiled against the original header.

### Phase 4

- `Phase Name`: Port Full Decoder, Slurp, And Rust-Only Core
- `Implement Phase ID`: `impl_04_decode_core`
- `Verification Phases`:
  - `check_04_decode_core`
    - type: `check`
    - fixed `bounce_target`: `impl_04_decode_core`
    - purpose: verify that the full read path, `DGifSlurp`, cleanup helpers, file/GCB reader APIs, and internal exported decoder helpers are now in Rust, and that the core library build no longer depends on original C sources.
    - commands:
      ```bash
      cargo build --manifest-path safe/Cargo.toml --release
      if rg -n '\.\./original/.*\.c|cc::Build|legacy backend|gif_legacy' safe/build.rs safe/Cargo.toml safe/src; then
        echo 'unexpected bootstrap reference remains in library build inputs after decoder port' >&2
        exit 1
      fi
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" render-regress gifclrmp-regress giffilter-regress giftext-regress legacy-regress fileio-regress alloc-regress gifbuild-regress gifsponge-regress giftool-regress giffix-regress link-compat-regress internal-export-regress
      objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
      ```
- `Preexisting Inputs`:
  - phase 3 write path
  - [`original/dgif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/dgif_lib.c)
  - current `safe/tests/` regression tree
- `New Outputs`:
  - Rust sequential decoder implementation
  - Rust `DGifSlurp`
  - Rust `DGifDecreaseImageCounter`
  - Rust extension/GCB read helpers
  - bootstrap-free Rust core library build
- `File Changes`:
  - create `safe/src/decode.rs`
  - create `safe/src/slurp.rs`
  - update `safe/src/io.rs`
  - update `safe/src/gcb.rs`
  - update `safe/src/state.rs`
  - update `safe/src/lib.rs`
  - update `safe/build.rs`
- `Implementation Details`:
  - Port `DGifOpenFileName`, `DGifOpenFileHandle`, `DGifOpen`, `DGifGetScreenDesc`, `DGifGetGifVersion`, `DGifGetRecordType`, `DGifGetImageHeader`, `DGifGetImageDesc`, `DGifGetLine`, `DGifGetPixel`, `DGifGetExtension`, `DGifGetExtensionNext`, `DGifExtensionToGCB`, `DGifSavedExtensionToGCB`, `DGifCloseFile`, `DGifGetCode`, `DGifGetCodeNext`, `DGifGetLZCodes`, `DGifDecreaseImageCounter`, and `DGifSlurp`.
  - Implement an opaque Rust `DecoderState` behind `GifFileType.Private`; preserve the original state-machine behavior for code-size setup, LZW buffering, pixel countdown, and extension streaming.
  - Preserve current malformed-image handling for bad code sizes, broken prefixes, early EOF, and wrong record types. Compatibility here matters more than inventing nicer Rust-specific errors.
  - Treat short callback reads, short `FILE *` reads, and premature block termination as the same `D_GIF_ERR_READ_FAILED` or `D_GIF_ERR_EOF_TOO_SOON` outcomes the current decoder exposes.
  - Preserve file-descriptor ownership semantics for the file-backed read APIs: `DGifOpenFileHandle` must assume ownership of the supplied descriptor via `fdopen(..., "rb")`, `DGifCloseFile` must close that descriptor through `fclose` in file-backed mode, and callback-mode `DGifOpen` handles must skip `fclose` while still freeing all other state.
  - Preserve callback-mode behavior through `InputFunc` and file-wrapper behavior through file handles and `fdopen`/`fclose` equivalents.
  - Because [`original/dgif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/dgif_lib.c) is a single export-bearing translation unit in the bootstrap backend, retire that whole original decoder object in this phase instead of trying to keep C `DGifSlurp` while replacing the other `DGif*` exports in Rust.
  - After this phase, `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/` should not require original C library sources to build `libgif`.
- `Verification`:
  - `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `legacy-regress`, `fileio-regress`, `alloc-regress`, `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, and `internal-export-regress` must all pass.
  - This is the first phase after which the core library build must be Rust-only.

### Phase 5

- `Phase Name`: Security Hardening, Malformed Fixtures, And Compatibility Baseline
- `Implement Phase ID`: `impl_05_security_baseline`
- `Verification Phases`:
  - `check_05_security_baseline`
    - type: `check`
    - fixed `bounce_target`: `impl_05_security_baseline`
    - purpose: verify that the derived malformed fixtures are committed with provenance, that the original malformed-input compatibility baseline is captured as an explicit artifact, that the safe library matches that baseline while rejecting the inputs without crashes or panics, and that decoder hardening does not regress the direct sequential decoder APIs.
    - commands:
      ```bash
      make -C original libgif.so libgif.a
      cargo build --manifest-path safe/Cargo.toml --release
      if rg -n '\.\./original/.*\.c|cc::Build|legacy backend|gif_legacy' safe/build.rs safe/Cargo.toml safe/src; then
        echo 'unexpected bootstrap reference remains in library build inputs during security hardening' >&2
        exit 1
      fi
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      cc -std=gnu99 -Wall -Wextra -I"$PWD/original" safe/tests/malformed_observe.c original/libgif.a -o /tmp/malformed_observe.original
      safe/tests/capture_malformed_baseline.sh /tmp/malformed_observe.original "$PWD/safe/tests/malformed" > /tmp/original-malformed-baseline.txt
      diff -u safe/tests/malformed/original-baseline.txt /tmp/original-malformed-baseline.txt
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" render-regress gifclrmp-regress giffilter-regress giftext-regress gifbuild-regress gifsponge-regress giftool-regress giffix-regress malformed-regress malformed-baseline-regress link-compat-regress internal-export-regress
      objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
      ```
- `Preexisting Inputs`:
  - phase 4 Rust-only decoder core
  - [`original/Makefile`](/home/yans/code/safelibs/ported/giflib/original/Makefile)
  - [`relevant_cves.json`](/home/yans/code/safelibs/ported/giflib/relevant_cves.json)
  - [`original/NEWS`](/home/yans/code/safelibs/ported/giflib/original/NEWS)
  - current `safe/tests/` regression tree
- `New Outputs`:
  - malformed-input regression fixtures plus provenance notes
  - deterministic malformed-input observation helper
  - deterministic malformed-baseline capture script
  - committed original malformed-input compatibility baseline artifact keyed by malformed fixture basename
  - hardened decoder cleanup/error-path behavior for the selected malformed inputs
- `File Changes`:
  - update `safe/src/decode.rs`
  - update `safe/src/slurp.rs`
  - update `safe/src/lib.rs`
  - update `safe/tests/Makefile`
  - create `safe/tests/malformed_observe.c`
  - create `safe/tests/capture_malformed_baseline.sh`
  - create `safe/tests/malformed/`
  - create `safe/tests/malformed/manifest.txt` or equivalent provenance file
  - create `safe/tests/malformed/original-baseline.txt`
- `Implementation Details`:
  - Replace arithmetic that can panic in safe Rust with checked arithmetic and explicit `GIF_ERROR` returns.
  - For `CVE-2019-15133`, derive at least one malformed fixture from an existing sample GIF that forces zero or otherwise invalid image dimensions and verify it is rejected without divide-by-zero, panic, or abort.
  - For `CVE-2005-2974`, derive at least one malformed fixture from an existing sample GIF that drives the decoder into a partial-image / invalid-state cleanup path and verify rejection without null-dereference-class behavior.
  - Record in `safe/tests/malformed/manifest.txt` which original fixture each malformed case was derived from and what bytes were changed. That keeps the consume-existing-artifacts contract explicit.
  - Add `safe/tests/malformed_observe.c` as a deterministic test-only helper that emits one tab-separated line per malformed fixture with the fixture basename as field 1, followed by integer fields `open_nonnull`, `open_error`, `slurp_rc`, `gif_error_after_slurp`, `close_rc`, and `close_error`.
  - Add `safe/tests/capture_malformed_baseline.sh` to resolve the repository root from its own path, run the helper only over the committed malformed `*.gif` inputs in `safe/tests/malformed/` in lexical order, exclude metadata files such as `manifest.txt` and `original-baseline.txt`, and write `safe/tests/malformed/original-baseline.txt`.
  - Add a `malformed-baseline-regress` target to `safe/tests/Makefile` that compiles the observation helper against the safe library, captures the same tab-separated matrix, and diffs it against `safe/tests/malformed/original-baseline.txt`.
  - Capture the baseline by compiling `safe/tests/malformed_observe.c` against the original library built from [`original/Makefile`](/home/yans/code/safelibs/ported/giflib/original/Makefile) and commit the resulting `safe/tests/malformed/original-baseline.txt` in this phase.
  - Keep the safe library's observable malformed-input results identical to the committed baseline for the committed fixtures. If a candidate malformed fixture would force a safety-motivated behavioral divergence, adjust or replace that fixture instead of relying on an undocumented mismatch.
  - Restrict `safe/src/lib.rs` changes in this phase to decoder/slurp hardening and decoder-side error/panic boundaries; defer unrelated helper, encoder, quantization, and package-surface refactors to later phases so the phase-local verifier can stay decoder-focused.
- `Verification`:
  - The baseline diff against `safe/tests/malformed/original-baseline.txt`, plus `malformed-regress` and `malformed-baseline-regress`, are the required malformed-input compatibility gates.
  - Because this phase may edit `safe/src/decode.rs`, `render-regress`, `gifclrmp-regress`, `giffilter-regress`, and `giftext-regress` are required low-level decoder gates.
  - `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, and `internal-export-regress` must still pass after the hardening changes.

### Phase 6

- `Phase Name`: Performance Baseline And Hot-Path Optimization
- `Implement Phase ID`: `impl_06_performance`
- `Verification Phases`:
  - `check_06_performance`
    - type: `check`
    - fixed `bounce_target`: `impl_06_performance`
    - purpose: verify that the Rust port stays within the fixed performance budget on the exact decode, slurp/spew, and quantization workloads named in the workflow contract, and that performance tuning does not regress behavior.
    - commands:
      ```bash
      make -C original libgif.so libgif.a
      cargo build --manifest-path safe/Cargo.toml --release
      cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c original/libgif.a -o /tmp/public_api_regress.original
      cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
      safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf.log
      grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf.log
      grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf.log
      grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf.log
      grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf.log
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" render-regress gifclrmp-regress giffilter-regress giftext-regress giftool-regress gif2rgb-regress
      ```
- `Preexisting Inputs`:
  - phase 5 Rust-only library
  - [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c)
  - [`original/pic/welcome2.gif`](/home/yans/code/safelibs/ported/giflib/original/pic/welcome2.gif)
  - [`original/pic/treescap-interlaced.gif`](/home/yans/code/safelibs/ported/giflib/original/pic/treescap-interlaced.gif)
  - [`original/pic/fire.gif`](/home/yans/code/safelibs/ported/giflib/original/pic/fire.gif)
  - [`original/tests/gifgrid.rgb`](/home/yans/code/safelibs/ported/giflib/original/tests/gifgrid.rgb)
  - original baseline library build from [`original/Makefile`](/home/yans/code/safelibs/ported/giflib/original/Makefile)
- `New Outputs`:
  - repeatable performance comparison script with fixed workload IDs and a fixed `2.00` ratio gate
  - any release-profile and hot-path code improvements needed to keep the Rust port competitive
- `File Changes`:
  - create `safe/tests/perf_compare.sh`
  - update `safe/Cargo.toml`
  - update hot-path modules such as `safe/src/decode.rs`, `safe/src/encode.rs`, `safe/src/quantize.rs`, `safe/src/helpers.rs`, and `safe/src/state.rs`
- `Implementation Details`:
  - Create `safe/tests/perf_compare.sh` as a deterministic local benchmark script that:
    - accepts two `public_api_regress` binaries, one linked to original `libgif.a` and one linked to safe `libgif.a`
    - resolves the repository root from its own path instead of assuming the caller's `PWD`
    - benchmarks exactly these four workload IDs and commands, always using the authoritative fixtures in place under that computed repository root: `render-welcome2` (`render "$repo_root/original/pic/welcome2.gif"`), `render-treescap-interlaced` (`render "$repo_root/original/pic/treescap-interlaced.gif"`), `highlevel-copy-fire` (`highlevel-copy "$repo_root/original/pic/fire.gif"`), and `rgb-to-gif-gifgrid` (`rgb-to-gif 3 100 100` with stdin from `"$repo_root/original/tests/gifgrid.rgb"`)
    - performs exactly 2 warmup samples and 7 measured samples per workload for each binary
    - makes each sample execute exactly 25 inner-loop invocations of the workload before recording elapsed time, so the median is not dominated by timer noise on the small fixture set
    - measures median elapsed wall-clock time per workload for the original-linked binary and the safe-linked binary separately
    - prints one machine-readable line per workload in the form `PERF workload=<id> original_median_s=<seconds> safe_median_s=<seconds> ratio=<safe/original> threshold=2.00`
    - exits nonzero if any reported ratio exceeds `2.00`
  - Use the fixed `2.00` regression threshold in this phase and in final verification. Do not leave the acceptable slowdown to checker or workflow-writer discretion.
  - Prefer performance work that preserves the final safety goal:
    - fixed-size arrays and reusable scratch storage for LZW state instead of allocation-heavy maps/vectors in hot loops
    - buffered slice iteration rather than per-pixel abstraction overhead
    - avoiding unnecessary cloning in slurp/spew helpers
    - preserving deterministic quantizer ordering while reducing pointer chasing
  - Tune `profile.release` only if measurements justify it. `thin` LTO and fewer codegen units are reasonable; `panic = "abort"` is not compatible with the required FFI panic boundaries.
- `Verification`:
  - `safe/tests/perf_compare.sh` is the required performance gate, and it must emit passing `PERF` lines for `render-welcome2`, `render-treescap-interlaced`, `highlevel-copy-fire`, and `rgb-to-gif-gifgrid`.
  - Because this phase may tune `safe/src/decode.rs`, `check_06_performance` must rerun `render-regress`, `gifclrmp-regress`, `giffilter-regress`, and `giftext-regress` in addition to `giftool-regress` and `gif2rgb-regress`.
  - Re-run a decode-heavy, encode-heavy, and quantize-heavy functional subset after any tuning.

### Phase 7

- `Phase Name`: Debian Packaging And Drop-In Downstream Harness
- `Implement Phase ID`: `impl_07_packaging`
- `Verification Phases`:
  - `check_07_package_build`
    - type: `check`
    - fixed `bounce_target`: `impl_07_packaging`
    - purpose: verify that `safe/` builds installable Debian packages with the expected names, distinct local version suffix, files, SONAME, pkg-config metadata, exported symbols, and no private headers anywhere in the extracted package trees.
    - commands:
      ```bash
      grep -x '3.0 (quilt)' safe/debian/source/format
      rm -f safe/../libgif7_*.deb safe/../libgif-dev_*.deb safe/../libgif7-dbgsym_*.deb
      (cd safe && dpkg-buildpackage -us -uc -b)
      multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
      runtime_deb="$(ls -1 safe/../libgif7_*.deb)"
      dev_deb="$(ls -1 safe/../libgif-dev_*.deb)"
      test "$(dpkg-deb -f "$runtime_deb" Package)" = "libgif7"
      test "$(dpkg-deb -f "$dev_deb" Package)" = "libgif-dev"
      runtime_version="$(dpkg-deb -f "$runtime_deb" Version)"
      dev_version="$(dpkg-deb -f "$dev_deb" Version)"
      test "$runtime_version" = "$dev_version"
      case "$runtime_version" in
        *+safelibs*) ;;
        *)
          echo 'expected local safelibs version suffix in Debian package version' >&2
          exit 1
          ;;
      esac
      runtime_tmp="$(mktemp -d)"
      dev_tmp="$(mktemp -d)"
      dpkg-deb -x "$runtime_deb" "$runtime_tmp"
      dpkg-deb -x "$dev_deb" "$dev_tmp"
      runtime_real="$(find "$runtime_tmp/usr/lib/$multiarch" -maxdepth 1 -type f -name 'libgif.so.7.*' | sort)"
      test "$(printf '%s\n' "$runtime_real" | sed '/^$/d' | wc -l)" -eq 1
      runtime_real="$(printf '%s\n' "$runtime_real" | head -n1)"
      readelf -d "$runtime_real" | grep -E 'SONAME.*libgif\.so\.7'
      objdump -T "$runtime_real" | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/pkg-safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/pkg-safe-symbols.txt
      test "$(objdump -T "$runtime_real" | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
      test -L "$runtime_tmp/usr/lib/$multiarch/libgif.so.7"
      test "$(readlink "$runtime_tmp/usr/lib/$multiarch/libgif.so.7")" = "$(basename "$runtime_real")"
      test -f "$dev_tmp/usr/include/gif_lib.h"
      find "$runtime_tmp" "$dev_tmp" -path '*/usr/include/*' \( -type f -o -type l \) | LC_ALL=C sort > /tmp/pkg-headers.txt
      printf '%s\n' "$dev_tmp/usr/include/gif_lib.h" > /tmp/pkg-headers-expected.txt
      diff -u /tmp/pkg-headers-expected.txt /tmp/pkg-headers.txt
      test -f "$dev_tmp/usr/lib/$multiarch/libgif.a"
      cmp -s "$dev_tmp/usr/include/gif_lib.h" original/gif_lib.h
      cc -std=gnu99 -Wall -Wextra -I"$dev_tmp/usr/include" original/tests/public_api_regress.c "$dev_tmp/usr/lib/$multiarch/libgif.a" -o /tmp/public_api_regress.pkg
      /tmp/public_api_regress.pkg legacy > /tmp/pkg-legacy.summary
      diff -u original/tests/legacy.summary /tmp/pkg-legacy.summary
      /tmp/public_api_regress.pkg alloc > /tmp/pkg-alloc.summary
      diff -u original/tests/alloc.summary /tmp/pkg-alloc.summary
      test -L "$dev_tmp/usr/lib/$multiarch/libgif.so"
      test "$(readlink "$dev_tmp/usr/lib/$multiarch/libgif.so")" = "libgif.so.7"
      pkgconfig_dir="$dev_tmp/usr/lib/$multiarch/pkgconfig"
      pkgcfg() {
        env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config "$@"
      }
      test -f "$pkgconfig_dir/libgif7.pc"
      grep -F 'Name: libgif' "$pkgconfig_dir/libgif7.pc"
      grep -F 'Libs: -L${libdir} -lgif' "$pkgconfig_dir/libgif7.pc"
      libgif_pc="$pkgconfig_dir/libgif.pc"
      if [ -L "$libgif_pc" ]; then
        test "$(readlink "$libgif_pc")" = "libgif7.pc"
      else
        test -f "$libgif_pc"
      fi
      grep -F 'Name: libgif' "$libgif_pc"
      grep -F 'Libs: -L${libdir} -lgif' "$libgif_pc"
      pkgcfg --exists libgif7
      test "$(pkgcfg --variable=libdir libgif7)" = "/usr/lib/$multiarch"
      test "$(pkgcfg --variable=includedir libgif7)" = "/usr/include"
      pkgcfg --exists libgif
      test "$(pkgcfg --variable=libdir libgif)" = "/usr/lib/$multiarch"
      test "$(pkgcfg --variable=includedir libgif)" = "/usr/include"
      if find "$runtime_tmp" "$dev_tmp" \( -type f -o -type l \) \( -name 'gif_hash.h' -o -name 'gif_lib_private.h' \) | grep -q .; then
        echo 'unexpected private header installed in Debian packages' >&2
        exit 1
      fi
      ```
  - `check_07_downstream`
    - type: `check`
    - fixed `bounce_target`: `impl_07_packaging`
    - purpose: verify that the modified downstream harness no longer relies on `/usr/local`, explicitly builds and installs the exact local safe packages it just produced, proves those local packages are the active installed `libgif7` and `libgif-dev`, routes every linkage assertion through the package-derived helper paths with fixed labels, and then proves that all sampled dependents still compile and run.
    - commands:
      ```bash
      if rg -n '/usr/local|build_original_giflib|assert_uses_original' test-original.sh; then
        echo 'stale original-install assumptions remain in downstream harness' >&2
        exit 1
      fi
      rg -n '^COPY[[:space:]]+\\.?/?safe/?[[:space:]]+/work/safe/?$' test-original.sh
      rg -n '^build_safe_packages\(\)' test-original.sh
      rg -n '^install_safe_packages\(\)' test-original.sh
      rg -n '^resolve_installed_shared_libgif\(\)' test-original.sh
      rg -n '^resolve_installed_static_libgif\(\)' test-original.sh
      rg -n '^assert_links_to_active_shared_libgif\(\)' test-original.sh
      rg -n '^assert_build_uses_active_giflib\(\)' test-original.sh
      test "$(rg -n '\bbuild_safe_packages\b' test-original.sh | awk 'END { print NR + 0 }')" -ge 2
      test "$(rg -n '\binstall_safe_packages\b' test-original.sh | awk 'END { print NR + 0 }')" -ge 2
      rg -n 'dpkg-buildpackage -us -uc -b' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_RUNTIME_DEB" Package' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_RUNTIME_DEB" Version' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_DEV_DEB" Package' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_DEV_DEB" Version' test-original.sh
      rg -n 'dpkg -i "\$SAFE_RUNTIME_DEB" "\$SAFE_DEV_DEB"' test-original.sh
      rg -n 'dpkg-query[[:space:]]+-W.*libgif7' test-original.sh
      rg -n 'dpkg-query[[:space:]]+-W.*libgif-dev' test-original.sh
      rg -n 'dpkg(-query)?[[:space:]].*-L[[:space:]]+libgif7\b' test-original.sh
      rg -n 'dpkg(-query)?[[:space:]].*-L[[:space:]]+libgif-dev\b' test-original.sh
      rg -n 'dpkg-query[[:space:]]+-S' test-original.sh
      rg -n 'ldconfig' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "giflib-tools-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "webp-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "fbi-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "mtpaint-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "tracker-extract-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "libextractor-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "camlimages-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "gdal-runtime"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "gdal-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "exactimage-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "sail-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "libwebp-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "imlib2-source"' test-original.sh
      bash -o pipefail -c './test-original.sh | tee /tmp/test-original.log'
      grep -E '^SAFE_RUNTIME_DEB=.*/libgif7_.*\.deb$' /tmp/test-original.log
      grep -E '^SAFE_DEV_DEB=.*/libgif-dev_.*\.deb$' /tmp/test-original.log
      grep -x 'SAFE_RUNTIME_PACKAGE=libgif7' /tmp/test-original.log
      grep -x 'SAFE_DEV_PACKAGE=libgif-dev' /tmp/test-original.log
      safe_runtime_version="$(sed -n 's/^SAFE_RUNTIME_VERSION=//p' /tmp/test-original.log | tail -n1)"
      safe_dev_version="$(sed -n 's/^SAFE_DEV_VERSION=//p' /tmp/test-original.log | tail -n1)"
      active_runtime_version="$(sed -n 's/^ACTIVE_RUNTIME_VERSION=//p' /tmp/test-original.log | tail -n1)"
      active_dev_version="$(sed -n 's/^ACTIVE_DEV_VERSION=//p' /tmp/test-original.log | tail -n1)"
      test -n "$safe_runtime_version"
      test "$safe_runtime_version" = "$safe_dev_version"
      case "$safe_runtime_version" in
        *+safelibs*) ;;
        *)
          echo 'expected local safelibs version in downstream harness output' >&2
          exit 1
          ;;
      esac
      test "$safe_runtime_version" = "$active_runtime_version"
      test "$safe_dev_version" = "$active_dev_version"
      grep -E '^ACTIVE_SHARED_LIBGIF\[giflib-tools-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[giflib-tools-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[webp-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[webp-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[fbi-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[fbi-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[mtpaint-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[mtpaint-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[tracker-extract-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[tracker-extract-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[libextractor-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[libextractor-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[camlimages-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[camlimages-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[gdal-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[gdal-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[gdal-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[gdal-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[gdal-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[gdal-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[gdal-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[exactimage-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[exactimage-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[exactimage-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[exactimage-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[exactimage-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[sail-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[sail-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[sail-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[sail-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[sail-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[libwebp-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[libwebp-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[libwebp-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[libwebp-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[libwebp-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[imlib2-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[imlib2-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[imlib2-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[imlib2-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[imlib2-source\]=(shared|static)$' /tmp/test-original.log
      ```
- `Preexisting Inputs`:
  - phase 6 Rust-only and performance-tuned library
  - [`original/debian/control`](/home/yans/code/safelibs/ported/giflib/original/debian/control)
  - [`original/debian/rules`](/home/yans/code/safelibs/ported/giflib/original/debian/rules)
  - [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols)
  - [`original/debian/libgif7.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.install)
  - [`original/debian/libgif-dev.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif-dev.install)
  - [`original/debian/pkgconfig/libgif7.pc.in`](/home/yans/code/safelibs/ported/giflib/original/debian/pkgconfig/libgif7.pc.in)
  - [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic)
  - [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh)
  - [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json)
- `New Outputs`:
  - Debian packaging for locally versioned `libgif7` and `libgif-dev`
  - modified Docker harness that builds the exact local safe packages, installs them, logs built/active package identity markers plus labeled resolved library/archive paths for every dependent linkage assertion, and uses `original/` only as a fixture/source oracle instead of building the original library into `/usr/local`
- `File Changes`:
  - create `safe/debian/control`
  - create `safe/debian/rules`
  - create `safe/debian/changelog`
  - create `safe/debian/libgif7.symbols`
  - create `safe/debian/libgif7.install`
  - create `safe/debian/libgif-dev.install`
  - create `safe/debian/pkgconfig/libgif7.pc.in`
  - create `safe/debian/source/format`
  - create any minimal additional Debian support files required by debhelper
  - update `test-original.sh`
- `Implementation Details`:
  - Adapt the packaging from `original/debian/` rather than inventing a new package structure. Preserve binary package names `libgif7` and `libgif-dev`.
  - Make `safe/` a library-only Debian source package. Do not build or ship a `giflib-tools` binary package from `safe/`; keep using Ubuntu’s existing `giflib-tools` package and the other downstream packages as consumers of the replacement `libgif7` and `libgif-dev`.
  - Update `safe/debian/control` so the safe packaging declares the Rust build dependencies it actually uses (`cargo`, `rustc`, and any debhelper cargo helper invoked by `debian/rules`) and drops doc-only dependencies such as `xmlto` unless the safe packaging truly consumes them.
  - Set `safe/debian/changelog` to a distinct local version derived from the current Ubuntu package version, for example `5.2.2-1ubuntu1+safelibs1`. Do not reuse the stock `5.2.2-1ubuntu1` version verbatim, because the downstream harness must be able to prove it installed the locally built replacement packages.
  - Set `safe/debian/source/format` to `3.0 (quilt)`. Do not use `3.0 (native)`, because the required local version string keeps the upstream `-1ubuntu1` Debian revision component.
  - Have `safe/debian/rules` drive the Rust build directly, stage a real versioned multiarch shared object filename that matches the upstream `LIBVER`/SONAME scheme, create the `libgif.so.7` and `libgif.so` symlinks, and install the header, static archive, and pkg-config files into the same multiarch locations the existing Debian templates expect.
  - Do not widen scope to port the CLI tools. Keep using the distribution’s `giflib-tools` and other downstream packages; the safe library package must be able to replace only the library/devel packages underneath them.
  - Install the multiarch library, symlinks, header, static archive, and pkg-config files exactly where the existing Debian templates expect them, and do not install `gif_hash.h` or `gif_lib_private.h`.
  - Install `libgif.pc` as either a regular file or a relative symlink `libgif.pc -> libgif7.pc`. Do not reuse the original absolute symlink form, because the phase-local extracted-package validation must stay self-contained and independent of the host `/usr/lib` tree.
  - Keep the symbol file semantically identical to [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols).
  - Update [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh) so the container:
    - keeps `original/` available as an existing fixture source, because the runtime smoke tests already choose `SAMPLE_GIF` from `original/pic/`
    - copies `safe/` into the container at `/work/safe`
    - installs Rust packaging/build dependencies
    - builds the local `.deb`s
    - installs those packages over Ubuntu’s stock `libgif7`/`libgif-dev`
    - reruns the existing runtime and compile-time dependent checks unchanged wherever possible
  - Remove the current manual original-library build/install path from [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh). After this phase, `original/` stays in the container only as an oracle for fixtures or source-inspection, not as something the harness compiles or installs.
  - Add explicit harness helpers named `build_safe_packages`, `install_safe_packages`, `resolve_installed_shared_libgif`, `resolve_installed_static_libgif`, `assert_links_to_active_shared_libgif`, and `assert_build_uses_active_giflib`. `build_safe_packages` must build the local `.deb`s from the staged `safe/` tree, store their exact paths in `SAFE_RUNTIME_DEB` and `SAFE_DEV_DEB`, capture `Package`/`Version` fields into `SAFE_RUNTIME_PACKAGE`, `SAFE_DEV_PACKAGE`, `SAFE_RUNTIME_VERSION`, and `SAFE_DEV_VERSION` via `dpkg-deb -f`, and print those key/value lines. `install_safe_packages` must install those exact `.deb` files via `dpkg -i`, assert with `dpkg-query -W` that the active `libgif7` and `libgif-dev` versions equal the recorded built versions, and print `ACTIVE_RUNTIME_VERSION=` and `ACTIVE_DEV_VERSION=`. `resolve_installed_shared_libgif` must derive the active runtime `libgif.so.7` path from `dpkg -L libgif7` or `dpkg-query -L libgif7` plus `ldconfig -p`, assert ownership with `dpkg-query -S`, export `ACTIVE_SHARED_LIBGIF` plus `ACTIVE_SHARED_OWNER`, and print labeled lines `ACTIVE_SHARED_LIBGIF[$label]=...` and `ACTIVE_SHARED_OWNER[$label]=...`. `resolve_installed_static_libgif` must derive the packaged development archive path from `dpkg -L libgif-dev` or `dpkg-query -L libgif-dev`, assert ownership with `dpkg-query -S`, export `ACTIVE_STATIC_LIBGIF` plus `ACTIVE_STATIC_OWNER`, and print labeled lines `ACTIVE_STATIC_LIBGIF[$label]=...` and `ACTIVE_STATIC_OWNER[$label]=...`. `assert_links_to_active_shared_libgif` must call `resolve_installed_shared_libgif "$label"` immediately before running `ldd` and must require the resulting `ACTIVE_SHARED_LIBGIF` path to appear in that `ldd` log; use it only for the runtime labels `giflib-tools-runtime`, `webp-runtime`, `fbi-runtime`, `mtpaint-runtime`, `tracker-extract-runtime`, `libextractor-runtime`, `camlimages-runtime`, and `gdal-runtime`. `assert_build_uses_active_giflib` must call both resolvers with the same label immediately before checking a source-built artifact, must accept either shared linkage via `ldd` containing `ACTIVE_SHARED_LIBGIF` or static linkage via the recorded build link command containing `ACTIVE_STATIC_LIBGIF`, and must print `LINK_ASSERT_MODE[$label]=shared|static`; use it for every source-build label `gdal-source`, `exactimage-source`, `sail-source`, `libwebp-source`, and `imlib2-source` at the exact assertion sites listed in `check_07_downstream`.
  - Remove helper functions and assertions tied to the original `/usr/local` install flow, including `build_original_giflib` and `assert_uses_original`, rather than leaving dead code alongside the new package-based path.
  - Replace the current `/usr/local/lib/libgif.so.7` assertions with package-path assertions derived from the installed `libgif7` package contents, for example via `gcc -print-multiarch`, `dpkg -L libgif7`, and `ldconfig`, so the harness verifies the packaged replacement rather than a manually installed `/usr/local` build.
  - Replace the `/usr/local/lib/libgif.a` fallback check in the downstream source-build cases with the packaged development-archive path resolved from `dpkg -L libgif-dev`, so both shared-link and static-link assertions point at the installed safe packages rather than the old manual install location.
  - For every source-build label that can legitimately fall back to static linkage, capture the exact final linker invocation before calling `assert_build_uses_active_giflib`. Reuse project-native evidence where available, such as CMake `link.txt`, and otherwise enable verbose build output or wrap the linker so the helper inspects the real final link command instead of guessing from configure logs.
  - Keep [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json) unchanged; the harness must continue to validate exactly that downstream matrix.
- `Verification`:
  - `check_07_package_build` must separately prove the runtime package ships the real versioned shared object plus `libgif.so.7`, and the development package ships `gif_lib.h`, `libgif.a`, `libgif.so`, `libgif7.pc`, and `libgif.pc`, while also proving that the extracted include trees contain only `gif_lib.h` and no private headers anywhere.
  - `check_07_package_build` must compile [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) against the extracted `libgif-dev` header and extracted `libgif.a`, then run at least `legacy` and `alloc`, so the installed package header is verified as a real source-compatible development surface rather than only a copied file.
  - `check_07_package_build` must validate `libgif7.pc` and `libgif.pc` individually with host-isolated `pkg-config` queries against the extracted package tree. `libgif.pc` may be a regular file or a relative symlink to `libgif7.pc`, but it must not be an absolute symlink into `/usr/lib`.
  - `check_07_downstream` must prove the harness no longer references `/usr/local`, `build_original_giflib`, or `assert_uses_original`, that it defines and uses the named helpers `build_safe_packages`, `install_safe_packages`, `resolve_installed_shared_libgif`, `resolve_installed_static_libgif`, `assert_links_to_active_shared_libgif`, and `assert_build_uses_active_giflib`, that every runtime label uses `assert_links_to_active_shared_libgif`, that every source-build label uses `assert_build_uses_active_giflib`, and that the captured `./test-original.sh` log contains the fixed labeled `ACTIVE_SHARED_*` markers for runtime labels plus `ACTIVE_SHARED_*`, `ACTIVE_STATIC_*`, and `LINK_ASSERT_MODE[...]` markers for every source-build label before the dependent matrix can count as downstream validation.
  - `check_07_package_build` and `check_07_downstream` are both required and both bounce back to `impl_07_packaging`.

### Phase 8

- `Phase Name`: Final Safe-Only Cleanup, Unsafe Audit, And Full Matrix Verification
- `Implement Phase ID`: `impl_08_final_cleanup`
- `Verification Phases`:
  - `check_08_final`
    - type: `check`
    - fixed `bounce_target`: `impl_08_final_cleanup`
    - purpose: verify that the final library build is Rust-only, that only justified `unsafe` remains, and that the entire ABI/source/runtime/package/performance matrix passes, including the standalone `gif2rgb-regress` quantization gate that the inherited `test` target does not cover.
    - commands:
      ```bash
      if rg -n '\.\./original/.*\.c|cc::Build|legacy backend|gif_legacy' safe/build.rs safe/Cargo.toml safe/src; then
        echo 'unexpected bootstrap reference remains in library build inputs' >&2
        exit 1
      fi
      if ! rg -n '\bunsafe\b' safe/src; then
        echo 'no unsafe blocks remain in safe/src'
      fi
      python3 - <<'PY'
      import pathlib
      import re
      import sys

      violations = []
      for path in sorted(pathlib.Path("safe/src").rglob("*.rs")):
          lines = path.read_text(encoding="utf-8").splitlines()
          for idx, line in enumerate(lines, 1):
              if re.search(r"\bunsafe\b", line):
                  window = lines[max(0, idx - 4):idx - 1]
                  if not any("SAFETY:" in prev for prev in window):
                      violations.append(f"{path}:{idx}")

      if violations:
          print("unsafe without nearby SAFETY comment:", file=sys.stderr)
          print("\n".join(violations), file=sys.stderr)
          sys.exit(1)
      PY
      make -C original libgif.so libgif.a
      cargo build --manifest-path safe/Cargo.toml --release
      cc -std=gnu99 -Wall -Wextra -I"$PWD/original" safe/tests/malformed_observe.c original/libgif.a -o /tmp/malformed_observe.original
      safe/tests/capture_malformed_baseline.sh /tmp/malformed_observe.original "$PWD/safe/tests/malformed" > /tmp/original-malformed-baseline.txt
      diff -u safe/tests/malformed/original-baseline.txt /tmp/original-malformed-baseline.txt
      cmp -s safe/include/gif_lib.h original/gif_lib.h
      cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
      /tmp/giflib-abi-layout
      if find safe/tests \( -type f -o -type l \) \( -name 'public_api_regress.c' -o -name '*.summary' -o -name '*.ico' -o -name '*.dmp' -o -name '*.map' -o -name '*.rgb' \) | grep -q .; then
        echo 'unexpected vendored original harness or oracle files under safe/tests' >&2
        exit 1
      fi
      if find safe/tests \( -type f -o -type l \) -name '*.gif' ! -path 'safe/tests/malformed/*' | grep -q .; then
        echo 'unexpected vendored original sample GIFs under safe/tests outside malformed fixtures' >&2
        exit 1
      fi
      header_only_dir="$(mktemp -d)"
      make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
      make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress malformed-regress malformed-baseline-regress link-compat-regress internal-export-regress
      cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c original/libgif.a -o /tmp/public_api_regress.original
      cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
      safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf.log
      grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf.log
      grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf.log
      grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf.log
      grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf.log
      readelf -d safe/target/release/libgif.so | grep -E 'SONAME.*libgif\.so\.7'
      objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
      sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
      test "$(objdump -T safe/target/release/libgif.so | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
      grep -x '3.0 (quilt)' safe/debian/source/format
      rm -f safe/../libgif7_*.deb safe/../libgif-dev_*.deb safe/../libgif7-dbgsym_*.deb
      (cd safe && dpkg-buildpackage -us -uc -b)
      multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
      runtime_deb="$(ls -1 safe/../libgif7_*.deb)"
      dev_deb="$(ls -1 safe/../libgif-dev_*.deb)"
      test "$(dpkg-deb -f "$runtime_deb" Package)" = "libgif7"
      test "$(dpkg-deb -f "$dev_deb" Package)" = "libgif-dev"
      runtime_version="$(dpkg-deb -f "$runtime_deb" Version)"
      dev_version="$(dpkg-deb -f "$dev_deb" Version)"
      test "$runtime_version" = "$dev_version"
      case "$runtime_version" in
        *+safelibs*) ;;
        *)
          echo 'expected local safelibs version suffix in Debian package version' >&2
          exit 1
          ;;
      esac
      runtime_tmp="$(mktemp -d)"
      dev_tmp="$(mktemp -d)"
      dpkg-deb -x "$runtime_deb" "$runtime_tmp"
      dpkg-deb -x "$dev_deb" "$dev_tmp"
      runtime_real="$(find "$runtime_tmp/usr/lib/$multiarch" -maxdepth 1 -type f -name 'libgif.so.7.*' | sort)"
      test "$(printf '%s\n' "$runtime_real" | sed '/^$/d' | wc -l)" -eq 1
      runtime_real="$(printf '%s\n' "$runtime_real" | head -n1)"
      readelf -d "$runtime_real" | grep -E 'SONAME.*libgif\.so\.7'
      objdump -T "$runtime_real" | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/pkg-safe-symbols.txt
      diff -u /tmp/original-symbols.txt /tmp/pkg-safe-symbols.txt
      test "$(objdump -T "$runtime_real" | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
      test -L "$runtime_tmp/usr/lib/$multiarch/libgif.so.7"
      test "$(readlink "$runtime_tmp/usr/lib/$multiarch/libgif.so.7")" = "$(basename "$runtime_real")"
      test -f "$dev_tmp/usr/include/gif_lib.h"
      find "$runtime_tmp" "$dev_tmp" -path '*/usr/include/*' \( -type f -o -type l \) | LC_ALL=C sort > /tmp/pkg-headers.txt
      printf '%s\n' "$dev_tmp/usr/include/gif_lib.h" > /tmp/pkg-headers-expected.txt
      diff -u /tmp/pkg-headers-expected.txt /tmp/pkg-headers.txt
      test -f "$dev_tmp/usr/lib/$multiarch/libgif.a"
      cmp -s "$dev_tmp/usr/include/gif_lib.h" original/gif_lib.h
      cc -std=gnu99 -Wall -Wextra -I"$dev_tmp/usr/include" original/tests/public_api_regress.c "$dev_tmp/usr/lib/$multiarch/libgif.a" -o /tmp/public_api_regress.pkg
      /tmp/public_api_regress.pkg legacy > /tmp/pkg-legacy.summary
      diff -u original/tests/legacy.summary /tmp/pkg-legacy.summary
      /tmp/public_api_regress.pkg alloc > /tmp/pkg-alloc.summary
      diff -u original/tests/alloc.summary /tmp/pkg-alloc.summary
      test -L "$dev_tmp/usr/lib/$multiarch/libgif.so"
      test "$(readlink "$dev_tmp/usr/lib/$multiarch/libgif.so")" = "libgif.so.7"
      pkgconfig_dir="$dev_tmp/usr/lib/$multiarch/pkgconfig"
      pkgcfg() {
        env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config "$@"
      }
      test -f "$pkgconfig_dir/libgif7.pc"
      grep -F 'Name: libgif' "$pkgconfig_dir/libgif7.pc"
      grep -F 'Libs: -L${libdir} -lgif' "$pkgconfig_dir/libgif7.pc"
      libgif_pc="$pkgconfig_dir/libgif.pc"
      if [ -L "$libgif_pc" ]; then
        test "$(readlink "$libgif_pc")" = "libgif7.pc"
      else
        test -f "$libgif_pc"
      fi
      grep -F 'Name: libgif' "$libgif_pc"
      grep -F 'Libs: -L${libdir} -lgif' "$libgif_pc"
      pkgcfg --exists libgif7
      test "$(pkgcfg --variable=libdir libgif7)" = "/usr/lib/$multiarch"
      test "$(pkgcfg --variable=includedir libgif7)" = "/usr/include"
      pkgcfg --exists libgif
      test "$(pkgcfg --variable=libdir libgif)" = "/usr/lib/$multiarch"
      test "$(pkgcfg --variable=includedir libgif)" = "/usr/include"
      if find "$runtime_tmp" "$dev_tmp" \( -type f -o -type l \) \( -name 'gif_hash.h' -o -name 'gif_lib_private.h' \) | grep -q .; then
        echo 'unexpected private header installed in Debian packages' >&2
        exit 1
      fi
      if rg -n '/usr/local|build_original_giflib|assert_uses_original' test-original.sh; then
        echo 'stale original-install assumptions remain in downstream harness' >&2
        exit 1
      fi
      rg -n '^COPY[[:space:]]+\\.?/?safe/?[[:space:]]+/work/safe/?$' test-original.sh
      rg -n '^build_safe_packages\(\)' test-original.sh
      rg -n '^install_safe_packages\(\)' test-original.sh
      rg -n '^resolve_installed_shared_libgif\(\)' test-original.sh
      rg -n '^resolve_installed_static_libgif\(\)' test-original.sh
      rg -n '^assert_links_to_active_shared_libgif\(\)' test-original.sh
      rg -n '^assert_build_uses_active_giflib\(\)' test-original.sh
      test "$(rg -n '\bbuild_safe_packages\b' test-original.sh | awk 'END { print NR + 0 }')" -ge 2
      test "$(rg -n '\binstall_safe_packages\b' test-original.sh | awk 'END { print NR + 0 }')" -ge 2
      rg -n 'dpkg-buildpackage -us -uc -b' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_RUNTIME_DEB" Package' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_RUNTIME_DEB" Version' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_DEV_DEB" Package' test-original.sh
      rg -n 'dpkg-deb -f "\$SAFE_DEV_DEB" Version' test-original.sh
      rg -n 'dpkg -i "\$SAFE_RUNTIME_DEB" "\$SAFE_DEV_DEB"' test-original.sh
      rg -n 'dpkg-query[[:space:]]+-W.*libgif7' test-original.sh
      rg -n 'dpkg-query[[:space:]]+-W.*libgif-dev' test-original.sh
      rg -n 'dpkg(-query)?[[:space:]].*-L[[:space:]]+libgif7\b' test-original.sh
      rg -n 'dpkg(-query)?[[:space:]].*-L[[:space:]]+libgif-dev\b' test-original.sh
      rg -n 'dpkg-query[[:space:]]+-S' test-original.sh
      rg -n 'ldconfig' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "giflib-tools-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "webp-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "fbi-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "mtpaint-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "tracker-extract-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "libextractor-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "camlimages-runtime"' test-original.sh
      rg -n 'assert_links_to_active_shared_libgif "gdal-runtime"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "gdal-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "exactimage-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "sail-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "libwebp-source"' test-original.sh
      rg -n 'assert_build_uses_active_giflib "imlib2-source"' test-original.sh
      bash -o pipefail -c './test-original.sh | tee /tmp/test-original.log'
      grep -E '^SAFE_RUNTIME_DEB=.*/libgif7_.*\.deb$' /tmp/test-original.log
      grep -E '^SAFE_DEV_DEB=.*/libgif-dev_.*\.deb$' /tmp/test-original.log
      grep -x 'SAFE_RUNTIME_PACKAGE=libgif7' /tmp/test-original.log
      grep -x 'SAFE_DEV_PACKAGE=libgif-dev' /tmp/test-original.log
      safe_runtime_version="$(sed -n 's/^SAFE_RUNTIME_VERSION=//p' /tmp/test-original.log | tail -n1)"
      safe_dev_version="$(sed -n 's/^SAFE_DEV_VERSION=//p' /tmp/test-original.log | tail -n1)"
      active_runtime_version="$(sed -n 's/^ACTIVE_RUNTIME_VERSION=//p' /tmp/test-original.log | tail -n1)"
      active_dev_version="$(sed -n 's/^ACTIVE_DEV_VERSION=//p' /tmp/test-original.log | tail -n1)"
      test -n "$safe_runtime_version"
      test "$safe_runtime_version" = "$safe_dev_version"
      case "$safe_runtime_version" in
        *+safelibs*) ;;
        *)
          echo 'expected local safelibs version in downstream harness output' >&2
          exit 1
          ;;
      esac
      test "$safe_runtime_version" = "$active_runtime_version"
      test "$safe_dev_version" = "$active_dev_version"
      grep -E '^ACTIVE_SHARED_LIBGIF\[giflib-tools-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[giflib-tools-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[webp-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[webp-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[fbi-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[fbi-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[mtpaint-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[mtpaint-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[tracker-extract-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[tracker-extract-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[libextractor-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[libextractor-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[camlimages-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[camlimages-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[gdal-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[gdal-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[gdal-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[gdal-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[gdal-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[gdal-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[gdal-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[exactimage-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[exactimage-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[exactimage-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[exactimage-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[exactimage-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[sail-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[sail-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[sail-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[sail-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[sail-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[libwebp-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[libwebp-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[libwebp-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[libwebp-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[libwebp-source\]=(shared|static)$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_LIBGIF\[imlib2-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
      grep -E '^ACTIVE_SHARED_OWNER\[imlib2-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_LIBGIF\[imlib2-source\]=/.*/libgif\.a$' /tmp/test-original.log
      grep -E '^ACTIVE_STATIC_OWNER\[imlib2-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
      grep -E '^LINK_ASSERT_MODE\[imlib2-source\]=(shared|static)$' /tmp/test-original.log
      ```
- `Preexisting Inputs`:
  - all prior phases
  - final `safe/tests/Makefile`, `safe/tests/internal_exports_smoke.c`, `safe/tests/malformed_observe.c`, and `safe/tests/capture_malformed_baseline.sh`
  - final malformed fixtures plus [`safe/tests/malformed/manifest.txt`](/home/yans/code/safelibs/ported/giflib/safe/tests/malformed/manifest.txt) and [`safe/tests/malformed/original-baseline.txt`](/home/yans/code/safelibs/ported/giflib/safe/tests/malformed/original-baseline.txt)
  - final `safe/tests/perf_compare.sh`
  - final `safe/debian/` packaging
- `New Outputs`:
  - final cleaned Rust-only library build inputs
  - final audited `unsafe` footprint
  - final verification evidence
- `File Changes`:
  - update whichever `safe/src/*.rs` files still contain temporary compatibility code
  - update `safe/build.rs`
  - update `safe/Cargo.toml`
  - update `safe/tests/Makefile` if the final link/performance targets need cleanup
  - update `safe/debian/*` only if final package fixes are required
- `Implementation Details`:
  - Remove any remaining bootstrap build logic from `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/`. References to original fixtures, original headers, original tests, or the original baseline library are still allowed in `safe/tests/` and verification scripts.
  - Audit every `unsafe` block and keep only those required for FFI entry points, raw-pointer field access, callback invocation, libc allocation/deallocation, and symbol export. Each remaining `unsafe` site should have a succinct nearby `SAFETY:` justification comment.
  - Ensure exported Rust entry points do not unwind across the C ABI boundary. Wrap them so panics become `NULL`/`GIF_ERROR` and update error outputs instead of aborting.
  - Use this phase as the catch-all bounce target for any final ABI drift, downstream breakage, safety cleanups, or packaging mismatches found by later checkers.
- `Verification`:
  - Run the full command block in `check_08_final`.
  - Treat any bootstrap reference in the library build inputs, malformed-baseline drift, recursive vendored-fixture detection under `safe/tests/`, internal-export regression, symbol drift, layout drift, missing runtime-package or development-package files, missing local `+safelibs` package version suffix, widened package header surface, mismatch between built and active downstream package markers, unreviewed `unsafe`, performance regression beyond the agreed threshold, or downstream harness failure as a blocker.

## 4. Critical Files

- [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h): authoritative public ABI. Copy verbatim into `safe/include/gif_lib.h`.
- [`original/gif_hash.h`](/home/yans/code/safelibs/ported/giflib/original/gif_hash.h): authoritative ABI for exported hash-table helpers.
- [`original/gif_lib_private.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib_private.h): source of truth for internal constants, state-machine fields, and behavioral intent, but not a final public ABI requirement.
- [`original/dgif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/dgif_lib.c): decoder behavior, `DGifSlurp`, and malformed-input handling.
- [`original/egif_lib.c`](/home/yans/code/safelibs/ported/giflib/original/egif_lib.c): encoder behavior, extension emission, and `EGifSpew`.
- [`original/gifalloc.c`](/home/yans/code/safelibs/ported/giflib/original/gifalloc.c): public ownership/allocation helpers.
- [`original/gif_err.c`](/home/yans/code/safelibs/ported/giflib/original/gif_err.c): `GifErrorString`.
- [`original/gif_font.c`](/home/yans/code/safelibs/ported/giflib/original/gif_font.c): `GifAsciiTable8x8` and drawing helpers.
- [`original/gif_hash.c`](/home/yans/code/safelibs/ported/giflib/original/gif_hash.c): encoder hash table implementation.
- [`original/openbsd-reallocarray.c`](/home/yans/code/safelibs/ported/giflib/original/openbsd-reallocarray.c): exported overflow-safe allocation semantics.
- [`original/quantize.c`](/home/yans/code/safelibs/ported/giflib/original/quantize.c): `GifQuantizeBuffer` implementation and deterministic ordering behavior.
- [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols): authoritative shared-library export list.
- [`original/debian/control`](/home/yans/code/safelibs/ported/giflib/original/debian/control), [`original/debian/rules`](/home/yans/code/safelibs/ported/giflib/original/debian/rules), [`original/debian/libgif7.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.install), [`original/debian/libgif-dev.install`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif-dev.install), and [`original/debian/pkgconfig/libgif7.pc.in`](/home/yans/code/safelibs/ported/giflib/original/debian/pkgconfig/libgif7.pc.in): packaging templates to adapt for the safe build.
- [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c): authoritative C regression harness and performance harness input, consumed in place by `safe/tests/Makefile`.
- [`original/tests/makefile`](/home/yans/code/safelibs/ported/giflib/original/tests/makefile): authoritative regression-target structure to port into `safe/tests/Makefile`.
- [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests) and [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic): authoritative fixtures and sample GIFs.
- [`relevant_cves.json`](/home/yans/code/safelibs/ported/giflib/relevant_cves.json): scoped security requirements that must inform the malformed-input tests.
- [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json): authoritative downstream inventory for replacement testing.
- [`test-original.sh`](/home/yans/code/safelibs/ported/giflib/test-original.sh): downstream replacement harness to update in place.
- `safe/Cargo.toml`: Rust crate manifest, release-profile tuning, and crate-type definitions.
- `safe/build.rs`: bootstrap linker in phase 1, then the place where bootstrap C linkage is reduced and removed by the Rust-only decoder cutover in phase 4.
- `safe/include/gif_lib.h`: installed public header.
- `safe/src/lib.rs`: exported `extern "C"` surface, panic boundaries, and module wiring.
- `safe/src/ffi.rs`: `#[repr(C)]` public ABI mirrors.
- `safe/src/memory.rs`: libc-backed allocation helpers and FFI-safe ownership utilities.
- `safe/src/helpers.rs`: map/image/extension helper implementations.
- `safe/src/error.rs`: `GifErrorString` and shared error helpers.
- `safe/src/draw.rs`: font table and drawing helpers.
- `safe/src/hash.rs`: exported hash-table functions.
- `safe/src/quantize.rs`: `GifQuantizeBuffer`.
- `safe/src/state.rs`: opaque encoder/decoder internal state types for `GifFileType.Private`.
- `safe/src/io.rs`: file and callback I/O helpers.
- `safe/src/gcb.rs`: GCB conversion helpers.
- `safe/src/encode.rs`: full write path and LZW encoder.
- `safe/src/decode.rs`: sequential read path and LZW decoder.
- `safe/src/slurp.rs`: `DGifSlurp` and rollback/error-cleanup helpers.
- `safe/tests/Makefile`: ported regression driver that compiles [`original/tests/public_api_regress.c`](/home/yans/code/safelibs/ported/giflib/original/tests/public_api_regress.c) and consumes oracle files from [`original/tests/`](/home/yans/code/safelibs/ported/giflib/original/tests) and [`original/pic/`](/home/yans/code/safelibs/ported/giflib/original/pic) in place, plus `internal-export-regress`, `malformed-regress`, `malformed-baseline-regress`, `link-compat-regress`, and any performance targets.
- `safe/tests/abi_layout.c`: layout checker for public ABI structs and `GifHashTableType`.
- `safe/tests/internal_exports_smoke.c`: smoke test for non-installed but exported helper symbols.
- `safe/tests/malformed_observe.c`: deterministic malformed-input behavior probe used to capture and re-check the original-library baseline.
- `safe/tests/capture_malformed_baseline.sh`: helper script that runs the malformed probe over the committed malformed fixture set in lexical order.
- `safe/tests/malformed/`: derived malformed fixtures plus provenance notes.
- `safe/tests/malformed/original-baseline.txt`: committed original-library malformed-input behavior matrix for the derived fixture set.
- `safe/tests/perf_compare.sh`: explicit performance comparison script against the original library baseline.
- `safe/debian/control`, `safe/debian/rules`, `safe/debian/changelog`, `safe/debian/libgif7.symbols`, `safe/debian/libgif7.install`, `safe/debian/libgif-dev.install`, `safe/debian/pkgconfig/libgif7.pc.in`, and `safe/debian/source/format`: Debian packaging for the safe build.

## 5. Final Verification

After all phases complete, verify the finished port with the following end-to-end sequence:

1. Confirm the library build inputs are Rust-only and inspect the remaining `unsafe` footprint:
   ```bash
   if rg -n '\.\./original/.*\.c|cc::Build|legacy backend|gif_legacy' safe/build.rs safe/Cargo.toml safe/src; then
     echo 'unexpected bootstrap reference remains in library build inputs' >&2
     exit 1
   fi
   if ! rg -n '\bunsafe\b' safe/src; then
     echo 'no unsafe blocks remain in safe/src'
   fi
   python3 - <<'PY'
   import pathlib
   import re
   import sys

   violations = []
   for path in sorted(pathlib.Path("safe/src").rglob("*.rs")):
       lines = path.read_text(encoding="utf-8").splitlines()
       for idx, line in enumerate(lines, 1):
           if re.search(r"\bunsafe\b", line):
               window = lines[max(0, idx - 4):idx - 1]
               if not any("SAFETY:" in prev for prev in window):
                   violations.append(f"{path}:{idx}")

   if violations:
       print("unsafe without nearby SAFETY comment:", file=sys.stderr)
       print("\n".join(violations), file=sys.stderr)
       sys.exit(1)
   PY
   ```
2. Build both the original baseline library and the final Rust library:
   ```bash
   make -C original libgif.so libgif.a
   cargo build --manifest-path safe/Cargo.toml --release
   ```
3. Verify the committed malformed-input baseline artifact, public ABI layout, the full C regression matrix including the standalone `gif2rgb-regress` quantization gate, internal-export smoke coverage, and object-link compatibility:
   ```bash
   cc -std=gnu99 -Wall -Wextra -I"$PWD/original" safe/tests/malformed_observe.c original/libgif.a -o /tmp/malformed_observe.original
   safe/tests/capture_malformed_baseline.sh /tmp/malformed_observe.original "$PWD/safe/tests/malformed" > /tmp/original-malformed-baseline.txt
   diff -u safe/tests/malformed/original-baseline.txt /tmp/original-malformed-baseline.txt
   cmp -s safe/include/gif_lib.h original/gif_lib.h
   cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
   /tmp/giflib-abi-layout
   if find safe/tests \( -type f -o -type l \) \( -name 'public_api_regress.c' -o -name '*.summary' -o -name '*.ico' -o -name '*.dmp' -o -name '*.map' -o -name '*.rgb' \) | grep -q .; then
     echo 'unexpected vendored original harness or oracle files under safe/tests' >&2
     exit 1
   fi
   if find safe/tests \( -type f -o -type l \) -name '*.gif' ! -path 'safe/tests/malformed/*' | grep -q .; then
     echo 'unexpected vendored original sample GIFs under safe/tests outside malformed fixtures' >&2
     exit 1
   fi
   header_only_dir="$(mktemp -d)"
   make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
   make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress malformed-regress malformed-baseline-regress link-compat-regress internal-export-regress
   ```
4. Verify SONAME and symbol export:
   ```bash
   readelf -d safe/target/release/libgif.so | grep -E 'SONAME.*libgif\.so\.7'
   objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
   sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
   diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
   test "$(objdump -T safe/target/release/libgif.so | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
   ```
5. Verify performance against the original baseline:
   ```bash
   cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c original/libgif.a -o /tmp/public_api_regress.original
   cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
   safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf.log
   grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf.log
   grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf.log
   grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf.log
   grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf.log
   ```
6. Build Debian packages and verify their contents and packaged symbol set with host-isolated `pkg-config` resolution:
   ```bash
   grep -x '3.0 (quilt)' safe/debian/source/format
   rm -f safe/../libgif7_*.deb safe/../libgif-dev_*.deb safe/../libgif7-dbgsym_*.deb
   (cd safe && dpkg-buildpackage -us -uc -b)
   multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
   runtime_deb="$(ls -1 safe/../libgif7_*.deb)"
   dev_deb="$(ls -1 safe/../libgif-dev_*.deb)"
   test "$(dpkg-deb -f "$runtime_deb" Package)" = "libgif7"
   test "$(dpkg-deb -f "$dev_deb" Package)" = "libgif-dev"
   runtime_version="$(dpkg-deb -f "$runtime_deb" Version)"
   dev_version="$(dpkg-deb -f "$dev_deb" Version)"
   test "$runtime_version" = "$dev_version"
   case "$runtime_version" in
     *+safelibs*) ;;
     *)
       echo 'expected local safelibs version suffix in Debian package version' >&2
       exit 1
       ;;
   esac
   runtime_tmp="$(mktemp -d)"
   dev_tmp="$(mktemp -d)"
   dpkg-deb -x "$runtime_deb" "$runtime_tmp"
   dpkg-deb -x "$dev_deb" "$dev_tmp"
   runtime_real="$(find "$runtime_tmp/usr/lib/$multiarch" -maxdepth 1 -type f -name 'libgif.so.7.*' | sort)"
   test "$(printf '%s\n' "$runtime_real" | sed '/^$/d' | wc -l)" -eq 1
   runtime_real="$(printf '%s\n' "$runtime_real" | head -n1)"
   readelf -d "$runtime_real" | grep -E 'SONAME.*libgif\.so\.7'
   objdump -T "$runtime_real" | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/pkg-safe-symbols.txt
   sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
   diff -u /tmp/original-symbols.txt /tmp/pkg-safe-symbols.txt
   test "$(objdump -T "$runtime_real" | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
   test -L "$runtime_tmp/usr/lib/$multiarch/libgif.so.7"
   test "$(readlink "$runtime_tmp/usr/lib/$multiarch/libgif.so.7")" = "$(basename "$runtime_real")"
   test -f "$dev_tmp/usr/include/gif_lib.h"
   find "$runtime_tmp" "$dev_tmp" -path '*/usr/include/*' \( -type f -o -type l \) | LC_ALL=C sort > /tmp/pkg-headers.txt
   printf '%s\n' "$dev_tmp/usr/include/gif_lib.h" > /tmp/pkg-headers-expected.txt
   diff -u /tmp/pkg-headers-expected.txt /tmp/pkg-headers.txt
   test -f "$dev_tmp/usr/lib/$multiarch/libgif.a"
   cmp -s "$dev_tmp/usr/include/gif_lib.h" original/gif_lib.h
   cc -std=gnu99 -Wall -Wextra -I"$dev_tmp/usr/include" original/tests/public_api_regress.c "$dev_tmp/usr/lib/$multiarch/libgif.a" -o /tmp/public_api_regress.pkg
   /tmp/public_api_regress.pkg legacy > /tmp/pkg-legacy.summary
   diff -u original/tests/legacy.summary /tmp/pkg-legacy.summary
   /tmp/public_api_regress.pkg alloc > /tmp/pkg-alloc.summary
   diff -u original/tests/alloc.summary /tmp/pkg-alloc.summary
   test -L "$dev_tmp/usr/lib/$multiarch/libgif.so"
   test "$(readlink "$dev_tmp/usr/lib/$multiarch/libgif.so")" = "libgif.so.7"
   pkgconfig_dir="$dev_tmp/usr/lib/$multiarch/pkgconfig"
   pkgcfg() {
     env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config "$@"
   }
   test -f "$pkgconfig_dir/libgif7.pc"
   grep -F 'Name: libgif' "$pkgconfig_dir/libgif7.pc"
   grep -F 'Libs: -L${libdir} -lgif' "$pkgconfig_dir/libgif7.pc"
   libgif_pc="$pkgconfig_dir/libgif.pc"
   if [ -L "$libgif_pc" ]; then
     test "$(readlink "$libgif_pc")" = "libgif7.pc"
   else
     test -f "$libgif_pc"
   fi
   grep -F 'Name: libgif' "$libgif_pc"
   grep -F 'Libs: -L${libdir} -lgif' "$libgif_pc"
   pkgcfg --exists libgif7
   test "$(pkgcfg --variable=libdir libgif7)" = "/usr/lib/$multiarch"
   test "$(pkgcfg --variable=includedir libgif7)" = "/usr/include"
   pkgcfg --exists libgif
   test "$(pkgcfg --variable=libdir libgif)" = "/usr/lib/$multiarch"
   test "$(pkgcfg --variable=includedir libgif)" = "/usr/include"
   if find "$runtime_tmp" "$dev_tmp" \( -type f -o -type l \) \( -name 'gif_hash.h' -o -name 'gif_lib_private.h' \) | grep -q .; then
     echo 'unexpected private header installed in Debian packages' >&2
     exit 1
   fi
   ```
7. Prove the downstream harness itself has been converted away from `/usr/local`, then run it:
   ```bash
   if rg -n '/usr/local|build_original_giflib|assert_uses_original' test-original.sh; then
     echo 'stale original-install assumptions remain in downstream harness' >&2
     exit 1
   fi
   rg -n '^COPY[[:space:]]+\\.?/?safe/?[[:space:]]+/work/safe/?$' test-original.sh
   rg -n '^build_safe_packages\(\)' test-original.sh
   rg -n '^install_safe_packages\(\)' test-original.sh
   rg -n '^resolve_installed_shared_libgif\(\)' test-original.sh
   rg -n '^resolve_installed_static_libgif\(\)' test-original.sh
   rg -n '^assert_links_to_active_shared_libgif\(\)' test-original.sh
   rg -n '^assert_build_uses_active_giflib\(\)' test-original.sh
   test "$(rg -n '\bbuild_safe_packages\b' test-original.sh | awk 'END { print NR + 0 }')" -ge 2
   test "$(rg -n '\binstall_safe_packages\b' test-original.sh | awk 'END { print NR + 0 }')" -ge 2
   rg -n 'dpkg-buildpackage -us -uc -b' test-original.sh
   rg -n 'dpkg-deb -f "\$SAFE_RUNTIME_DEB" Package' test-original.sh
   rg -n 'dpkg-deb -f "\$SAFE_RUNTIME_DEB" Version' test-original.sh
   rg -n 'dpkg-deb -f "\$SAFE_DEV_DEB" Package' test-original.sh
   rg -n 'dpkg-deb -f "\$SAFE_DEV_DEB" Version' test-original.sh
   rg -n 'dpkg -i "\$SAFE_RUNTIME_DEB" "\$SAFE_DEV_DEB"' test-original.sh
   rg -n 'dpkg-query[[:space:]]+-W.*libgif7' test-original.sh
   rg -n 'dpkg-query[[:space:]]+-W.*libgif-dev' test-original.sh
   rg -n 'dpkg(-query)?[[:space:]].*-L[[:space:]]+libgif7\b' test-original.sh
   rg -n 'dpkg(-query)?[[:space:]].*-L[[:space:]]+libgif-dev\b' test-original.sh
   rg -n 'dpkg-query[[:space:]]+-S' test-original.sh
   rg -n 'ldconfig' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "giflib-tools-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "webp-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "fbi-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "mtpaint-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "tracker-extract-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "libextractor-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "camlimages-runtime"' test-original.sh
   rg -n 'assert_links_to_active_shared_libgif "gdal-runtime"' test-original.sh
   rg -n 'assert_build_uses_active_giflib "gdal-source"' test-original.sh
   rg -n 'assert_build_uses_active_giflib "exactimage-source"' test-original.sh
   rg -n 'assert_build_uses_active_giflib "sail-source"' test-original.sh
   rg -n 'assert_build_uses_active_giflib "libwebp-source"' test-original.sh
   rg -n 'assert_build_uses_active_giflib "imlib2-source"' test-original.sh
   bash -o pipefail -c './test-original.sh | tee /tmp/test-original.log'
   grep -E '^SAFE_RUNTIME_DEB=.*/libgif7_.*\.deb$' /tmp/test-original.log
   grep -E '^SAFE_DEV_DEB=.*/libgif-dev_.*\.deb$' /tmp/test-original.log
   grep -x 'SAFE_RUNTIME_PACKAGE=libgif7' /tmp/test-original.log
   grep -x 'SAFE_DEV_PACKAGE=libgif-dev' /tmp/test-original.log
   safe_runtime_version="$(sed -n 's/^SAFE_RUNTIME_VERSION=//p' /tmp/test-original.log | tail -n1)"
   safe_dev_version="$(sed -n 's/^SAFE_DEV_VERSION=//p' /tmp/test-original.log | tail -n1)"
   active_runtime_version="$(sed -n 's/^ACTIVE_RUNTIME_VERSION=//p' /tmp/test-original.log | tail -n1)"
   active_dev_version="$(sed -n 's/^ACTIVE_DEV_VERSION=//p' /tmp/test-original.log | tail -n1)"
   test -n "$safe_runtime_version"
   test "$safe_runtime_version" = "$safe_dev_version"
   case "$safe_runtime_version" in
     *+safelibs*) ;;
     *)
       echo 'expected local safelibs version in downstream harness output' >&2
       exit 1
     ;;
   esac
   test "$safe_runtime_version" = "$active_runtime_version"
   test "$safe_dev_version" = "$active_dev_version"
   grep -E '^ACTIVE_SHARED_LIBGIF\[giflib-tools-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[giflib-tools-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[webp-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[webp-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[fbi-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[fbi-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[mtpaint-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[mtpaint-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[tracker-extract-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[tracker-extract-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[libextractor-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[libextractor-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[camlimages-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[camlimages-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[gdal-runtime\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[gdal-runtime\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[gdal-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[gdal-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_LIBGIF\[gdal-source\]=/.*/libgif\.a$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_OWNER\[gdal-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^LINK_ASSERT_MODE\[gdal-source\]=(shared|static)$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[exactimage-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[exactimage-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_LIBGIF\[exactimage-source\]=/.*/libgif\.a$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_OWNER\[exactimage-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^LINK_ASSERT_MODE\[exactimage-source\]=(shared|static)$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[sail-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[sail-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_LIBGIF\[sail-source\]=/.*/libgif\.a$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_OWNER\[sail-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^LINK_ASSERT_MODE\[sail-source\]=(shared|static)$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[libwebp-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[libwebp-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_LIBGIF\[libwebp-source\]=/.*/libgif\.a$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_OWNER\[libwebp-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^LINK_ASSERT_MODE\[libwebp-source\]=(shared|static)$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_LIBGIF\[imlib2-source\]=/.*/libgif\.so\.7(\.[0-9]+)*$' /tmp/test-original.log
   grep -E '^ACTIVE_SHARED_OWNER\[imlib2-source\]=libgif7(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_LIBGIF\[imlib2-source\]=/.*/libgif\.a$' /tmp/test-original.log
   grep -E '^ACTIVE_STATIC_OWNER\[imlib2-source\]=libgif-dev(:[[:alnum:]_.+-]+)?$' /tmp/test-original.log
   grep -E '^LINK_ASSERT_MODE\[imlib2-source\]=(shared|static)$' /tmp/test-original.log
   ```

Success criteria:

- `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/` no longer depend on original C source files
- the safe shared object exports the exact versioned symbol set required by [`original/debian/libgif7.symbols`](/home/yans/code/safelibs/ported/giflib/original/debian/libgif7.symbols), and `GifAsciiTable8x8` remains a `DO Base` data symbol
- the safe header remains source-compatible with [`original/gif_lib.h`](/home/yans/code/safelibs/ported/giflib/original/gif_lib.h), proved both by byte-for-byte parity and by compiling/running the ordinary `public_api_regress` build against `safe/include` without access to the original header, plus the same compile/run smoke against the extracted `libgif-dev` header and archive
- the committed malformed baseline in [`safe/tests/malformed/original-baseline.txt`](/home/yans/code/safelibs/ported/giflib/safe/tests/malformed/original-baseline.txt) still matches the original library on the committed malformed fixture set
- objects compiled against the original header link successfully against safe static and shared libraries
- all ported C regression tests, including `gif2rgb-regress`, `internal-export-regress`, and `malformed-baseline-regress`, pass
- the malformed-input regressions derived from [`relevant_cves.json`](/home/yans/code/safelibs/ported/giflib/relevant_cves.json) pass without crashes or panics
- the performance comparison script passes its fixed `2.00` threshold on the exact workload IDs `render-welcome2`, `render-treescap-interlaced`, `highlevel-copy-fire`, and `rgb-to-gif-gifgrid`
- every remaining `unsafe` site under `safe/src/` is justified with a nearby `SAFETY:` comment, and no unchecked unwind can cross the exported C ABI
- Debian packages use a distinct local `+safelibs` version suffix, `safe/debian/source/format` is `3.0 (quilt)`, packages install the expected files and names with exactly one installed header file, `gif_lib.h`, no private headers anywhere in the extracted package trees, and individually valid `libgif7.pc` and `libgif.pc` entries verified through host-isolated `pkg-config` queries against the extracted package tree. `libgif.pc` is either a regular file or a relative symlink to `libgif7.pc`, never an absolute symlink into `/usr/lib`
- the modified downstream Docker harness contains no `/usr/local` assumptions or original-install helpers, defines and uses `build_safe_packages`, `install_safe_packages`, `resolve_installed_shared_libgif`, `resolve_installed_static_libgif`, `assert_links_to_active_shared_libgif`, and `assert_build_uses_active_giflib`, uses `assert_links_to_active_shared_libgif` for every runtime label and `assert_build_uses_active_giflib` for every source-build label, logs matching built-versus-active package versions plus the fixed labeled `ACTIVE_SHARED_*` markers for runtime labels and `ACTIVE_SHARED_*`, `ACTIVE_STATIC_*`, and `LINK_ASSERT_MODE[...]` evidence for all five source-build labels, resolves runtime and development paths from package metadata plus `ldconfig`, and passes for every dependent listed in [`dependents.json`](/home/yans/code/safelibs/ported/giflib/dependents.json)
