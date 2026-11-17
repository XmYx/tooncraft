# shellcheck shell=bash
# Shared packaging settings. Source it: `. "$(dirname "$0")/../env.sh"`.
#   VERSION                [workspace.package] version from Cargo.toml (override: TOONCRAFT_VERSION)
#   TOONCRAFT_BUILD_SHA    git commit shown in About
#   TOONCRAFT_BUILD_DATE   UTC build date, YYYY-MM-DD
#   DIST                   output directory (default: $ROOT/dist/release)
# Package names: tooncraft-<version>-<platform>-<arch>.<ext>

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
  return
}

VERSION="${TOONCRAFT_VERSION:-$(workspace_version)}"
if [[ -z "$VERSION" ]]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION
export TOONCRAFT_BUILD_SHA="${TOONCRAFT_BUILD_SHA:-$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)}"
export TOONCRAFT_BUILD_DATE="${TOONCRAFT_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export DIST="${DIST:-$ROOT/dist/release}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
mkdir -p "$DIST"
