# Phase 5

## Phase Name
Security Hardening, Malformed Fixtures, And Compatibility Baseline

## Implement Phase ID
`impl_05_security_baseline`

## Workflow Contract Notes
- Consume existing artifacts in place. Derive malformed fixtures from existing in-repo GIF fixtures, keep provenance, and use `relevant_cves.json`, `original/tests/`, and `original/pic/` as authoritative inputs instead of inventing unrelated replacement assets.
- If an original-library oracle build is needed, stage it from a temporary copy of `original/`; do not run destructive build or cleanup flows in the tracked `original/` tree.
- Do not rely on `original/libgif.so`, `original/libgif.a`, or any other generated build output already present under the tracked `original/` tree. If this phase needs an original-library oracle, rebuild it from a temporary copy of `original/` and point compile or link steps at that copy.

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/gif_lib.h`
- `safe/src/`
- `safe/tests/Makefile`
- `safe/tests/abi_layout.c`
- `safe/tests/internal_exports_smoke.c`
- `original/Makefile`
- `relevant_cves.json`
- `original/NEWS`
- `original/tests/public_api_regress.c`
- `original/tests/`
- `original/pic/`

## New Outputs
- Malformed-input regression fixtures plus provenance notes
- Deterministic malformed-input observation helper
- Deterministic malformed-baseline capture script
- Committed original malformed-input compatibility baseline artifact keyed by malformed fixture basename
- Hardened decoder cleanup/error-path behavior for the selected malformed inputs

## File Changes
- Update `safe/src/decode.rs`
- Update `safe/src/slurp.rs`
- Update `safe/src/lib.rs`
- Update `safe/tests/Makefile`
- Create `safe/tests/malformed_observe.c`
- Create `safe/tests/capture_malformed_baseline.sh`
- Create `safe/tests/malformed/`
- Create `safe/tests/malformed/manifest.txt` or equivalent provenance file
- Create `safe/tests/malformed/original-baseline.txt`

## Implementation Details
- Replace arithmetic that can panic in safe Rust with checked arithmetic and explicit `GIF_ERROR` returns.
- Preserve or add panic-boundary wrappers for every Rust-defined C ABI entry point touched in this phase so panics become C-compatible failure values instead of unwinding across the ABI boundary.
- For `CVE-2019-15133`, derive at least one malformed fixture from an existing sample GIF that forces zero or otherwise invalid image dimensions and verify that it is rejected without divide-by-zero, panic, or abort.
- For `CVE-2005-2974`, derive at least one malformed fixture from an existing sample GIF that drives the decoder into a partial-image or invalid-state cleanup path and verify rejection without null-dereference-class behavior.
- Record in `safe/tests/malformed/manifest.txt` which original fixture each malformed case was derived from and what bytes changed.
- Add `safe/tests/malformed_observe.c` as a deterministic helper that emits one tab-separated line per malformed fixture with fields `basename`, `open_nonnull`, `open_error`, `slurp_rc`, `gif_error_after_slurp`, `close_rc`, and `close_error`.
- Add `safe/tests/capture_malformed_baseline.sh` so it resolves the repository root from its own path, runs the helper over committed malformed `*.gif` inputs in lexical order, excludes metadata files, and writes `safe/tests/malformed/original-baseline.txt`.
- Add `malformed-baseline-regress` to `safe/tests/Makefile` so it compiles the observation helper against the safe library, captures the same tab-separated matrix, and diffs it against `safe/tests/malformed/original-baseline.txt`.
- Capture the baseline by compiling `safe/tests/malformed_observe.c` against an original library built from a temporary copy of `original/` using `original/Makefile`, then commit the resulting baseline artifact in this phase.
- Keep the safe library's observable malformed-input results identical to the committed baseline for the committed fixtures. If a candidate fixture would force a safety-motivated divergence, adjust or replace the fixture instead of relying on an undocumented mismatch.
- Restrict `safe/src/lib.rs` changes in this phase to decoder/slurp hardening and decoder-side error/panic boundaries.

## Verification Phases

### `check_05_security_baseline`
- Phase ID: `check_05_security_baseline`
- Type: `check`
- Bounce Target: `impl_05_security_baseline`
- Purpose: Verify that the derived malformed fixtures are committed with provenance, the original malformed-input compatibility baseline is captured as an explicit artifact, the safe library matches that baseline while rejecting the inputs without crashes or panics, and decoder hardening does not regress the direct sequential decoder APIs.
- Commands:
```bash
original_build_dir="$(mktemp -d)"
trap 'rm -rf "$original_build_dir"' EXIT
cp -a original/. "$original_build_dir"
make -C "$original_build_dir" libgif.so libgif.a
cargo build --manifest-path safe/Cargo.toml --release
if rg -n '\.\./original/.*\.c|cc::Build|legacy backend|gif_legacy' safe/build.rs safe/Cargo.toml safe/src; then
  echo 'unexpected bootstrap reference remains in library build inputs during security hardening' >&2
  exit 1
fi
cmp -s safe/include/gif_lib.h original/gif_lib.h
cc -std=gnu99 -Wall -Wextra -I"$original_build_dir" safe/tests/malformed_observe.c "$original_build_dir/libgif.a" -o /tmp/malformed_observe.original
safe/tests/capture_malformed_baseline.sh /tmp/malformed_observe.original "$PWD/safe/tests/malformed" > /tmp/original-malformed-baseline.txt
diff -u safe/tests/malformed/original-baseline.txt /tmp/original-malformed-baseline.txt
header_only_dir="$(mktemp -d)"
make -C safe/tests ORIGINAL_INCLUDEDIR="$header_only_dir" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" safe-header-regress
make -C safe/tests ORIGINAL_INCLUDEDIR="$PWD/original" ORIGINAL_TESTS_DIR="$PWD/original/tests" ORIGINAL_PIC_DIR="$PWD/original/pic" LIBGIF_INCLUDEDIR="$PWD/safe/include" LIBGIF_LIBDIR="$PWD/safe/target/release" render-regress gifclrmp-regress giffilter-regress giftext-regress gifbuild-regress gifsponge-regress giftool-regress giffix-regress malformed-regress malformed-baseline-regress link-compat-regress internal-export-regress
objdump -T safe/target/release/libgif.so | awk '$4 != "*UND*" && $6 == "Base" { print $7 "@Base" }' | sort > /tmp/safe-symbols.txt
sed -n '3,$p' original/debian/libgif7.symbols | awk '{print $1}' | sort > /tmp/original-symbols.txt
diff -u /tmp/original-symbols.txt /tmp/safe-symbols.txt
```

## Success Criteria
- Committed malformed fixtures exist under `safe/tests/malformed/` with explicit provenance recorded against existing source fixtures.
- `safe/tests/malformed/original-baseline.txt` is reproducible from an original-library build staged from a temporary copy of `original/`.
- The safe library matches the committed malformed baseline, rejects the selected inputs safely, and preserves decoder behavior on the required direct low-level regressions.
- `render-regress`, `gifclrmp-regress`, `giffilter-regress`, `giftext-regress`, `gifbuild-regress`, `gifsponge-regress`, `giftool-regress`, `giffix-regress`, `malformed-regress`, `malformed-baseline-regress`, `link-compat-regress`, and `internal-export-regress` all pass.
- Any Rust-defined ABI entry point touched in this phase catches panics and returns C-compatible failure values instead of unwinding across the ABI boundary.

## Git Commit Requirement
The implementer must commit all phase work to git before yielding.
