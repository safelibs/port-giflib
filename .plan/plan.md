# GIFLIB Rust Port Plan

## 1. Context

The goal is no longer to bootstrap a Rust port from nothing. This repository already contains a substantial Rust implementation in `safe/`, plus local and downstream verification infrastructure. The remaining work is to turn that existing port into a thoroughly proven, drop-in `giflib` replacement for Ubuntu 24.04, with a workflow that finds compatibility gaps through real consumers, adds permanent regressions for them, and fixes them in small linear steps.

Relevant current codebase facts:

- The authoritative C/oracle tree is `original/`.
- The Rust replacement crate is `safe/`.
- The installed public C header is `safe/include/gif_lib.h`, which currently matches `original/gif_lib.h` byte-for-byte.
- The Rust crate is already configured as `crate-type = ["cdylib", "staticlib"]` in `safe/Cargo.toml`.
- `safe/build.rs` currently sets the Linux SONAME to `libgif.so.7` and does not compile any `original/*.c` files. The production library build is already Rust-only.
- The Rust implementation is split by original subsystem:
  - `safe/src/ffi.rs`: `#[repr(C)]` ABI mirrors, including padded `GifBool` and compile-time size assertions.
  - `safe/src/bootstrap.rs`: panic fencing and C-visible error propagation helpers.
  - `safe/src/state.rs`: opaque `EncoderState` and `DecoderState` stored behind `GifFileType.Private`.
  - `safe/src/decode.rs` and `safe/src/slurp.rs`: sequential decoder and `DGifSlurp`.
  - `safe/src/encode.rs`: sequential encoder and `EGifSpew`.
  - `safe/src/helpers.rs`, `safe/src/gcb.rs`, `safe/src/hash.rs`, `safe/src/draw.rs`, `safe/src/quantize.rs`, `safe/src/error.rs`, `safe/src/io.rs`, `safe/src/memory.rs`: helper/data-path exports.
- The local regression harness already exists in `safe/tests/Makefile` and intentionally consumes:
  - `original/tests/public_api_regress.c`
  - oracle files under `original/tests/`
  - GIF fixtures under `original/pic/`
  in place rather than vendoring them into `safe/tests/`.
- Local extra verification already exists:
  - `safe/tests/abi_layout.c`
  - `safe/tests/internal_exports_smoke.c`
  - `safe/tests/malformed_observe.c`
  - `safe/tests/capture_malformed_baseline.sh`
  - `safe/tests/malformed/manifest.txt`
  - `safe/tests/malformed/original-baseline.txt`
  - `safe/tests/perf_compare.sh`
- The repository does not yet contain `safe/tests/compat/` or a `compat-regress` target in `safe/tests/Makefile`; phase 1 must add that scaffolding before downstream bug-fix phases start landing issue-specific reproducers.
- Debian packaging already exists under `safe/debian/` and currently builds `libgif7` and `libgif-dev` with version `5.2.2-1ubuntu1+safelibs1`.
- The downstream Docker harness already exists in `test-original.sh` and already:
  - copies `safe/` and `original/` into the container
  - builds local safe `.deb` packages
  - installs them
  - validates the fixed dependent inventory in `dependents.json`
  - runs runtime checks for `giflib-tools`, `webp`, `fbi`, `mtpaint`, `tracker-extract`, `libextractor-plugin-gif`, `libcamlimages-ocaml`, and `libgdal34t64`
  - runs source-build checks for `gdal`, `exactimage`, `sail`, `libwebp`, and `imlib2`

Observed state during exploration on April 3, 2026:

- `cargo build --manifest-path safe/Cargo.toml --release` succeeded.
- `make -C safe/tests ... test gif2rgb-regress link-compat-regress internal-export-regress` succeeded against the Rust library.
- `cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c ...` succeeded.
- `objdump -T safe/target/release/libgif.so` matched the exported symbol list from `original/debian/libgif7.symbols`, including `GifAsciiTable8x8` as `DO Base`.
- `dpkg-buildpackage -us -uc -b` succeeded in `safe/`.
- `test-original.sh` still runs the full runtime-plus-source matrix in one pass and does not yet expose `--scope runtime|source|all`; phase 2 must add that interface in place instead of replacing the harness.

Those observations matter for planning:

- The workflow should not spend phases recreating `safe/`, re-porting already present exports, or reintroducing a bootstrap C backend.
- The workflow should preserve the existing Rust-only library build and current passing local checks.
- The real remaining risk is not “can we compile a Rust libgif?” but “can we prove the current Rust libgif is source-compatible, link-compatible, runtime-compatible, package-compatible, and safe enough when exercised by real consumers?”

