#!/usr/bin/env bash
# 用 linuxdeploy 打 glibc AppImage（GUI 依赖 OpenGL/X11，不要 musl 静态）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
ARCH="$(aida_arch)"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
APPDIR="${APPDIR:-$ROOT/AppDir}"
CACHE="${CACHE_DIR:-$ROOT/.cache}"
LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage}"

if [[ "$ARCH" != "x86_64" && "$ARCH" != "aarch64" ]]; then
  echo "仅支持 x86_64/aarch64，当前: $ARCH" >&2
  exit 1
fi

mkdir -p "$OUT_DIR" "$CACHE"
# 清掉上次留在 dist/ 的 linuxdeploy 和旧 AppImage，避免把 0.2.0 产物 mv 到新文件名上。
rm -f "$OUT_DIR"/linuxdeploy*.AppImage \
  "$OUT_DIR"/AIDA_Linux*.AppImage \
  "$OUT_DIR"/aida*.AppImage

echo "==> cargo build --release --features gui  (version $VERSION)"
cd "$ROOT"
cargo build --release --features gui

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
  "$APPDIR/usr/share/polkit-1/actions" "$APPDIR/usr/share/metainfo"

install -m 0755 "$ROOT/target/release/aida" "$APPDIR/usr/bin/aida"
install -m 0644 "$ROOT/packaging/aida.desktop" "$APPDIR/usr/share/applications/aida.desktop"
install -m 0644 "$ROOT/packaging/aida.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/aida.svg"
install -m 0644 "$ROOT/packaging/aida.svg" "$APPDIR/aida.svg"
install -m 0644 "$ROOT/packaging/polkit/com.aida.linux.policy" \
  "$APPDIR/usr/share/polkit-1/actions/com.aida.linux.policy"
if [[ -f "$ROOT/packaging/com.aida.linux.metainfo.xml" ]]; then
  sed "s/@VERSION@/${VERSION}/g" "$ROOT/packaging/com.aida.linux.metainfo.xml" \
    >"$APPDIR/usr/share/metainfo/com.aida.linux.metainfo.xml"
  # appimagetool 按 desktop id 找 aida.appdata.xml。
  cp "$APPDIR/usr/share/metainfo/com.aida.linux.metainfo.xml" \
    "$APPDIR/usr/share/metainfo/aida.appdata.xml"
fi
cp "$ROOT/packaging/aida.desktop" "$APPDIR/aida.desktop"

TOOL="$CACHE/linuxdeploy-${ARCH}.AppImage"
if [[ ! -x "$TOOL" ]]; then
  echo "==> download linuxdeploy"
  curl -L --fail --retry 4 --retry-delay 4 -o "$TOOL" "$LINUXDEPLOY_URL"
  chmod +x "$TOOL"
fi

# 容器里 FUSE 常不可用，强制解包运行。
export APPIMAGE_EXTRACT_AND_RUN=1
export LINUXDEPLOY_OUTPUT_VERSION="$VERSION"
export VERSION
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

dest="$OUT_DIR/AIDA_Linux-${VERSION}-${ARCH}.AppImage"
shopt -s nullglob
produced=()
for img in "$OUT_DIR"/AIDA_Linux*.AppImage "$OUT_DIR"/aida*.AppImage; do
  case "$(basename "$img")" in
    linuxdeploy*) continue ;;
  esac
  produced+=("$img")
done
shopt -u nullglob
if ((${#produced[@]} == 0)); then
  echo "linuxdeploy 没有产出 AppImage" >&2
  exit 1
fi
# 只保留一份带 Cargo 版本的名字；不要把旧文件 mv 到已写好的 dest 上。
keep=""
for img in "${produced[@]}"; do
  if [[ "$img" == "$dest" ]]; then
    keep="$img"
    break
  fi
done
if [[ -z "$keep" ]]; then
  mv -f "${produced[0]}" "$dest"
  keep="$dest"
fi
for img in "${produced[@]}"; do
  if [[ "$img" != "$keep" ]]; then
    rm -f "$img"
  fi
done
rm -f "$OUT_DIR"/linuxdeploy*.AppImage

echo "==> 校验 $dest version == $VERSION"
got="$(APPIMAGE_EXTRACT_AND_RUN=1 "$dest" version)"
echo "    $got"
if [[ "$got" != "aida $VERSION" ]]; then
  echo "AppImage 版本不是 $VERSION: $got" >&2
  exit 1
fi

echo "==> done"
ls -lh "$dest"
