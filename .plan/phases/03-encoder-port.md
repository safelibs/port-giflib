# Phase 3

## Phase Name
Port Encoder, Extension Writers, And High-Level Spew

## Implement Phase ID
`impl_03_encode`

## Workflow Contract Notes
- Consume existing artifacts in place. Keep using the committed regression harness and fixtures under `original/` as the compatibility oracle for all write-side behavior.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. If this phase needs an original-library oracle, rebuild it from a temporary copy of `original/` and point compile or link steps at that copy.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/`
- `original/egif_lib.c`

## New Outputs
- Rust encoder implementation
- Rust extension/GCB write helpers
- Bootstrap backend with `egif_lib.c` removed

## File Changes
- Create `safe/src/state.rs`
- Create `safe/src/io.rs`
- Create `safe/src/gcb.rs`
- Create `safe/src/encode.rs`
- Update `safe/src/lib.rs`
- Update `safe/build.rs`

## Implementation Details
- Port `EGifOpenFileName`, `EGifOpenFileHandle`, `EGifOpen`, `EGifGetGifVersion`, `EGifSetGifVersion`, `EGifPutScreenDesc`, `EGifPutImageDesc`, `EGifPutLine`, `EGifPutPixel`, `EGifPutComment`, `EGifPutExtensionLeader`, `EGifPutExtensionBlock`, `EGifPutExtensionTrailer`, `EGifPutExtension`, `EGifGCBToExtension`, `EGifGCBToSavedExtension`, `EGifPutCode`, `EGifPutCodeNext`, `EGifCloseFile`, and `EGifSpew`.
- For every Rust-defined C ABI entry point introduced or updated in this phase, catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.
- Use `original/gif_lib_private.h` as the source of truth for encoder-side internal constants, state-machine fields, and behavioral intent. Implement an opaque Rust `EncoderState` behind `GifFileType.Private`; do not reproduce `GifFilePrivateType` layout as a public ABI requirement, but preserve the write-side transitions that the original encoder drives through `BitsPerPixel`, `ClearCode`, `EOFCode`, `RunningCode`, `RunningBits`, `MaxCode1`, `CrntCode`, `CrntShiftState`, `CrntShiftDWord`, and the output buffer.
- Preserve exact interlace pass order and extension-block emission rules.
- Preserve `EGifOpenFileName` file-creation semantics for `GifTestExistence`.
- Preserve `EGifGetGifVersion` automatic promotion to GIF89 when extensions require it.
- Keep `EGifSetGifVersion` object-compatible with the current `bool`-based ABI.
- Preserve file-descriptor ownership semantics for `EGifOpenFileHandle` and `EGifCloseFile`, while callback-mode handles skip `fclose`.
- Preserve sequential-API misuse behavior and the current error codes, including `E_GIF_ERR_HAS_SCRN_DSCR`, `E_GIF_ERR_HAS_IMAG_DSCR`, `E_GIF_ERR_NO_COLOR_MAP`, `E_GIF_ERR_DATA_TOO_BIG`, and `E_GIF_ERR_DISK_IS_FULL`.
- Treat short callback writes and short `FILE *` writes as the same observable failures the C library reports today.
- Keep comment splitting and graphics-control-block byte layout byte-for-byte compatible.
- Keep `giffix-regress` phase-local because the `repair` path in `original/tests/public_api_regress.c` directly exercises encoder APIs that move in this phase.
- Remove `egif_lib.c` from the bootstrap archive only after the write-side regressions pass.

## Verification Phases

### `check_03_encode`
- Phase ID: `check_03_encode`
- Type: `check`
- Bounce Target: `impl_03_encode`
- Purpose: Verify that the full write path is in Rust and remains output-compatible with the existing write-side regressions.
- Commands:
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

## Success Criteria
- No `egif_lib.c` bootstrap reference remains in the library build inputs.
- The Rust encoder preserves write-side source compatibility, symbol parity, and header parity.
- `fileio-regress`, `alloc-regress`, `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, `gif2rgb-regress`, `gifecho-regress`, `drawing-regress`, `gifwedge-regress`, and `link-compat-regress` all pass.
- Objects compiled earlier against `original/gif_lib.h` still link and run correctly against the ported library.
- Encoder ABI entry points ported in this phase catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
