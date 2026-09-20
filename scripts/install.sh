#!/usr/bin/env bash
# 开箱即用安装：不需要 Rust。
#
# 解压 GitHub Release / Artifact 的 tar.gz 后：
#   ./install.sh
#   ./install.sh --deps                 # 顺带装 GUI 运行库（apt 或 dnf/yum）
#   ./install.sh --prefix /usr/local
#
# 源码树：
#   ./scripts/install.sh                # 优先装 dist/ 里已打好的包
#   ./scripts/install.sh --from-source  # cargo build --features gui（开发机）
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
if [[ -f "$HERE/os-family.sh" ]]; then
  # shellcheck source=os-family.sh
  source "$HERE/os-family.sh"
elif [[ -f "$HERE/scripts/os-family.sh" ]]; then
  # shellcheck source=os-family.sh
  source "$HERE/scripts/os-family.sh"
else
  echo "找不到 os-family.sh（应与 install.sh 放在一起）" >&2
  exit 1
fi

if [[ -f "$HERE/../Cargo.toml" && "$(basename "$HERE")" == "scripts" ]]; then
  ROOT="$(cd "$HERE/.." && pwd)"
  KIND=source
elif [[ -f "$HERE/Cargo.toml" ]]; then
  ROOT="$HERE"
  KIND=source
else
  ROOT="$HERE"
  KIND=bundle
fi

PREFIX="${PREFIX:-$HOME/.local}"
FROM_SOURCE=0
WANT_DEPS=0
DEPS_ONLY=0

usage() {
  cat <<EOF
AIDA Linux 安装（Ubuntu/Debian 与 CentOS/RHEL/Rocky/Fedora 都能用）

用法:
  $0 [--prefix DIR] [--deps] [--deps-only] [--from-source]

  --prefix DIR     安装前缀（默认 \$HOME/.local；系统级用 --prefix /usr）
  --deps           用本机包管理器安装 GUI 运行库（apt-get / dnf / yum / zypper / pacman）
  --deps-only      只装运行库，不拷贝二进制
  --from-source    源码树里 cargo build --release --features gui（需要 Rust 1.88+）
  -h, --help       本说明

不需要 cargo：解压 tar.gz 后直接 ./install.sh。
采集在 CentOS 7 及以上 / Ubuntu 16.04 及以上用 musl CLI；桌面 AppImage 需要较新 glibc（见 GLIBC_GUI）。
先体检：./aida-cli doctor   或   ./run-doctor.sh
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="${2:-}"
      [[ -n "$PREFIX" ]] || { echo "--prefix 需要目录" >&2; exit 2; }
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#--prefix=}"
      [[ -n "$PREFIX" ]] || { echo "--prefix 需要目录" >&2; exit 2; }
      shift
      ;;
    --deps) WANT_DEPS=1; shift ;;
    --deps-only) DEPS_ONLY=1; WANT_DEPS=1; shift ;;
    --from-source) FROM_SOURCE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "未知参数: $1" >&2
      usage
      exit 2
      ;;
  esac
done

BIN="$PREFIX/bin"
LIB="$PREFIX/lib/aida"
APP="$PREFIX/share/applications"
ICON="$PREFIX/share/icons/hicolor/scalable/apps"
META="$PREFIX/share/metainfo"
POLKIT="$PREFIX/share/polkit-1/actions"

FAMILY="$(aida_os_family)"
PRETTY="$(aida_os_release_field PRETTY_NAME || true)"
OS_ID="$(aida_os_release_field ID || true)"
echo "==> 本机 ${PRETTY:-?}  id=${OS_ID:-?}  family=$FAMILY"
echo "==> prefix $PREFIX"

