# Phase 1

## Phase Name
Rust Package Scaffold, ABI Lock-In, And Bootstrap Backend

## Implement Phase ID
`impl_01_scaffold`

## Workflow Contract Notes
- Workflow prerequisite: `.plan/plan.md` is already committed in `HEAD` in a planning-only commit or a commit whose only tracked changes are under `.plan/`, `git diff --quiet HEAD -- .plan/plan.md` passes, `git status --short` shows no tracked changes outside `.plan/`, and no tracked file under `original/` is modified or deleted.
- Consume existing artifacts in place. Read the authoritative header, export list, harness source, fixtures, and oracle files directly from `original/`; do not rediscover them, regenerate them, or vendor duplicate copies under `safe/tests/`.
- Treat tracked files under `original/` as immutable oracle inputs. If any command would rewrite or remove them, copy `original/` to a temporary directory first and operate only on the copy.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. If this phase needs an original-library oracle, rebuild it from a temporary copy of `original/` and point compile or link steps at that copy.

## Preexisting Inputs
- `original/gif_lib.h`
- `original/Makefile`
- `original/debian/libgif7.symbols`
- `original/tests/makefile`
- `original/tests/public_api_regress.c`
- `original/tests/`
- `original/pic/`

## New Outputs
- Buildable Rust crate under `safe/`
- Cargo-built `safe/target/release/libgif.so`
- Cargo-built `safe/target/release/libgif.a`
- Verbatim installed header at `safe/include/gif_lib.h`
- Ported regression driver under `safe/tests/` that consumes the authoritative harness and fixtures in place
- `safe/tests/Makefile` targets `safe-header-regress`, `link-compat-regress`, and `internal-export-regress`
- `safe/tests/abi_layout.c`
- `safe/tests/internal_exports_smoke.c`
- Temporary bootstrap backend that still uses the original C core through the Rust build

## File Changes
- Create `safe/Cargo.toml`
- Create `safe/build.rs`
- Create `safe/include/gif_lib.h`
- Create `safe/src/lib.rs`
- Create `safe/src/bootstrap.rs` or an equivalent bootstrap-only module
- Create `safe/tests/Makefile`
- Create `safe/tests/abi_layout.c`
- Create `safe/tests/internal_exports_smoke.c`

