#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_TAG="${GIFLIB_ORIGINAL_TEST_IMAGE:-giflib-original-test:ubuntu24.04}"
GIFLIB_TEST_SCOPE="${GIFLIB_TEST_SCOPE:-all}"

usage() {
  cat <<'EOF'
usage: test-original.sh [--scope runtime|source|all] [--help]

Run downstream GIFLIB replacement checks inside the Docker harness.

Options:
  --scope runtime|source|all  Select which downstream subset to run.
                              Default: all.
  --help                      Show this help and exit.

Environment:
  GIFLIB_TEST_SCOPE           Default scope when --scope is not passed.
                              Default: all.
  GIFLIB_ORIGINAL_TEST_IMAGE  Docker image tag to use for the harness image.
EOF
}

parse_args() {
  while (($#)); do
    case "$1" in
      --help|-h)
        usage
        exit 0
        ;;
      --scope)
        shift
        if (($# == 0)); then
          echo "missing value for --scope" >&2
          usage >&2
          exit 1
        fi
        GIFLIB_TEST_SCOPE="$1"
        ;;
      --scope=*)
        GIFLIB_TEST_SCOPE="${1#*=}"
        ;;
      --)
        shift
        if (($# != 0)); then
          echo "unexpected argument: $1" >&2
          usage >&2
          exit 1
        fi
        break
        ;;
      *)
        echo "unexpected argument: $1" >&2
        usage >&2
        exit 1
        ;;
    esac
    shift
  done

  case "$GIFLIB_TEST_SCOPE" in
    runtime|source|all) ;;
    *)
      echo "invalid scope: $GIFLIB_TEST_SCOPE" >&2
      usage >&2
      exit 1
      ;;
  esac

  export GIFLIB_TEST_SCOPE
}

parse_args "$@"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to run $0" >&2
  exit 1
fi

if [[ ! -d "$ROOT/original" ]]; then
  echo "missing original source tree" >&2
  exit 1
fi

if [[ ! -d "$ROOT/safe" ]]; then
  echo "missing safe source tree" >&2
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
      cargo \
      cmake \
      dbus-x11 \
      debhelper \
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
      rustc \
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
COPY \./safe /work/safe/
COPY \./original /work/original/
WORKDIR /work
DOCKERFILE

docker run --rm -i -e GIFLIB_TEST_SCOPE="$GIFLIB_TEST_SCOPE" "$IMAGE_TAG" bash <<'CONTAINER_SCRIPT'
set -euo pipefail

export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export DEBIAN_FRONTEND=noninteractive

ROOT=/work
SAFE_ROOT=/work/safe
ORIGINAL_ROOT=/work/original
DOWNSTREAM_ROOT=/tmp/giflib-dependent-sources
GIFLIB_TEST_SCOPE="${GIFLIB_TEST_SCOPE:-all}"

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

