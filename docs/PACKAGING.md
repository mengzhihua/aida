# 打包与提权

`package.sh` 开始时清掉 `dist/` 里上一版的 CLI / AppImage / tar.gz，避免 `SHA256SUMS` 和自测扫到旧二进制。`make-bundle.sh` / `smoke-dist.sh` 只认当前 `Cargo.toml` 版本号。

```bash
./scripts/package.sh
ls -lh dist/
```

| 产物 | 脚本 | 说明 |
| --- | --- | --- |
| `dist/aida-linux-<ver>-<arch>.tar.gz` | `scripts/make-bundle.sh` | 解压即用：CLI + AppImage + `install.sh` + `INSTALL.txt` + `run-gui.sh` / `run-doctor.sh` |
| `dist/aida-cli-<ver>-<arch>-musl` 或 `-gnu` | `scripts/build-cli.sh` | 无 GUI。有 musl 工具链则静态，否则 glibc `--no-default-features` |
| `dist/AIDA_Linux-<ver>-<arch>.AppImage` | `scripts/build-appimage.sh` | GUI + CLI。版本取自 `Cargo.toml`；写出 `dist/GLIBC_GUI` |
| `dist/SHA256SUMS` | `scripts/make-bundle.sh` | 上述产物的 sha256 |
| `~/.local/bin/aida`（可选） | `install.sh` / `scripts/install.sh` | **不需要 cargo**：从 tar 或 `dist/` 拷贝 CLI/AppImage；`--deps` 按发行版装 GUI 库 |

GitHub Actions：`.github/workflows/package.yml` 在**每个 PR** 和 tag 上传 Artifact `aida-linux`（tar.gz + 二进制 + SHA256SUMS），并对成品跑 `scripts/smoke-dist.sh`（version / JSON / HTML / bench / 校验和 / 解压 tar）。`package.sh` 打完包也会跑同一套自测。

## 发版给用户（GitHub Release）

