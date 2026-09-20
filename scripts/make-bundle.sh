#!/usr/bin/env bash
# 把 dist/ 里已经打好的 CLI / AppImage 收成一份可直接拷走的 tar.gz。
# 不重新编译。CI 的 bundle 作业和本地 package.sh 都走这里。
# 包内带 install.sh：Ubuntu/Debian 与 CentOS/RHEL 解压后都能装，不需要 cargo。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)/.."
ROOT="$(cd "$ROOT" && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
ARCH="${ARCH:-$(aida_arch)}"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
BUNDLE_NAME="aida-linux-${VERSION}-${ARCH}"
STAGE="$OUT_DIR/$BUNDLE_NAME"

mkdir -p "$OUT_DIR"
rm -rf "$STAGE"
mkdir -p "$STAGE/packaging/polkit"

shopt -s nullglob
cli_files=("$OUT_DIR"/aida-cli-${VERSION}-*)
if [[ "${AIDA_BUNDLE_SKIP_APPIMAGE:-}" == 1 ]]; then
  app_files=()
else
  app_files=("$OUT_DIR"/AIDA_Linux-${VERSION}-*.AppImage)
fi
shopt -u nullglob

if ((${#cli_files[@]} == 0)) && ((${#app_files[@]} == 0)); then
  echo "dist/ 里没有 aida-cli-* 也没有 AppImage" >&2
  exit 1
fi

copied=()
if ((${#cli_files[@]})); then
  cli="${cli_files[0]}"
  install -m 0755 "$cli" "$STAGE/$(basename "$cli")"
  ln -sfn "$(basename "$cli")" "$STAGE/aida-cli"
  copied+=("$(basename "$cli")")
fi
if ((${#app_files[@]})); then
  img="${app_files[0]}"
  install -m 0755 "$img" "$STAGE/$(basename "$img")"
  ln -sfn "$(basename "$img")" "$STAGE/AIDA_Linux.AppImage"
  copied+=("$(basename "$img")")
fi

install -m 0644 "$ROOT/README.md" "$STAGE/README.md"
if [[ -f "$ROOT/packaging/INSTALL.txt" ]]; then
  install -m 0644 "$ROOT/packaging/INSTALL.txt" "$STAGE/INSTALL.txt"
fi
install -m 0755 "$ROOT/scripts/install.sh" "$STAGE/install.sh"
install -m 0755 "$ROOT/scripts/os-family.sh" "$STAGE/os-family.sh"
install -m 0644 "$ROOT/packaging/aida.desktop" "$STAGE/packaging/aida.desktop"
install -m 0644 "$ROOT/packaging/aida.svg" "$STAGE/packaging/aida.svg"
if [[ -f "$ROOT/packaging/com.aida.linux.metainfo.xml" ]]; then
  install -m 0644 "$ROOT/packaging/com.aida.linux.metainfo.xml" \
    "$STAGE/packaging/com.aida.linux.metainfo.xml"
fi
if [[ -f "$ROOT/packaging/polkit/com.aida.linux.policy" ]]; then
  install -m 0644 "$ROOT/packaging/polkit/com.aida.linux.policy" \
    "$STAGE/packaging/polkit/com.aida.linux.policy"
fi
if [[ -f "$OUT_DIR/GLIBC_GUI" ]]; then
  install -m 0644 "$OUT_DIR/GLIBC_GUI" "$STAGE/GLIBC_GUI"
elif [[ -f "$ROOT/packaging/GLIBC_GUI" ]]; then
  install -m 0644 "$ROOT/packaging/GLIBC_GUI" "$STAGE/GLIBC_GUI"
fi

cat >"$STAGE/run-collect.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
if [[ ! -x "$DIR/aida-cli" ]]; then
  echo "这个包没有 CLI（aida-cli）" >&2
  exit 1
fi
exec "$DIR/aida-cli" collect "$@"
EOF

cat >"$STAGE/run-doctor.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
if [[ ! -x "$DIR/aida-cli" ]]; then
  echo "这个包没有 CLI（aida-cli）" >&2
  exit 1
fi
exec "$DIR/aida-cli" doctor "$@"
EOF

cat >"$STAGE/run-gui.sh" <<'EOF'
#!/usr/bin/env bash
# 桌面 AppImage。CentOS 7 / 旧 Ubuntu 若报 GLIBC_ 缺失，请改用 ./run-collect.sh。
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
img="$DIR/AIDA_Linux.AppImage"
if [[ ! -e "$img" ]]; then
  echo "这个包没有桌面 AppImage。采集请用 ./run-collect.sh 或 ./aida-cli doctor" >&2
  exit 1
fi
export APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}"
if [[ -f "$DIR/os-family.sh" ]]; then
  # shellcheck disable=SC1091
  source "$DIR/os-family.sh"
  echo "note: 发行版 family=$(aida_os_family)。GUI 库: $(aida_gui_install_cmd)" >&2
fi
if [[ -f "$DIR/GLIBC_GUI" ]]; then
  echo "note: 本 AppImage 按 glibc $(tr -d '[:space:]' <"$DIR/GLIBC_GUI") 构建；更旧请用 musl CLI。" >&2
fi
set +e
"$img" gui "$@"
st=$?
set -e
if [[ $st -ne 0 ]]; then
  echo >&2
  echo "GUI 没起来（exit $st）。常见原因：" >&2
  echo "  - 本机 glibc 低于 AppImage 构建目标（CentOS 7/8、Ubuntu 20.04/22.04 常见）" >&2
  echo "  - 缺 libEGL / libGL / libxkbcommon-x11" >&2
  echo "采集不受影响：" >&2
  echo "  ./aida-cli doctor" >&2
  echo "  ./run-collect.sh --html report.html" >&2
  echo "装 GUI 库： ./install.sh --deps" >&2
  exit "$st"
fi
EOF
chmod +x "$STAGE/run-collect.sh" "$STAGE/run-gui.sh" "$STAGE/run-doctor.sh" "$STAGE/install.sh"

(
  cd "$STAGE"
  if ((${#copied[@]})); then
    sha256sum "${copied[@]}" >SHA256SUMS
  fi
)

tar -C "$OUT_DIR" -czf "$OUT_DIR/${BUNDLE_NAME}.tar.gz" "$BUNDLE_NAME"

echo "==> bundle $OUT_DIR/${BUNDLE_NAME}.tar.gz"
ls -lh "$OUT_DIR/${BUNDLE_NAME}.tar.gz"

# 顶层 SHA256SUMS：CLI、AppImage、以及这份 tar.gz。
(
  cd "$OUT_DIR"
  shopt -s nullglob
  existing=()
  for f in "aida-cli-${VERSION}-"* "AIDA_Linux-${VERSION}-"*.AppImage "${BUNDLE_NAME}.tar.gz"; do
    [[ -e "$f" ]] || continue
    if [[ "${AIDA_BUNDLE_SKIP_APPIMAGE:-}" == 1 && "$f" == AIDA_Linux-*.AppImage ]]; then
      continue
    fi
    existing+=("$f")
  done
  if ((${#existing[@]})); then
    sha256sum "${existing[@]}" | tee SHA256SUMS
  fi
  shopt -u nullglob
)