Key compatibility oracles and constraints:

- Public C API oracle: `original/gif_lib.h`
- ELF export oracle: `original/debian/libgif7.symbols`
- Existing public regression harness: `original/tests/public_api_regress.c`
- Existing fixture/oracle corpus: `original/tests/` and `original/pic/`
- Existing malformed-input scope: `relevant_cves.json` and `safe/tests/malformed/`
- Existing downstream inventory: `dependents.json`
- `dependents.json` is a metadata object whose fixed `.dependents[]` array currently contains 13 downstream entries, which is sufficient to satisfy the “identify a dozen applications” requirement without recollecting a new inventory.
- Existing downstream package-replacement harness: `test-original.sh`

Code and test conventions already present and worth preserving:

- Rust modules follow the original C subsystem split.
- FFI-facing functions keep C-style names and mostly C-style parameter casing.
- `#![deny(unsafe_op_in_unsafe_fn)]` is enabled; remaining `unsafe` is explicit and typically preceded by `SAFETY:` comments.
- The public header must remain byte-for-byte compatible with the original header.
- `safe/tests/Makefile` mirrors the upstream test structure, which means `gif2rgb-regress` is still not part of the default `test` target and must always be invoked explicitly when full local coverage is claimed.

Planning artifacts already exist and must be updated in place later:

- `.plan/phases/*.md`
- `.plan/workflow-structure.yaml`
- top-level `workflow.yaml`

Those existing planning artifacts are historical inputs, not the contract. This `plan.md` becomes the authoritative source for regenerated phase docs and workflow structure.

## 2. Generated Workflow Contract

The generated workflow derived from this plan must obey all of the following:

- Linear execution only. Do not use `parallel_groups`.
- Use self-contained inline YAML only.
- Do not use top-level `include`.
- Do not use phase-level `prompt_file`, `workflow_file`, `workflow_dir`, `checks`, or any other YAML-source indirection.
- Do not use agent-guided `bounce_targets` lists. Every verifier must use exactly one fixed `bounce_target`.
- Every verifier must be an explicit top-level `check` phase.
- Every verifier must stay in the implement block it verifies and bounce only to that implement phase.
- Every implement prompt in the generated workflow must instruct the agent to commit work to git before yielding.
- Each implementation phase must yield exactly one non-merge git commit. Do not yield multiple commits for one phase; if iterative local work happened, collapse it into one final phase commit before yielding.
- Each implementation phase commit subject must start with its implement phase ID, so succeeding verifiers can reason about the exact phase diff for that phase.
- Any review command that inspects a phase diff must inspect the full phase commit as `HEAD^..HEAD` or an equivalent explicit single-commit range, not an unspecified history summary.
- Immediately before any implement phase yields, after the phase commit is created, the tracked worktree and index must be clean: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
- Use the tracked-clean contract above, not a fully pristine tree requirement: disposable untracked build outputs may exist, but no tracked file may remain modified outside the yielded commit.
- Immediately before any verifier, plan review, phase-doc regeneration, or workflow-regeneration step begins, that same tracked-clean condition must already hold. Verifiers must fail fast on tracked dirt before running broader checks so they review only the committed artifact under test.
- This plan refinement itself must be delivered as a planning-only git commit before any follow-on plan review, phase-doc regeneration, or workflow regeneration, and the tracked worktree must be clean afterward.
- Any plan review or workflow-generation handoff is invalid if `.plan/plan.md` is modified relative to `HEAD`; succeeding checkers must review the committed plan artifact, not a dirty working copy.
- Before regenerating `.plan/phases/*.md`, `.plan/workflow-structure.yaml`, or `workflow.yaml`, land the planning artifacts in a planning-only commit and satisfy the tracked-clean handoff contract. That planning commit may update only `.plan/*` and `workflow.yaml`; it must not modify `safe/`, `original/`, `dependents.json`, `relevant_cves.json`, or other tracked implementation files.
- Every major implementation phase must have:
  - at least one command-heavy tester check
  - at least one senior-review check
- If a verifier needs to run build, package, Docker, benchmark, lint, or test commands, those commands must be written directly into the verifier’s instructions. Do not model them as separate non-agentic phases.
- Update existing planning outputs in place later:
  - regenerate `.plan/phases/*.md` in place
  - regenerate `.plan/workflow-structure.yaml` in place
  - regenerate `workflow.yaml` in place
  - do not create alternate sibling planning or workflow files
