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

# 清掉上一版 leftover，避免 SHA256SUMS / smoke 扫到旧二进制。
shopt -s nullglob
for f in "$OUT_DIR"/aida-cli "$OUT_DIR"/aida-cli-* \
  "$OUT_DIR"/AIDA_Linux-*.AppImage "$OUT_DIR"/aida-linux-*.tar.gz \
  "$OUT_DIR"/SHA256SUMS; do
  [[ -e "$f" ]] || continue
  rm -f "$f"
done
for d in "$OUT_DIR"/aida-linux-*/; do
  rm -rf "$d"
done
shopt -u nullglob

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

if [[ "$appimage_ok" -eq 1 ]]; then
  AIDA_SMOKE_REQUIRE=cli,appimage,tarball "$ROOT/scripts/smoke-dist.sh"
else
  AIDA_SMOKE_REQUIRE=cli,tarball "$ROOT/scripts/smoke-dist.sh"
fi

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
echo "本机安装: 解压 tar.gz 后 ./install.sh   （Ubuntu apt / CentOS dnf|yum，不需要 cargo）"
echo "          源码树也可 ./scripts/install.sh --deps"
echo "体检:     dist/aida-cli doctor"
echo "校验:     (cd $OUT_DIR && sha256sum -c SHA256SUMS)"
