#!/usr/bin/env bash
# 对 dist/ 成品做发版前自测，模拟用户从 GitHub Release 解压后的用法。
#
# AIDA_SMOKE_REQUIRE=cli,appimage,tarball  缺哪一项就失败（CI 各 job 用）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
ARCH="${ARCH:-$(aida_arch)}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/aida-smoke.XXXXXX")"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

fail() { echo "SMOKE FAIL: $*" >&2; exit 1; }
ok() { echo "SMOKE OK: $*"; }

[[ -d "$OUT_DIR" ]] || fail "没有 $OUT_DIR"

require_has() {
  local want="$1"
  local token
  local -a reqs=()
  [[ -z "${AIDA_SMOKE_REQUIRE:-}" ]] && return 1
  IFS=',' read -r -a reqs <<<"$AIDA_SMOKE_REQUIRE"
  for token in "${reqs[@]}"; do
    token="${token// /}"
    [[ "$token" == "$want" ]] && return 0
  done
  return 1
}

expect_version() {
  local bin="$1"
  local got
  got="$("$bin" version)"
  [[ "$got" == "aida $VERSION" ]] || fail "$bin version 是 '$got'，期望 'aida $VERSION'"
  ok "$bin version = $got"
}

smoke_collect() {
  local bin="$1"
  local prefix="$2"
  local json="$WORK/${prefix}.json"
  local html="$WORK/${prefix}.html"
  "$bin" collect --json "$json" --html "$html" >/dev/null
  [[ -s "$json" ]] || fail "$bin collect --json 空文件"
  [[ -s "$html" ]] || fail "$bin collect --html 空文件"
  python3 - "$json" "$VERSION" <<'PY'
import json, sys
path, ver = sys.argv[1], sys.argv[2]
with open(path) as f:
    d = json.load(f)
assert d.get("app") == "aida", d.get("app")
assert d.get("version") == ver, d.get("version")
assert d.get("cpu", {}).get("logical_cpus", 0) >= 1, d.get("cpu")
assert "sysctl" in d and "net" in d and "buses" in d
PY
  grep -qi "aida" "$html" || fail "$bin HTML 不含 AIDA"
  ok "$bin collect json+html"
}

smoke_bench() {
  local bin="$1"
  local out="$WORK/${2:-bench}.json"
  "$bin" bench --quick --no-direct >"$out"
  python3 - "$out" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    json.load(f)
PY
  ok "$bin bench --quick"
}

shopt -s nullglob
cli=("$OUT_DIR"/aida-cli-[0-9]*)
app=("$OUT_DIR"/AIDA_Linux-*.AppImage)
tarball=("$OUT_DIR"/aida-linux-*.tar.gz)
shopt -u nullglob

if ((${#cli[@]})); then
  bin="${cli[0]}"
  chmod +x "$bin" 2>/dev/null || true
  [[ -x "$bin" ]] || fail "$bin 不可执行"
  expect_version "$bin"
  smoke_collect "$bin" cli
  smoke_bench "$bin" cli
  if command -v file >/dev/null; then
    if [[ "$bin" == *-musl ]]; then
      file "$bin" | grep -qi 'static' || fail "$bin 文件名是 musl 但不是静态链接"
      ok "$bin 静态链接"
    else
      file "$bin" | grep -qi 'static' || echo "note: $bin 不是 static（gnu 构建时正常）"
    fi
  fi
else
  require_has cli && fail "要求 CLI，但 $OUT_DIR 没有 aida-cli-*"
fi

if ((${#app[@]})); then
  img="${app[0]}"
  chmod +x "$img" 2>/dev/null || true
  [[ -x "$img" ]] || fail "$img 不可执行"
  export APPIMAGE_EXTRACT_AND_RUN=1
  expect_version "$img"
  smoke_collect "$img" appimage
  ok "$img collect（无 DISPLAY 走 CLI）"
else
  require_has appimage && fail "要求 AppImage，但 $OUT_DIR 没有 AIDA_Linux-*.AppImage"
fi

if ((${#tarball[@]})); then
  tar="${tarball[0]}"
  listing="$(tar -tzf "$tar")"
  echo "$listing" | grep -q INSTALL.txt || fail "$tar 缺少 INSTALL.txt"
  echo "$listing" | grep -q README.md || fail "$tar 缺少 README.md"
  echo "$listing" | grep -q run-collect.sh || fail "$tar 缺少 run-collect.sh"
  echo "$listing" | grep -q run-gui.sh || fail "$tar 缺少 run-gui.sh"
  tar -xzf "$tar" -C "$WORK"
  bundle="$WORK/aida-linux-${VERSION}-${ARCH}"
  [[ -d "$bundle" ]] || fail "解压后没有 $bundle"
  [[ -x "$bundle/run-collect.sh" ]] || fail "没有 run-collect.sh"
  [[ -x "$bundle/run-gui.sh" ]] || fail "没有 run-gui.sh"
  "$bundle/run-collect.sh" --json "$WORK/bundle.json" --html "$WORK/bundle.html" >/dev/null
  python3 - "$WORK/bundle.json" "$VERSION" <<'PY'
import json, sys
path, ver = sys.argv[1], sys.argv[2]
with open(path) as f:
    d = json.load(f)
assert d["app"] == "aida" and d["version"] == ver
assert d.get("cpu", {}).get("logical_cpus", 0) >= 1
PY
  grep -qi "aida" "$WORK/bundle.html" || fail "bundle HTML 不含 AIDA"
  if [[ -f "$bundle/SHA256SUMS" ]]; then
    (cd "$bundle" && sha256sum -c SHA256SUMS) >/dev/null
    ok "bundle SHA256SUMS"
  fi
  ok "$tar 解压后 run-collect.sh"
else
  require_has tarball && fail "要求 tar.gz，但 $OUT_DIR 没有 aida-linux-*.tar.gz"
fi

if [[ -f "$OUT_DIR/SHA256SUMS" ]]; then
  (cd "$OUT_DIR" && sha256sum -c SHA256SUMS) >/dev/null
  ok "dist/SHA256SUMS"
fi

if ((${#cli[@]} == 0)) && ((${#app[@]} == 0)) && ((${#tarball[@]} == 0)); then
  fail "dist/ 里没有 CLI、AppImage 或 tar.gz"
fi

echo "SMOKE PASS  version=$VERSION  dist=$OUT_DIR"