- Consume existing artifacts in place instead of rediscovering or regenerating them:
  - `original/` is the authoritative code and behavior oracle
  - `original/tests/` and `original/pic/` are the authoritative fixture/oracle corpus
  - `safe/` is the existing Rust implementation to refine, not something to recreate
  - `safe/tests/` is the existing local regression layer to extend
  - `safe/debian/` is the existing package surface to refine
  - `test-original.sh` is the existing downstream harness to update in place
  - `dependents.json` is the fixed downstream inventory to consume in place
  - `relevant_cves.json` plus `safe/tests/malformed/*` are the existing malformed-input scope and artifacts
- Do not modify `dependents.json` unless the inventory itself is proven wrong. The default assumption is that it is authoritative.
- Do not recollect a new downstream app list. The existing 13-entry `dependents.json` inventory already satisfies the requirement to test roughly a dozen real consumers.
- Do not modify `relevant_cves.json` unless the scoped CVE analysis itself is proven wrong. The default assumption is that it is authoritative.
- Treat tracked files under `original/` as immutable oracle inputs. Read them in place; do not edit them and do not run destructive clean/build flows in the tracked `original/` tree.
- If the planning worktree or the commit being handed to workflow generation contains tracked edits under `original/`, stop and repair that state before proceeding. Planning is not allowed to carry oracle mutations forward.
- If a verifier needs a rebuilt original-library oracle, it must build from a temporary copy of `original/`, not from the tracked tree.
- Existing untracked root-level `.deb` files and generated `safe/tests/` binaries such as `safe/tests/malformed_observe`, `safe/tests/internal_exports_smoke`, and `safe/tests/public_api_regress*` are disposable build outputs, not authoritative inputs. Verifiers must rebuild or overwrite them rather than trusting them.
- Preserve the current Rust-only production build. No generated implementation phase may reintroduce compilation of `original/*.c` into `safe/build.rs`, `safe/Cargo.toml`, or `safe/src/`.
- Later generated workflow phases must assume the current crate already exports the full symbol surface and already passes the current local matrix. The workflow should focus on verification-driven fixes, not from-zero port bootstrap.
- `test-original.sh` must be updated in place rather than replaced by a second downstream harness.
- The final workflow must add scoped downstream execution to `test-original.sh`, with `--scope runtime|source|all` and default `all`, so runtime and source-build dependent classes can be tested in separate linear phases before the final full run.
- `test-original.sh` argument parsing must complete before any `docker build` or `docker run` side effects. `--help`, missing `--scope` values, invalid `--scope` values, and unexpected positional arguments must all exit before container work begins.
- The downstream scope contract is fixed:
  - `runtime`: run the shared setup exactly once (`validate_dependents_inventory`, package build/install, sample discovery, and packaged-linkage assertions), then run only the runtime-dependent app checks.
  - `source`: run that same shared setup exactly once, then run only the source-build dependent checks.
  - `all`: run the shared setup exactly once, then run the full runtime subset followed by the full source subset in that order.
- Later generated verifier phases must assert both positive and negative scope behavior:
  - `--scope runtime` must include every runtime marker and exclude every source marker.
  - `--scope source` must include every source marker and exclude every runtime-app marker.
  - `--scope all` must include both marker sets, keep runtime markers before source markers, and keep the shared setup single-shot rather than repeated per subset.
- Any downstream failure discovered in a runtime or source-build phase must be paired with a committed regression under `safe/tests/compat/` or a committed extension to `safe/tests/Makefile` before the fix phase yields.
- `safe/tests/compat/` must be deterministic and repo-local:
  - no vendored copies of downstream source trees
  - use minimal reproducers
  - derive any fixtures from existing in-repo inputs where possible
  - allow tiny locally-authored fixtures or expected-output files only when existing repo inputs cannot express the bug; do not copy oracle corpora out of `original/` or downstream source trees
- `safe/tests/Makefile` must grow a deterministic `compat-regress` aggregator target. Do not rely on “scan arbitrary files and execute them” behavior.
- Any verifier that rebuilds Debian packages must assert that the build emits only `libgif7_*.deb`, `libgif-dev_*.deb`, `libgif7-dbgsym_*.ddeb`, `giflib_*.changes`, and `giflib_*.buildinfo`; no additional binary packages are allowed.
- Any phase that claims full local regression coverage must explicitly run `gif2rgb-regress`; `make test` alone is insufficient.
- Any phase that edits decoder/slurp/data-path files such as `safe/src/decode.rs`, `safe/src/slurp.rs`, `safe/src/io.rs`, `safe/src/state.rs`, or `safe/src/helpers.rs` must rerun:
  - `render-regress`
  - `gifclrmp-regress`
  - `giffilter-regress`
  - `giftext-regress`
  - `malformed-regress`
  - `malformed-baseline-regress`
