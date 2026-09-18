# 打包与提权

一次打出可带走的 CLI 和桌面 AppImage：

```bash
./scripts/package.sh
ls -lh dist/
```

| 产物 | 脚本 | 说明 |
| --- | --- | --- |
| `dist/aida-cli-<ver>-<arch>-musl` 或 `-gnu` | `scripts/build-cli.sh` | 无 GUI。有 musl 工具链则静态，否则 glibc `--no-default-features` |
| `dist/AIDA_Linux-<ver>-<arch>.AppImage` | `scripts/build-appimage.sh` | GUI + CLI。版本取自 `Cargo.toml` |
| `dist/SHA256SUMS` | `scripts/package.sh` | 上述产物的 sha256 |
| `~/.local/bin/aida`（可选） | `scripts/install.sh` | 本机安装桌面文件与图标；菜单 `Exec` 写成绝对路径 |

GitHub Actions：`.github/workflows/package.yml` 在 PR / tag 上传上述产物。tag `v*` 时再挂到 GitHub Release。工作流 `ci` 跑单元测试与 `live_collect`。

## AppImage（GUI，glibc）

不要把 egui/glow 链到 musl 静态。发行桌面版用 linuxdeploy 收集 `.so`：

```bash
./scripts/build-appimage.sh
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/AIDA_Linux-*.AppImage gui
```

`VERSION` / `LINUXDEPLOY_OUTPUT_VERSION` 默认等于 `Cargo.toml` 的 `version`。linuxdeploy 下到 `.cache/`，不进 `dist/`。

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