require_not_contains() {
  local path="$1"
  local needle="$2"

  if grep -F -- "$needle" "$path" >/dev/null 2>&1; then
    printf 'found unexpected text in %s: %s\n' "$path" "$needle" >&2
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

build_safe_packages() {
  local runtime_matches
  local dev_matches
  local dbgsym_matches
  local changes_matches
  local buildinfo_matches

  log_step "Building safe Debian packages"

  rm -f \
    "$ROOT"/libgif7_*.deb \
    "$ROOT"/libgif-dev_*.deb \
    "$ROOT"/libgif7-dbgsym_*.ddeb \
    "$ROOT"/giflib_*.changes \
    "$ROOT"/giflib_*.buildinfo
  if ! (
    cd "$SAFE_ROOT"
    dpkg-buildpackage -us -uc -b >/tmp/safe-dpkg-build.log 2>&1
  ); then
    cat /tmp/safe-dpkg-build.log >&2
    exit 1
  fi

  if find "$ROOT" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.ddeb' \) \
    ! -name 'libgif7_*.deb' \
    ! -name 'libgif-dev_*.deb' \
    ! -name 'libgif7-dbgsym_*.ddeb' \
    | grep -q .; then
    find "$ROOT" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.ddeb' \) >&2
    die "unexpected non-library Debian package artifact"
  fi

  if find "$ROOT" -maxdepth 1 -type f \( -name '*.changes' -o -name '*.buildinfo' \) \
    ! -name 'giflib_*.changes' \
    ! -name 'giflib_*.buildinfo' \
    | grep -q .; then
    find "$ROOT" -maxdepth 1 -type f \( -name '*.changes' -o -name '*.buildinfo' \) >&2
    die "unexpected non-giflib build metadata artifact"
  fi

  runtime_matches="$(find "$ROOT" -maxdepth 1 -type f -name 'libgif7_*.deb' | LC_ALL=C sort)"
  dev_matches="$(find "$ROOT" -maxdepth 1 -type f -name 'libgif-dev_*.deb' | LC_ALL=C sort)"
  dbgsym_matches="$(find "$ROOT" -maxdepth 1 -type f -name 'libgif7-dbgsym_*.ddeb' | LC_ALL=C sort)"
  changes_matches="$(find "$ROOT" -maxdepth 1 -type f -name 'giflib_*.changes' | LC_ALL=C sort)"
  buildinfo_matches="$(find "$ROOT" -maxdepth 1 -type f -name 'giflib_*.buildinfo' | LC_ALL=C sort)"

  [[ "$(printf '%s\n' "$runtime_matches" | sed '/^$/d' | wc -l)" -eq 1 ]] || die "expected exactly one libgif7 Debian package"
  [[ "$(printf '%s\n' "$dev_matches" | sed '/^$/d' | wc -l)" -eq 1 ]] || die "expected exactly one libgif-dev Debian package"
  [[ "$(printf '%s\n' "$dbgsym_matches" | sed '/^$/d' | wc -l)" -eq 1 ]] || die "expected exactly one libgif7-dbgsym package"
  [[ "$(printf '%s\n' "$changes_matches" | sed '/^$/d' | wc -l)" -eq 1 ]] || die "expected exactly one giflib .changes file"
  [[ "$(printf '%s\n' "$buildinfo_matches" | sed '/^$/d' | wc -l)" -eq 1 ]] || die "expected exactly one giflib .buildinfo file"

  SAFE_RUNTIME_DEB="$(printf '%s\n' "$runtime_matches" | head -n1)"
  SAFE_DEV_DEB="$(printf '%s\n' "$dev_matches" | head -n1)"
  SAFE_RUNTIME_DBGSYM="$(printf '%s\n' "$dbgsym_matches" | head -n1)"
  SAFE_CHANGES_FILE="$(printf '%s\n' "$changes_matches" | head -n1)"
  SAFE_BUILDINFO_FILE="$(printf '%s\n' "$buildinfo_matches" | head -n1)"
  SAFE_RUNTIME_PACKAGE="$(dpkg-deb -f "$SAFE_RUNTIME_DEB" Package)"
  SAFE_RUNTIME_VERSION="$(dpkg-deb -f "$SAFE_RUNTIME_DEB" Version)"
  SAFE_DEV_PACKAGE="$(dpkg-deb -f "$SAFE_DEV_DEB" Package)"
  SAFE_DEV_VERSION="$(dpkg-deb -f "$SAFE_DEV_DEB" Version)"

  [[ "$SAFE_RUNTIME_PACKAGE" == "libgif7" ]] || die "unexpected runtime package name: $SAFE_RUNTIME_PACKAGE"
  [[ "$SAFE_DEV_PACKAGE" == "libgif-dev" ]] || die "unexpected development package name: $SAFE_DEV_PACKAGE"
  case "$SAFE_RUNTIME_VERSION" in
    *+safelibs*) ;;
    *) die "missing local safelibs version suffix in runtime package version" ;;
  esac
  [[ "$SAFE_DEV_VERSION" == "$SAFE_RUNTIME_VERSION" ]] || die "development package version mismatch"

  export SAFE_RUNTIME_DEB SAFE_DEV_DEB
  export SAFE_RUNTIME_DBGSYM SAFE_CHANGES_FILE SAFE_BUILDINFO_FILE
  export SAFE_RUNTIME_PACKAGE SAFE_RUNTIME_VERSION
  export SAFE_DEV_PACKAGE SAFE_DEV_VERSION

  printf 'SAFE_RUNTIME_DEB=%s\n' "$SAFE_RUNTIME_DEB"
  printf 'SAFE_DEV_DEB=%s\n' "$SAFE_DEV_DEB"
  printf 'SAFE_RUNTIME_DBGSYM=%s\n' "$SAFE_RUNTIME_DBGSYM"
  printf 'SAFE_CHANGES_FILE=%s\n' "$SAFE_CHANGES_FILE"
  printf 'SAFE_BUILDINFO_FILE=%s\n' "$SAFE_BUILDINFO_FILE"
  printf 'SAFE_RUNTIME_PACKAGE=%s\n' "$SAFE_RUNTIME_PACKAGE"
  printf 'SAFE_DEV_PACKAGE=%s\n' "$SAFE_DEV_PACKAGE"
  printf 'SAFE_RUNTIME_VERSION=%s\n' "$SAFE_RUNTIME_VERSION"
  printf 'SAFE_DEV_VERSION=%s\n' "$SAFE_DEV_VERSION"
}

