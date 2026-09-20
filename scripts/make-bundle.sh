#!/usr/bin/env bash
# 把 dist/ 里已经打好的 CLI / AppImage 收成一份可直接拷走的 tar.gz。
# 不重新编译。CI 的 bundle 作业和本地 package.sh 都走这里。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
ARCH="${ARCH:-$(aida_arch)}"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
BUNDLE_NAME="aida-linux-${VERSION}-${ARCH}"
STAGE="$OUT_DIR/$BUNDLE_NAME"

mkdir -p "$OUT_DIR"
rm -rf "$STAGE"
mkdir -p "$STAGE"

shopt -s nullglob
cli_files=("$OUT_DIR"/aida-cli-[0-9]*)
if [[ "${AIDA_BUNDLE_SKIP_APPIMAGE:-}" == 1 ]]; then
  app_files=()
else
  app_files=("$OUT_DIR"/AIDA_Linux-*.AppImage)
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
cat >"$STAGE/run-gui.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
img="$DIR/AIDA_Linux.AppImage"
if [[ ! -e "$img" ]]; then
  echo "这个包没有桌面 AppImage" >&2
  exit 1
fi
export APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}"
exec "$img" gui "$@"
EOF
chmod +x "$STAGE/run-collect.sh" "$STAGE/run-gui.sh"

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
  for f in aida-cli-[0-9]* AIDA_Linux-*.AppImage "${BUNDLE_NAME}.tar.gz"; do
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