install_gui_deps() {
  echo "==> GUI 运行库 family=$FAMILY"
  echo "    $(aida_gui_install_cmd "$FAMILY")"
  local sudo_cmd=()
  if [[ "$(id -u)" -ne 0 ]]; then
    if command -v sudo >/dev/null 2>&1; then
      sudo_cmd=(sudo)
    else
      echo "note: 需要 root 或 sudo 才能装库。请手工执行：" >&2
      echo "  $(aida_gui_install_cmd "$FAMILY")" >&2
      return 0
    fi
  fi
  set +e
  case "$FAMILY" in
    debian)
      "${sudo_cmd[@]}" apt-get install -y libxkbcommon-x11-0 libegl1 libgl1 pkexec
      ;;
    rhel)
      if command -v dnf >/dev/null 2>&1; then
        "${sudo_cmd[@]}" dnf install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit
      else
        "${sudo_cmd[@]}" yum install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit
      fi
      ;;
    suse)
      "${sudo_cmd[@]}" zypper install -y libxkbcommon-x11-0 Mesa-libEGL1 Mesa-libGL1 polkit
      ;;
    arch)
      "${sudo_cmd[@]}" pacman -S --needed --noconfirm libxkbcommon mesa polkit
      ;;
    alpine)
      "${sudo_cmd[@]}" apk add --no-cache mesa-egl mesa-gl libxkbcommon libxkbcommon-x11 polkit
      ;;
    *)
      echo "note: 未能识别发行版，请按 README 手工安装 OpenGL/EGL 与 libxkbcommon" >&2
      set -e
      return 0
      ;;
  esac
  local st=$?
  set -e
  if [[ $st -ne 0 ]]; then
    echo "note: 包管理器未成功（缺权限 / 无网）。请手工执行：" >&2
    echo "  $(aida_gui_install_cmd "$FAMILY")" >&2
    return 0
  fi
}

if [[ "$WANT_DEPS" -eq 1 ]]; then
  install_gui_deps
fi
if [[ "$DEPS_ONLY" -eq 1 ]]; then
  echo "只安装了运行库。"
  exit 0
fi

CLI_SRC=""
APP_SRC=""
GUI_SRC=""
PACK="$ROOT/packaging"