install_safe_packages() {
  log_step "Installing safe Debian packages"

  dpkg -i "$SAFE_RUNTIME_DEB" "$SAFE_DEV_DEB" >/tmp/safe-dpkg-install.log 2>&1 || {
    cat /tmp/safe-dpkg-install.log >&2
    exit 1
  }
  ldconfig

  ACTIVE_RUNTIME_VERSION="$(dpkg-query -W -f='${Version}\n' libgif7)"
  ACTIVE_DEV_VERSION="$(dpkg-query -W -f='${Version}\n' libgif-dev)"

  [[ "$ACTIVE_RUNTIME_VERSION" == "$SAFE_RUNTIME_VERSION" ]] || die "active libgif7 version mismatch"
  [[ "$ACTIVE_DEV_VERSION" == "$SAFE_DEV_VERSION" ]] || die "active libgif-dev version mismatch"

  export ACTIVE_RUNTIME_VERSION ACTIVE_DEV_VERSION

  printf 'ACTIVE_RUNTIME_VERSION=%s\n' "$ACTIVE_RUNTIME_VERSION"
  printf 'ACTIVE_DEV_VERSION=%s\n' "$ACTIVE_DEV_VERSION"
}

resolve_installed_shared_libgif() {
  local label="$1"
  local cache_path
  local real_cache_path
  local package_paths
  local owner
  local matched=0

  cache_path="$(ldconfig -p | awk '/libgif\.so\.7 \(/{ print $NF; exit }')"
  [[ -n "$cache_path" ]] || die "unable to locate active shared libgif via ldconfig"
  real_cache_path="$(readlink -f "$cache_path")"

  package_paths="$(dpkg-query -L libgif7 | grep -E '/libgif\.so\.7(\.[0-9]+)*$' || true)"
  while IFS= read -r package_path; do
    [[ -n "$package_path" ]] || continue
    if [[ "$package_path" == "$cache_path" || "$package_path" == "$real_cache_path" || "$(readlink -f "$package_path")" == "$real_cache_path" ]]; then
      matched=1
      break
    fi
  done <<< "$package_paths"
  [[ "$matched" -eq 1 ]] || {
    printf 'active shared libgif path %s is not owned by libgif7 package contents\n' "$cache_path" >&2
    printf '%s\n' "$package_paths" >&2
    exit 1
  }

  owner="$(dpkg-query -S "$cache_path" 2>/dev/null | sed 's/: .*//' | head -n1 || true)"
  if [[ -z "$owner" ]]; then
    owner="$(dpkg-query -S "$real_cache_path" 2>/dev/null | sed 's/: .*//' | head -n1 || true)"
  fi
  [[ -n "$owner" ]] || die "unable to determine owner for $cache_path"

  ACTIVE_SHARED_LIBGIF="$cache_path"
  ACTIVE_SHARED_OWNER="$owner"
  export ACTIVE_SHARED_LIBGIF ACTIVE_SHARED_OWNER

  printf 'ACTIVE_SHARED_LIBGIF[%s]=%s\n' "$label" "$ACTIVE_SHARED_LIBGIF"
  printf 'ACTIVE_SHARED_OWNER[%s]=%s\n' "$label" "$ACTIVE_SHARED_OWNER"
}

