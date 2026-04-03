# Phase 5

## Phase Name
Catch-All Compatibility Fixes, Review, And Final Full Matrix

## Implement Phase ID
`impl_05_regression_catchall`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/compat/`
- `safe/tests/compat/README.md`
- `safe/tests/abi_layout.c`
- `safe/tests/internal_exports_smoke.c`
- `safe/tests/malformed_observe.c`
- `safe/tests/capture_malformed_baseline.sh`
- `safe/tests/malformed/`
- `safe/tests/malformed/manifest.txt`
- `safe/tests/malformed/original-baseline.txt`
- `safe/tests/perf_compare.sh`
- `safe/debian/control`
- `safe/debian/rules`
- `safe/debian/changelog`
- `safe/debian/libgif7.symbols`
- `safe/debian/libgif7.install`
- `safe/debian/libgif-dev.install`
- `safe/debian/pkgconfig/libgif7.pc.in`
- `safe/debian/source/format`
- `test-original.sh`
- `dependents.json`
- `relevant_cves.json`
- `original/gif_lib.h`
- `original/gif_hash.h`
- `original/debian/libgif7.symbols`
- `original/tests/public_api_regress.c`
- `original/tests/legacy.summary`
- `original/tests/alloc.summary`
- `original/tests/`
- `original/pic/`

## New Outputs
- final catch-all fixes only
- complete local regression inventory for every discovered downstream issue
- final verified full replacement matrix

## File Changes
- update `safe/src/*.rs` as required by remaining compatibility bugs
- update `safe/tests/Makefile`
- update `safe/tests/compat/*`
- update `safe/tests/perf_compare.sh` only if a benchmark bug is found
- update `safe/debian/*` only if package-surface fixes remain
- update `test-original.sh` only if final scope orchestration or logging still needs cleanup

## Implementation Details
- Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_05_regression_catchall:`.
- After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
- This is the catch-all phase and the only bounce target for the final full-matrix verifier.
- Do not open new fronts here. Only fix issues proven by earlier checks or by `check_05_final_full`.
- Every remaining issue must leave behind a local regression in `safe/tests/compat/` or an existing deterministic target in `safe/tests/Makefile`.
- Consume the existing 13-entry `dependents.json` inventory in place. Do not recollect, regenerate, or replace the downstream app list unless that inventory is proven wrong.
- Preserve the current Rust conventions while fixing remaining bugs: keep the original subsystem split across `safe/src/`, keep C-style FFI names and parameter casing at the ABI boundary, keep `#![deny(unsafe_op_in_unsafe_fn)]` enabled, and keep remaining `unsafe` explicit with nearby `SAFETY:` comments.
- Preserve the Rust-only production build and the byte-for-byte public header match.
- Preserve current malformed baseline behavior unless intentionally adding new malformed fixtures with explicit provenance updates.
- Keep panic fencing and `SAFETY:` comments intact while resolving final bugs.
- Treat tracked files under `original/` as immutable oracle inputs. If a local oracle rebuild is needed, build from a temporary copy instead of the tracked tree.
- Do not change `dependents.json` or `relevant_cves.json` unless the underlying inventory or scoped CVE analysis is proven wrong.
- Treat untracked root-level `.deb` files and generated `safe/tests/` binaries as disposable build outputs rather than trusted inputs; rebuild or overwrite them as part of verification.
- If local iteration edits decoder/slurp/data-path files such as `safe/src/decode.rs`, `safe/src/slurp.rs`, `safe/src/io.rs`, `safe/src/state.rs`, or `safe/src/helpers.rs`, rerun `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `malformed-regress`, and `malformed-baseline-regress` before yielding.
- If local iteration edits encoder/write/quantize/drawing files such as `safe/src/encode.rs`, `safe/src/gcb.rs`, `safe/src/helpers.rs`, `safe/src/quantize.rs`, or `safe/src/draw.rs`, rerun `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, `gif2rgb-regress`, `gifecho-regress`, `drawing-regress`, and `gifwedge-regress` before yielding.
- If local iteration edits hot-path files `safe/src/decode.rs`, `safe/src/encode.rs`, `safe/src/quantize.rs`, `safe/src/helpers.rs`, `safe/src/state.rs`, or `safe/src/io.rs`, rerun `safe/tests/perf_compare.sh` before yielding.
- If local iteration edits `safe/debian/*`, `safe/build.rs`, `safe/Cargo.toml`, or public install/layout behavior, rerun the extracted-package assertions and compile `original/tests/public_api_regress.c` against the extracted `libgif-dev` contents before yielding.
- If local iteration edits FFI entry points or raw-pointer-heavy code, rerun the `SAFETY:` audit and keep panic fencing at the C ABI boundary before yielding.

## Verification Phases
### `check_05_regression_matrix`
- Phase ID: `check_05_regression_matrix`
- Type: `check`
- Bounce Target: `impl_05_regression_catchall`
- Purpose: software-tester verification that all discovered issues now have local regressions and that the full local/package/performance matrix is stable before the last Docker pass.
- Commands:
```bash
if [ -n "$(git status --short --untracked-files=no)" ]; then
  git status --short --untracked-files=no >&2
  echo 'tracked worktree must be clean before verification' >&2
  exit 1
fi
git diff --quiet --exit-code
git diff --cached --quiet --exit-code
cargo build --manifest-path safe/Cargo.toml --release
if rg -n 'cc::Build|legacy backend|gif_legacy|\.\./original/.*\.c' safe/build.rs safe/Cargo.toml safe/src; then
  echo 'production build reintroduced original C sources' >&2
  exit 1
fi
cmp -s safe/include/gif_lib.h original/gif_lib.h
cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
/tmp/giflib-abi-layout
diff -u original/debian/libgif7.symbols safe/debian/libgif7.symbols
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress link-compat-regress internal-export-regress malformed-regress malformed-baseline-regress compat-regress
rm -f libgif7_*.deb libgif-dev_*.deb libgif7-dbgsym_*.ddeb giflib_*.changes giflib_*.buildinfo
(cd safe && dpkg-buildpackage -us -uc -b)
if find . -maxdepth 1 -type f \( -name '*.deb' -o -name '*.ddeb' \) ! -name 'libgif7_*.deb' ! -name 'libgif-dev_*.deb' ! -name 'libgif7-dbgsym_*.ddeb' | grep -q .; then
  echo 'unexpected non-library Debian package artifact' >&2
  find . -maxdepth 1 -type f \( -name '*.deb' -o -name '*.ddeb' \) >&2
  exit 1
fi
if find . -maxdepth 1 -type f \( -name '*.changes' -o -name '*.buildinfo' \) ! -name 'giflib_*.changes' ! -name 'giflib_*.buildinfo' | grep -q .; then
  echo 'unexpected non-giflib build metadata artifact' >&2
  find . -maxdepth 1 -type f \( -name '*.changes' -o -name '*.buildinfo' \) >&2
  exit 1
fi
runtime_matches="$(find . -maxdepth 1 -type f -name 'libgif7_*.deb' | LC_ALL=C sort)"
dev_matches="$(find . -maxdepth 1 -type f -name 'libgif-dev_*.deb' | LC_ALL=C sort)"
dbgsym_matches="$(find . -maxdepth 1 -type f -name 'libgif7-dbgsym_*.ddeb' | LC_ALL=C sort)"
changes_matches="$(find . -maxdepth 1 -type f -name 'giflib_*.changes' | LC_ALL=C sort)"
buildinfo_matches="$(find . -maxdepth 1 -type f -name 'giflib_*.buildinfo' | LC_ALL=C sort)"
[[ "$(printf '%s\n' "$runtime_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$dev_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$dbgsym_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$changes_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$buildinfo_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
runtime_deb="$(printf '%s\n' "$runtime_matches" | head -n1)"
dev_deb="$(printf '%s\n' "$dev_matches" | head -n1)"
dbgsym_ddeb="$(printf '%s\n' "$dbgsym_matches" | head -n1)"
changes_file="$(printf '%s\n' "$changes_matches" | head -n1)"
buildinfo_file="$(printf '%s\n' "$buildinfo_matches" | head -n1)"
test -n "$runtime_deb"
test -n "$dev_deb"
test -n "$dbgsym_ddeb"
test -n "$changes_file"
test -n "$buildinfo_file"
runtime_tmp="$(mktemp -d)"
dev_tmp="$(mktemp -d)"
dpkg-deb -x "$runtime_deb" "$runtime_tmp"
dpkg-deb -x "$dev_deb" "$dev_tmp"
runtime_real="$(find "$runtime_tmp/usr/lib/$multiarch" -maxdepth 1 -type f -name 'libgif.so.7.*' | LC_ALL=C sort | head -n1)"
test -n "$runtime_real"
readelf -d "$runtime_real" | grep -E 'SONAME.*libgif\.so\.7'
test -L "$runtime_tmp/usr/lib/$multiarch/libgif.so.7"
test -f "$dev_tmp/usr/include/gif_lib.h"
cmp -s "$dev_tmp/usr/include/gif_lib.h" original/gif_lib.h
test -f "$dev_tmp/usr/lib/$multiarch/libgif.a"
test -L "$dev_tmp/usr/lib/$multiarch/libgif.so"
pkgconfig_dir="$dev_tmp/usr/lib/$multiarch/pkgconfig"
test -f "$pkgconfig_dir/libgif7.pc"
libgif_pc="$pkgconfig_dir/libgif.pc"
if [ -L "$libgif_pc" ]; then
  test "$(readlink "$libgif_pc")" = "libgif7.pc"
else
  test -f "$libgif_pc"
fi
env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --exists libgif7
env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --exists libgif
test "$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --variable=libdir libgif7)" = "/usr/lib/$multiarch"
test "$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --variable=includedir libgif7)" = "/usr/include"
if find "$runtime_tmp" "$dev_tmp" \( -type f -o -type l \) \( -name 'gif_hash.h' -o -name 'gif_lib_private.h' \) | grep -q .; then
  echo 'unexpected private header installed in packages' >&2
  exit 1
fi
cc -std=gnu99 -Wall -Wextra -I"$dev_tmp/usr/include" original/tests/public_api_regress.c "$dev_tmp/usr/lib/$multiarch/libgif.a" -o /tmp/public_api_regress.pkg
/tmp/public_api_regress.pkg legacy | diff -u original/tests/legacy.summary -
/tmp/public_api_regress.pkg alloc | diff -u original/tests/alloc.summary -
original_build_dir="$(mktemp -d)"
trap 'rm -rf "$original_build_dir"' EXIT
cp -a original/. "$original_build_dir"
make -C "$original_build_dir" libgif.so libgif.a
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" "$original_build_dir/tests/public_api_regress.c" "$original_build_dir/libgif.a" -o /tmp/public_api_regress.original
cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf.log
grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf.log
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

### `check_05_senior_review`
- Phase ID: `check_05_senior_review`
- Type: `check`
- Bounce Target: `impl_05_regression_catchall`
- Purpose: senior-tester review of the final catch-all fix set, with emphasis on regression completeness, minimality, and safety boundaries.
- Commands:
```bash
if [ -n "$(git status --short --untracked-files=no)" ]; then
  git status --short --untracked-files=no >&2
  echo 'tracked worktree must be clean before verification' >&2
  exit 1
fi
git diff --quiet --exit-code
git diff --cached --quiet --exit-code
git show --stat --name-only --format=fuller HEAD^..HEAD
find safe/tests/compat -maxdepth 2 -type f | LC_ALL=C sort
rg -n '^compat-regress:' safe/tests/Makefile
rg -n 'SAFETY:' safe/src
rg -n 'catch_panic_or|catch_error_or|catch_gif_error_or|catch_gif_and_error_or' safe/src
```
- Review Checks:
  - Every issue found in phases 3 and 4 must be traceable to a committed local reproducer.
  - `safe/tests/malformed/original-baseline.txt` must not change unless a new malformed fixture was intentionally added and its provenance was documented.
  - `dependents.json` must remain unchanged unless the inventory itself was proven wrong and that decision was explicitly justified.

### `check_05_final_full`
- Phase ID: `check_05_final_full`
- Type: `check`
- Bounce Target: `impl_05_regression_catchall`
- Purpose: final software-tester gate across the complete local, package, performance, and downstream-replacement matrix.
- Commands:
```bash
if [ -n "$(git status --short --untracked-files=no)" ]; then
  git status --short --untracked-files=no >&2
  echo 'tracked worktree must be clean before verification' >&2
  exit 1
fi
git diff --quiet --exit-code
git diff --cached --quiet --exit-code
cargo build --manifest-path safe/Cargo.toml --release
if rg -n 'cc::Build|legacy backend|gif_legacy|\.\./original/.*\.c' safe/build.rs safe/Cargo.toml safe/src; then
  echo 'production build reintroduced original C sources' >&2
  exit 1
fi
cmp -s safe/include/gif_lib.h original/gif_lib.h
cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
/tmp/giflib-abi-layout
objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
test "$(objdump -T safe/target/release/libgif.so | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
diff -u original/debian/libgif7.symbols safe/debian/libgif7.symbols
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress link-compat-regress internal-export-regress malformed-regress malformed-baseline-regress compat-regress
rm -f libgif7_*.deb libgif-dev_*.deb libgif7-dbgsym_*.ddeb giflib_*.changes giflib_*.buildinfo
(cd safe && dpkg-buildpackage -us -uc -b)
if find . -maxdepth 1 -type f \( -name '*.deb' -o -name '*.ddeb' \) ! -name 'libgif7_*.deb' ! -name 'libgif-dev_*.deb' ! -name 'libgif7-dbgsym_*.ddeb' | grep -q .; then
  echo 'unexpected non-library Debian package artifact' >&2
  find . -maxdepth 1 -type f \( -name '*.deb' -o -name '*.ddeb' \) >&2
  exit 1
fi
if find . -maxdepth 1 -type f \( -name '*.changes' -o -name '*.buildinfo' \) ! -name 'giflib_*.changes' ! -name 'giflib_*.buildinfo' | grep -q .; then
  echo 'unexpected non-giflib build metadata artifact' >&2
  find . -maxdepth 1 -type f \( -name '*.changes' -o -name '*.buildinfo' \) >&2
  exit 1
fi
runtime_matches="$(find . -maxdepth 1 -type f -name 'libgif7_*.deb' | LC_ALL=C sort)"
dev_matches="$(find . -maxdepth 1 -type f -name 'libgif-dev_*.deb' | LC_ALL=C sort)"
dbgsym_matches="$(find . -maxdepth 1 -type f -name 'libgif7-dbgsym_*.ddeb' | LC_ALL=C sort)"
changes_matches="$(find . -maxdepth 1 -type f -name 'giflib_*.changes' | LC_ALL=C sort)"
buildinfo_matches="$(find . -maxdepth 1 -type f -name 'giflib_*.buildinfo' | LC_ALL=C sort)"
[[ "$(printf '%s\n' "$runtime_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$dev_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$dbgsym_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$changes_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
[[ "$(printf '%s\n' "$buildinfo_matches" | sed '/^$/d' | wc -l)" -eq 1 ]]
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
runtime_deb="$(printf '%s\n' "$runtime_matches" | head -n1)"
dev_deb="$(printf '%s\n' "$dev_matches" | head -n1)"
dbgsym_ddeb="$(printf '%s\n' "$dbgsym_matches" | head -n1)"
changes_file="$(printf '%s\n' "$changes_matches" | head -n1)"
buildinfo_file="$(printf '%s\n' "$buildinfo_matches" | head -n1)"
test -n "$runtime_deb"
test -n "$dev_deb"
test -n "$dbgsym_ddeb"
test -n "$changes_file"
test -n "$buildinfo_file"
runtime_tmp="$(mktemp -d)"
dev_tmp="$(mktemp -d)"
dpkg-deb -x "$runtime_deb" "$runtime_tmp"
dpkg-deb -x "$dev_deb" "$dev_tmp"
runtime_real="$(find "$runtime_tmp/usr/lib/$multiarch" -maxdepth 1 -type f -name 'libgif.so.7.*' | LC_ALL=C sort | head -n1)"
test -n "$runtime_real"
readelf -d "$runtime_real" | grep -E 'SONAME.*libgif\.so\.7'
test -L "$runtime_tmp/usr/lib/$multiarch/libgif.so.7"
test -f "$dev_tmp/usr/include/gif_lib.h"
cmp -s "$dev_tmp/usr/include/gif_lib.h" original/gif_lib.h
test -f "$dev_tmp/usr/lib/$multiarch/libgif.a"
test -L "$dev_tmp/usr/lib/$multiarch/libgif.so"
pkgconfig_dir="$dev_tmp/usr/lib/$multiarch/pkgconfig"
test -f "$pkgconfig_dir/libgif7.pc"
libgif_pc="$pkgconfig_dir/libgif.pc"
if [ -L "$libgif_pc" ]; then
  test "$(readlink "$libgif_pc")" = "libgif7.pc"
else
  test -f "$libgif_pc"
fi
env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --exists libgif7
env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --exists libgif
test "$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --variable=libdir libgif7)" = "/usr/lib/$multiarch"
test "$(env PKG_CONFIG_PATH= PKG_CONFIG_LIBDIR="$pkgconfig_dir" PKG_CONFIG_SYSROOT_DIR= pkg-config --variable=includedir libgif7)" = "/usr/include"
if find "$runtime_tmp" "$dev_tmp" \( -type f -o -type l \) \( -name 'gif_hash.h' -o -name 'gif_lib_private.h' \) | grep -q .; then
  echo 'unexpected private header installed in packages' >&2
  exit 1
fi
cc -std=gnu99 -Wall -Wextra -I"$dev_tmp/usr/include" original/tests/public_api_regress.c "$dev_tmp/usr/lib/$multiarch/libgif.a" -o /tmp/public_api_regress.pkg
/tmp/public_api_regress.pkg legacy | diff -u original/tests/legacy.summary -
/tmp/public_api_regress.pkg alloc | diff -u original/tests/alloc.summary -
bash -o pipefail -c './test-original.sh --scope all | tee /tmp/test-all.log'
python3 - <<'PY'
from pathlib import Path
import sys

log = Path('/tmp/test-all.log').read_text(encoding='utf-8')
shared = [
    '==> Building safe Debian packages',
    '==> Installing safe Debian packages',
    '==> Verifying runtime linkage to active packaged giflib',
]
runtime = [
    '==> giflib-tools',
    '==> webp',
    '==> fbi',
    '==> mtpaint',
    '==> tracker-extract',
    '==> libextractor-plugin-gif',
    '==> libcamlimages-ocaml',
    '==> libgdal34t64',
]
source = [
    '==> gdal (source)',
    '==> exactimage (source)',
    '==> sail (source)',
    '==> libwebp (source)',
    '==> imlib2 (source)',
]
required = shared + runtime + source + ['All downstream checks passed']

missing = [marker for marker in required if marker not in log]
count_errors = [marker for marker in shared if log.count(marker) != 1]

if missing or count_errors:
    if missing:
        print('missing all-scope markers:', *missing, sep='\n', file=sys.stderr)
    if count_errors:
        print('shared setup markers must appear exactly once during all scope:', *count_errors, sep='\n', file=sys.stderr)
    sys.exit(1)

if max(log.index(marker) for marker in runtime) >= min(log.index(marker) for marker in source):
    print('runtime markers must complete before source markers begin during all scope', file=sys.stderr)
    sys.exit(1)
PY
original_build_dir="$(mktemp -d)"
trap 'rm -rf "$original_build_dir"' EXIT
cp -a original/. "$original_build_dir"
make -C "$original_build_dir" libgif.so libgif.a
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" "$original_build_dir/tests/public_api_regress.c" "$original_build_dir/libgif.a" -o /tmp/public_api_regress.original
cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf.log
grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf.log
```

## Success Criteria
- Every issue found in phases 3 and 4 is traceable to a committed local regression or deterministic regression target.
- The full local regression matrix, package assertions, and downstream replacement matrix pass, including `./test-original.sh --scope all` with one shared setup pass and runtime markers completing before source markers begin.
- Rust-only build constraints, public-header parity, malformed baseline expectations, panic fencing, and `SAFETY:` coverage remain intact.
- `check_05_regression_matrix`, `check_05_senior_review`, and `check_05_final_full` all pass.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding. The phase must end as exactly one non-merge commit whose subject starts with `impl_05_regression_catchall:`, followed by a clean tracked worktree and index.
