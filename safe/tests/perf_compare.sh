#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 <public_api_regress.original> <public_api_regress.safe>" >&2
    exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "$script_dir/../.." && pwd)"
original_bin="$1"
safe_bin="$2"

for bin in "$original_bin" "$safe_bin"; do
    if [[ ! -x "$bin" ]]; then
        echo "missing executable benchmark target: $bin" >&2
        exit 2
    fi
done

python3 - "$repo_root" "$original_bin" "$safe_bin" <<'PY'
import os
import statistics
import subprocess
import sys
import time

repo_root, original_bin, safe_bin = sys.argv[1:4]
warmups = 2
samples = 7
inner_loops = 25
threshold = 2.0

workloads = [
    (
        "render-welcome2",
        ["render", os.path.join(repo_root, "original/pic/welcome2.gif")],
        None,
    ),
    (
        "render-treescap-interlaced",
        ["render", os.path.join(repo_root, "original/pic/treescap-interlaced.gif")],
        None,
    ),
    (
        "highlevel-copy-fire",
        ["highlevel-copy", os.path.join(repo_root, "original/pic/fire.gif")],
        None,
    ),
    (
        "rgb-to-gif-gifgrid",
        ["rgb-to-gif", "3", "100", "100"],
        os.path.join(repo_root, "original/tests/gifgrid.rgb"),
    ),
]


def run_sample(binary, argv, stdin_path):
    start = time.perf_counter()
    for _ in range(inner_loops):
        if stdin_path is None:
            subprocess.run(
                [binary, *argv],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        else:
            with open(stdin_path, "rb") as stdin_file:
                subprocess.run(
                    [binary, *argv],
                    check=True,
                    stdin=stdin_file,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
    return time.perf_counter() - start


def median_elapsed(binary, argv, stdin_path):
    for _ in range(warmups):
        run_sample(binary, argv, stdin_path)
    measured = [run_sample(binary, argv, stdin_path) for _ in range(samples)]
    return statistics.median(measured)


failed = False
for workload_id, argv, stdin_path in workloads:
    original_median = median_elapsed(original_bin, argv, stdin_path)
    safe_median = median_elapsed(safe_bin, argv, stdin_path)
    ratio = safe_median / original_median if original_median else float("inf")
    print(
        "PERF "
        f"workload={workload_id} "
        f"original_median_s={original_median:.9f} "
        f"safe_median_s={safe_median:.9f} "
        f"ratio={ratio:.9f} "
        "threshold=2.00"
    )
    if ratio > threshold:
        failed = True

sys.exit(1 if failed else 0)
PY
