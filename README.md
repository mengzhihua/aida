# AIDA Linux

开源 Linux 硬件检测与监控工具，对标 Windows [AIDA64](https://www.aida64.com/) 的常用能力：硬件信息、传感器监控、微基准、系统软件信息、报告导出。

**第四轮** 补齐内存细节、电源/电池、声卡、EFI/Secure Boot、CPU 漏洞缓解、每核利用率、磁盘 I/O 速率，以及 CPU/网卡/磁盘实时曲线。

| 模块 | 状态 |
| --- | --- |
| CPU / DMI / hwmon / NVMe / PCI / 块设备 / 软件 | 可读 sysfs/procfs |
| CPU 漏洞 / 每核利用率 | `vulnerabilities/*` + `/proc/stat` cpuN |
| GPU | DRM（amdgpu/i915/xe/nouveau）+ NVIDIA procfs，不调用 nvidia-smi |
| 网络 | `/sys/class/net` 计数 + `getifaddrs` 地址，不调用 `ip` |
| USB | `/sys/bus/usb/devices` 树，可选 `usb.ids` |
| 输入设备 | `/proc/bus/input/devices` |
| 内存 | `/proc/meminfo` + hugepages + THP |
| 电源 | `/sys/class/power_supply` |
| 声卡 | `/proc/asound/cards` |
| 固件 | EFI sysfs / Secure Boot efivar |
| NUMA | `/sys/devices/system/node/nodeN` |
| 磁盘 I/O | `/proc/diskstats` 差分 |
| 传感器告警 | hwmon `*_max`/`*_crit`/`*_min`，越限写 JSONL |
| 权限模型 | 每个字段带 `ok / permission_denied / not_found` |
| 提权 | `aida elevate` / GUI 按钮：pkexec，否则 sudo -E |
| egui 界面 | 左侧树 + 右侧详情 + 温度/CPU/磁盘/网络折线 |
| 微基准 | CPU / 内存带宽 / 磁盘 buffered + O_DIRECT |
| JSON / HTML 导出 | CLI + GUI |
| AppImage | `scripts/build-appimage.sh`（glibc + linuxdeploy） |

技术选型：**Rust + egui**，采集路径优先内核文件，不调用 `dmidecode`、`lspci`、`nvme-cli`、`smartctl`、`lscpu`。

## 思路（采集架构）

```
                  ┌──────────── GUI (egui) ────────────┐
                  │  树形菜单 / 详情表 / 折线图 / 导出  │
                  └───────────────┬────────────────────┘
                                  │ HardwareSnapshot
┌──────── CLI ────────┐           │
│ collect / bench     │───────────┤
└─────────────────────┘           │
                                  ▼
                         snapshot::collect
                                  │
        ┌─────────────┬───────────┼───────────┬────────────┬──────────┐
        ▼             ▼           ▼           ▼            ▼          ▼
      CPU           DMI        hwmon        NVMe        GPU/PCI     Net/USB
   /proc/cpuinfo  /sys/class   /sys/class  sysfs +     DRM + pci   sysfs +
   /sys/.../cpu   /dmi/id      /hwmon      ioctl       class 03    getifaddrs
                                  │
                                  ├── input / audio / power / firmware
                                  └── memory: /proc/meminfo + hugepages
```

约定：

1. **所有探测函数只读文件或发 ioctl**，把结果放进 `Sample<T>`，失败原因跟着字段走。
2. **`ProbeCtx` 把 `/proc` `/sys` `/dev` 做成可替换根**，单元测试用临时目录夹具，不 mock 整个操作系统。
3. **GUI 与 CLI 共用同一套 snapshot**，GUI 每 ~0.8s 刷新传感器、告警、网卡/磁盘速率、内存和 `/proc/stat`，不全量重扫 PCI/USB。

## 运行

需要 **Rust 1.88+**（GUI 依赖树含 edition 2024 与较新的 `icu`/`image`）。无显示器时请用 `--no-default-features` 只编采集 CLI。

```bash
# 采集 JSON（无显示器的服务器/CI 可直接用）
cargo run --release -- collect

# HTML 报告
cargo run --release -- collect --html aida-report.html

# 微基准（--quick 约 200ms，适合测试）
cargo run --release -- bench --quick

# 磁盘只跑 buffered
cargo run --release -- bench --disk --no-direct

# 桌面界面（需要 X11/Wayland）
cargo run --release -- gui

# 提权后重开 GUI（pkexec / sudo -E）
cargo run --release -- elevate gui
```

无 `DISPLAY`/`WAYLAND_DISPLAY` 时，裸跑 `aida` 会退化为 `collect`。

GUI 运行时依赖：X11 或 Wayland、OpenGL/EGL、`libxkbcommon`（X11 还要 `libxkbcommon-x11`）。采集 CLI 无此依赖。

无 GUI 的精简构建：

```bash
cargo build --release --no-default-features
```

## 权限

| 数据 | 普通用户 | root / 额外组 |
| --- | --- | --- |
| `/proc/cpuinfo`、`/proc/meminfo`、os-release | 通常可读 | — |
| `/sys/bus/pci/devices` | 通常可读 | 设备名依赖 `pci.ids` 包 |
| `/sys/block/*/size` | 通常可读 | — |
| `/sys/class/dmi/id/product_serial`、`product_uuid` | 多数发行版 `0400` | root |
| `/sys/firmware/dmi/tables/DMI` | 通常 `0400` | root |
| NVMe SMART（`NVME_IOCTL_ADMIN_CMD`） | `/dev/nvmeN` 常为 `0660 root:disk` | `disk` 组或 root |
| 部分 hwmon | 视 udev 规则 | 有时需 `lm_sensors` 相关规则 |

界面顶部有权限条：非 root 会明确提示哪些信息会缺。**缺权限时显示“权限不足”和路径，不填假数据。**

完整检测建议：

```bash
aida elevate gui
# 或：pkexec env DISPLAY=$DISPLAY XAUTHORITY=$XAUTHORITY $(command -v aida) gui
# 无 pkexec 时：sudo -E ./target/release/aida gui
```

策略文件：`packaging/polkit/com.aida.linux.policy`，安装方法见 [docs/PACKAGING.md](docs/PACKAGING.md)。

## 基准测试思路

| 项目 | 做法 | 坑 |
| --- | --- | --- |
| CPU | 多线程整数 LCG + 浮点 `mul_add`/`sin`，按墙钟时间计 Mops/MFLOPS | Turbo、CPU 亲和性、同机后台负载 |
| 内存 | STREAM 风格 copy / scale / triad | 编译器优化（已 `black_box`）、缓存大小、NUMA |
| 磁盘 | 先 buffered 顺序写+fsync+读，再 `O_DIRECT` 对齐 4KiB | tmpfs / 部分 overlay 会 EINVAL，代码回退并写明原因 |

这些是相对分，不是 SPEC、也不是 `fio`。

## 报告导出

- JSON：完整 `HardwareSnapshot`（含每个字段的 `access` / `source` / `hint`，以及 `net` / `usb` / `input` / `numa` / `alerts`）。
- HTML：单文件内嵌 CSS，表格展示摘要；字段值做了 `<>&` 转义。
- 告警日志：GUI 热刷新时把阈值状态变化追加到 `$AIDA_ALERT_LOG`，未设置则 `$XDG_STATE_HOME/aida/alerts.jsonl`（常见为 `~/.local/state/aida/alerts.jsonl`）。只在进入/离开越限时写一行，避免刷盘。

```bash
aida collect --json out.json --html out.html
```

## 跨发行版

见 [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。要点：不要假设 `/sys/class/dmi` 存在（容器/部分云主机没有）；不要假设有 `pci.ids`；ARM/RISC-V 上 CPU 字段名与 x86 不同，解析按键名而不是位置。

## 打包

见 [docs/PACKAGING.md](docs/PACKAGING.md)。GUI 走 glibc AppImage；CLI 可另编 musl。

```bash
./scripts/build-appimage.sh
```

## 开发

```bash
cargo test --no-default-features
cargo test --features gui   # 不启动窗口，只编进 ui 模块
```

模块入口：

- 权限原语：`src/access.rs`
- 探测：`src/probes/`（含 `net` / `usb` / `input` / `numa` / `memory` / `power` / `audio` / `firmware`）
- 快照：`src/snapshot.rs`
- 导出：`src/export.rs`
- 基准：`src/bench.rs`
- 界面：`src/ui/app.rs`
- 提权：`src/elevate.rs`
- 告警：`src/alerts.rs`
- 打包：`scripts/build-appimage.sh`、`packaging/`

架构说明：[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
