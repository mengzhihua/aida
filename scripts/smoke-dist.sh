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
  local text="$WORK/${prefix}.txt"
  local csv="$WORK/${prefix}.csv"
  local md="$WORK/${prefix}.md"
  "$bin" collect --json "$json" --html "$html" --text "$text" --csv "$csv" --md "$md" >/dev/null
  [[ -s "$json" ]] || fail "$bin collect --json 空文件"
  [[ -s "$html" ]] || fail "$bin collect --html 空文件"
  [[ -s "$text" ]] || fail "$bin collect --text 空文件"
  [[ -s "$csv" ]] || fail "$bin collect --csv 空文件"
  [[ -s "$md" ]] || fail "$bin collect --md 空文件"
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
  grep -q "AIDA Linux 硬件报告" "$text" || fail "$bin TEXT 不含标题"
  grep -q "^\[CPU\]" "$text" || fail "$bin TEXT 不含 [CPU]"
  head -n1 "$csv" | grep -qx 'section,key,value' || fail "$bin CSV 表头不对"
  grep -q "CPU" "$csv" || fail "$bin CSV 不含 CPU"
  grep -q "# AIDA Linux 硬件报告" "$md" || fail "$bin MD 不含标题"
  "$bin" collect --format text >"$WORK/${prefix}-fmt.txt"
  grep -q "AIDA Linux 硬件报告" "$WORK/${prefix}-fmt.txt" || fail "$bin --format text 空"
  "$bin" collect --format text --csv - >"$WORK/${prefix}-fmt-csv.out"
  grep -q "AIDA Linux 硬件报告" "$WORK/${prefix}-fmt-csv.out" || fail "$bin --format text --csv - 缺 TEXT"
  grep -q "^section,key,value" "$WORK/${prefix}-fmt-csv.out" || fail "$bin --format text --csv - 缺 CSV"
  ok "$bin collect json+html+text+csv+md"
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

smoke_doctor() {
  local bin="$1"
  local prefix="$2"
  local txt="$WORK/${prefix}-doctor.txt"
  local js="$WORK/${prefix}-doctor.json"
  "$bin" doctor >"$txt"
  grep -q "family=" "$txt" || fail "$bin doctor 没有 family="
  grep -qE "apt-get|dnf|yum|zypper|pacman|apk |musl|glibc" "$txt" \
    || fail "$bin doctor 没有发行版安装提示"
  "$bin" doctor --json >"$js"
  python3 - "$js" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
assert "family" in d, d.keys()
assert d["family"] in ("debian", "rhel", "suse", "arch", "alpine", "unknown"), d["family"]
assert isinstance(d.get("hints"), list) and d["hints"], d
assert "gui_libs" in d
assert "gui_need_glibc" in d and d["gui_need_glibc"]
PY
  ok "$bin doctor + --json family=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["family"])' "$js")"
}

shopt -s nullglob
cli=("$OUT_DIR"/aida-cli-${VERSION}-*)
app=("$OUT_DIR"/AIDA_Linux-${VERSION}-*.AppImage)
tarball=("$OUT_DIR"/aida-linux-${VERSION}-*.tar.gz)
shopt -u nullglob

if ((${#cli[@]})); then
  bin="${cli[0]}"
  chmod +x "$bin" 2>/dev/null || true
  [[ -x "$bin" ]] || fail "$bin 不可执行"
  expect_version "$bin"
  smoke_collect "$bin" cli
  smoke_bench "$bin" cli
  smoke_doctor "$bin" cli
  set +e
  "$bin" doctor --jsno >/dev/null 2>"$WORK/doctor-bad.err"
  st=$?
  set -e
  [[ "$st" -eq 2 ]] || fail "$bin doctor --jsno 应退出 2，实际 $st"
  grep -q "未知参数" "$WORK/doctor-bad.err" || fail "$bin doctor --jsno 应提示未知参数"
  ok "$bin doctor 拒绝未知参数"
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
  echo "$listing" | grep -q install.sh || fail "$tar 缺少 install.sh"
  echo "$listing" | grep -q os-family.sh || fail "$tar 缺少 os-family.sh"
  echo "$listing" | grep -q run-doctor.sh || fail "$tar 缺少 run-doctor.sh"
  echo "$listing" | grep -q packaging/aida.desktop || fail "$tar 缺少 packaging/aida.desktop"
  tar -xzf "$tar" -C "$WORK"
  bundle="$WORK/aida-linux-${VERSION}-${ARCH}"
  [[ -d "$bundle" ]] || fail "解压后没有 $bundle"
  [[ -x "$bundle/run-collect.sh" ]] || fail "没有 run-collect.sh"
  [[ -x "$bundle/run-gui.sh" ]] || fail "没有 run-gui.sh"
  [[ -x "$bundle/install.sh" ]] || fail "没有 install.sh"
  [[ -x "$bundle/run-doctor.sh" ]] || fail "没有 run-doctor.sh"
  "$bundle/install.sh" --help >/dev/null
  set +e
  "$bundle/install.sh" --prefix= >/dev/null 2>"$WORK/prefix-empty.err"
  st=$?
  set -e
  [[ "$st" -eq 2 ]] || fail "install.sh --prefix= 应退出 2，实际 $st"
  grep -q "需要目录" "$WORK/prefix-empty.err" || fail "install.sh --prefix= 应拒绝空前缀"
  ok "install.sh 拒绝空 --prefix="
  grep -q "CentOS" "$bundle/INSTALL.txt" || fail "INSTALL.txt 未提到 CentOS"
  grep -q "Ubuntu" "$bundle/INSTALL.txt" || fail "INSTALL.txt 未提到 Ubuntu"
  "$bundle/run-doctor.sh" >"$WORK/bundle-doctor.txt"
  grep -q "family=" "$WORK/bundle-doctor.txt" || fail "run-doctor.sh 没有 family="
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
  grep -qi "ID_LIKE" "$WORK/bundle.html" || fail "bundle HTML 不含 ID_LIKE"
  SMOKE_PREFIX="$WORK/install-prefix"
  mkdir -p "$SMOKE_PREFIX/bin" "$SMOKE_PREFIX/lib/aida"
  echo leftover >"$SMOKE_PREFIX/bin/aida-gui-bin"
  chmod +x "$SMOKE_PREFIX/bin/aida-gui-bin"
  PREFIX="$SMOKE_PREFIX" "$bundle/install.sh" >/dev/null
  [[ -x "$SMOKE_PREFIX/bin/aida" ]] || fail "install.sh 没有装出 bin/aida"
  [[ -x "$SMOKE_PREFIX/bin/aida-cli" ]] || fail "install.sh 没有装出 bin/aida-cli"
  [[ ! -e "$SMOKE_PREFIX/bin/aida-gui-bin" ]] || fail "install.sh 留下了旧的 aida-gui-bin"
  grep -q elevate "$SMOKE_PREFIX/bin/aida" || fail "安装入口没有处理 elevate"
  got="$("$SMOKE_PREFIX/bin/aida" version)"
  [[ "$got" == "aida $VERSION" ]] || fail "安装后 aida version 是 '$got'"
  "$SMOKE_PREFIX/bin/aida" doctor >/dev/null
  [[ -f "$SMOKE_PREFIX/lib/aida/GLIBC_GUI" ]] || fail "install.sh 没有装出 GLIBC_GUI"
  ok "install.sh --prefix 后 aida doctor（并清掉 leftover GUI）"
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
