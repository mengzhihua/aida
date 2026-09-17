# 打包与提权

## AppImage（GUI，glibc）

不要把 egui/glow 链到 musl 静态。发行桌面版用 linuxdeploy 收集 `.so`：

```bash
chmod +x scripts/build-appimage.sh
./scripts/build-appimage.sh
# 产物在 dist/*.AppImage
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/AIDA_Linux-*.AppImage gui
```

脚本会下载 [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy) continuous 构建。容器无 FUSE 时靠 `APPIMAGE_EXTRACT_AND_RUN=1`。

GUI 在运行时 `dlopen` `libxkbcommon-x11`。打包机请安装：

```bash
# Debian/Ubuntu
sudo apt install libxkbcommon-x11-0 libegl1 libgl1
```

脚本会把能找到的这些 `.so` 打进 AppImage。本 CI 镜像可能缺 `libxkbcommon-x11`，那时 AppImage 的 `collect`/`bench` 仍可用，GUI 需目标桌面自带该库。

安装系统图标/策略（可选，deb/rpm 包装时）：

```bash
sudo install -m 0644 packaging/polkit/com.aida.linux.policy \
  /usr/share/polkit-1/actions/
sudo install -m 0755 target/release/aida /usr/bin/aida
```

## 采集 CLI 静态链接（可选）

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --no-default-features --target x86_64-unknown-linux-musl
```

无 GUI、体积小，适合装进救援盘。

## 提权

| 方式 | 何时 |
| --- | --- |
| `aida elevate gui` / 界面按钮 | 桌面，优先 pkexec |
| `sudo -E aida gui` | 无 pkexec；必须 `-E` 保留 DISPLAY |
| `sudo aida collect` | 无显示器的服务器 |

pkexec 不在 PATH 时（只装了 `polkitd` 未装 `pkexec` 包）会退到 sudo，无 TTY 会失败。Debian/Ubuntu：`apt install pkexec`。
