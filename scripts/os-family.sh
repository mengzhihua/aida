#!/usr/bin/env bash
# 从 /etc/os-release 判断发行版族。不调用 lsb_release / hostnamectl。
# 给 install.sh / run-gui.sh 用：Ubuntu/Debian → debian；CentOS/RHEL/Rocky → rhel。

aida_os_release_field() {
  local key="$1"
  local file="${2:-/etc/os-release}"
  [[ -r "$file" ]] || return 0
  local line val
  while IFS= read -r line || [[ -n "$line" ]]; do
    case "$line" in
      "${key}="*)
        val="${line#*=}"
        val="${val%\"}"
        val="${val#\"}"
        val="${val%\'}"
        val="${val#\'}"
        printf '%s' "$val"
        return 0
        ;;
    esac
  done <"$file"
}

aida_os_family() {
  local id like blob tok
  id="$(aida_os_release_field ID "${1:-/etc/os-release}" | tr '[:upper:]' '[:lower:]')"
  like="$(aida_os_release_field ID_LIKE "${1:-/etc/os-release}" | tr '[:upper:]' '[:lower:]')"
  blob="$id $like"
  for tok in $blob; do
    tok="${tok%,}"
    case "$tok" in
      debian|ubuntu|linuxmint|pop|raspbian|elementary|kali) echo debian; return 0 ;;
      rhel|centos|fedora|rocky|alma|almalinux|ol|amzn|scientific|redhat|anolis|opencloudos|kylin|uos)
        echo rhel; return 0 ;;
      suse|opensuse|sles|opensuse-leap|opensuse-tumbleweed) echo suse; return 0 ;;
      arch|manjaro|endeavouros|archlinux) echo arch; return 0 ;;
      alpine) echo alpine; return 0 ;;
    esac
  done
  echo unknown
}

aida_gui_install_cmd() {
  local family="${1:-$(aida_os_family)}"
  case "$family" in
    debian)
      echo "sudo apt-get install -y libxkbcommon-x11-0 libegl1 libgl1 pkexec"
      ;;
    rhel)
      echo "sudo dnf install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit || sudo yum install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit"
      ;;
    suse)
      echo "sudo zypper install -y libxkbcommon-x11-0 Mesa-libEGL1 Mesa-libGL1 polkit"
      ;;
    arch)
      echo "sudo pacman -S --needed libxkbcommon mesa polkit"
      ;;
    alpine)
      echo "sudo apk add --no-cache mesa-egl mesa-gl libxkbcommon libxkbcommon-x11 polkit"
      ;;
    *)
      echo "需要 OpenGL/EGL 与 libxkbcommon（含 x11）；发行版包名见 README"
      ;;
  esac
}