- Any phase that edits encoder/write/quantize/drawing files such as `safe/src/encode.rs`, `safe/src/gcb.rs`, `safe/src/helpers.rs`, `safe/src/quantize.rs`, or `safe/src/draw.rs` must rerun:
  - `gifbuild-regress`
  - `gifsponge-regress`
  - `giftool-regress`
  - `giffix-regress`
  - `gif2rgb-regress`
  - `gifecho-regress`
  - `drawing-regress`
  - `gifwedge-regress`
- Any phase that edits `safe/debian/*`, `safe/build.rs`, `safe/Cargo.toml`, or public install/layout behavior must rerun extracted-package checks and compile `original/tests/public_api_regress.c` against the extracted `libgif-dev` package contents.
- Any phase that edits hot-path files `safe/src/decode.rs`, `safe/src/encode.rs`, `safe/src/quantize.rs`, `safe/src/helpers.rs`, `safe/src/state.rs`, or `safe/src/io.rs` must rerun `safe/tests/perf_compare.sh`.
- Any phase that edits FFI entry points or raw-pointer-heavy code must rerun the `SAFETY:` audit and must keep panic fencing at the C ABI boundary.
- Because `impl_01_local_contract`, `impl_03_runtime_dependents`, `impl_04_source_dependents`, and `impl_05_regression_catchall` all leave `safe/src/` fixes in scope, each of their generated matrix verifiers must already include the full local regression matrix and the original-vs-safe `safe/tests/perf_compare.sh` comparison rather than relying on conditional follow-up phases.
- Because `impl_02_package_and_harness`, `impl_04_source_dependents`, and `impl_05_regression_catchall` all leave package/install-surface edits in scope, each of their generated matrix verifiers must already include the extracted-package assertions plus `original/tests/public_api_regress.c` compiled against the extracted `libgif-dev` contents.
- The final generated workflow must emit phases in exactly this order:
  - `impl_01_local_contract`
  - `check_01_local_matrix`
  - `check_01_local_review`
  - `impl_02_package_and_harness`
  - `check_02_package_surface`
  - `check_02_harness_review`
  - `impl_03_runtime_dependents`
  - `check_03_runtime_matrix`
  - `check_03_runtime_review`
  - `impl_04_source_dependents`
  - `check_04_source_matrix`
  - `check_04_source_review`
  - `impl_05_regression_catchall`
  - `check_05_regression_matrix`
  - `check_05_senior_review`
  - `check_05_final_full`

## 3. Implementation Phases

### Phase 1

- `Phase Name`: Local Contract Lock And Regression Gap Closure
- `Implement Phase ID`: `impl_01_local_contract`
- `Verification Phases`:
  - `check_01_local_matrix`
    - type: `check`
    - fixed `bounce_target`: `impl_01_local_contract`
    - purpose: software-tester verification of the current Rust-only library contract before expensive downstream work begins.
    - commands:
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
  - `check_01_local_review`
    - type: `check`
    - fixed `bounce_target`: `impl_01_local_contract`
    - purpose: senior-tester review of phase-1 changes, with emphasis on preserving the already passing local contract and avoiding unnecessary churn.
    - commands or review checks:
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
      Review checks:
      - Confirm the phase did not edit `original/`, `dependents.json`, or `relevant_cves.json`.
      - Confirm any new local reproducer consumes existing fixtures or generates its own temporary inputs instead of vendoring new upstream corpora.
      - Confirm any file committed under `safe/tests/compat/` is locally authored, minimal, and documented policy-wise rather than copied from `original/tests/`, `original/pic/`, or downstream source packages.
      - Confirm `compat-regress` is deterministic and no-op-safe when there are no issue-specific tests yet.
- `Preexisting Inputs`:
  - `safe/Cargo.toml`
  - `safe/build.rs`
  - `safe/src/*.rs`
  - `safe/include/gif_lib.h`
  - `safe/tests/Makefile`
  - `safe/tests/abi_layout.c`
  - `safe/tests/internal_exports_smoke.c`
  - `safe/tests/malformed_observe.c`
  - `safe/tests/capture_malformed_baseline.sh`
  - `safe/tests/malformed/manifest.txt`
  - `safe/tests/malformed/original-baseline.txt`
  - `safe/tests/perf_compare.sh`
  - `original/gif_lib.h`
  - `original/gif_hash.h`
  - `original/debian/libgif7.symbols`
  - `original/tests/public_api_regress.c`
  - `original/tests/`
  - `original/pic/`
