#!/usr/bin/env bash
# 一次打出 CLI +（可选）AppImage，方便以后拷走。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"

echo "==> AIDA Linux $VERSION  packing into $OUT_DIR"

"$ROOT/scripts/build-cli.sh"

appimage_ok=0
if "$ROOT/scripts/build-appimage.sh"; then
  appimage_ok=1
else
  echo "warning: AppImage 失败（常见于缺 FUSE/GL 库）。CLI 仍在 dist/" >&2
fi

echo
echo "==> SHA256SUMS"
(
  cd "$OUT_DIR"
  # 只对真正带走的产物做校验和；跳过 linuxdeploy 缓存与短名 symlink。
  shopt -s nullglob
  files=(aida-cli-[0-9]* AIDA_Linux-*.AppImage)
  if ((${#files[@]})); then
    sha256sum "${files[@]}" | tee SHA256SUMS
  fi
  shopt -u nullglob
) | sed 's/^/  /'

echo
echo "==> dist/"
ls -lh "$OUT_DIR" | sed 's/^/  /'
echo
echo "CLI:      $OUT_DIR/aida-cli"
if [[ "$appimage_ok" -eq 1 ]]; then
  echo "AppImage: $OUT_DIR/AIDA_Linux-${VERSION}-$(uname -m).AppImage"
  echo "          APPIMAGE_EXTRACT_AND_RUN=1 $OUT_DIR/AIDA_Linux-${VERSION}-$(uname -m).AppImage collect"
fi
echo "本机安装: ./scripts/install.sh"