用户下载入口是 [Releases](https://github.com/mengzhihua/aida/releases)，不需要 Rust。

无问题的版本会自动发行：把 `Cargo.toml` 的 `version` 升一档，合并到 `main` 或 `cursor/rc-stm-peci-587c`。`package.yml` 发现还没有 `v$VERSION` tag 时，会再编一次、跑 `cargo test` 和 `smoke-dist.sh`，**全部通过才**用 `softprops/action-gh-release` 打 tag（`target_commitish` 为当前 SHA）并挂上成品。main 与栈顶共用一把 `concurrency` 锁；已经发过的版本不会覆盖别人的 tag。`./scripts/release.sh` 若本地同名 tag 不指向 HEAD 会直接失败。

也可以手工打 tag（不要用 `gh release create` 挂未经自测的文件）：

```bash
# 1. Cargo.toml 的 version 已改（例如 0.43.0）
# 2. 本地打包装并自测
./scripts/package.sh
# 3. 合并 PR 后打 tag（必须是 v 开头，且等于 Cargo.toml）
./scripts/release.sh --push
# 等价于：git tag v0.43.0 && git push origin v0.43.0
```

推送 `v*` tag 后同一套工作流再编一次、再跑 smoke，通过才挂：

- `aida-linux-<ver>-<arch>.tar.gz`（解压即用）
- `AIDA_Linux-<ver>-<arch>.AppImage`
- `aida-cli-<ver>-<arch>-musl`
- `SHA256SUMS`

不要用 `gh release create` 手工挂未经自测的文件。工作流 `ci` 跑单元测试与 `live_collect`。

## AppImage（GUI，glibc）

不要把 egui/glow 链到 musl 静态。发行桌面版用 linuxdeploy 收集 `.so`：

```bash
./scripts/build-appimage.sh
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/AIDA_Linux-*.AppImage gui
```

`VERSION` / `LINUXDEPLOY_OUTPUT_VERSION` 默认等于 `Cargo.toml` 的 `version`。linuxdeploy 下到 `.cache/`，不进 `dist/`。AppStream 的 `<release date>` 默认是 UTC 当天，可用 `DATE=` 或 `SOURCE_DATE_EPOCH` 覆盖。AppDir 只放 `com.aida.linux.metainfo.xml`，桌面文件用 `com.aida.linux.desktop`（与 component id 一致）。ubuntu-latest 的 `appstreamcli` 把 warning 当失败：文件名对不上 id 会 `metainfo-filename-cid-mismatch`；放两份会把同一条错误计两次。

linuxdeploy 在临时目录出包，`aida version` 对得上 `Cargo.toml` 之后才替换 `dist/AIDA_Linux-*.AppImage`。cargo 或 linuxdeploy 失败时保留上次成功的包。`package.sh` 只把这一次成功的 AppImage 写进 `SHA256SUMS`。

容器无 FUSE 时必须 `APPIMAGE_EXTRACT_AND_RUN=1`。

GUI 在运行时 `dlopen` `libxkbcommon-x11`。打包机请安装：

```bash
# Debian/Ubuntu
sudo apt-get install -y libxkbcommon-x11-0 libegl1 libgl1
# CentOS / RHEL / Rocky / Fedora
sudo dnf install -y libxkbcommon-x11 mesa-libEGL mesa-libGL || \
  sudo yum install -y libxkbcommon-x11 mesa-libEGL mesa-libGL
```

脚本会把能找到的这些 `.so` 打进 AppImage。构建机缺库时，`collect`/`bench` 仍可用，GUI 需目标桌面自带该库。`build-appimage.sh` 会扫描 GUI ELF 里最高的 `GLIBC_*` 符号，写入 `dist/GLIBC_GUI`（当前 GitHub `ubuntu-latest` / Ubuntu 24.04 常见 **2.39**）。低于该版本的发行版（CentOS 7=2.17、Rocky 8=2.28、Ubuntu 22.04=2.35）请用 musl CLI，不要指望 AppImage。

## 用户安装（开箱，无 Rust）

tar.gz 里带 `install.sh` 和 `os-family.sh`，读 `/etc/os-release` 的 `ID` / `ID_LIKE`：

| family | 系统 | `--deps` |
| --- | --- | --- |
| debian | Ubuntu、Debian、Mint | `apt-get`：`libxkbcommon-x11-0 libegl1 libgl1 pkexec` |
| rhel | CentOS、RHEL、Rocky、Alma、Fedora | `dnf`，没有则 `yum`：`libxkbcommon-x11 mesa-libEGL mesa-libGL polkit` |
| suse | openSUSE / SLES | `zypper` |
| arch | Arch / Manjaro | `pacman` |

```bash
# 解压 Release / Artifact 后
./aida-cli doctor
./install.sh                  # → ~/.local/bin/aida （采集走 musl CLI，gui 走 AppImage）
./install.sh --deps
sudo ./install.sh --prefix /usr

# 源码树：先 package 再装；开发机才需要 --from-source
./scripts/package.sh
./scripts/install.sh
./scripts/install.sh --from-source
```

`aida doctor` 不调用 `lsb_release` / `ldd` / `hostnamectl`。

## 采集 CLI 静态链接

```bash
./scripts/build-cli.sh
# 等价于：
rustup target add x86_64-unknown-linux-musl
cargo build --release --no-default-features --target x86_64-unknown-linux-musl
```

无 GUI、体积小，适合装进救援盘。没有 `musl-gcc` 时脚本会退回本机 glibc CLI，并在文件名里标 `gnu`。

## 提权

| 方式 | 何时 |
| --- | --- |
| `aida elevate gui` / 界面按钮 | 桌面，优先 pkexec |
| `sudo -E aida gui` | 无 pkexec；必须 `-E` 保留 DISPLAY |
| `sudo aida collect` | 无显示器的服务器 |

pkexec 不在 PATH 时（只装了 `polkitd` 未装 `pkexec` 包）会退到 sudo，无 TTY 会失败。Debian/Ubuntu：`apt-get install pkexec`。RHEL 系：`dnf install polkit`（提供 `pkexec`）。