resolve_installed_static_libgif() {
  local label="$1"
  local archive_path
  local owner

  archive_path="$(dpkg-query -L libgif-dev | grep -E '/libgif\.a$' | head -n1)"
  [[ -n "$archive_path" ]] || die "unable to locate packaged static libgif archive"

  owner="$(dpkg-query -S "$archive_path" 2>/dev/null | sed 's/: .*//' | head -n1 || true)"
  [[ -n "$owner" ]] || die "unable to determine owner for $archive_path"

  ACTIVE_STATIC_LIBGIF="$archive_path"
  ACTIVE_STATIC_OWNER="$owner"
  export ACTIVE_STATIC_LIBGIF ACTIVE_STATIC_OWNER

  printf 'ACTIVE_STATIC_LIBGIF[%s]=%s\n' "$label" "$ACTIVE_STATIC_LIBGIF"
  printf 'ACTIVE_STATIC_OWNER[%s]=%s\n' "$label" "$ACTIVE_STATIC_OWNER"
}

assert_links_to_active_shared_libgif() {
  local label="$1"
  local path="$2"
  local log="$3"

  resolve_installed_shared_libgif "$label"
  ldd "$path" > "$log"
  require_contains "$log" "$ACTIVE_SHARED_LIBGIF"
}

assert_build_uses_active_giflib() {
  local label="$1"
  local path="$2"
  local log="$3"
  local link_cmd="$4"
  local mode

  require_nonempty_file "$link_cmd"
  resolve_installed_shared_libgif "$label"
  resolve_installed_static_libgif "$label"

  if ldd "$path" > "$log" 2>&1 && grep -F -- "$ACTIVE_SHARED_LIBGIF" "$log" >/dev/null 2>&1; then
    mode=shared
  else
    require_contains "$link_cmd" "$ACTIVE_STATIC_LIBGIF"
    mode=static
  fi

  printf 'LINK_ASSERT_MODE[%s]=%s\n' "$label" "$mode"
}

