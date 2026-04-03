#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
	echo "usage: $0 /path/to/malformed_observe [malformed_dir]" >&2
	exit 64
fi

helper=$1
malformed_dir=${2:-"$repo_root/safe/tests/malformed"}

if [ ! -x "$helper" ]; then
	echo "malformed helper is not executable: $helper" >&2
	exit 1
fi

if [ ! -d "$malformed_dir" ]; then
	echo "malformed fixture directory not found: $malformed_dir" >&2
	exit 1
fi

find "$malformed_dir" -maxdepth 1 -type f -name '*.gif' | LC_ALL=C sort | while IFS= read -r fixture
do
	"$helper" "$fixture"
done
