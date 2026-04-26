# Phase 02

## Phase Name
Package Surface Lock And Downstream Harness Scoping

## Implement Phase ID
`impl_02_package_and_harness`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/debian/control`
- `safe/debian/rules`
- `safe/debian/changelog`
- `safe/debian/libgif7.symbols`
- `safe/debian/libgif7.install`
- `safe/debian/libgif-dev.install`
- `safe/debian/pkgconfig/libgif7.pc.in`
- `safe/debian/source/format`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/compat/`
- `safe/tests/compat/README.md`
- `test-original.sh`
- `dependents.json`
- `original/gif_lib.h`
- `original/debian/libgif7.symbols`
- `original/tests/public_api_regress.c`
- `original/tests/legacy.summary`
- `original/tests/alloc.summary`

## New Outputs
- package surface locked to the original contract and locally versioned safe package suffix
- scoped downstream harness interface via `test-original.sh --scope runtime|source|all`
- retained package-derived linkage assertion helpers in the downstream harness

## File Changes
- update `safe/debian/control`
- update `safe/debian/rules`
- update `safe/debian/changelog`
- update `safe/debian/libgif7.symbols`
- update `safe/debian/libgif7.install`
- update `safe/debian/libgif-dev.install`
- update `safe/debian/pkgconfig/libgif7.pc.in`
- update `safe/debian/source/format` only if it drifted
- update `test-original.sh`
- update `safe/build.rs` only if package/install-path behavior requires it

## Implementation Details
- Keep the package names `libgif7` and `libgif-dev`.
- Keep the local package version suffix `+safelibs...`.
- Preserve the current library-only packaging surface; do not introduce a `giflib-tools` package from `safe/`.
- Preserve the existing relative `libgif.pc -> libgif7.pc` behavior or an equivalent regular-file implementation; do not use an absolute symlink.
- Consume the existing 13-entry `dependents.json` inventory in place. Do not recollect, regenerate, or replace the downstream app list unless that inventory is proven wrong.
- Consume existing packaging and downstream artifacts in place rather than creating sibling harnesses or alternate package metadata files.
- Add scoped downstream execution to `test-original.sh`:
  - `--scope runtime`
  - `--scope source`
  - `--scope all`
  - default `all`
- Parse CLI arguments before any `docker build` or `docker run`.
- Make `--help`, missing `--scope` arguments, invalid `--scope` values, and unexpected positional arguments fail or exit cleanly before any Docker side effects.
- Keep package build/install and runtime-linkage resolution shared across scopes so runtime and source phases verify the same installation path.
- `--scope runtime` must run the shared setup exactly once and then only the runtime dependent checks.
- `--scope source` must run the shared setup exactly once and then only the source-build dependent checks.
- `--scope all`, and the implicit no-flag default path, must run the shared setup exactly once and then execute the runtime subset followed by the source subset.
- Preserve the existing dependent-specific `log_step` markers because later verifier phases grep for those exact markers.
- Do not split the downstream logic into a second script; update `test-original.sh` in place.
- Do not change `dependents.json`; use its current fixed matrix as the source of truth.
- Treat tracked files under `original/` as immutable oracle inputs. If a local original build is needed while iterating on packaging checks, build from a temporary copy instead of the tracked tree.
- Treat untracked root-level `.deb` files and generated `safe/tests/` binaries as disposable build outputs; rebuild or overwrite them during verification rather than trusting leftovers.

## Verification Phases
### `check_02_package_surface`
- Phase ID: `check_02_package_surface`
- Type: `check`
- Bounce Target: `impl_02_package_and_harness`
- Purpose: software-tester verification of Debian package contents, installed development surface, and package metadata.
- Commands:
```bash
if [ -n "$(git status --short --untracked-files=no)" ]; then
  git status --short --untracked-files=no >&2
  echo 'tracked worktree must be clean before verification' >&2
  exit 1
fi
git diff --quiet --exit-code
git diff --cached --quiet --exit-code
diff -u original/debian/libgif7.symbols safe/debian/libgif7.symbols
grep -x '3.0 (quilt)' safe/debian/source/format
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
test "$(dpkg-deb -f "$runtime_deb" Package)" = "libgif7"
test "$(dpkg-deb -f "$dev_deb" Package)" = "libgif-dev"
case "$(dpkg-deb -f "$runtime_deb" Version)" in
  *+safelibs*) ;;
  *) echo 'missing local safelibs version suffix' >&2; exit 1 ;;
esac
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
```

