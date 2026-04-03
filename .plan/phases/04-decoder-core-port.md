# Phase 4

## Phase Name
Port Full Decoder, Slurp, And Rust-Only Core

## Implement Phase ID
`impl_04_decode_core`

## Workflow Contract Notes
- Consume existing artifacts in place. Use the existing regression harness and fixture trees under `original/` as the decoder compatibility oracle and do not vendor duplicates under `safe/tests/`.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. If this phase needs an original-library oracle, rebuild it from a temporary copy of `original/` and point compile or link steps at that copy.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/`
- `original/dgif_lib.c`

## New Outputs
- Rust sequential decoder implementation
- Rust `DGifSlurp`
- Rust `DGifDecreaseImageCounter`
- Rust extension/GCB read helpers
- Bootstrap-free Rust core library build

## File Changes
- Create `safe/src/decode.rs`
- Create `safe/src/slurp.rs`
- Update `safe/src/io.rs`
- Update `safe/src/gcb.rs`
- Update `safe/src/state.rs`
- Update `safe/src/lib.rs`
- Update `safe/build.rs`

## Implementation Details
- Port `DGifOpenFileName`, `DGifOpenFileHandle`, `DGifOpen`, `DGifGetScreenDesc`, `DGifGetGifVersion`, `DGifGetRecordType`, `DGifGetImageHeader`, `DGifGetImageDesc`, `DGifGetLine`, `DGifGetPixel`, `DGifGetExtension`, `DGifGetExtensionNext`, `DGifExtensionToGCB`, `DGifSavedExtensionToGCB`, `DGifCloseFile`, `DGifGetCode`, `DGifGetCodeNext`, `DGifGetLZCodes`, `DGifDecreaseImageCounter`, and `DGifSlurp`.
- For every Rust-defined C ABI entry point introduced or updated in this phase, catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.
- Use `original/gif_lib_private.h` as the source of truth for decoder-side internal constants, state-machine fields, and behavioral intent. Implement an opaque Rust `DecoderState` behind `GifFileType.Private`; do not reproduce `GifFilePrivateType` layout as a public ABI requirement, but preserve the original state-machine behavior for code-size setup, LZW buffering, pixel countdown, and extension streaming.
- Preserve malformed-image handling for bad code sizes, broken prefixes, early EOF, and wrong record types. Compatibility is more important than inventing Rust-specific errors here.
- Treat short callback reads, short `FILE *` reads, and premature block termination as the same `D_GIF_ERR_READ_FAILED` or `D_GIF_ERR_EOF_TOO_SOON` outcomes the current decoder exposes.
- Preserve file-descriptor ownership semantics for `DGifOpenFileHandle` and `DGifCloseFile`, while callback-mode handles skip `fclose`.
- Preserve callback-mode behavior through `InputFunc` and file-wrapper behavior through file handles and `fdopen`/`fclose` equivalents.
- Retire the whole original decoder object in this phase; do not try to keep C `DGifSlurp` while replacing the other `DGif*` exports.
- After this phase, `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/` must not require original C library sources to build `libgif`.

## Verification Phases

### `check_04_decode_core`
- Phase ID: `check_04_decode_core`
- Type: `check`
- Bounce Target: `impl_04_decode_core`
- Purpose: Verify that the full read path, `DGifSlurp`, cleanup helpers, file/GCB reader APIs, and internal exported decoder helpers are in Rust, and the core library build no longer depends on original C sources.
- Commands:
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

## Success Criteria
- The library build is Rust-only across `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/`.
- Decoder, slurp, GCB read helpers, and `DGifDecreaseImageCounter` are all implemented in Rust without symbol drift or header drift.
- `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `legacy-regress`, `fileio-regress`, `alloc-regress`, `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, `link-compat-regress`, and `internal-export-regress` all pass.
- Decoder-side ABI entry points ported in this phase catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
