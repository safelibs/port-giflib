# Phase 6

## Phase Name
Performance Baseline And Hot-Path Optimization

## Implement Phase ID
`impl_06_performance`

## Workflow Contract Notes
- Consume existing artifacts in place. Benchmark only against the committed authoritative fixtures and the original baseline library built from a temporary copy of `original/`.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. If this phase needs an original-library oracle, rebuild it from a temporary copy of `original/` and point compile or link steps at that copy.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/malformed_observe.c`
- `safe/tests/capture_malformed_baseline.sh`
- `safe/tests/malformed/`
- `safe/tests/malformed/manifest.txt`
- `safe/tests/malformed/original-baseline.txt`
- `original/Makefile`
- `original/tests/public_api_regress.c`
- `original/pic/welcome2.gif`
- `original/pic/treescap-interlaced.gif`
- `original/pic/fire.gif`
- `original/tests/gifgrid.rgb`

## New Outputs
- Repeatable performance comparison script with fixed workload IDs and a fixed `2.00` ratio gate
- Any release-profile and hot-path code improvements needed to keep the Rust port competitive

## File Changes
- Create `safe/tests/perf_compare.sh`
- Update `safe/Cargo.toml`
- Update hot-path modules such as `safe/src/decode.rs`, `safe/src/encode.rs`, `safe/src/quantize.rs`, `safe/src/helpers.rs`, and `safe/src/state.rs`

## Implementation Details
- Build the original performance baseline from a temporary copy of `original/`, not by mutating the tracked oracle tree.
- Preserve panic-boundary wrappers on every Rust-defined C ABI entry point touched during performance tuning so panics still become C-compatible failure values instead of unwinding across the ABI boundary.
- Create `safe/tests/perf_compare.sh` so it resolves the repository root from its own path instead of assuming the caller's `PWD`.
- Make `safe/tests/perf_compare.sh` accept two `public_api_regress` binaries, one linked to original `libgif.a` and one linked to safe `libgif.a`.
- Benchmark exactly these four workload IDs and commands against the authoritative fixtures in place:
- `render-welcome2`: `render "$repo_root/original/pic/welcome2.gif"`
- `render-treescap-interlaced`: `render "$repo_root/original/pic/treescap-interlaced.gif"`
- `highlevel-copy-fire`: `highlevel-copy "$repo_root/original/pic/fire.gif"`
- `rgb-to-gif-gifgrid`: `rgb-to-gif 3 100 100` with stdin from `"$repo_root/original/tests/gifgrid.rgb"`
- Perform exactly 2 warmup samples and 7 measured samples per workload for each binary.
- Make each sample execute exactly 25 inner-loop invocations of the workload before recording elapsed time.
- Measure median elapsed wall-clock time for the original-linked binary and the safe-linked binary separately.
- Print one machine-readable line per workload in the form `PERF workload=<id> original_median_s=<seconds> safe_median_s=<seconds> ratio=<safe/original> threshold=2.00`.
- Exit nonzero if any reported ratio exceeds `2.00`.
- Prefer optimizations that preserve safety goals, including reusable fixed-size/scratch structures for hot loops, buffered slice iteration, fewer clones in slurp/spew helpers, and lower pointer-chasing cost in quantization while preserving deterministic ordering.
- Tune `profile.release` only if measurements justify it. `panic = "abort"` is not compatible with the required FFI panic boundaries.

## Verification Phases

### `check_06_performance`
- Phase ID: `check_06_performance`
- Type: `check`
- Bounce Target: `impl_06_performance`
- Purpose: Verify that the Rust port stays within the fixed performance budget on the exact decode, slurp/spew, and quantization workloads named in the workflow contract, and that tuning does not regress behavior.
- Commands:
```bash
original_build_dir="$(mktemp -d)"
trap 'rm -rf "$original_build_dir"' EXIT
cp -a original/. "$original_build_dir"
make -C "$original_build_dir" libgif.so libgif.a
cargo build --manifest-path safe/Cargo.toml --release
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" "$original_build_dir/tests/public_api_regress.c" "$original_build_dir/libgif.a" -o /tmp/public_api_regress.original
cc -std=gnu99 -Wall -Wextra -I"$PWD/original" original/tests/public_api_regress.c "$PWD/safe/target/release/libgif.a" -o /tmp/public_api_regress.safe
safe/tests/perf_compare.sh /tmp/public_api_regress.original /tmp/public_api_regress.safe | tee /tmp/perf.log
grep -E '^PERF workload=render-welcome2 .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=render-treescap-interlaced .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=highlevel-copy-fire .* threshold=2\.00$' /tmp/perf.log
grep -E '^PERF workload=rgb-to-gif-gifgrid .* threshold=2\.00$' /tmp/perf.log
cmp -s safe/include/gif_lib.h original/gif_lib.h
header_only_dir="$(mktemp -d)"
make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" render-regress gifclrmp-regress giffilter-regress giftext-regress giftool-regress gif2rgb-regress
```

## Success Criteria
- `safe/tests/perf_compare.sh` benchmarks the exact four required workloads with the required warmup, measurement, inner-loop, output, and failure semantics.
- Every `PERF` line reports a ratio at or below `2.00`.
- Performance tuning preserves header parity and passes `safe-header-regress`, `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `giftool-regress`, and `gif2rgb-regress`.
- Performance changes do not remove the required panic-boundary behavior on exported Rust ABI entry points.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
