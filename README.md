# AIDA Linux

开源 Linux 硬件检测与监控工具，对标 Windows [AIDA64](https://www.aida64.com/) 和 macOS iStat Menus 的常用能力：硬件信息、传感器监控、状态栏、微基准、JSON/HTML 报告。

采集只走内核文件（`/proc` `/sys` `/dev`）和少量 ioctl，**不调用** `dmidecode`、`lspci`、`smartctl`、`nvme-cli`、`lshw` 等外部命令。GUI 与 CLI 共用同一套快照。

| 你想做的事 | 命令 |
| --- | --- |
| 桌面界面 | `aida gui` |
| 采集 JSON | `aida collect` |
| HTML 报告 | `aida collect --html aida-report.html` |
| 微基准 | `aida bench --quick` |
| 以管理员重开 GUI | `aida elevate gui` |

## 能做什么

- **硬件与拓扑**：CPU（拓扑 / cpuidle / 漏洞 / 利用率）、DMI、PCI/PCIe、NVMe、GPU/DRM、USB、输入设备、NUMA、virtio / KVM / IOMMU
- **传感器**：hwmon + thermal，阈值告警写 JSONL；RAPL 瓦特、PSI、EDAC
- **状态栏**：窗口内常驻 CPU / 内存 / 网络 / 磁盘 / 温度 / loadavg；可选置顶窄条（对标 iStat Menus，不引入托盘库）
- **记录**：GUI 开始/停止，把每次采样追加到 `$AIDA_RECORD_LOG` 或 `~/.local/state/aida/history.jsonl`
- **存储与总线**：块设备 / MD / SCSI / iSCSI / NBD / zram / zswap，以及 rfkill、串口、HID、GPIO 等 leftover class（空 = 无硬件，不是失败）
- **内核与网络**：sysctl、cgroup、lockdown、conntrack、TCP/IPv6 knobs（缺权限标 `permission_denied`，不填假数据）
- **导出**：JSON（含每个字段的 `access` / `source` / `hint`）和单文件 HTML
- **基准**：CPU / 内存 / 磁盘相对分（`--quick` 约 200ms）

刻意未做：GPU OpenCL/Vulkan 计算基准（会引入额外运行时，和可打包目标冲突）。

## 安装

需要 **Rust 1.88+**。无显示器的机器请编 CLI（`--no-default-features`）。

### 从源码（开发机 / 桌面）

```bash
cargo build --release
./target/release/aida --help
./target/release/aida gui          # 需要 X11 或 Wayland
```

装到用户目录（桌面文件 + 图标）：

```bash
./scripts/install.sh
# 或：PREFIX=/usr/local sudo ./scripts/install.sh
```

### 打好的包（给以后用）

```bash
./scripts/package.sh
ls dist/
```

产物：

| 文件 | 用途 |
| --- | --- |
| `aida-cli-<ver>-<arch>-musl` 或 `-gnu` | 无 GUI 采集/基准。优先 musl 静态，没有 musl 工具链则退回 glibc |
| `AIDA_Linux-<ver>-<arch>.AppImage` | 桌面 GUI + CLI。容器无 FUSE 时加 `APPIMAGE_EXTRACT_AND_RUN=1` |
| `SHA256SUMS` | 上述产物的 sha256，拷走后可 `sha256sum -c` |

```bash
# 服务器 / CI
./dist/aida-cli collect --html report.html

# 桌面
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/AIDA_Linux-*.AppImage gui
```

现成包：

- [GitHub Releases](https://github.com/mengzhihua/aida/releases)（打 `v*` tag 后自动挂上）
- 任意 PR：Actions 工作流 `package` 的 Artifacts

GitHub Actions 工作流 [`.github/workflows/package.yml`](.github/workflows/package.yml) 会在 PR 和 tag 时上传同样的产物。

### 只要采集 CLI

```bash
cargo build --release --no-default-features
# 或静态：
./scripts/build-cli.sh
```

## 使用

```bash
aida                         # 有图形会话则 GUI，否则打印 JSON
aida collect                 # JSON 到 stdout
aida collect --json out.json --html out.html
aida bench --quick
aida bench --disk --no-direct
aida gui
aida elevate gui             # pkexec；没有则 sudo -E
aida version
```

无 `DISPLAY` / `WAYLAND_DISPLAY` 时，裸跑 `aida` 会退化为 `collect`。

GUI 运行时需要 OpenGL/EGL 和 `libxkbcommon`（X11 还要 `libxkbcommon-x11`）。采集 CLI 无此依赖。

环境变量：

| 变量 | 默认 | 含义 |
| --- | --- | --- |
| `AIDA_ALERT_LOG` | `$XDG_STATE_HOME/aida/alerts.jsonl` | 传感器越限 JSONL |
| `AIDA_RECORD_LOG` | `$XDG_STATE_HOME/aida/history.jsonl` | 状态栏历史 JSONL |

## 权限

缺权限时字段标记为 `permission_denied` 并给出路径，**不会伪造数据**。界面顶部有权限条。

| 数据 | 普通用户 | 通常需要 root / disk 组 |
| --- | --- | --- |
| cpuinfo、meminfo、os-release、PCI、块设备容量、网卡、USB sysfs | 可读 | — |
| DMI serial / UUID、SMBIOS 表 | 多数发行版 `0400` | root |
| NVMe SMART（ioctl） | `/dev/nvmeN` 常为 `0660` | `disk` 组或 root |
| `/proc/iomem` 地址 | 常被清零 | root |

```bash
aida elevate gui
# 无 pkexec：sudo -E ./target/release/aida gui
```

PolicyKit 策略：`packaging/polkit/com.aida.linux.policy`。安装说明见 [docs/PACKAGING.md](docs/PACKAGING.md)。

## 打包细节

见 [docs/PACKAGING.md](docs/PACKAGING.md)。要点：

- **桌面版用 glibc AppImage**，不要把 egui/glow 链到 musl
- **CLI 可 musl 静态**，适合救援盘和容器
- AppImage 版本号从 `Cargo.toml` 读取，不再写死
- 本机构建：`./scripts/package.sh`；CI：workflow `package`

## 采集架构

```
                  ┌──────────── GUI (egui) ────────────┐
                  │  树形菜单 / iStat 状态栏 / 详情表 / 折线图 / 导出  │
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
   topology/idle  /dmi/id      /hwmon      ioctl       class 03    queues
```

约定：探测只读文件或 ioctl，结果进 `Sample<T>`；`ProbeCtx` 可替换 `/proc` `/sys` `/dev` 做夹具；GUI 约 0.8s 热刷新传感器与速率，不全量重扫 PCI/USB。

内核 ABI 差异与「空 class 不是失败」见 [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。模块分层见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 开发

```bash
cargo test --offline --lib
cargo test --offline --test live_collect
cargo test --no-default-features
```

| 路径 | 职责 |
| --- | --- |
| `src/access.rs` | 读文件 → `Sample` / `AccessKind` |
| `src/probes/` | 一类内核 ABI 一个文件 |
| `src/snapshot.rs` | 拼装快照与 `refresh_live` |
| `src/export.rs` / `src/bench.rs` | JSON/HTML、微基准 |
| `src/ui/app.rs` | egui |
| `src/record.rs` / `src/alerts.rs` | 状态栏历史、阈值告警 |
| `src/elevate.rs` | pkexec / sudo |
| `scripts/` `packaging/` | AppImage、CLI、桌面文件、polkit |

许可证：MIT。
