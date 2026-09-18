#!/usr/bin/env bash
# 把 GUI 二进制、桌面文件、图标装到 PREFIX（默认 ~/.local）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin"
APP="$PREFIX/share/applications"
ICON="$PREFIX/share/icons/hicolor/scalable/apps"
META="$PREFIX/share/metainfo"
POLKIT="$PREFIX/share/polkit-1/actions"

cd "$ROOT"
# 桌面入口必须是 GUI feature；不要把先前打的 CLI-only 二进制装进菜单。
echo "==> cargo build --release --features gui  (version $VERSION)"
cargo build --release --features gui

mkdir -p "$BIN" "$APP" "$ICON" "$META"
install -m 0755 "$ROOT/target/release/aida" "$BIN/aida"
# 菜单启动用绝对路径，不依赖 ~/.local/bin 是否在 PATH。
sed "s|^Exec=aida |Exec=$BIN/aida |" "$ROOT/packaging/aida.desktop" >"$APP/aida.desktop"
install -m 0644 "$ROOT/packaging/aida.svg" "$ICON/aida.svg"
sed "s/@VERSION@/${VERSION}/g" "$ROOT/packaging/com.aida.linux.metainfo.xml" \
  >"$META/com.aida.linux.metainfo.xml"

# polkit 只读系统目录；用户级 PREFIX 装了也不会生效。
if [[ -w "$(dirname "$POLKIT")" ]] || [[ -w "$POLKIT" ]]; then
  mkdir -p "$POLKIT"
  install -m 0644 "$ROOT/packaging/polkit/com.aida.linux.policy" \
    "$POLKIT/com.aida.linux.policy"
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP" >/dev/null 2>&1 || true
fi

echo "installed: $BIN/aida"
echo "desktop:   $APP/aida.desktop"
echo "run:       $BIN/aida gui"
if [[ ":$PATH:" != *":$BIN:"* ]]; then
  echo "note: 把 $BIN 加进 PATH，例如 echo 'export PATH=\"$BIN:\$PATH\"' >> ~/.profile"
fi
