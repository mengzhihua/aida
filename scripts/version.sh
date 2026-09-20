# 供其它打包脚本 source。不要直接执行。
# shellcheck shell=bash

aida_root() {
  (cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
}

aida_version() {
  local root="${1:-$(aida_root)}"
  sed -n 's/^version = "\([0-9][0-9.]*\)"/\1/p' "$root/Cargo.toml" | head -1
}

aida_arch() {
  uname -m
}

# AppStream `<release date>`。可复现构建用 SOURCE_DATE_EPOCH。
aida_date() {
  if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
    date -u -d "@${SOURCE_DATE_EPOCH}" +%Y-%m-%d
  else
    date -u +%Y-%m-%d
  fi
}