- `New Outputs`:
  - locked and explicitly verified local ABI/export/regression baseline
  - `safe/tests/compat/` scaffolding for future downstream bug reproducers
  - deterministic `compat-regress` target in `safe/tests/Makefile`
  - any local-only regression additions needed before downstream testing starts
- `File Changes`:
  - update `safe/tests/Makefile`
  - create `safe/tests/compat/README.md`
  - create `safe/tests/compat/`
  - update `safe/tests/abi_layout.c` only if the probe itself is incomplete
  - update `safe/tests/internal_exports_smoke.c` only if local export coverage is incomplete
  - update `safe/tests/perf_compare.sh` only if its current contract is incomplete
  - update `safe/src/*.rs` only if a local verification rerun exposes a real bug
- `Implementation Details`:
  - Preserve the current Rust-only build. This phase must not add bootstrap C compilation back into `safe/build.rs`.
  - Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_01_local_contract:`.
  - After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
  - Preserve the byte-for-byte public header match between `safe/include/gif_lib.h` and `original/gif_lib.h`.
  - Add `compat-regress` to `safe/tests/Makefile` as the single aggregator for future issue-specific tests.
  - Add `safe/tests/compat/README.md` describing:
    - naming convention for reproducers
    - requirement to register them in `safe/tests/Makefile`
    - requirement to keep them local, deterministic, and minimal
    - when a tiny local fixture or expected-output file is acceptable and how to document its provenance
  - Do not refactor `safe/src/` for style in this phase. Only change code if the current local verification rerun proves an actual defect.
  - Preserve existing panic fencing patterns in `safe/src/bootstrap.rs` and the exported `extern "C"` entry points.
  - Keep consuming `original/tests/public_api_regress.c` and all oracle files in place; do not duplicate them under `safe/tests/`.
- `Verification`:
  - Use the full `check_01_local_matrix` command block.
  - Treat any symbol drift, ABI-layout drift, missing `gif2rgb-regress`, missing `compat-regress`, performance ratio regression, vendored original oracles, missing `SAFETY:` comments, or reintroduced C build inputs as blockers.

### Phase 2

- `Phase Name`: Package Surface Lock And Downstream Harness Scoping
- `Implement Phase ID`: `impl_02_package_and_harness`
- `Verification Phases`:
  - `check_02_package_surface`
    - type: `check`
    - fixed `bounce_target`: `impl_02_package_and_harness`
    - purpose: software-tester verification of Debian package contents, installed development surface, and package metadata.
    - commands:
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
  - `check_02_harness_review`
    - type: `check`
    - fixed `bounce_target`: `impl_02_package_and_harness`
    - purpose: senior-tester review of `test-original.sh` changes needed for scoped downstream execution.
    - commands or review checks:
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
      Review checks:
      - Confirm the script still builds and installs the exact local safe `.deb`s before any downstream checks.
      - Confirm `--help`, missing-argument, invalid-scope, and unexpected-argument paths all return before any `docker build` or `docker run`.
      - Confirm runtime and source subsets are gated separately, the default remains `all`, and `--scope all` still runs runtime markers before source markers after one shared setup pass.
      - Confirm the script still consumes `dependents.json`, `safe/`, and `original/` in place instead of duplicating them.
- `Preexisting Inputs`:
  - `safe/debian/control`
  - `safe/debian/rules`
  - `safe/debian/changelog`
  - `safe/debian/libgif7.symbols`
  - `safe/debian/libgif7.install`
  - `safe/debian/libgif-dev.install`
  - `safe/debian/pkgconfig/libgif7.pc.in`
  - `safe/debian/source/format`
  - `safe/build.rs`
  - `test-original.sh`
  - `dependents.json`
  - `original/debian/libgif7.symbols`
- `New Outputs`:
  - package surface locked to the original contract and locally versioned safe package suffix
  - scoped downstream harness interface via `test-original.sh --scope runtime|source|all`
  - retained package-derived linkage assertion helpers in the downstream harness
- `File Changes`:
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
- `Implementation Details`:
  - Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_02_package_and_harness:`.
  - After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
  - Keep the package names `libgif7` and `libgif-dev`.
  - Keep the local package version suffix `+safelibs...`.
  - Preserve the current library-only packaging surface; do not introduce a `giflib-tools` package from `safe/`.
  - Preserve the existing relative `libgif.pc -> libgif7.pc` behavior or an equivalent regular-file implementation; do not use an absolute symlink.
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
- `Verification`:
  - `check_02_package_surface` must pass before downstream scope phases begin.
  - `check_02_harness_review` must confirm side-effect-free parse-only paths, the concrete runtime/source/all scope split, and the continued absence of `/usr/local` assumptions.

