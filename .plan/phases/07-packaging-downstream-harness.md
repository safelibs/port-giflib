# Phase 7

## Phase Name
Debian Packaging And Drop-In Downstream Harness

## Implement Phase ID
`impl_07_packaging`

## Workflow Contract Notes
- Consume existing artifacts in place. Adapt packaging from `original/debian/`, keep `original/pic/` and the committed downstream matrix as existing oracle inputs, and do not modify `dependents.json`.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. This phase must package and install the locally rebuilt safe artifacts, while `original/` stays only a source, header, and fixture oracle.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/perf_compare.sh`
- `original/debian/control`
- `original/debian/rules`
- `original/debian/libgif7.symbols`
- `original/debian/libgif7.install`
- `original/debian/libgif-dev.install`
- `original/debian/pkgconfig/libgif7.pc.in`
- `original/pic/`
- `test-original.sh`
- `dependents.json`

## New Outputs
- Debian packaging for locally versioned `libgif7` and `libgif-dev`
- Modified Docker harness that builds the exact local safe packages, installs them, logs built and active package identity markers plus labeled resolved library/archive paths for every dependent linkage assertion, and uses `original/` only as a fixture/source oracle

## File Changes
- Create `safe/debian/control`
- Create `safe/debian/rules`
- Create `safe/debian/changelog`
- Create `safe/debian/libgif7.symbols`
- Create `safe/debian/libgif7.install`
- Create `safe/debian/libgif-dev.install`
- Create `safe/debian/pkgconfig/libgif7.pc.in`
- Create `safe/debian/source/format`
- Create any minimal additional Debian support files required by debhelper
- Update `test-original.sh`

## Implementation Details
- Adapt the packaging from `original/debian/` rather than inventing a new package structure. Preserve binary package names `libgif7` and `libgif-dev`.
- Preserve panic-boundary wrappers on any Rust-defined C ABI entry point touched while wiring the final build and packaging flow so panics still become C-compatible failure values instead of unwinding across the ABI boundary.
- Make `safe/` a library-only Debian source package. Do not build or ship a `giflib-tools` binary package from `safe/`; keep using Ubuntu’s existing `giflib-tools` package and the other downstream packages as consumers of the replacement `libgif7` and `libgif-dev`.
- Update `safe/debian/control` so the safe packaging declares the Rust build dependencies it actually uses (`cargo`, `rustc`, and any debhelper cargo helper invoked by `debian/rules`) and drops doc-only dependencies such as `xmlto` unless the safe packaging truly consumes them.
- Set `safe/debian/changelog` to a distinct local version derived from the current Ubuntu package version recorded in `original/debian/changelog`, for example `5.2.2-1ubuntu1+safelibs1`. Do not reuse the stock `5.2.2-1ubuntu1` version verbatim, because the downstream harness must be able to prove it installed the locally built replacement packages.
- Set `safe/debian/source/format` to `3.0 (quilt)`. Do not use `3.0 (native)`, because the required local version string keeps the upstream `-1ubuntu1` Debian revision component.
- Have `safe/debian/rules` drive the Rust build directly, stage a real versioned multiarch shared object filename that matches the upstream `LIBVER` and SONAME scheme, create the `libgif.so.7` and `libgif.so` symlinks, and install the header, static archive, and pkg-config files into the same multiarch locations the existing Debian templates expect.
- Do not widen scope to port the CLI tools. Keep using the distribution’s `giflib-tools` and other downstream packages; the safe library package must be able to replace only the library/devel packages underneath them.
- Install the multiarch library, symlinks, header, static archive, and pkg-config files exactly where the existing Debian templates expect them, and do not install `gif_hash.h` or `gif_lib_private.h`.
- Install `libgif.pc` as either a regular file or a relative symlink `libgif.pc -> libgif7.pc`. Do not reuse the original absolute symlink form, because the phase-local extracted-package validation must stay self-contained and independent of the host `/usr/lib` tree.
- Keep the symbol file semantically identical to `original/debian/libgif7.symbols`.
- Update `test-original.sh` so the container copies `original/` into `/work/original` as an existing fixture source, copies `safe/` into `/work/safe`, installs Rust packaging/build dependencies, builds the local `.deb`s, installs those packages over Ubuntu’s stock `libgif7` and `libgif-dev`, and reruns the existing runtime and compile-time dependent checks unchanged wherever possible.
- Remove the current manual original-library build/install path from `test-original.sh`. After this phase, `original/` stays in the container only as an oracle for fixtures or source-inspection, not as something the harness compiles or installs.
- Add explicit harness helpers named `build_safe_packages`, `install_safe_packages`, `resolve_installed_shared_libgif`, `resolve_installed_static_libgif`, `assert_links_to_active_shared_libgif`, and `assert_build_uses_active_giflib`.
- `build_safe_packages` must build the local `.deb`s from the staged `safe/` tree, store their exact paths in `SAFE_RUNTIME_DEB` and `SAFE_DEV_DEB`, capture `Package` and `Version` fields into `SAFE_RUNTIME_PACKAGE`, `SAFE_DEV_PACKAGE`, `SAFE_RUNTIME_VERSION`, and `SAFE_DEV_VERSION` via `dpkg-deb -f`, and print those key/value lines.
- `install_safe_packages` must install those exact `.deb` files via `dpkg -i`, assert with `dpkg-query -W` that the active `libgif7` and `libgif-dev` versions equal the recorded built versions, and print `ACTIVE_RUNTIME_VERSION=` and `ACTIVE_DEV_VERSION=`.
- `resolve_installed_shared_libgif` must derive the active runtime `libgif.so.7` path from `dpkg -L libgif7` or `dpkg-query -L libgif7` plus `ldconfig -p`, assert ownership with `dpkg-query -S`, export `ACTIVE_SHARED_LIBGIF` plus `ACTIVE_SHARED_OWNER`, and print labeled lines `ACTIVE_SHARED_LIBGIF[$label]=...` and `ACTIVE_SHARED_OWNER[$label]=...`.
- `resolve_installed_static_libgif` must derive the packaged development archive path from `dpkg -L libgif-dev` or `dpkg-query -L libgif-dev`, assert ownership with `dpkg-query -S`, export `ACTIVE_STATIC_LIBGIF` plus `ACTIVE_STATIC_OWNER`, and print labeled lines `ACTIVE_STATIC_LIBGIF[$label]=...` and `ACTIVE_STATIC_OWNER[$label]=...`.
- `assert_links_to_active_shared_libgif` must call `resolve_installed_shared_libgif "$label"` immediately before running `ldd`, require the resulting `ACTIVE_SHARED_LIBGIF` path to appear in that `ldd` log, and be used only for the runtime labels `giflib-tools-runtime`, `webp-runtime`, `fbi-runtime`, `mtpaint-runtime`, `tracker-extract-runtime`, `libextractor-runtime`, `camlimages-runtime`, and `gdal-runtime`.
- `assert_build_uses_active_giflib` must call both resolvers with the same label immediately before checking a source-built artifact, accept either shared linkage via `ldd` containing `ACTIVE_SHARED_LIBGIF` or static linkage via the recorded build link command containing `ACTIVE_STATIC_LIBGIF`, print `LINK_ASSERT_MODE[$label]=shared|static`, and be used for every source-build label `gdal-source`, `exactimage-source`, `sail-source`, `libwebp-source`, and `imlib2-source` at the exact assertion sites listed in `check_07_downstream`.
- Remove helper functions and assertions tied to the original `/usr/local` install flow, including `build_original_giflib` and `assert_uses_original`, rather than leaving dead code alongside the new package-based path.
- Replace the current `/usr/local/lib/libgif.so.7` assertions with package-path assertions derived from the installed `libgif7` package contents, for example via `gcc -print-multiarch`, `dpkg -L libgif7`, and `ldconfig`, so the harness verifies the packaged replacement rather than a manually installed `/usr/local` build.
- Replace the `/usr/local/lib/libgif.a` fallback check in the downstream source-build cases with the packaged development-archive path resolved from `dpkg -L libgif-dev`, so both shared-link and static-link assertions point at the installed safe packages rather than the old manual install location.
- For every source-build label that can legitimately fall back to static linkage, capture the exact final linker invocation before calling `assert_build_uses_active_giflib`. Reuse project-native evidence where available, such as CMake `link.txt`, and otherwise enable verbose build output or wrap the linker so the helper inspects the real final link command instead of guessing from configure logs.
- Keep `dependents.json` unchanged; the harness must continue to validate exactly that downstream matrix.

## Verification Phases

### `check_07_package_build`
- Phase ID: `check_07_package_build`
- Type: `check`
- Bounce Target: `impl_07_packaging`
- Purpose: Verify that `safe/` builds installable Debian packages with the expected names, local version suffix, files, SONAME, pkg-config metadata, exported symbols, and no private headers anywhere in the extracted package trees.
- Commands:
```bash
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

### `check_07_downstream`
- Phase ID: `check_07_downstream`
- Type: `check`
- Bounce Target: `impl_07_packaging`
- Purpose: Verify that the modified downstream harness no longer relies on `/usr/local`, builds and installs the exact local safe packages it produces, proves those packages are the active installed `libgif7` and `libgif-dev`, routes every linkage assertion through the package-derived helper paths with fixed labels, and proves that all sampled dependents still compile and run.
- Commands:
```bash
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
- `safe/debian/` produces library-only Debian artifacts: exactly one `libgif7_*.deb`, exactly one `libgif-dev_*.deb`, and optionally `libgif7-dbgsym_*.deb`, with no unexpected package outputs.
- The extracted runtime and development packages each prove the required files, symbol surface, SONAME, isolated pkg-config metadata, local `+safelibs` version, and header-surface restrictions.
- `test-original.sh` no longer relies on `/usr/local`, consumes `dependents.json` in place, copies `safe/` and `original/` into the container at the required paths, defines and uses the required helper functions, and logs all required package and linkage markers for every runtime and source-build label.
- Both `check_07_package_build` and `check_07_downstream` pass.
- Packaging and harness changes preserve the required panic-boundary behavior on exported Rust ABI entry points.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