capture_last_matching_line() {
  local input="$1"
  local pattern="$2"
  local output="$3"

  grep -F -- "$pattern" "$input" | tail -n1 > "$output" || true
  require_nonempty_file "$output"
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

build_fbi_fakefb_shim() {
  local src=/tmp/fbi-fakefb.c
  local so=/tmp/fbi-fakefb.so

  if [[ -x "$so" ]]; then
    return
  fi

  cat > "$src" <<'C'
#define _GNU_SOURCE

#include <dlfcn.h>
#include <fcntl.h>
#include <linux/fb.h>
#include <linux/kd.h>
#include <linux/vt.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>

static int (*real_open_fn)(const char *, int, ...);
static int (*real_ioctl_fn)(int, unsigned long, ...);
static const char *fakefb_path;
static int fakefb_fd = -1;
static struct fb_var_screeninfo fake_var;
static struct fb_fix_screeninfo fake_fix;

static void init_fakefb(void) {
  if (!real_open_fn) real_open_fn = dlsym(RTLD_NEXT, "open");
  if (!real_ioctl_fn) real_ioctl_fn = dlsym(RTLD_NEXT, "ioctl");
  if (!fakefb_path) fakefb_path = getenv("FBI_FAKEFB_PATH");
  if (!fakefb_path) fakefb_path = "/tmp/fbi-fakefb";
  if (fake_fix.smem_len != 0) return;

  memset(&fake_var, 0, sizeof(fake_var));
  memset(&fake_fix, 0, sizeof(fake_fix));

  fake_var.xres = 64;
  fake_var.yres = 64;
  fake_var.xres_virtual = 64;
  fake_var.yres_virtual = 64;
  fake_var.bits_per_pixel = 32;
  fake_var.red.offset = 16;
  fake_var.red.length = 8;
  fake_var.green.offset = 8;
  fake_var.green.length = 8;
  fake_var.blue.offset = 0;
  fake_var.blue.length = 8;
  fake_var.transp.offset = 24;
  fake_var.transp.length = 8;

  fake_fix.type = FB_TYPE_PACKED_PIXELS;
  fake_fix.visual = FB_VISUAL_TRUECOLOR;
  fake_fix.line_length = fake_var.xres * 4;
  fake_fix.smem_len = fake_fix.line_length * fake_var.yres;
  memcpy(fake_fix.id, "fakefb", 7);
}

int open(const char *path, int flags, ...) {
  int fd;
  mode_t mode = 0;

  init_fakefb();
  if (flags & O_CREAT) {
    va_list ap;
    va_start(ap, flags);
    mode = va_arg(ap, mode_t);
    va_end(ap);
    fd = real_open_fn(path, flags, mode);
  } else {
    fd = real_open_fn(path, flags);
  }

  if (fd >= 0 && strcmp(path, fakefb_path) == 0) {
    fakefb_fd = fd;
  }
  return fd;
}

int ioctl(int fd, unsigned long request, ...) {
  void *arg;

  init_fakefb();
  va_list ap;
  va_start(ap, request);
  arg = va_arg(ap, void *);
  va_end(ap);

  if (fd == 0) {
    switch (request) {
    case VT_GETSTATE: {
      struct vt_stat *state = arg;
      memset(state, 0, sizeof(*state));
      state->v_active = 1;
      return 0;
    }
    case KDGETMODE:
      *(int *)arg = KD_TEXT;
      return 0;
    case VT_GETMODE: {
      struct vt_mode *mode = arg;
      memset(mode, 0, sizeof(*mode));
      mode->mode = VT_AUTO;
      return 0;
    }
    case VT_SETMODE:
    case VT_ACTIVATE:
    case VT_WAITACTIVE:
    case VT_RELDISP:
    case KDSETMODE:
      return 0;
    default:
      break;
    }
  }

  if (fd == fakefb_fd) {
    switch (request) {
    case FBIOGET_VSCREENINFO:
      memcpy(arg, &fake_var, sizeof(fake_var));
      return 0;
    case FBIOPUT_VSCREENINFO:
      memcpy(&fake_var, arg, sizeof(fake_var));
      return 0;
    case FBIOGET_FSCREENINFO:
      memcpy(arg, &fake_fix, sizeof(fake_fix));
      return 0;
    case FBIOPAN_DISPLAY:
      return 0;
    default:
      break;
    }
  }

  return real_ioctl_fn(fd, request, arg);
}
C

  cc -shared -fPIC -O2 -o "$so" "$src" -ldl >/tmp/fbi-fakefb-build.log 2>&1
}

capture_mtpaint_window_title() {
  local input="$1"
  local log="$2"

  INPUT_IMAGE="$input" WINDOW_LOG="$log" timeout 20 xvfb-run -a bash -c '
    set -euo pipefail
    mtpaint -v "$INPUT_IMAGE" >/tmp/mtpaint-runtime.log 2>&1 &
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
    xdotool getwindowname "$wid" > "$WINDOW_LOG" || true
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" || true
  '
}

assert_runtime_linkage() {
  local multiarch
  local libgdal_path

  log_step "Verifying runtime linkage to active packaged giflib"

  multiarch="$(gcc -print-multiarch)"
  libgdal_path="$(ldconfig -p | awk '/libgdal\.so/ { print $NF; exit }')"
  [[ -n "$libgdal_path" ]] || die "unable to locate libgdal shared library"

  assert_links_to_active_shared_libgif "giflib-tools-runtime" /usr/bin/giftext /tmp/ldd-giftext.log
  assert_links_to_active_shared_libgif "webp-runtime" /usr/bin/gif2webp /tmp/ldd-gif2webp.log
  assert_links_to_active_shared_libgif "fbi-runtime" /usr/bin/fbi /tmp/ldd-fbi.log
  assert_links_to_active_shared_libgif "mtpaint-runtime" /usr/bin/mtpaint /tmp/ldd-mtpaint.log
  assert_links_to_active_shared_libgif "tracker-extract-runtime" "/usr/lib/$multiarch/tracker-miners-3.0/extract-modules/libextract-gif.so" /tmp/ldd-tracker-gif.log
  assert_links_to_active_shared_libgif "libextractor-runtime" "/usr/lib/$multiarch/libextractor/libextractor_gif.so" /tmp/ldd-libextractor-gif.log
  assert_links_to_active_shared_libgif "camlimages-runtime" /usr/lib/ocaml/stublibs/dllcamlimages_gif_stubs.so /tmp/ldd-camlimages-gif.log
  assert_links_to_active_shared_libgif "gdal-runtime" "$libgdal_path" /tmp/ldd-libgdal.log
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
  local fakefb=/tmp/fbi-fakefb
  local bad_input=/tmp/fbi-not-a-gif.gif

  log_step "fbi"

  build_fbi_fakefb_shim

  truncate -s 16384 "$fakefb"
  printf '' | env LD_PRELOAD=/tmp/fbi-fakefb.so FBI_FAKEFB_PATH="$fakefb" \
    strace -f -e execve -o /tmp/fbi-gif-execve.log \
    fbi -d "$fakefb" -1 -t 1 "$SAMPLE_GIF" >/tmp/fbi-runtime.log 2>&1

  require_contains /tmp/fbi-gif-execve.log "$SAMPLE_GIF"
  require_not_contains /tmp/fbi-gif-execve.log "convert"
  require_not_contains /tmp/fbi-runtime.log "FAILED"

  printf 'not a gif\n' > "$bad_input"
  truncate -s 16384 "$fakefb"
  printf '' | env LD_PRELOAD=/tmp/fbi-fakefb.so FBI_FAKEFB_PATH="$fakefb" \
    strace -f -e execve -o /tmp/fbi-bad-execve.log \
    fbi -d "$fakefb" -1 -t 1 "$bad_input" >/tmp/fbi-bad.log 2>&1 || true

  require_contains /tmp/fbi-bad-execve.log "convert"
  require_contains /tmp/fbi-bad.log "loading $bad_input"
  require_contains /tmp/fbi-bad.log "FAILED"
}

test_mtpaint_runtime() {
  local bad_input=/tmp/mtpaint-not-a-gif.gif

  log_step "mtpaint"

  capture_mtpaint_window_title "$SAMPLE_GIF" /tmp/mtpaint-window.log

  require_nonempty_file /tmp/mtpaint-window.log
  require_contains /tmp/mtpaint-window.log "$SAMPLE_GIF"

  printf 'not a gif\n' > "$bad_input"
  capture_mtpaint_window_title "$bad_input" /tmp/mtpaint-bad-window.log
  require_nonempty_file /tmp/mtpaint-bad-window.log
  require_not_contains /tmp/mtpaint-bad-window.log "$bad_input"
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
  local link_cmd

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
  link_cmd="$(find "$build" -path '*link.txt' -exec grep -l -- 'libgdal.so' {} + | LC_ALL=C sort | head -n1)"
  [[ -n "$link_cmd" ]] || die "unable to locate GDAL linker command"
  assert_build_uses_active_giflib "gdal-source" "$libgdal_path" /tmp/gdal-build-ldd.log "$link_cmd"

  GDAL_DATA="$src/data" \
  LD_LIBRARY_PATH="$(dirname "$libgdal_path")${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$build/apps/gdalinfo" "$SAMPLE_GIF" > /tmp/gdal-source.log

  require_contains /tmp/gdal-source.log "Driver: GIF/Graphics Interchange Format"
  require_contains /tmp/gdal-source.log "Size is $SAMPLE_WIDTH, $SAMPLE_HEIGHT"
}

test_exactimage_source() {
  local src out
  local link_cmd=/tmp/exactimage-link.txt

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
    make -j"$(nproc)" V=1 >/tmp/exactimage-build.log 2>&1
  )

  capture_last_matching_line /tmp/exactimage-build.log "objdir/frontends/econvert" "$link_cmd"
  assert_build_uses_active_giflib "exactimage-source" "$src/objdir/frontends/econvert" /tmp/exactimage-ldd.log "$link_cmd"

  out=/tmp/exactimage-output.png
  "$src/objdir/frontends/econvert" -i "$SAMPLE_GIF" -o "$out" >/tmp/exactimage-runtime.log 2>&1
  require_nonempty_file "$out"
  require_contains <(file "$out") "PNG image data"
}