### Phase 3

- `Phase Name`: Runtime Dependent Matrix Fixes
- `Implement Phase ID`: `impl_03_runtime_dependents`
- `Verification Phases`:
  - `check_03_runtime_matrix`
    - type: `check`
    - fixed `bounce_target`: `impl_03_runtime_dependents`
    - purpose: software-tester execution of the runtime-dependent Docker subset plus the relevant local regression matrix.
    - commands:
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
  - `check_03_runtime_review`
    - type: `check`
    - fixed `bounce_target`: `impl_03_runtime_dependents`
    - purpose: senior-tester review of runtime-dependent fixes and their regression coverage.
    - commands or review checks:
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
      Review checks:
      - Every runtime-only bug found in Docker must have a permanent local reproducer under `safe/tests/compat/` or an explicit extension in `safe/tests/Makefile`.
      - Do not vendor any downstream source package contents into the repository.
      - Keep fixes minimal to the failing runtime behavior; avoid unrelated package or source-build churn unless required by the bug.
- `Preexisting Inputs`:
  - phase 2 package surface and scoped downstream harness
  - `dependents.json`
  - runtime checks already encoded in `test-original.sh`
  - current `safe/src/*.rs`
- `New Outputs`:
  - fixes for runtime-dependent compatibility failures
  - issue-specific runtime reproducers under `safe/tests/compat/`
  - corresponding `compat-regress` registrations