if [[ "$KIND" == "source" && "$FROM_SOURCE" -eq 1 ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "没有 cargo。解压 Release tar.gz 后跑 ./install.sh，或先 ./scripts/package.sh" >&2
    exit 1
  fi
  echo "==> cargo build --release --features gui"
  (cd "$ROOT" && cargo build --release --features gui)
  GUI_SRC="$ROOT/target/release/aida"
elif [[ "$KIND" == "bundle" ]]; then
  [[ -x "$ROOT/aida-cli" ]] && CLI_SRC="$ROOT/aida-cli"
  if [[ -e "$ROOT/AIDA_Linux.AppImage" ]]; then
    APP_SRC="$ROOT/AIDA_Linux.AppImage"
  fi
else
  # 源码树默认用 dist/ 成品，避免强迫用户装 Rust。
  if [[ -x "$ROOT/dist/aida-cli" ]]; then
    CLI_SRC="$ROOT/dist/aida-cli"
  else
    shopt -s nullglob
    local_cli=("$ROOT"/dist/aida-cli-*)
    shopt -u nullglob
    if ((${#local_cli[@]})); then
      CLI_SRC="${local_cli[0]}"
    fi
  fi
  shopt -s nullglob
  local_app=("$ROOT"/dist/AIDA_Linux-*.AppImage)
  shopt -u nullglob
  if ((${#local_app[@]})); then
    APP_SRC="${local_app[0]}"
  fi
  if [[ -z "$CLI_SRC" && -z "$APP_SRC" ]]; then
    if command -v cargo >/dev/null 2>&1; then
      echo "dist/ 没有成品包，改为 cargo build（也可用 --from-source）"
      (cd "$ROOT" && cargo build --release --features gui)
      GUI_SRC="$ROOT/target/release/aida"
    else
      echo "没有可安装的二进制，也没有 cargo。" >&2
      echo "请从 GitHub Releases 下载 aida-linux-*.tar.gz，解压后 ./install.sh" >&2
      echo "或在有 Rust 的机器上先跑 ./scripts/package.sh" >&2
      exit 1
    fi
  fi
fi

mkdir -p "$BIN" "$LIB"

# 本次没带的旧产物不要留着，避免 0.41 GUI 跟 0.42 CLI 混用。
rm -f "$BIN/aida-cli" "$BIN/aida-gui-bin" \
  "$LIB/AIDA_Linux.AppImage" "$LIB/GLIBC_GUI"

if [[ -n "$CLI_SRC" ]]; then
  install -m 0755 "$CLI_SRC" "$BIN/aida-cli"
  echo "cli:      $BIN/aida-cli"
fi

if [[ -n "$APP_SRC" ]]; then
  mkdir -p "$LIB"
  install -m 0755 "$APP_SRC" "$LIB/AIDA_Linux.AppImage"
  echo "appimage: $LIB/AIDA_Linux.AppImage"
fi

if [[ -n "$GUI_SRC" ]]; then
  install -m 0755 "$GUI_SRC" "$BIN/aida-gui-bin"
  echo "gui bin:  $BIN/aida-gui-bin"
fi

GLIBC_SRC=""
if [[ -f "$ROOT/GLIBC_GUI" ]]; then
  GLIBC_SRC="$ROOT/GLIBC_GUI"
elif [[ -f "$PACK/GLIBC_GUI" ]]; then
  GLIBC_SRC="$PACK/GLIBC_GUI"
elif [[ -f "$ROOT/packaging/GLIBC_GUI" ]]; then
  GLIBC_SRC="$ROOT/packaging/GLIBC_GUI"
elif [[ -f "$ROOT/dist/GLIBC_GUI" ]]; then
  GLIBC_SRC="$ROOT/dist/GLIBC_GUI"
fi
if [[ -n "$GLIBC_SRC" ]]; then
  mkdir -p "$LIB"
  install -m 0644 "$GLIBC_SRC" "$LIB/GLIBC_GUI"
fi

# 统一入口：采集走 musl/CLI（CentOS 也能跑）；gui / elevate gui 走 AppImage 或本机编的 GUI。
# elevate 必须进 GUI 二进制：musl CLI 没有 gui feature，pkexec 后 current_exe 会变成 aida-cli。
cat >"$BIN/aida" <<EOF
#!/usr/bin/env bash
set -euo pipefail
BIN_DIR="\$(cd "\$(dirname "\$0")" && pwd)"
CLI="\$BIN_DIR/aida-cli"
GUI_BIN="\$BIN_DIR/aida-gui-bin"
APPIMAGE="${LIB}/AIDA_Linux.AppImage"
cmd="\${1:-}"

run_gui() {
  export APPIMAGE_EXTRACT_AND_RUN="\${APPIMAGE_EXTRACT_AND_RUN:-1}"
  if [[ -x "\$GUI_BIN" ]]; then
    exec "\$GUI_BIN" "\$@"
  fi
  if [[ -e "\$APPIMAGE" ]]; then
    exec "\$APPIMAGE" "\$@"
  fi
  echo "没有桌面 GUI。采集请用: \$CLI collect" >&2
  echo "先跑: \$CLI doctor   （CentOS 等旧 glibc 请只用 CLI）" >&2
  return 1
}

if [[ "\$cmd" == "elevate" ]]; then
  shift
  inner="\${1:-gui}"
  if [[ "\$inner" == "gui" ]]; then
    run_gui elevate "\$@"
    exit 1
  fi
  if [[ -x "\$CLI" ]]; then
    exec "\$CLI" elevate "\$@"
  fi
  run_gui elevate "\$@"
  exit 1
fi

want_gui=0
if [[ -z "\$cmd" ]]; then
  if [[ -n "\${DISPLAY:-}" || -n "\${WAYLAND_DISPLAY:-}" ]]; then
    want_gui=1
  fi
elif [[ "\$cmd" == "gui" ]]; then
  want_gui=1
  shift
fi
if [[ "\$want_gui" -eq 1 ]]; then
  if [[ -n "\$cmd" && "\$cmd" == "gui" ]]; then
    run_gui gui "\$@"
  else
    run_gui
  fi
  [[ -x "\$CLI" ]] && exec "\$CLI" "\$@"
  exit 1
fi
if [[ -x "\$CLI" ]]; then
  exec "\$CLI" "\$@"
fi
run_gui "\$@"
echo "未找到 aida 二进制" >&2
exit 1
EOF
chmod 0755 "$BIN/aida"

DESKTOP_SRC=""
ICON_SRC=""
META_SRC=""
POLICY_SRC=""
if [[ -d "$PACK" ]]; then
  [[ -f "$PACK/aida.desktop" ]] && DESKTOP_SRC="$PACK/aida.desktop"
  [[ -f "$PACK/aida.svg" ]] && ICON_SRC="$PACK/aida.svg"
  [[ -f "$PACK/com.aida.linux.metainfo.xml" ]] && META_SRC="$PACK/com.aida.linux.metainfo.xml"
  [[ -f "$PACK/polkit/com.aida.linux.policy" ]] && POLICY_SRC="$PACK/polkit/com.aida.linux.policy"
fi

if [[ -n "$DESKTOP_SRC" ]]; then
  mkdir -p "$APP" "$ICON"
  sed "s|^Exec=aida |Exec=$BIN/aida |" "$DESKTOP_SRC" >"$APP/com.aida.linux.desktop"
  if [[ -n "$ICON_SRC" ]]; then
    install -m 0644 "$ICON_SRC" "$ICON/aida.svg"
  fi
  echo "desktop:  $APP/com.aida.linux.desktop"
fi

if [[ -n "$META_SRC" ]]; then
  mkdir -p "$META"
  VERSION="unknown"
  DATE="$(date -u +%Y-%m-%d)"
  if [[ -x "$BIN/aida-cli" ]]; then
    VERSION="$("$BIN/aida-cli" version 2>/dev/null | awk '{print $2}')"
  elif [[ -x "$BIN/aida" ]]; then
    VERSION="$("$BIN/aida" version 2>/dev/null | awk '{print $2}')"
  fi
  VERSION="${VERSION:-unknown}"
  sed -e "s/@VERSION@/${VERSION}/g" -e "s/@DATE@/${DATE}/g" \
    "$META_SRC" >"$META/com.aida.linux.metainfo.xml"
fi

if [[ -n "$POLICY_SRC" ]]; then
  if [[ -w "$(dirname "$POLKIT")" ]] || [[ -w "$POLKIT" ]] || [[ "$(id -u)" -eq 0 ]]; then
    mkdir -p "$POLKIT"
    install -m 0644 "$POLICY_SRC" "$POLKIT/com.aida.linux.policy"
    echo "polkit:   $POLKIT/com.aida.linux.policy"
  else
    echo "note: polkit 策略需要写 $POLKIT（用 --prefix /usr 并 sudo）"
  fi
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP" >/dev/null 2>&1 || true
fi

echo
echo "installed: $BIN/aida"
echo "run:       $BIN/aida doctor"
echo "           $BIN/aida collect --html report.html"
echo "           $BIN/aida collect --format text > report.txt"
echo "           $BIN/aida collect --csv report.csv --md report.md"
if [[ -n "$APP_SRC" || -n "$GUI_SRC" ]]; then
  echo "           $BIN/aida gui"
fi
if [[ ":$PATH:" != *":$BIN:"* ]]; then
  echo "note: 把 $BIN 加进 PATH，例如 echo 'export PATH=\"$BIN:\$PATH\"' >> ~/.bashrc"
fi
echo "GUI 库未齐时：$BIN/aida doctor ，或 $0 --deps"
echo "  $(aida_gui_install_cmd "$FAMILY")"