test_sail_source() {
  local src build prefix pkgconfig_dir gif_artifact link_dir
  local link_cmd

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

  link_cmd="$(find "$build" -path '*link.txt' -exec grep -l -- 'libgif' {} + | LC_ALL=C sort | head -n1)"
  [[ -n "$link_cmd" ]] || die "unable to locate SAIL linker command"
  gif_artifact="$(
    awk '
      {
        for (i = 1; i < NF; i++) {
          if ($i == "-o") {
            print $(i + 1)
            exit
          }
        }
      }
    ' "$link_cmd"
  )"
  [[ -n "$gif_artifact" ]] || die "unable to determine SAIL build artifact from $link_cmd"
  if [[ ! -e "$gif_artifact" ]]; then
    link_dir="$(dirname "$link_cmd")"
    if [[ -e "$link_dir/$gif_artifact" ]]; then
      gif_artifact="$link_dir/$gif_artifact"
    fi
  fi
  if [[ ! -e "$gif_artifact" ]]; then
    gif_artifact="$(find_artifact_using_libgif "$prefix")" || die "no GIF-linked SAIL artifact found"
  fi
  assert_build_uses_active_giflib "sail-source" "$gif_artifact" /tmp/sail-ldd.log "$link_cmd"

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
  assert_build_uses_active_giflib "libwebp-source" "$build/gif2webp" /tmp/libwebp-ldd.log "$link_cmd"

  "$build/gif2webp" "$SAMPLE_GIF" -o /tmp/libwebp-source.webp >/tmp/libwebp-runtime.log 2>&1
  require_nonempty_file /tmp/libwebp-source.webp
  require_contains <(file /tmp/libwebp-source.webp) "Web/P image"
}

