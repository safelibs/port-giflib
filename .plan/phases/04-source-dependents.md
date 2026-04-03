# Phase 4

## Phase Name
Source-Build Dependent Matrix Fixes

## Implement Phase ID
`impl_04_source_dependents`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/compat/`
- `safe/tests/compat/README.md`
- `safe/tests/internal_exports_smoke.c`
- `safe/tests/malformed_observe.c`
- `safe/tests/capture_malformed_baseline.sh`
- `safe/tests/malformed/`
- `safe/tests/malformed/manifest.txt`
- `safe/tests/malformed/original-baseline.txt`
- `safe/tests/perf_compare.sh`
- `relevant_cves.json`
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
- `original/gif_lib.h`
- `original/gif_hash.h`
- `original/debian/libgif7.symbols`
- `original/tests/public_api_regress.c`
- `original/tests/legacy.summary`
- `original/tests/alloc.summary`
- `original/tests/`
- `original/pic/`

## New Outputs
- fixes for source-build and package-surface compatibility failures
- issue-specific source-build reproducers under `safe/tests/compat/`
- corresponding `compat-regress` registrations

## File Changes
- update `safe/debian/control`
- update `safe/debian/rules`
- update `safe/debian/changelog`
- update `safe/debian/libgif7.symbols`
- update `safe/debian/libgif7.install`
- update `safe/debian/libgif-dev.install`
- update `safe/debian/pkgconfig/libgif7.pc.in`
- update `safe/build.rs`
- update `safe/Cargo.toml`
- update `safe/src/lib.rs`
- update `safe/src/ffi.rs`
- update `safe/src/*.rs` only if a source-build failure proves a true library bug
- update `safe/tests/Makefile`
- create or update issue-specific reproducers under `safe/tests/compat/`
- update `test-original.sh` only if source-scope orchestration itself needs refinement

## Implementation Details
- Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_04_source_dependents:`.
- After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
- Focus on compile/link/install surface compatibility:
  - extracted package contents
  - `pkg-config` behavior
  - static and shared linkability
  - symbol/export completeness
  - header-only source compatibility
- Consume the existing 13-entry `dependents.json` inventory in place. Do not recollect, regenerate, or replace the downstream app list unless that inventory is proven wrong.
- Preserve the current Rust conventions while fixing source-build failures: keep the original subsystem split across `safe/src/`, keep C-style FFI names and parameter casing at the ABI boundary, keep `#![deny(unsafe_op_in_unsafe_fn)]` enabled, and keep remaining `unsafe` explicit with nearby `SAFETY:` comments.
- Treat these source consumers as the concrete package/build gate:
  - `gdal`
  - `exactimage`
  - `sail`
  - `libwebp`
  - `imlib2`
- When a full downstream source build reveals a bug, add the smallest stable local reproducer:
  - a small compile/link smoke test
  - a pkg-config assertion
  - a static/shared link test
  - a small C reproducer
  rather than depending only on the heavyweight downstream rebuild.
- Keep `safe/debian/libgif7.symbols` aligned to `original/debian/libgif7.symbols`.
- Preserve the library-only package set and the `+safelibs` version suffix.
- Do not change `safe/include/gif_lib.h` unless the original header itself is being copied verbatim again.
- Do not vendor downstream source snapshots into the repository.
- Treat tracked files under `original/` as immutable oracle inputs. If a local oracle rebuild is needed, build from a temporary copy instead of the tracked tree.
- Treat untracked root-level `.deb` files and generated `safe/tests/` binaries as disposable build outputs rather than trusted inputs; rebuild or overwrite them as part of verification.
- If local iteration edits `safe/debian/*`, `safe/build.rs`, `safe/Cargo.toml`, or public install/layout behavior, rerun the extracted-package assertions and compile `original/tests/public_api_regress.c` against the extracted `libgif-dev` contents before yielding.
- If local iteration edits hot-path files `safe/src/decode.rs`, `safe/src/encode.rs`, `safe/src/quantize.rs`, `safe/src/helpers.rs`, `safe/src/state.rs`, or `safe/src/io.rs`, rerun `safe/tests/perf_compare.sh` before yielding.
- If local iteration edits FFI entry points or raw-pointer-heavy code, rerun the `SAFETY:` audit and keep panic fencing at the C ABI boundary.

## Verification Phases
### `check_04_source_matrix`
- Phase ID: `check_04_source_matrix`
- Type: `check`
- Bounce Target: `impl_04_source_dependents`
- Purpose: software-tester execution of the source-build dependent Docker subset plus package-surface and local compatibility checks most likely to catch header/export/pkg-config regressions.
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
diff -u original/debian/libgif7.symbols safe/debian/libgif7.symbols
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
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress test gif2rgb-regress link-compat-regress internal-export-regress malformed-regress malformed-baseline-regress compat-regress
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
safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf-source.log
grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf-source.log
grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf-source.log
grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf-source.log
grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf-source.log
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
bash -o pipefail -c './test-original.sh --scope source | tee /tmp/test-source.log'
python3 - <<'PY'
from pathlib import Path
import sys

log = Path('/tmp/test-source.log').read_text(encoding='utf-8')
required = [
    '==> Building safe Debian packages',
    '==> Installing safe Debian packages',
    '==> Verifying runtime linkage to active packaged giflib',
    '==> gdal (source)',
    '==> exactimage (source)',
    '==> sail (source)',
    '==> libwebp (source)',
    '==> imlib2 (source)',
    'All downstream checks passed',
]
forbidden = [
    '==> giflib-tools',
    '==> webp',
    '==> fbi',
    '==> mtpaint',
    '==> tracker-extract',
    '==> libextractor-plugin-gif',
    '==> libcamlimages-ocaml',
    '==> libgdal34t64',
]

missing = [marker for marker in required if marker not in log]
unexpected = [marker for marker in forbidden if marker in log]
count_errors = [
    marker for marker in required[:3]
    if log.count(marker) != 1
]

if missing or unexpected or count_errors:
    if missing:
        print('missing source-scope markers:', *missing, sep='\n', file=sys.stderr)
    if unexpected:
        print('unexpected runtime-app markers during source scope:', *unexpected, sep='\n', file=sys.stderr)
    if count_errors:
        print('shared setup markers must appear exactly once during source scope:', *count_errors, sep='\n', file=sys.stderr)
    sys.exit(1)
PY
```

### `check_04_source_review`
- Phase ID: `check_04_source_review`
- Type: `check`
- Bounce Target: `impl_04_source_dependents`
- Purpose: senior-tester review of source-build/package-surface fixes and their regression coverage.
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
rg -n 'pkg-config|libgif7\.pc|libgif\.pc|libgif\.so|libgif\.a|symbols|--scope source' safe/debian test-original.sh safe/tests/Makefile safe/tests/compat
find safe/tests/compat -maxdepth 2 -type f | LC_ALL=C sort
rg -n 'catch_panic_or|catch_error_or|catch_gif_error_or|catch_gif_and_error_or' safe/src
```
- Review Checks:
  - Every source-build failure found in Docker must have a local reproducer or package-surface check that can fail without rebuilding a full downstream source tree.
  - Do not change `safe/include/gif_lib.h` unless the original header itself is being copied verbatim again; source-compat fixes should happen in Rust implementation or packaging, not by inventing a new header surface.
  - Do not vendor downstream source snapshots into the repository.

## Success Criteria
- Every source-build or package-surface failure found in Docker leaves behind a stable local reproducer or extracted-package assertion that can fail without rebuilding a full downstream tree.
- `./test-original.sh --scope source` runs the shared setup once, includes all source-build markers, and excludes all runtime-app markers.
- Header, symbol, package-layout, extracted-`libgif-dev`, and performance expectations remain aligned with the original contract after the fixes.
- `check_04_source_matrix` and `check_04_source_review` both pass.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding. The phase must end as exactly one non-merge commit whose subject starts with `impl_04_source_dependents:`, followed by a clean tracked worktree and index.
