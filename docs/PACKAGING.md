# 打包与提权

每一轮开发结束都要打出可直接使用的安装包：

```bash
./scripts/package.sh
ls -lh dist/
```

| 产物 | 脚本 | 说明 |
| --- | --- | --- |
| `dist/aida-linux-<ver>-<arch>.tar.gz` | `scripts/make-bundle.sh` | 解压即用：CLI + AppImage + `INSTALL.txt` + `run-gui.sh` |
| `dist/aida-cli-<ver>-<arch>-musl` 或 `-gnu` | `scripts/build-cli.sh` | 无 GUI。有 musl 工具链则静态，否则 glibc `--no-default-features` |
| `dist/AIDA_Linux-<ver>-<arch>.AppImage` | `scripts/build-appimage.sh` | GUI + CLI。版本取自 `Cargo.toml` |
| `dist/SHA256SUMS` | `scripts/make-bundle.sh` | 上述产物的 sha256 |
| `~/.local/bin/aida`（可选） | `scripts/install.sh` | 本机安装桌面文件与图标；菜单 `Exec` 写成绝对路径 |

GitHub Actions：`.github/workflows/package.yml` 在**每个 PR** 和 tag 上传 Artifact `aida-linux`（tar.gz + 二进制 + SHA256SUMS），并对成品跑 `scripts/smoke-dist.sh`（version / JSON / HTML / bench / 校验和 / 解压 tar）。`package.sh` 打完包也会跑同一套自测。

## 发版给用户（GitHub Release）

用户下载入口是 [Releases](https://github.com/mengzhihua/aida/releases)，不需要 Rust。

```bash
# 1. Cargo.toml 的 version 已改（例如 0.40.0）
# 2. 本地打包装并自测
./scripts/package.sh
# 3. 合并 PR 后打 tag（必须是 v 开头）
git tag v0.40.0
git push origin v0.40.0
```

推送 `v*` tag 后 `package` 工作流会再编一次、再跑 `smoke-dist.sh`，**全部通过才**用 `softprops/action-gh-release` 挂上：

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
sudo apt install libxkbcommon-x11-0 libegl1 libgl1
```

脚本会把能找到的这些 `.so` 打进 AppImage。构建机缺库时，`collect`/`bench` 仍可用，GUI 需目标桌面自带该库。

安装系统图标/策略（deb/rpm 或 `PREFIX=/usr`）：

```bash
sudo PREFIX=/usr ./scripts/install.sh
# 或手工：
sudo install -m 0644 packaging/polkit/com.aida.linux.policy \
  /usr/share/polkit-1/actions/
sudo install -m 0755 target/release/aida /usr/bin/aida
```

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

pkexec 不在 PATH 时（只装了 `polkitd` 未装 `pkexec` 包）会退到 sudo，无 TTY 会失败。Debian/Ubuntu：`apt install pkexec`。