test_imlib2_source() {
  local src prefix loader_path lib_path pkgconfig_dir
  local link_cmd=/tmp/imlib2-link.txt

  log_step "imlib2 (source)"

  src="$(fetch_source_dir imlib2)"
  prefix=/tmp/install-imlib2
  rm -rf "$prefix"

  (
    cd "$src"
    autoreconf -fi >/tmp/imlib2-autoreconf.log 2>&1
    ./configure --prefix="$prefix" >/tmp/imlib2-configure.log 2>&1
    make -j"$(nproc)" V=1 >/tmp/imlib2-build.log 2>&1
    make install >/tmp/imlib2-install.log 2>&1
  )

  loader_path="$prefix/lib/imlib2/loaders/gif.so"
  lib_path="$(find "$prefix/lib" -maxdepth 1 \( -type f -o -type l \) -name 'libImlib2.so*' | sort | head -n1)"
  pkgconfig_dir="$(dirname "$(find "$prefix" -type f -name imlib2.pc | head -n1)")"

  [[ -f "$loader_path" ]] || die "unable to locate installed Imlib2 GIF loader"
  [[ -n "$lib_path" ]] || die "unable to locate installed libImlib2 shared library"
  [[ -n "$pkgconfig_dir" ]] || die "unable to locate imlib2.pc"

  capture_last_matching_line /tmp/imlib2-build.log "gif.la" "$link_cmd"
  assert_build_uses_active_giflib "imlib2-source" "$loader_path" /tmp/imlib2-loader-ldd.log "$link_cmd"

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

SAMPLE_GIF="$ORIGINAL_ROOT/pic/welcome2.gif"
if [[ ! -f "$SAMPLE_GIF" ]]; then
  SAMPLE_GIF="$ORIGINAL_ROOT/pic/treescap.gif"
fi
[[ -f "$SAMPLE_GIF" ]] || die "unable to locate a sample GIF fixture"

run_shared_setup() {
  validate_dependents_inventory
  build_safe_packages
  install_safe_packages
  discover_sample_dimensions
  assert_runtime_linkage
}

run_runtime_checks() {
  test_giflib_tools
  test_webp_runtime
  test_fbi_runtime
  test_mtpaint_runtime
  test_tracker_extract_runtime
  test_libextractor_runtime
  test_camlimages_runtime
  test_gdal_runtime
}

run_source_checks() {
  test_gdal_source
  test_exactimage_source
  test_sail_source
  test_libwebp_source
  test_imlib2_source
}

run_shared_setup

case "$GIFLIB_TEST_SCOPE" in
  runtime)
    run_runtime_checks
    ;;
  source)
    run_source_checks
    ;;
  all)
    run_runtime_checks
    run_source_checks
    ;;
esac

log_step "All downstream checks passed"
CONTAINER_SCRIPT
