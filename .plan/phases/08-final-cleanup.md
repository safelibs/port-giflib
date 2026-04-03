# Phase 8

## Phase Name
Final Safe-Only Cleanup, Unsafe Audit, And Full Matrix Verification

## Implement Phase ID
`impl_08_final_cleanup`

## Workflow Contract Notes
- Consume existing artifacts in place. Keep using the original headers, fixtures, test harness, Debian symbol list, malformed baseline inputs, and downstream matrix as the authoritative final verification oracles.
- If a rebuilt original-library oracle is needed, create it from a temporary copy of `original/`; do not run destructive build or cleanup flows in the tracked oracle tree.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. Any original-library comparison in this phase must come from a temporary rebuild of `original/`.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/abi_layout.c`
- `safe/tests/internal_exports_smoke.c`
- `safe/tests/malformed_observe.c`
- `safe/tests/capture_malformed_baseline.sh`
- `safe/tests/malformed/`
- `safe/tests/malformed/manifest.txt`
- `safe/tests/malformed/original-baseline.txt`
- `safe/tests/perf_compare.sh`
- `safe/debian/`
- `test-original.sh`
- `dependents.json`
- `original/Makefile`
- `original/gif_lib.h`
- `original/gif_hash.h`
- `original/debian/libgif7.symbols`
- `original/tests/public_api_regress.c`
- `original/tests/`
- `original/pic/`

## New Outputs
- Final cleaned Rust-only library build inputs
- Final audited `unsafe` footprint
- Final verification evidence

## File Changes
- Update whichever `safe/src/*.rs` files still contain temporary compatibility code
- Update `safe/build.rs`
- Update `safe/Cargo.toml`
- Update `safe/tests/Makefile` if the final link or performance targets need cleanup
- Update `safe/debian/*` only if final package fixes are required

## Implementation Details
- Remove any remaining bootstrap build logic from `safe/build.rs`, `safe/Cargo.toml`, and `safe/src/`. References to original fixtures, original headers, original tests, or the original baseline library remain allowed in `safe/tests/` and verification scripts.
- Audit every `unsafe` block and keep only those required for FFI entry points, raw-pointer field access, callback invocation, libc allocation/deallocation, and symbol export.
- Add a succinct nearby `SAFETY:` justification comment for every remaining `unsafe` block, `unsafe fn`, or `unsafe impl`.
- Ensure exported Rust entry points do not unwind across the C ABI boundary. Wrap them so panics become `NULL` or `GIF_ERROR` and update error outputs instead of aborting.
- Use this phase as the catch-all bounce target for any remaining ABI drift, downstream breakage, safety cleanup, or packaging mismatch found by later verification.

## Verification Phases

### `check_08_final`
- Phase ID: `check_08_final`
- Type: `check`
- Bounce Target: `impl_08_final_cleanup`
- Purpose: Verify that the final library build is Rust-only, only justified `unsafe` remains, and the full ABI, source, runtime, package, malformed-input, and performance matrix passes, including the standalone `gif2rgb-regress` quantization gate.
- Commands:
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
original_build_dir="$(mktemp -d)"
trap 'rm -rf "$original_build_dir"' EXIT
cp -a original/. "$original_build_dir"
make -C "$original_build_dir" libgif.so libgif.a
cargo build --manifest-path safe/Cargo.toml --release
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" safe/tests/malformed_observe.c "$original_build_dir/libgif.a" -o /tmp/malformed_observe.original
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
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" "$original_build_dir/tests/public_api_regress.c" "$original_build_dir/libgif.a" -o /tmp/public_api_regress.original
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
if find safe/.. -maxdepth 1 -type f -name '*.deb' ! -name 'libgif7_*.deb' ! -name 'libgif-dev_*.deb' ! -name 'libgif7-dbgsym_*.deb' | grep -q .; then
  echo 'unexpected non-library Debian package artifact built from safe/' >&2
  find safe/.. -maxdepth 1 -type f -name '*.deb' >&2
  exit 1
fi
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
runtime_matches="$(find safe/.. -maxdepth 1 -type f -name 'libgif7_*.deb' | LC_ALL=C sort)"
dev_matches="$(find safe/.. -maxdepth 1 -type f -name 'libgif-dev_*.deb' | LC_ALL=C sort)"
test "$(printf '%s\n' "$runtime_matches" | sed '/^$/d' | wc -l)" -eq 1
test "$(printf '%s\n' "$dev_matches" | sed '/^$/d' | wc -l)" -eq 1
runtime_deb="$(printf '%s\n' "$runtime_matches" | head -n1)"
dev_deb="$(printf '%s\n' "$dev_matches" | head -n1)"
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
rg -n '^COPY[[:space:]]+\\.?/?original/?[[:space:]]+/work/original/?$' test-original.sh
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

## Success Criteria
- No bootstrap build reference remains in the final library build inputs.
- Every remaining `unsafe` site in `safe/src/` has a nearby `SAFETY:` justification comment.
- The final library passes the full matrix: ABI layout, symbol parity, source compatibility, recursive fixture-consumption checks, malformed baseline checks, regression suite including `gif2rgb-regress`, performance gate, Debian package verification, and downstream harness verification.
- Exported Rust ABI entry points catch panics and return C-compatible failures instead of unwinding across the ABI boundary.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
