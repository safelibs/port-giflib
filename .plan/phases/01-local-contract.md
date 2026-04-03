# Phase 1

## Phase Name
Local Contract Lock And Regression Gap Closure

## Implement Phase ID
`impl_01_local_contract`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/`
- `safe/include/gif_lib.h`
- `safe/tests/Makefile`
- `safe/tests/abi_layout.c`
- `safe/tests/internal_exports_smoke.c`
- `safe/tests/malformed_observe.c`
- `safe/tests/capture_malformed_baseline.sh`
- `safe/tests/malformed/`
- `safe/tests/malformed/manifest.txt`
- `safe/tests/malformed/original-baseline.txt`
- `safe/tests/perf_compare.sh`
- `relevant_cves.json`
- `original/gif_lib.h`
- `original/gif_hash.h`
- `original/debian/libgif7.symbols`
- `original/tests/public_api_regress.c`
- `original/tests/`
- `original/pic/`

## New Outputs
- locked and explicitly verified local ABI/export/regression baseline
- `safe/tests/compat/` scaffolding for future downstream bug reproducers
- deterministic `compat-regress` target in `safe/tests/Makefile`
- any local-only regression additions needed before downstream testing starts

## File Changes
- update `safe/tests/Makefile`
- create `safe/tests/compat/README.md`
- create `safe/tests/compat/`
- update `safe/tests/abi_layout.c` only if the probe itself is incomplete
- update `safe/tests/internal_exports_smoke.c` only if local export coverage is incomplete
- update `safe/tests/perf_compare.sh` only if its current contract is incomplete
- update `safe/src/*.rs` only if a local verification rerun exposes a real bug

## Implementation Details
- Preserve the current Rust-only build. This phase must not add bootstrap C compilation back into `safe/build.rs`.
- Preserve the current Rust code and test conventions: keep the original subsystem split across `safe/src/`, keep C-style FFI names and mostly C-style parameter casing at the ABI boundary, keep `#![deny(unsafe_op_in_unsafe_fn)]` enabled, and keep remaining `unsafe` explicit with nearby `SAFETY:` comments.
- Preserve the byte-for-byte public header match between `safe/include/gif_lib.h` and `original/gif_lib.h`.
- Add `compat-regress` to `safe/tests/Makefile` as the single aggregator for future issue-specific tests.
- Keep `safe/tests/Makefile` aligned with the upstream test structure, which means `gif2rgb-regress` remains an explicit target whenever full local coverage is claimed.
- Add `safe/tests/compat/README.md` describing:
  - naming convention for reproducers
  - requirement to register them in `safe/tests/Makefile`
  - requirement to keep them local, deterministic, and minimal
  - when a tiny local fixture or expected-output file is acceptable and how to document its provenance
- Do not refactor `safe/src/` for style in this phase. Only change code if the current local verification rerun proves an actual defect.
- Preserve existing panic fencing patterns in `safe/src/bootstrap.rs` and the exported `extern "C"` entry points.
- Keep consuming `original/tests/public_api_regress.c` and all oracle files in place; do not duplicate them under `safe/tests/`.
- Treat tracked files under `original/` as immutable oracle inputs. If a local oracle rebuild is needed, build from a temporary copy instead of the tracked tree.
- Treat untracked root-level `.deb` files and generated `safe/tests/` binaries such as `safe/tests/internal_exports_smoke`, `safe/tests/malformed_observe`, and `safe/tests/public_api_regress*` as disposable build outputs rather than trusted inputs; verifiers must rebuild or overwrite them.
- Preserve the plan/workflow regeneration handoff contract from `.plan/plan.md`: `.plan/phases/*.md`, `.plan/workflow-structure.yaml`, and `workflow.yaml` are regenerated in place only, and any later plan review or workflow generation must begin from a planning-only commit that touches only `.plan/*` and `workflow.yaml`, with the tracked worktree clean and `.plan/plan.md` unchanged relative to `HEAD`.
- Do not modify `dependents.json` or `relevant_cves.json` in this phase.
- If local iteration edits decoder/slurp/data-path files such as `safe/src/decode.rs`, `safe/src/slurp.rs`, `safe/src/io.rs`, `safe/src/state.rs`, or `safe/src/helpers.rs`, rerun `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `malformed-regress`, and `malformed-baseline-regress` before yielding.
- If local iteration edits encoder/write/quantize/drawing files such as `safe/src/encode.rs`, `safe/src/gcb.rs`, `safe/src/helpers.rs`, `safe/src/quantize.rs`, or `safe/src/draw.rs`, rerun `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, `gif2rgb-regress`, `gifecho-regress`, `drawing-regress`, and `gifwedge-regress` before yielding.
- If local iteration edits hot-path files `safe/src/decode.rs`, `safe/src/encode.rs`, `safe/src/quantize.rs`, `safe/src/helpers.rs`, `safe/src/state.rs`, or `safe/src/io.rs`, rerun `safe/tests/perf_compare.sh` before yielding.
- If local iteration edits FFI entry points or raw-pointer-heavy code, rerun the `SAFETY:` audit and keep panic fencing at the C ABI boundary before yielding.
- Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_01_local_contract:`.
- After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.

## Verification Phases
### `check_01_local_matrix`
- Phase ID: `check_01_local_matrix`
- Type: `check`
- Bounce Target: `impl_01_local_contract`
- Purpose: software-tester verification of the current Rust-only library contract before expensive downstream work begins.
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
header_only_dir="$(mktemp -d)"
make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress link-compat-regress internal-export-regress malformed-regress malformed-baseline-regress compat-regress
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
if find safe/tests \
  \( -path 'safe/tests/malformed/*' -o -path 'safe/tests/compat/*' \) -prune -o \
  \( -type f -o -type l \) \
  \( -name 'public_api_regress.c' -o -name '*.summary' -o -name '*.ico' -o -name '*.dmp' -o -name '*.map' -o -name '*.rgb' -o -name '*.gif' \) \
  -print | grep -q .; then
  echo 'unexpected vendored original harness or oracle files under safe/tests outside malformed/ and compat/' >&2
  exit 1
fi
```

### `check_01_local_review`
- Phase ID: `check_01_local_review`
- Type: `check`
- Bounce Target: `impl_01_local_contract`
- Purpose: senior-tester review of phase-1 changes, with emphasis on preserving the already passing local contract and avoiding unnecessary churn.
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
rg -n 'catch_panic_or|catch_error_or|catch_gif_error_or|catch_gif_and_error_or' safe/src
rg -n 'SAFETY:' safe/src
rg -n '^compat-regress:' safe/tests/Makefile
find safe/tests/compat -maxdepth 2 -type f | LC_ALL=C sort
```
- Review Checks:
  - Confirm the phase did not edit `original/`, `dependents.json`, or `relevant_cves.json`.
  - Confirm any new local reproducer consumes existing fixtures or generates its own temporary inputs instead of vendoring new upstream corpora.
  - Confirm any file committed under `safe/tests/compat/` is locally authored, minimal, and documented policy-wise rather than copied from `original/tests/`, `original/pic/`, or downstream source packages.
  - Confirm `compat-regress` is deterministic and no-op-safe when there are no issue-specific tests yet.

## Success Criteria
- `safe/tests/compat/README.md` exists and `safe/tests/Makefile` exposes a deterministic `compat-regress` aggregator for future issue-specific tests.
- The production library build stays Rust-only, the public header remains byte-for-byte identical to `original/gif_lib.h`, and the local ABI/export/regression/performance gates remain green.
- No upstream oracle corpus is vendored under `safe/tests/` outside the allowed malformed and compat areas, and tracked `original/`, `dependents.json`, and `relevant_cves.json` remain untouched.
- `check_01_local_matrix` and `check_01_local_review` both pass.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding. The phase must end as exactly one non-merge commit whose subject starts with `impl_01_local_contract:`, followed by a clean tracked worktree and index.