### `check_02_harness_review`
- Phase ID: `check_02_harness_review`
- Type: `check`
- Bounce Target: `impl_02_package_and_harness`
- Purpose: senior-tester review of `test-original.sh` changes needed for scoped downstream execution.
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
bash -n test-original.sh
stub_dir="$(mktemp -d)"
cat > "$stub_dir/docker" <<'SH'
#!/bin/sh
echo 'docker should not run for parse-only paths' >&2
exit 97
SH
chmod +x "$stub_dir/docker"
PATH="$stub_dir:$PATH" bash test-original.sh --help > /tmp/test-original-help.txt
grep -E -- '--scope( |=)(runtime|source|all)|runtime\|source\|all' /tmp/test-original-help.txt
if PATH="$stub_dir:$PATH" bash test-original.sh --scope bogus >/tmp/test-original-invalid.log 2>&1; then
  echo 'invalid scope unexpectedly succeeded' >&2
  exit 1
fi
grep -Ei 'invalid scope|usage' /tmp/test-original-invalid.log
if grep -F 'docker should not run for parse-only paths' /tmp/test-original-invalid.log; then
  echo 'invalid scope path invoked docker before rejecting arguments' >&2
  exit 1
fi
if PATH="$stub_dir:$PATH" bash test-original.sh --scope >/tmp/test-original-missing.log 2>&1; then
  echo 'missing scope argument unexpectedly succeeded' >&2
  exit 1
fi
grep -Ei 'missing.*scope|usage' /tmp/test-original-missing.log
if grep -F 'docker should not run for parse-only paths' /tmp/test-original-missing.log; then
  echo 'missing scope path invoked docker before rejecting arguments' >&2
  exit 1
fi
if PATH="$stub_dir:$PATH" bash test-original.sh unexpected >/tmp/test-original-extra-arg.log 2>&1; then
  echo 'unexpected positional argument unexpectedly succeeded' >&2
  exit 1
fi
grep -Ei 'unexpected argument|usage' /tmp/test-original-extra-arg.log
if grep -F 'docker should not run for parse-only paths' /tmp/test-original-extra-arg.log; then
  echo 'unexpected-argument path invoked docker before rejecting arguments' >&2
  exit 1
fi
rg -n 'GIFLIB_TEST_SCOPE|--scope' test-original.sh
rg -n 'scope=.*all|GIFLIB_TEST_SCOPE=.*all' test-original.sh
rg -n '^usage\(\)|^parse_args\(\)|^build_safe_packages\(\)|^install_safe_packages\(\)|^resolve_installed_shared_libgif\(\)|^resolve_installed_static_libgif\(\)|^assert_links_to_active_shared_libgif\(\)|^assert_build_uses_active_giflib\(\)' test-original.sh
if rg -n '/usr/local|build_original_giflib|assert_uses_original' test-original.sh; then
  echo 'stale original-install logic remains' >&2
  exit 1
fi
```
- Review Checks:
  - Confirm the script still builds and installs the exact local safe `.deb`s before any downstream checks.
  - Confirm `--help`, missing-argument, invalid-scope, and unexpected-argument paths all return before any `docker build` or `docker run`.
  - Confirm runtime and source subsets are gated separately, the default remains `all`, and `--scope all` still runs runtime markers before source markers after one shared setup pass.
  - Confirm the script still consumes `dependents.json`, `safe/`, and `original/` in place instead of duplicating them.

## Success Criteria
- Debian packaging stays limited to `libgif7`, `libgif-dev`, the matching dbgsym package, and the expected `giflib_*.changes` and `giflib_*.buildinfo` artifacts.
- The extracted `libgif-dev` package preserves the original public header, exports no private headers, and passes `original/tests/public_api_regress.c` against the packaged static library.
- `test-original.sh` supports `--scope runtime|source|all`, defaults to `all`, rejects parse-only errors before Docker work, and preserves the shared-setup-plus-subset behavior required by later phases.
- `check_02_package_surface` and `check_02_harness_review` both pass.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding. The phase must end as exactly one non-merge commit whose subject starts with `impl_02_package_and_harness:`, followed by a clean tracked worktree and index.
