# Phase 2

## Phase Name
Port Public Types, Memory Helpers, Error Strings, Font, Hash, And Quantization

## Implement Phase ID
`impl_02_helpers`

## Workflow Contract Notes
- Consume existing artifacts in place. Continue using `original/tests/`, `original/pic/`, and the original source files as the authoritative behavior and fixture oracles instead of duplicating them under `safe/`.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. If this phase needs an original-library oracle, rebuild it from a temporary copy of `original/` and point compile or link steps at that copy.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/abi_layout.c`
- `safe/tests/internal_exports_smoke.c`
- `original/gifalloc.c`
- `original/gif_err.c`
- `original/gif_font.c`
- `original/gif_hash.c`
- `original/gif_hash.h`
- `original/openbsd-reallocarray.c`
- `original/quantize.c`
- `original/debian/changelog`
- `original/tests/public_api_regress.c`
- `original/tests/`
- `original/pic/`

## New Outputs
- Rust FFI mirrors for all public ABI types plus the non-installed but exported `GifHashTableType`
- Rust implementations of allocation, extension, map, error, font, hash, `openbsd_reallocarray`, and quantization exports
- Reduced bootstrap backend with helper/data C sources removed

## File Changes
- Create `safe/src/ffi.rs`
- Create `safe/src/memory.rs`
- Create `safe/src/helpers.rs`
- Create `safe/src/error.rs`
- Create `safe/src/draw.rs`
- Create `safe/src/hash.rs`
- Create `safe/src/quantize.rs`
- Update `safe/src/lib.rs`
- Update `safe/build.rs`

## Implementation Details
- Define `#[repr(C)]` Rust mirrors for the public structs with exact C integer widths.
- For every Rust-defined C ABI entry point introduced or updated in this phase, catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.
- Do not trust foreign-written `_Bool` fields as Rust `bool`; use a layout-compatible byte representation plus normalization/accessors to avoid UB.
- Keep C-visible heap allocations on the C allocator for `ColorMapObject.Colors`, `SavedImage.RasterBits`, `ExtensionBlock.Bytes`, `GifFileType.SavedImages`, and extension arrays.
- Port `GifBitSize`, `GifMakeMapObject`, `GifFreeMapObject`, `GifUnionColorMap`, `GifApplyTranslation`, `GifAddExtensionBlock`, `GifFreeExtensions`, `GifMakeSavedImage`, `GifFreeSavedImages`, and `FreeLastSavedImage`.
- Fix the shallow-copy weakness in `GifMakeSavedImage` by deep-copying `ExtensionBlock.Bytes`.
- Keep `gifbuild-regress` in this phase because `highlevel-copy $(PICS)/fire.gif` is the direct behavioral gate for `GifMakeSavedImage(gif_out, &gif_in->SavedImages[i])` and copied extension-block contents.
- Port `GifErrorString` exactly, including returning `NULL` for unknown codes.
- Port the exported font data and drawing helpers precisely enough to preserve the `gifecho`, drawing, and wedge fixtures, and export `GifAsciiTable8x8` as read-only data.
- Port `_InitHashTable`, `_ClearHashTable`, `_InsertHashTable`, `_ExistsHashTable`, and `openbsd_reallocarray`.
- Preserve `openbsd_reallocarray` overflow and zero-size semantics exactly: overflow returns `NULL` with `errno = ENOMEM`, and any zero `nmemb` or `size` returns `NULL`.
- Port `GifQuantizeBuffer`, preserving deterministic palette ordering and the Debian-restored ABI contract.
- Remove `gifalloc.c`, `gif_err.c`, `gif_font.c`, `gif_hash.c`, `openbsd-reallocarray.c`, and `quantize.c` from the bootstrap archive only after the Rust exports and helper regressions pass.

## Verification Phases

### `check_02_helpers`
- Phase ID: `check_02_helpers`
- Type: `check`
- Bounce Target: `impl_02_helpers`
- Purpose: Verify that the helper/data modules are implemented in Rust, the public layout still matches C, precompiled-object link compatibility still holds for the newly replaced helper exports, and the helper-focused regressions including the `GifMakeSavedImage` source-copy path still pass after removing the corresponding bootstrap C sources.
- Commands:
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

## Success Criteria
- Helper/data exports move to Rust without changing the public header, layout, or shared-library symbol set.
- No helper/data bootstrap C source remains referenced by `safe/build.rs`, `safe/Cargo.toml`, or `safe/src/`.
- `gifbuild-regress`, `alloc-regress`, `gif2rgb-regress`, `gifecho-regress`, `drawing-regress`, `gifwedge-regress`, `link-compat-regress`, and `internal-export-regress` all pass.
- The `GifMakeSavedImage` deep-copy behavior is fixed without breaking compatibility elsewhere.
- Helper/data ABI entry points ported in this phase catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
