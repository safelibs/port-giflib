# Phase 3

## Phase Name
Runtime Dependent Matrix Fixes

## Implement Phase ID
`impl_03_runtime_dependents`

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
- `original/tests/public_api_regress.c`
- `original/tests/`
- `original/pic/`

## New Outputs
- fixes for runtime-dependent compatibility failures
- issue-specific runtime reproducers under `safe/tests/compat/`
- corresponding `compat-regress` registrations

## File Changes
- update `safe/src/decode.rs`
- update `safe/src/slurp.rs`
- update `safe/src/helpers.rs`
- update `safe/src/io.rs`
- update `safe/src/state.rs`
- update `safe/src/gcb.rs`
- update `safe/src/draw.rs`
- update `safe/src/error.rs`
- update `safe/tests/Makefile`
- create or update issue-specific reproducers under `safe/tests/compat/`
- update `test-original.sh` only if the runtime probe itself needs stabilization

## Implementation Details
- Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_03_runtime_dependents:`.
- After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
- Focus on runtime behavior exercised by the installed library, not source-build/package metadata.
- Consume the existing 13-entry `dependents.json` inventory in place. Do not recollect, regenerate, or replace the downstream app list unless that inventory is proven wrong.
- Preserve the current Rust conventions while fixing runtime bugs: keep the original subsystem split across `safe/src/`, keep C-style FFI names and parameter casing at the ABI boundary, keep `#![deny(unsafe_op_in_unsafe_fn)]` enabled, and keep remaining `unsafe` explicit with nearby `SAFETY:` comments.
- Preserve the byte-for-byte public header match and do not invent new API surface while resolving runtime-only failures.
- Treat these downstream apps as the concrete behavior gate for runtime compatibility:
  - `giflib-tools`: text dump and general decode path
  - `webp`/`gif2webp`: valid GIF decode and animation ingestion
  - `fbi`: native GIF handling versus `convert` fallback behavior
  - `mtpaint`: valid-versus-invalid GIF UI behavior
  - `tracker-extract`: metadata and dimensions
  - `libextractor-plugin-gif`: metadata extraction
  - `libcamlimages-ocaml`: successful image load and dimensions
  - `libgdal34t64`: runtime GIF driver behavior
- Before yielding, add a permanent local regression for each runtime bug found. Prefer a small C or shell reproducer under `safe/tests/compat/` over relying solely on Docker reproduction.
- Do not vendor any downstream source package contents into the repository.
- Treat tracked files under `original/` as immutable oracle inputs. If a local oracle rebuild is needed, build from a temporary copy instead of the tracked tree.
- Treat untracked root-level `.deb` files and generated `safe/tests/` binaries as disposable build outputs rather than trusted inputs; rebuild or overwrite them as part of verification.
- If a runtime fix touches decoder or slurp behavior, keep `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `malformed-regress`, and `malformed-baseline-regress` green.
- If a runtime fix touches encoder/write/quantize/drawing files such as `safe/src/encode.rs`, `safe/src/gcb.rs`, `safe/src/helpers.rs`, `safe/src/quantize.rs`, or `safe/src/draw.rs`, keep `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, `gif2rgb-regress`, `gifecho-regress`, `drawing-regress`, and `gifwedge-regress` green.
- If a runtime fix touches hot-path code, keep `safe/tests/perf_compare.sh` passing.
- If a runtime fix touches FFI entry points or raw-pointer-heavy code, rerun the `SAFETY:` audit and keep panic fencing at the C ABI boundary.

## Verification Phases
### `check_03_runtime_matrix`
- Phase ID: `check_03_runtime_matrix`
- Type: `check`
- Bounce Target: `impl_03_runtime_dependents`
- Purpose: software-tester execution of the runtime-dependent Docker subset plus the relevant local regression matrix.
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
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress link-compat-regress internal-export-regress malformed-regress malformed-baseline-regress compat-regress
original_build_dir="$(mktemp -d)"
trap 'rm -rf "$original_build_dir"' EXIT
cp -a original/. "$original_build_dir"
make -C "$original_build_dir" libgif.so libgif.a
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" "$original_build_dir/tests/public_api_regress.c" "$original_build_dir/libgif.a" -o /tmp/public_api_regress.original
cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf-runtime.log
grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf-runtime.log
grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf-runtime.log
grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf-runtime.log
grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf-runtime.log
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
bash -o pipefail -c './test-original.sh --scope runtime | tee /tmp/test-runtime.log'
python3 - <<'PY'
from pathlib import Path
import sys

log = Path('/tmp/test-runtime.log').read_text(encoding='utf-8')
required = [
    '==> Building safe Debian packages',
    '==> Installing safe Debian packages',
    '==> Verifying runtime linkage to active packaged giflib',
    '==> giflib-tools',
    '==> webp',
    '==> fbi',
    '==> mtpaint',
    '==> tracker-extract',
    '==> libextractor-plugin-gif',
    '==> libcamlimages-ocaml',
    '==> libgdal34t64',
    'All downstream checks passed',
]
forbidden = [
    '==> gdal (source)',
    '==> exactimage (source)',
    '==> sail (source)',
    '==> libwebp (source)',
    '==> imlib2 (source)',
]

missing = [marker for marker in required if marker not in log]
unexpected = [marker for marker in forbidden if marker in log]
count_errors = [
    marker for marker in required[:3]
    if log.count(marker) != 1
]

if missing or unexpected or count_errors:
    if missing:
        print('missing runtime-scope markers:', *missing, sep='\n', file=sys.stderr)
    if unexpected:
        print('unexpected source-scope markers during runtime scope:', *unexpected, sep='\n', file=sys.stderr)
    if count_errors:
        print('shared setup markers must appear exactly once during runtime scope:', *count_errors, sep='\n', file=sys.stderr)
    sys.exit(1)
PY
```

### `check_03_runtime_review`
- Phase ID: `check_03_runtime_review`
- Type: `check`
- Bounce Target: `impl_03_runtime_dependents`
- Purpose: senior-tester review of runtime-dependent fixes and their regression coverage.
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
rg -n 'runtime|giftext|gif2webp|fbi|mtpaint|tracker|extractor|camlimages|gdal' safe/tests/compat safe/tests/Makefile test-original.sh
rg -n 'SAFETY:' safe/src
rg -n 'catch_panic_or|catch_error_or|catch_gif_error_or|catch_gif_and_error_or' safe/src
```
- Review Checks:
  - Every runtime-only bug found in Docker must have a permanent local reproducer under `safe/tests/compat/` or an explicit extension in `safe/tests/Makefile`.
  - Do not vendor any downstream source package contents into the repository.
  - Keep fixes minimal to the failing runtime behavior; avoid unrelated package or source-build churn unless required by the bug.

## Success Criteria
- Every runtime compatibility failure found in Docker leaves behind a permanent local reproducer under `safe/tests/compat/` or an equivalent deterministic `safe/tests/Makefile` extension.
- `./test-original.sh --scope runtime` runs the shared setup once, includes all runtime markers, and excludes all source-build markers.
- The relevant local regression matrix, malformed-input coverage, performance gate, panic fencing, and `SAFETY:` expectations remain green after the runtime fixes.
- `check_03_runtime_matrix` and `check_03_runtime_review` both pass.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding. The phase must end as exactly one non-merge commit whose subject starts with `impl_03_runtime_dependents:`, followed by a clean tracked worktree and index.
