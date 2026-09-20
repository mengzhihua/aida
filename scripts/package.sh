#!/usr/bin/env bash
# 一次打出 CLI +（可选）AppImage + 可拷走的 tar.gz。每一轮开发结束都要跑。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
ARCH="${ARCH:-$(aida_arch)}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"

echo "==> AIDA Linux $VERSION  packing into $OUT_DIR"

"$ROOT/scripts/build-cli.sh"

appimage_ok=0
if "$ROOT/scripts/build-appimage.sh"; then
  appimage_ok=1
  unset AIDA_BUNDLE_SKIP_APPIMAGE || true
else
  echo "warning: AppImage 失败（常见于缺 FUSE/GL 库）。CLI 仍会打进安装包。" >&2
  export AIDA_BUNDLE_SKIP_APPIMAGE=1
fi

"$ROOT/scripts/make-bundle.sh"

echo
echo "==> dist/"
ls -lh "$OUT_DIR" | sed 's/^/  /'
echo
echo "安装包:   $OUT_DIR/aida-linux-${VERSION}-${ARCH}.tar.gz"
echo "CLI:      $OUT_DIR/aida-cli"
if [[ "$appimage_ok" -eq 1 ]]; then
  echo "AppImage: $OUT_DIR/AIDA_Linux-${VERSION}-${ARCH}.AppImage"
  echo "          APPIMAGE_EXTRACT_AND_RUN=1 $OUT_DIR/AIDA_Linux-${VERSION}-${ARCH}.AppImage collect"
fi
echo "本机安装: ./scripts/install.sh"
echo "校验:     (cd $OUT_DIR && sha256sum -c SHA256SUMS)"
