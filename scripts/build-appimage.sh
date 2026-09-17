#!/usr/bin/env bash
# 用 linuxdeploy 打 glibc AppImage（GUI 依赖 OpenGL/X11，不要 musl 静态）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="$(uname -m)"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
APPDIR="${APPDIR:-$ROOT/AppDir}"
LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage}"

if [[ "$ARCH" != "x86_64" && "$ARCH" != "aarch64" ]]; then
  echo "仅支持 x86_64/aarch64，当前: $ARCH" >&2
  exit 1
fi

mkdir -p "$OUT_DIR" "$APPDIR"

echo "==> cargo build --release --features gui"
cd "$ROOT"
cargo build --release --features gui

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
  "$APPDIR/usr/share/polkit-1/actions" "$APPDIR/usr/share/metainfo"

install -m 0755 "$ROOT/target/release/aida" "$APPDIR/usr/bin/aida"
install -m 0644 "$ROOT/packaging/aida.desktop" "$APPDIR/usr/share/applications/aida.desktop"
install -m 0644 "$ROOT/packaging/aida.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/aida.svg"
install -m 0644 "$ROOT/packaging/aida.svg" "$APPDIR/aida.svg"
install -m 0644 "$ROOT/packaging/polkit/com.aida.linux.policy" \
  "$APPDIR/usr/share/polkit-1/actions/com.aida.linux.policy"

# linuxdeploy 要求桌面文件在 AppDir 根或 applications 下；再放一份 Exec 用的图标。
cp "$ROOT/packaging/aida.desktop" "$APPDIR/aida.desktop"

TOOL="$OUT_DIR/linuxdeploy-${ARCH}.AppImage"
if [[ ! -x "$TOOL" ]]; then
  echo "==> download linuxdeploy"
  curl -L --fail -o "$TOOL" "$LINUXDEPLOY_URL"
  chmod +x "$TOOL"
fi

# 容器里 FUSE 常不可用，强制解包运行。
export APPIMAGE_EXTRACT_AND_RUN=1
export LINUXDEPLOY_OUTPUT_VERSION="${LINUXDEPLOY_OUTPUT_VERSION:-0.2.0}"
export VERSION="${VERSION:-0.2.0}"
export ARCH

echo "==> linuxdeploy"
cd "$OUT_DIR"

# winit 通过 dlopen 加载 xkb/GL，不在 ELF NEEDED 里，linuxdeploy 默认不会打包。
EXTRA_LIBS=()
for lib in libxkbcommon.so.0 libxkbcommon-x11.so.0 libEGL.so.1 libGL.so.1 libGLdispatch.so.0 libGLX.so.0; do
  p="$(ldconfig -p 2>/dev/null | awk -v l="$lib" '$1 == l { print $NF; exit }')"
  if [[ -n "${p:-}" && -f "$p" ]]; then
    EXTRA_LIBS+=(--library "$p")
  else
    echo "note: builder 上没有 $lib，AppImage 将依赖目标机系统库" >&2
  fi
done

"$TOOL" \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/aida" \
  --desktop-file "$APPDIR/aida.desktop" \
  --icon-file "$APPDIR/aida.svg" \
  "${EXTRA_LIBS[@]}" \
  --output appimage

echo "==> done"
ls -lh "$OUT_DIR"/*.AppImage 2>/dev/null || ls -lh ./*.AppImage
