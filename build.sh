#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

OUT="AccountChanger"
GOFLAGS_BUILD="-trimpath"
LDFLAGS="-s -w -buildid="
TAGS=""
VARIANT="webkit41"

export GOTOOLCHAIN="${GOTOOLCHAIN:-local}"
export CGO_ENABLED=1

for arg in "$@"; do
  case "$arg" in
    nogui)     export CGO_ENABLED=0; VARIANT="nogui" ;;
    webkit40)  TAGS="webkit40"; VARIANT="webkit40" ;;
    *)         echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

export GOCACHE="${GOCACHE:-$PWD/.gocache-linux-$VARIANT}"

if ! command -v go >/dev/null 2>&1; then
  echo "Go not found in PATH. Install Go 1.25+ first." >&2
  exit 1
fi
echo "Using $(go version)"

if [ "$CGO_ENABLED" = "1" ]; then
  pkg="webkit2gtk-4.1"
  [ "$TAGS" = "webkit40" ] && pkg="webkit2gtk-4.0"
  for dep in gtk+-3.0 "$pkg"; do
    if ! pkg-config --exists "$dep" 2>/dev/null; then
      echo "Missing build dependency: $dep" >&2
      echo "Install the -dev packages (see README, Сборка/Linux)," >&2
      echo "or run './build.sh nogui' to build without the native window." >&2
      exit 1
    fi
  done
  echo "Building with native window ($pkg)"
else
  echo "Building without cgo - the UI will open in a browser window"
fi

BUILD_TAGS=()
[ -n "$TAGS" ] && BUILD_TAGS=(-tags "$TAGS")

go build $GOFLAGS_BUILD "${BUILD_TAGS[@]}" -ldflags "$LDFLAGS" -o "$OUT" .

echo "Built $OUT"
