#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
archive="$root/vendor/registry.tar.xz"
if [[ -d "$root/vendor/registry" ]]; then
  exit 0
fi
if [[ ! -f "$archive" ]]; then
  echo "missing vendored registry archive: $archive" >&2
  exit 1
fi
tar -xJf "$archive" -C "$root"
test -d "$root/vendor/registry"