- `File Changes`:
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
- `Implementation Details`:
  - Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_03_runtime_dependents:`.
  - After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
  - Focus on runtime behavior exercised by the installed library, not source-build/package metadata.
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
  - If a runtime fix touches decoder or slurp behavior, keep the malformed baseline and decode-heavy local regressions green.
  - If a runtime fix touches hot-path code, keep `safe/tests/perf_compare.sh` passing.
- `Verification`:
  - `check_03_runtime_matrix` is the required runtime gate.
  - A runtime fix is incomplete if it only makes Docker pass but does not leave behind a local reproducible regression, if `--scope runtime` still executes any source markers, or if it regresses the original-vs-safe performance gate.

### Phase 4

- `Phase Name`: Source-Build Dependent Matrix Fixes
- `Implement Phase ID`: `impl_04_source_dependents`
- `Verification Phases`:
  - `check_04_source_matrix`
    - type: `check`
    - fixed `bounce_target`: `impl_04_source_dependents`
    - purpose: software-tester execution of the source-build dependent Docker subset plus package-surface and local compatibility checks most likely to catch header/export/pkg-config regressions.
    - commands:
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
  - `check_04_source_review`
    - type: `check`
    - fixed `bounce_target`: `impl_04_source_dependents`
    - purpose: senior-tester review of source-build/package-surface fixes and their regression coverage.
    - commands or review checks:
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
      Review checks:
      - Every source-build failure found in Docker must have a local reproducer or package-surface check that can fail without rebuilding a full downstream source tree.
      - Do not change `safe/include/gif_lib.h` unless the original header itself is being copied verbatim again; source-compat fixes should happen in Rust implementation or packaging, not by inventing a new header surface.
      - Do not vendor downstream source snapshots into the repository.
- `Preexisting Inputs`:
  - phase 2 package surface and scoped downstream harness
  - phase 3 runtime fixes
  - source-build checks already encoded in `test-original.sh`
  - `safe/debian/*`
  - `safe/build.rs`
  - `safe/Cargo.toml`
- `New Outputs`:
  - fixes for source-build and package-surface compatibility failures
  - issue-specific source-build reproducers under `safe/tests/compat/`
  - corresponding `compat-regress` registrations
- `File Changes`:
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
- `Implementation Details`:
  - Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_04_source_dependents:`.
  - After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
  - Focus on compile/link/install surface compatibility:
    - extracted package contents
    - `pkg-config` behavior
    - static and shared linkability
    - symbol/export completeness
    - header-only source compatibility
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
- `Verification`:
  - `check_04_source_matrix` is the required source-build gate.
  - Source-build fixes are incomplete if they make Docker pass but leave no local reproducer for the underlying problem class, if `--scope source` still executes any runtime-app markers, if they fail the extracted-package assertions, or if they regress the original-vs-safe performance gate.

### Phase 5

- `Phase Name`: Catch-All Compatibility Fixes, Review, And Final Full Matrix
- `Implement Phase ID`: `impl_05_regression_catchall`
- `Verification Phases`:
  - `check_05_regression_matrix`
    - type: `check`
    - fixed `bounce_target`: `impl_05_regression_catchall`
    - purpose: software-tester verification that all discovered issues now have local regressions and that the full local/package/performance matrix is stable before the last Docker pass.
    - commands:
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
  - `check_05_senior_review`
    - type: `check`
    - fixed `bounce_target`: `impl_05_regression_catchall`
    - purpose: senior-tester review of the final catch-all fix set, with emphasis on regression completeness, minimality, and safety boundaries.
    - commands or review checks:
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
      Review checks:
      - Every issue found in phases 3 and 4 must be traceable to a committed local reproducer.
      - `safe/tests/malformed/original-baseline.txt` must not change unless a new malformed fixture was intentionally added and its provenance was documented.
      - `dependents.json` must remain unchanged unless the inventory itself was proven wrong and that decision was explicitly justified.
  - `check_05_final_full`
    - type: `check`
    - fixed `bounce_target`: `impl_05_regression_catchall`
    - purpose: final software-tester gate across the complete local, package, performance, and downstream-replacement matrix.
    - commands:
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
- `Preexisting Inputs`:
  - all prior phases
  - `safe/tests/compat/`
  - `safe/tests/malformed/*`
  - `safe/tests/perf_compare.sh`
  - `safe/debian/*`
  - `test-original.sh`
- `New Outputs`:
  - final catch-all fixes only
  - complete local regression inventory for every discovered downstream issue
  - final verified full replacement matrix
- `File Changes`:
  - update `safe/src/*.rs` as required by remaining compatibility bugs
  - update `safe/tests/Makefile`
  - update `safe/tests/compat/*`
  - update `safe/tests/perf_compare.sh` only if a benchmark bug is found
  - update `safe/debian/*` only if package-surface fixes remain
  - update `test-original.sh` only if final scope orchestration or logging still needs cleanup
- `Implementation Details`:
  - Before yielding, create exactly one non-merge git commit for this phase, with a subject that starts with `impl_05_regression_catchall:`.
  - After that commit, leave the tracked worktree and index clean before yielding: `git status --short --untracked-files=no` must be empty, `git diff --quiet --exit-code` must succeed, and `git diff --cached --quiet --exit-code` must succeed.
  - This is the catch-all phase and the only bounce target for the final full-matrix verifier.
  - Do not open new fronts here. Only fix issues proven by earlier checks or by `check_05_final_full`.
  - Every remaining issue must leave behind a local regression in `safe/tests/compat/` or an existing deterministic target in `safe/tests/Makefile`.
  - Preserve the Rust-only production build and the byte-for-byte public header match.
  - Preserve current malformed baseline behavior unless intentionally adding new malformed fixtures with explicit provenance updates.
  - Keep panic fencing and `SAFETY:` comments intact while resolving final bugs.
- `Verification`:
  - `check_05_regression_matrix` must pass before the final full Docker run.
  - `check_05_final_full` is the terminal blocker for the entire workflow, and it must prove that `--scope all` runs the shared setup once, executes runtime markers before source markers, and covers both dependent classes completely.

## 4. Critical Files

- `safe/Cargo.toml`: crate type, release profile, and the fact that the production library build is already Rust-only.
- `safe/build.rs`: must stay free of `cc::Build` or any `original/*.c` production linkage; owns the SONAME behavior.
- `safe/include/gif_lib.h`: public header; must continue to match `original/gif_lib.h` byte-for-byte.
- `safe/src/bootstrap.rs`: central panic/error fencing; later fixes must preserve its role instead of bypassing it.
- `safe/src/ffi.rs`: public ABI mirrors and layout assertions; likely touched only if an ABI bug is uncovered.
- `safe/src/state.rs`: private `EncoderState`/`DecoderState` backing `GifFileType.Private`.
- `safe/src/io.rs`: file-handle and callback I/O behavior; common source of runtime compatibility issues.
- `safe/src/decode.rs`: sequential read path and many runtime-dependent behaviors.
- `safe/src/slurp.rs`: `DGifSlurp` behavior and malformed-input handling.
- `safe/src/encode.rs`: sequential write path and round-trip behavior.
- `safe/src/helpers.rs`: `GifMakeSavedImage`, extension handling, map/image helpers, and other ownership-sensitive behavior.
- `safe/src/gcb.rs`: GCB conversion helpers used by both the local harness and consumers.
- `safe/src/draw.rs`: font-table and drawing exports used by local API regressions.
- `safe/src/hash.rs`: exported private hash helpers required for link compatibility.
- `safe/src/quantize.rs`: `GifQuantizeBuffer` and performance-sensitive palette behavior.
- `safe/src/error.rs`: `GifErrorString` and visible error strings.
- `safe/tests/Makefile`: authoritative local regression driver for the Rust port; must also own `compat-regress`.
- `safe/tests/compat/README.md`: policy and registration point for issue-specific reproducers.
- `safe/tests/compat/*`: issue-specific local reproducers created in response to downstream findings.
- `safe/tests/abi_layout.c`: public ABI layout probe.
- `safe/tests/internal_exports_smoke.c`: non-installed-but-exported helper smoke test.
- `safe/tests/malformed_observe.c`: baseline capture helper for malformed fixtures.
- `safe/tests/capture_malformed_baseline.sh`: deterministic malformed baseline capture.
- `safe/tests/malformed/manifest.txt`: provenance for malformed fixtures; should change only when the malformed set changes.
- `safe/tests/malformed/original-baseline.txt`: committed original-library malformed baseline; treat as an existing oracle.
- `safe/tests/perf_compare.sh`: fixed performance gate against original `libgif.a`.
- `safe/debian/control`: package metadata and build dependencies.
- `safe/debian/rules`: Rust build/install logic for Debian packaging.
- `safe/debian/changelog`: package version suffix and release metadata.
- `safe/debian/libgif7.symbols`: shared-library export contract on the package side.
- `safe/debian/libgif7.install` and `safe/debian/libgif-dev.install`: package file layout.
- `safe/debian/pkgconfig/libgif7.pc.in`: pkg-config metadata for build-time consumers.
- `safe/debian/source/format`: must remain compatible with the local versioning scheme.
- `test-original.sh`: existing downstream Docker harness to update in place with scope selection and any stability fixes.
- `dependents.json`: fixed downstream inventory; consume in place and normally do not edit.
- `relevant_cves.json`: fixed malformed-input scope; consume in place and normally do not edit.
- `original/gif_lib.h`: authoritative public API/header oracle.
- `original/gif_hash.h`: authoritative ABI oracle for exported hash helpers used by tests.
- `original/debian/libgif7.symbols`: authoritative export list oracle.
- `original/tests/public_api_regress.c`: authoritative public API regression driver and original-performance harness input.
- `original/tests/` and `original/pic/`: authoritative local oracle corpus.
- `.plan/phases/*.md`: later generated phase documents to update in place from this plan.
- `.plan/workflow-structure.yaml`: later generated workflow structure to update in place from this plan.
- `workflow.yaml`: top-level generated inline workflow to update in place from this plan.

## 5. Final Verification

After all implementation phases complete, verify the finished port with this end-to-end sequence:

1. Confirm the tracked worktree is clean, the production library build is still Rust-only, and the public header still matches:
   ```bash
   if [ -n "$(git status --short --untracked-files=no)" ]; then
     git status --short --untracked-files=no >&2
     echo 'tracked worktree must be clean before final verification' >&2
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
   ```

2. Verify ABI layout and exported symbol parity:
   ```bash
   cc -I"$PWD/safe/include" -I"$PWD/original" safe/tests/abi_layout.c -o /tmp/giflib-abi-layout
   /tmp/giflib-abi-layout
   objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
   sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
   diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
   test "$(objdump -T safe/target/release/libgif.so | awk '/ GifAsciiTable8x8$/{print $3, $6, $7}')" = "DO Base GifAsciiTable8x8"
   ```

3. Run the full local regression matrix, including explicit quantization, malformed, link-compat, internal-export, and downstream-issue reproducers:
   ```bash
   make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" test gif2rgb-regress link-compat-regress internal-export-regress malformed-regress malformed-baseline-regress compat-regress
   ```

4. Rebuild Debian packages from `safe/` and verify their install surface:
   ```bash
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
   ```

5. Rebuild the original baseline from a temporary copy and run the fixed performance gate:
   ```bash
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

6. Run the complete downstream replacement matrix through the scoped Docker harness:
   ```bash
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
   ```

7. Run the final `unsafe` audit:
   ```bash
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

Any failure in steps 1 through 7 should bounce to `impl_05_regression_catchall`, because that phase is explicitly reserved as the final catch-all repair phase.
