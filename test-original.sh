#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_TAG="${GIFLIB_ORIGINAL_TEST_IMAGE:-giflib-original-test:ubuntu24.04}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to run $0" >&2
  exit 1
fi

if [[ ! -d "$ROOT/original" ]]; then
  echo "missing original source tree" >&2
  exit 1
fi

if [[ ! -f "$ROOT/dependents.json" ]]; then
  echo "missing dependents.json" >&2
  exit 1
fi

docker build -t "$IMAGE_TAG" -f - "$ROOT" <<'DOCKERFILE'
FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      autoconf \
      automake \
      build-essential \
      ca-certificates \
      cmake \
      dbus-x11 \
      dpkg-dev \
      extract \
      fbi \
      file \
      gdal-bin \
      giflib-tools \
      jq \
      libcamlimages-ocaml \
      libcamlimages-ocaml-dev \
      libextractor-plugin-gif \
      libtool \
      mtpaint \
      ocaml-findlib \
      ocaml-nox \
      pkg-config \
      python3 \
      strace \
      tracker-extract \
      webp \
      xauth \
      xdotool \
      xvfb \
 && apt-get build-dep -y --no-install-recommends \
      gdal \
      exactimage \
      sail \
      libwebp \
      imlib2 \
 && rm -rf /var/lib/apt/lists/*

COPY dependents.json /work/dependents.json
COPY original /work/original
WORKDIR /work
DOCKERFILE

docker run --rm -i "$IMAGE_TAG" bash <<'CONTAINER_SCRIPT'
set -euo pipefail

export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export DEBIAN_FRONTEND=noninteractive

ROOT=/work
SRC_ROOT=/tmp/giflib-original
DOWNSTREAM_ROOT=/tmp/giflib-dependent-sources
export LD_LIBRARY_PATH="/usr/local/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

log_step() {
  printf '\n==> %s\n' "$1"
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_nonempty_file() {
  local path="$1"

  [[ -s "$path" ]] || die "expected non-empty file: $path"
}

require_contains() {
  local path="$1"
  local needle="$2"

  if ! grep -F -- "$needle" "$path" >/dev/null 2>&1; then
    printf 'missing expected text in %s: %s\n' "$path" "$needle" >&2
    printf -- '--- %s ---\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

require_regex() {
  local path="$1"
  local regex="$2"

  if ! grep -E -- "$regex" "$path" >/dev/null 2>&1; then
    printf 'missing expected pattern in %s: %s\n' "$path" "$regex" >&2
    printf -- '--- %s ---\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

assert_uses_original() {
  local path="$1"
  local log="$2"

  ldd "$path" > "$log"
  require_contains "$log" "/usr/local/lib/libgif.so.7"
}

find_artifact_using_libgif() {
  local root="$1"
  local candidate

  while IFS= read -r -d '' candidate; do
    if ldd "$candidate" 2>/dev/null | grep -F 'libgif.so' >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done < <(find "$root" -type f \( -name '*.so' -o -name '*.so.*' -o -perm -u+x \) -print0)

  return 1
}

fetch_source_dir() {
  local package="$1"
  local base="$DOWNSTREAM_ROOT/$package"
  local source_log="/tmp/apt-source-${package}.log"

  mkdir -p "$base"

  if ! find "$base" -mindepth 1 -maxdepth 1 -type d | grep -q .; then
    if [[ ! -f /tmp/apt-source-ready ]]; then
      apt-get update >/tmp/apt-source-update.log 2>&1
      touch /tmp/apt-source-ready
    fi
    (
      cd "$base"
      apt-get source -o APT::Sandbox::User=root -qq "$package" >"$source_log" 2>&1
    )
  fi

  find "$base" -mindepth 1 -maxdepth 1 -type d | sort | head -n1
}

validate_dependents_inventory() {
  local expected actual

  expected=$'giflib-tools\tbinary\truntime\nwebp\tbinary\truntime\nfbi\tbinary\truntime\nmtpaint\tbinary\truntime\ntracker-extract\tbinary\truntime\nlibextractor-plugin-gif\tbinary\truntime\nlibcamlimages-ocaml\tbinary\truntime\nlibgdal34t64\tbinary\truntime\ngdal\tsource\tcompile-time\nexactimage\tsource\tcompile-time\nsail\tsource\tcompile-time\nlibwebp\tsource\tcompile-time\nimlib2\tsource\tcompile-time'
  actual="$(jq -r '.dependents[] | [.name, .package_kind, .dependency_path] | @tsv' "$ROOT/dependents.json")"

  if [[ "$actual" != "$expected" ]]; then
    echo "dependents.json does not match the expected dependent matrix" >&2
    diff -u <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") >&2 || true
    exit 1
  fi
}

build_original_giflib() {
  log_step "Building original giflib"

  rm -rf "$SRC_ROOT"
  cp -a "$ROOT/original" "$SRC_ROOT"
  make -C "$SRC_ROOT" clean >/tmp/giflib-clean.log 2>&1 || true
  make -C "$SRC_ROOT" -j"$(nproc)" libgif.so libgif.a >/tmp/giflib-build.log 2>&1
  make -C "$SRC_ROOT" install-lib install-include >/tmp/giflib-install.log 2>&1

  printf '/usr/local/lib\n' > /etc/ld.so.conf.d/zz-giflib-local.conf
  ldconfig
}

discover_sample_dimensions() {
  read -r SAMPLE_WIDTH SAMPLE_HEIGHT < <(
    python3 - "$SAMPLE_GIF" <<'PY'
import struct
import sys

with open(sys.argv[1], "rb") as fh:
    header = fh.read(10)

if header[:6] not in (b"GIF87a", b"GIF89a"):
    raise SystemExit("not a GIF")

width, height = struct.unpack("<HH", header[6:10])
print(width, height)
PY
  )
}

assert_runtime_linkage() {
  local multiarch
  local libgdal_path

  log_step "Verifying runtime linkage to original giflib"

  multiarch="$(gcc -print-multiarch)"
  libgdal_path="$(ldconfig -p | awk '/libgdal\.so/ { print $NF; exit }')"
  [[ -n "$libgdal_path" ]] || die "unable to locate libgdal shared library"

  assert_uses_original /usr/bin/giftext /tmp/ldd-giftext.log
  assert_uses_original /usr/bin/gif2webp /tmp/ldd-gif2webp.log
  assert_uses_original /usr/bin/fbi /tmp/ldd-fbi.log
  assert_uses_original /usr/bin/mtpaint /tmp/ldd-mtpaint.log
  assert_uses_original "/usr/lib/$multiarch/tracker-miners-3.0/extract-modules/libextract-gif.so" /tmp/ldd-tracker-gif.log
  assert_uses_original "/usr/lib/$multiarch/libextractor/libextractor_gif.so" /tmp/ldd-libextractor-gif.log
  assert_uses_original /usr/lib/ocaml/stublibs/dllcamlimages_gif_stubs.so /tmp/ldd-camlimages-gif.log
  assert_uses_original "$libgdal_path" /tmp/ldd-libgdal.log
}

test_giflib_tools() {
  log_step "giflib-tools"

  giftext "$SAMPLE_GIF" > /tmp/giftext-runtime.log
  require_contains /tmp/giftext-runtime.log "Screen Size - Width = $SAMPLE_WIDTH, Height = $SAMPLE_HEIGHT"
}

test_webp_runtime() {
  log_step "webp"

  gif2webp "$SAMPLE_GIF" -o /tmp/runtime-sample.webp >/tmp/gif2webp-runtime.log 2>&1
  require_nonempty_file /tmp/runtime-sample.webp
  require_contains <(file /tmp/runtime-sample.webp) "Web/P image"
}

test_fbi_runtime() {
  local status=0

  log_step "fbi"

  set +e
  strace -f -e trace=file -o /tmp/fbi-runtime.strace fbi "$SAMPLE_GIF" >/tmp/fbi-runtime.log 2>&1
  status=$?
  set -e

  if [[ "$status" -ne 0 ]]; then
    require_regex /tmp/fbi-runtime.log 'No such file or directory|framebuffer|open /dev/fb|not a linux console'
  fi

  require_contains /tmp/fbi-runtime.strace "$SAMPLE_GIF"
}

test_mtpaint_runtime() {
  log_step "mtpaint"

  SAMPLE_GIF="$SAMPLE_GIF" timeout 20 xvfb-run -a bash -c '
    set -euo pipefail
    mtpaint -v "$SAMPLE_GIF" >/tmp/mtpaint-runtime.log 2>&1 &
    pid=$!
    wid=""
    for _ in $(seq 1 40); do
      if ! kill -0 "$pid" 2>/dev/null; then
        wait "$pid"
        exit 1
      fi
      wid="$(xdotool search --onlyvisible --pid "$pid" 2>/dev/null | head -n1 || true)"
      if [[ -n "$wid" ]]; then
        break
      fi
      sleep 0.25
    done
    [[ -n "$wid" ]]
    xdotool getwindowname "$wid" > /tmp/mtpaint-window.log || true
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" || true
  '

  require_nonempty_file /tmp/mtpaint-window.log
  require_regex /tmp/mtpaint-window.log '(mtPaint|welcome2|treescap|gif)'
}

test_tracker_extract_runtime() {
  log_step "tracker-extract"

  dbus-run-session -- tracker3 extract --output-format=turtle "$SAMPLE_GIF" > /tmp/tracker-extract.log
  require_contains /tmp/tracker-extract.log "nfo:width \"$SAMPLE_WIDTH\""
  require_contains /tmp/tracker-extract.log "nfo:height \"$SAMPLE_HEIGHT\""
}

test_libextractor_runtime() {
  local multiarch
  local plugin_path
  local dimensions_regex

  log_step "libextractor-plugin-gif"

  dpkg-query -W -f='${Status}\n' libextractor-plugin-gif > /tmp/libextractor-package.log
  require_contains /tmp/libextractor-package.log "install ok installed"

  multiarch="$(gcc -print-multiarch)"
  plugin_path="/usr/lib/$multiarch/libextractor/libextractor_gif.so"
  [[ -f "$plugin_path" ]] || die "unable to locate libextractor GIF plugin"

  assert_uses_original "$plugin_path" /tmp/ldd-libextractor-gif.log
  extract -n -l gif -V "$SAMPLE_GIF" > /tmp/libextractor-gif.log
  require_contains /tmp/libextractor-gif.log "mimetype - image/gif"
  dimensions_regex="image dimensions - (${SAMPLE_WIDTH}x${SAMPLE_HEIGHT}|${SAMPLE_HEIGHT}x${SAMPLE_WIDTH})"
  require_regex /tmp/libextractor-gif.log "$dimensions_regex"
}

test_camlimages_runtime() {
  log_step "libcamlimages-ocaml"

  cat > /tmp/camlimages-smoke.ml <<'OCAML'
let image = OImages.load Sys.argv.(1) [] in
Printf.printf "%dx%d\n" image#width image#height
OCAML

  ocamlfind ocamlc -package camlimages.core,camlimages.gif -linkpkg \
    /tmp/camlimages-smoke.ml -o /tmp/camlimages-smoke >/tmp/camlimages-build.log 2>&1
  /tmp/camlimages-smoke "$SAMPLE_GIF" > /tmp/camlimages-runtime.log
  require_contains /tmp/camlimages-runtime.log "${SAMPLE_WIDTH}x${SAMPLE_HEIGHT}"
}

test_gdal_runtime() {
  log_step "libgdal34t64"

  gdalinfo "$SAMPLE_GIF" > /tmp/gdal-runtime.log
  require_contains /tmp/gdal-runtime.log "Driver: GIF/Graphics Interchange Format"
  require_contains /tmp/gdal-runtime.log "Size is $SAMPLE_WIDTH, $SAMPLE_HEIGHT"
}

test_gdal_source() {
  local src build libgdal_path

  log_step "gdal (source)"

  src="$(fetch_source_dir gdal)"
  build=/tmp/build-gdal
  rm -rf "$build"

  cmake -S "$src" -B "$build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_APPS=ON \
    -DBUILD_PYTHON_BINDINGS=OFF \
    -DGDAL_BUILD_OPTIONAL_DRIVERS=OFF \
    -DOGR_BUILD_OPTIONAL_DRIVERS=OFF \
    -DGDAL_ENABLE_DRIVER_GIF=ON \
    -DGDAL_USE_GIF=ON \
    >/tmp/gdal-configure.log 2>&1
  cmake --build "$build" --target gdalinfo -j"$(nproc)" >/tmp/gdal-build.log 2>&1

  libgdal_path="$(find "$build" -type f -name 'libgdal.so*' | sort | head -n1)"
  [[ -n "$libgdal_path" ]] || die "unable to locate built GDAL shared library"
  assert_uses_original "$libgdal_path" /tmp/gdal-build-ldd.log

  GDAL_DATA="$src/data" \
  LD_LIBRARY_PATH="$(dirname "$libgdal_path")${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$build/apps/gdalinfo" "$SAMPLE_GIF" > /tmp/gdal-source.log

  require_contains /tmp/gdal-source.log "Driver: GIF/Graphics Interchange Format"
  require_contains /tmp/gdal-source.log "Size is $SAMPLE_WIDTH, $SAMPLE_HEIGHT"
}

test_exactimage_source() {
  local src out

  log_step "exactimage (source)"

  src="$(fetch_source_dir exactimage)"

  (
    cd "$src"
    ./configure \
      --prefix=/usr \
      --includedir=/usr/include \
      --mandir=/usr/share/man \
      --infodir=/usr/share/info \
      --sysconfdir=/etc \
      --libdir=/usr/lib \
      --libexecdir=/usr/lib \
      --with-ruby=no \
      --with-php=no \
      --with-evas=no \
      >/tmp/exactimage-configure.log 2>&1
    make -j"$(nproc)" >/tmp/exactimage-build.log 2>&1
  )

  assert_uses_original "$src/objdir/frontends/econvert" /tmp/exactimage-ldd.log

  out=/tmp/exactimage-output.png
  "$src/objdir/frontends/econvert" -i "$SAMPLE_GIF" -o "$out" >/tmp/exactimage-runtime.log 2>&1
  require_nonempty_file "$out"
  require_contains <(file "$out") "PNG image data"
}

test_sail_source() {
  local src build prefix pkgconfig_dir gif_artifact

  log_step "sail (source)"

  src="$(fetch_source_dir sail)"
  build=/tmp/build-sail
  prefix=/tmp/install-sail
  rm -rf "$build" "$prefix"

  cmake -S "$src" -B "$build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$prefix" \
    -DSAIL_ONLY_CODECS=gif \
    -DSAIL_BUILD_APPS=OFF \
    -DSAIL_BUILD_EXAMPLES=OFF \
    -DBUILD_TESTING=OFF \
    -DSAIL_COMBINE_CODECS=ON \
    >/tmp/sail-configure.log 2>&1
  cmake --build "$build" -j"$(nproc)" >/tmp/sail-build.log 2>&1
  cmake --install "$build" >/tmp/sail-install.log 2>&1

  gif_artifact="$(find_artifact_using_libgif "$prefix")" || die "no GIF-linked SAIL artifact found"
  assert_uses_original "$gif_artifact" /tmp/sail-ldd.log

  pkgconfig_dir="$(dirname "$(find "$prefix" -type f -name sail.pc | head -n1)")"
  [[ -n "$pkgconfig_dir" ]] || die "unable to locate sail.pc"

  cat > /tmp/sail-smoke.c <<'C'
#include <stdio.h>
#include <sail/sail.h>

int main(int argc, char **argv) {
  struct sail_image *image = NULL;
  if (argc != 2) {
    return 2;
  }
  if (sail_load_from_file(argv[1], &image) != SAIL_OK || image == NULL) {
    return 1;
  }
  printf("%d x %d\n", image->width, image->height);
  sail_destroy_image(image);
  return 0;
}
C

  PKG_CONFIG_PATH="$pkgconfig_dir" cc /tmp/sail-smoke.c -o /tmp/sail-smoke \
    $(PKG_CONFIG_PATH="$pkgconfig_dir" pkg-config --cflags --libs sail) \
    -Wl,-rpath-link,"$prefix/lib" \
    >/tmp/sail-smoke-build.log 2>&1

  LD_LIBRARY_PATH="$prefix/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    /tmp/sail-smoke "$SAMPLE_GIF" > /tmp/sail-runtime.log

  require_contains /tmp/sail-runtime.log "$SAMPLE_WIDTH x $SAMPLE_HEIGHT"
}

test_libwebp_source() {
  local src build
  local link_cmd

  log_step "libwebp (source)"

  src="$(fetch_source_dir libwebp)"
  build=/tmp/build-libwebp
  rm -rf "$build"

  cmake -S "$src" -B "$build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DWEBP_BUILD_CWEBP=OFF \
    -DWEBP_BUILD_DWEBP=OFF \
    -DWEBP_BUILD_GIF2WEBP=ON \
    -DWEBP_BUILD_IMG2WEBP=OFF \
    -DWEBP_BUILD_VWEBP=OFF \
    -DWEBP_BUILD_WEBPINFO=OFF \
    -DWEBP_BUILD_WEBPMUX=OFF \
    -DWEBP_BUILD_EXTRAS=OFF \
    >/tmp/libwebp-configure.log 2>&1
  cmake --build "$build" --target gif2webp -j"$(nproc)" >/tmp/libwebp-build.log 2>&1

  link_cmd="$build/CMakeFiles/gif2webp.dir/link.txt"
  ldd "$build/gif2webp" > /tmp/libwebp-ldd.log
  if ! grep -F -- '/usr/local/lib/libgif.so.7' /tmp/libwebp-ldd.log >/dev/null 2>&1; then
    require_contains "$link_cmd" "/usr/local/lib/libgif.a"
  fi

  "$build/gif2webp" "$SAMPLE_GIF" -o /tmp/libwebp-source.webp >/tmp/libwebp-runtime.log 2>&1
  require_nonempty_file /tmp/libwebp-source.webp
  require_contains <(file /tmp/libwebp-source.webp) "Web/P image"
}

test_imlib2_source() {
  local src prefix loader_path lib_path pkgconfig_dir

  log_step "imlib2 (source)"

  src="$(fetch_source_dir imlib2)"
  prefix=/tmp/install-imlib2
  rm -rf "$prefix"

  (
    cd "$src"
    autoreconf -fi >/tmp/imlib2-autoreconf.log 2>&1
    ./configure --prefix="$prefix" >/tmp/imlib2-configure.log 2>&1
    make -j"$(nproc)" >/tmp/imlib2-build.log 2>&1
    make install >/tmp/imlib2-install.log 2>&1
  )

  loader_path="$prefix/lib/imlib2/loaders/gif.so"
  lib_path="$(find "$prefix/lib" -maxdepth 1 \( -type f -o -type l \) -name 'libImlib2.so*' | sort | head -n1)"
  pkgconfig_dir="$(dirname "$(find "$prefix" -type f -name imlib2.pc | head -n1)")"

  [[ -f "$loader_path" ]] || die "unable to locate installed Imlib2 GIF loader"
  [[ -n "$lib_path" ]] || die "unable to locate installed libImlib2 shared library"
  [[ -n "$pkgconfig_dir" ]] || die "unable to locate imlib2.pc"

  assert_uses_original "$loader_path" /tmp/imlib2-loader-ldd.log

  cat > /tmp/imlib2-smoke.c <<'C'
#include <stdio.h>
#include <Imlib2.h>

int main(int argc, char **argv) {
  Imlib_Image image;

  if (argc != 2) {
    return 2;
  }

  image = imlib_load_image(argv[1]);
  if (image == NULL) {
    return 1;
  }

  imlib_context_set_image(image);
  printf("%d x %d\n", imlib_image_get_width(), imlib_image_get_height());
  imlib_free_image();
  return 0;
}
C

  PKG_CONFIG_PATH="$pkgconfig_dir" cc /tmp/imlib2-smoke.c -o /tmp/imlib2-smoke \
    $(PKG_CONFIG_PATH="$pkgconfig_dir" pkg-config --cflags --libs imlib2) \
    >/tmp/imlib2-smoke-build.log 2>&1

  IMLIB2_LOADER_PATH="$prefix/lib/imlib2/loaders" \
  LD_LIBRARY_PATH="$prefix/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    /tmp/imlib2-smoke "$SAMPLE_GIF" > /tmp/imlib2-runtime.log

  require_contains /tmp/imlib2-runtime.log "$SAMPLE_WIDTH x $SAMPLE_HEIGHT"
}

SAMPLE_GIF="$ROOT/original/pic/welcome2.gif"
if [[ ! -f "$SAMPLE_GIF" ]]; then
  SAMPLE_GIF="$ROOT/original/pic/treescap.gif"
fi
[[ -f "$SAMPLE_GIF" ]] || die "unable to locate a sample GIF fixture"

validate_dependents_inventory
build_original_giflib
discover_sample_dimensions
assert_runtime_linkage

test_giflib_tools
test_webp_runtime
test_fbi_runtime
test_mtpaint_runtime
test_tracker_extract_runtime
test_libextractor_runtime
test_camlimages_runtime
test_gdal_runtime

test_gdal_source
test_exactimage_source
test_sail_source
test_libwebp_source
test_imlib2_source

log_step "All downstream checks passed"
CONTAINER_SCRIPT
