#!/usr/bin/env bash
# 无 GUI 采集 CLI。优先 musl 静态，没有工具链则退回 glibc。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=version.sh
source "$ROOT/scripts/version.sh"
ARCH="$(aida_arch)"
VERSION="${VERSION:-$(aida_version "$ROOT")}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"

cd "$ROOT"

abi="gnu"
target=""
musl_cc=""
case "$ARCH" in
  x86_64) target="x86_64-unknown-linux-musl" ;;
  aarch64) target="aarch64-unknown-linux-musl" ;;
esac

if command -v rustup >/dev/null 2>&1 && [[ -n "$target" ]]; then
  if command -v musl-gcc >/dev/null 2>&1; then
    musl_cc="musl-gcc"
  elif [[ -x "/usr/bin/${ARCH}-linux-musl-gcc" ]]; then
    musl_cc="/usr/bin/${ARCH}-linux-musl-gcc"
  fi
fi

if [[ -n "$musl_cc" ]]; then
  echo "==> rustup target add $target"
  rustup target add "$target" >/dev/null
  export CC="$musl_cc"
  echo "==> cargo build --release --no-default-features --target $target (CC=$CC)"
  cargo build --release --no-default-features --target "$target"
  src="$ROOT/target/$target/release/aida"
  abi="musl"
else
  echo "note: 没有 musl-gcc，改为本机 glibc CLI（--no-default-features）" >&2
  cargo build --release --no-default-features
  src="$ROOT/target/release/aida"
fi

if [[ ! -x "$src" ]]; then
  echo "未找到 CLI 二进制: $src" >&2
  exit 1
fi

dest="$OUT_DIR/aida-cli-${VERSION}-${ARCH}-${abi}"
install -m 0755 "$src" "$dest"
# 方便脚本固定引用
ln -sfn "$(basename "$dest")" "$OUT_DIR/aida-cli"
echo "==> $dest"
ls -lh "$dest"
"$dest" version