## Implementation Details
- Set `[lib] name = "gif"` and `crate-type = ["cdylib", "staticlib"]`.
- Copy `original/gif_lib.h` verbatim into `safe/include/gif_lib.h`. Do not redesign the public C header.
- Use `build.rs` to compile the original core library sources into a bootstrap archive such as `libgif_legacy.a` and link it into the Rust outputs with whole-archive semantics so the exported ABI surface stays complete during bootstrap.
- Use `build.rs` to set the shared-library SONAME to `libgif.so.7`.
- From the first Rust-defined C ABI export introduced in this phase onward, every Rust entry point must catch panics and return C-compatible failure values instead of unwinding across the ABI boundary.
- Port `original/tests/makefile` into `safe/tests/Makefile`, but parameterize `ORIGINAL_INCLUDEDIR`, `ORIGINAL_TESTS_DIR`, `ORIGINAL_PIC_DIR`, `LIBGIF_INCLUDEDIR`, and `LIBGIF_LIBDIR`.
- In `safe/tests/Makefile`, compile `original/tests/public_api_regress.c` directly from `$(ORIGINAL_TESTS_DIR)` and point every comparison oracle at files under `$(ORIGINAL_TESTS_DIR)` and `$(ORIGINAL_PIC_DIR)`.
- The ordinary regression binary used by `test`, `render-regress`, `alloc-regress`, and the other normal targets must compile with `-I$(LIBGIF_INCLUDEDIR)` and must not use `$(ORIGINAL_INCLUDEDIR)` for `gif_lib.h`.
- Do not vendor `public_api_regress.c`, `*.summary`, `*.ico`, `*.dmp`, `*.map`, `*.rgb`, or the original sample GIFs into `safe/tests/`.
- Add `safe-header-regress` so the ordinary regression binary rebuilds against `$(LIBGIF_INCLUDEDIR)` only and runs at least `legacy`, `fileio`, and `alloc`.
- Add `link-compat-regress` so `original/tests/public_api_regress.c` is compiled exactly once with `$(ORIGINAL_INCLUDEDIR)/gif_lib.h`, then linked against both the safe static library and the safe shared library, and run through at least `legacy`, `alloc`, `render`, `malformed`, and `highlevel-copy $(ORIGINAL_PIC_DIR)/fire.gif` checked against the existing `fire.dmp` or `fire.rgb` oracle under `$(ORIGINAL_TESTS_DIR)`.
- No other target may compile `original/tests/public_api_regress.c` against `$(ORIGINAL_INCLUDEDIR)/gif_lib.h`; every ordinary regression target must keep using `$(LIBGIF_INCLUDEDIR)`.
- Add `internal-export-regress` backed by `safe/tests/internal_exports_smoke.c` to exercise `_InitHashTable`, `_ClearHashTable`, `_InsertHashTable`, `_ExistsHashTable`, `FreeLastSavedImage`, `DGifDecreaseImageCounter`, and `openbsd_reallocarray` without turning private headers into installed interfaces.
- Make `safe/tests/abi_layout.c` assert these current Ubuntu 24.04 x86_64 public-ABI facts directly, using `safe/include/gif_lib.h` and `original/gif_hash.h` through an explicit test-only include path:
```text
GifColorType: size 3
ColorMapObject: size 24; offsets ColorCount=0, BitsPerPixel=4, SortFlag=8, Colors=16
GifImageDesc: size 32; offsets Left=0, Top=4, Width=8, Height=12, Interlace=16, ColorMap=24
ExtensionBlock: size 24; offsets ByteCount=0, Bytes=8, Function=16
SavedImage: size 56; offsets ImageDesc=0, RasterBits=32, ExtensionBlockCount=40, ExtensionBlocks=48
GifFileType: size 120; offsets SWidth=0, SHeight=4, SColorResolution=8, SBackGroundColor=12, AspectByte=16, SColorMap=24, ImageCount=32, Image=40, SavedImages=72, ExtensionBlockCount=80, ExtensionBlocks=88, Error=96, UserData=104, Private=112
GraphicsControlBlock: size 16; offsets DisposalMode=0, UserInputFlag=4, DelayTime=8, TransparentColor=12
GifHashTableType: size 32768
```
- Treat `GifFileType.Private` as opaque in the public ABI. Do not require `GifFilePrivateType` layout parity, and do not turn `gif_hash.h` into an installed header.

## Verification Phases

### `check_01_scaffold_local`
- Phase ID: `check_01_scaffold_local`
- Type: `check`
- Bounce Target: `impl_01_scaffold`
- Purpose: Verify that `safe/` exists, emits `libgif.so` and `libgif.a` with the correct SONAME and exported symbol set, preserves the public header, provides a regression driver that consumes the authoritative harness/oracles from `original/` in place, and supports object-link compatibility through a temporary bootstrap backend.
- Commands:
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

## Success Criteria
- Cargo builds `safe/` and emits `libgif.so` and `libgif.a` with SONAME `libgif.so.7`.
- The exported shared-library symbol set matches `original/debian/libgif7.symbols`, including `GifAsciiTable8x8` as `DO Base`.
- `safe/include/gif_lib.h` matches `original/gif_lib.h` byte-for-byte and the ABI layout probe passes.
- `safe/tests/Makefile` consumes `original/tests/` and `original/pic/` in place and `safe/tests/` contains no committed duplicate harness, oracle, or fixture files.
- `link-compat-regress` exercises the `highlevel-copy $(ORIGINAL_PIC_DIR)/fire.gif` saved-image copy case against the existing `fire.dmp` or `fire.rgb` oracle while remaining the only target that compiles `public_api_regress.c` against `$(ORIGINAL_INCLUDEDIR)/gif_lib.h`.
- `safe-header-regress`, `link-compat-regress`, and `internal-export-regress` pass against the bootstrap-backed safe library.
- Any Rust-defined exported ABI entry point introduced in this phase catches panics and returns C-compatible failure values instead of unwinding across the ABI boundary.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
