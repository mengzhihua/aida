# AIDA Linux

开源 Linux 硬件检测与监控，对标 Windows [AIDA64](https://www.aida64.com/) 和 macOS iStat Menus：硬件信息、传感器、状态栏、微基准、JSON/HTML 报告。

采集只走内核文件（`/proc` `/sys` `/dev`）和少量 ioctl，**不调用** `dmidecode`、`lspci`、`smartctl`、`nvme-cli`、`lshw`。GUI 与 CLI 共用同一套快照。

**不需要安装 Rust。** 每个 PR 和每次发版都会打出可直接运行的包。

## 下载即用

| 包 | 给谁 | 怎么跑 |
| --- | --- | --- |
| `aida-linux-<ver>-<arch>.tar.gz` | 拷到 U 盘 / 另一台机器 | 解压后看里面的 `INSTALL.txt` |
| `AIDA_Linux-<ver>-<arch>.AppImage` | 桌面（GUI + CLI） | `chmod +x` 后直接跑 |
| `aida-cli-<ver>-<arch>-musl`（或 `-gnu`） | 服务器 / 无显示器 | `chmod +x` 后 `./aida-cli collect` |

去哪下：

1. **发版**：[GitHub Releases](https://github.com/mengzhihua/aida/releases)（打 `v*` tag 自动挂上）
2. **每个 PR / 每次开发**：Actions 工作流 [package](https://github.com/mengzhihua/aida/actions/workflows/package.yml) → 最新成功的 run → Artifact **`aida-linux`**

```bash
# 桌面（容器或无 FUSE 时加 APPIMAGE_EXTRACT_AND_RUN=1）
chmod +x AIDA_Linux-*.AppImage
APPIMAGE_EXTRACT_AND_RUN=1 ./AIDA_Linux-*.AppImage gui

# 服务器采集
chmod +x aida-cli-*-musl
./aida-cli-*-musl collect --html report.html
./aida-cli-*-musl bench --quick
```

本机从源码打出同样的包：

```bash
./scripts/package.sh
ls -lh dist/
# dist/aida-linux-<ver>-<arch>.tar.gz 即可拷走
```

## 命令

| 你想做的事 | 命令 |
| --- | --- |
| 桌面界面 | `aida gui` |
| 采集 JSON | `aida collect` |
| HTML 报告 | `aida collect --html aida-report.html` |
| 微基准 | `aida bench --quick` |
| 以管理员重开 GUI | `aida elevate gui` |
| 版本 | `aida version` |

无图形会话（没有 `DISPLAY` / `WAYLAND_DISPLAY`）时，裸跑 `aida` 会变成 `collect`。

GUI 需要 OpenGL/EGL 和 `libxkbcommon`（X11 还要 `libxkbcommon-x11`）。musl CLI 无此依赖。

## 能做什么

- **硬件与拓扑**：CPU（拓扑 / cpuidle / 漏洞 / 利用率）、DMI、PCI/PCIe、NVMe、GPU/DRM、USB、输入设备、NUMA、virtio / KVM / IOMMU
- **传感器**：hwmon + thermal，阈值告警写 JSONL；RAPL 瓦特、PSI、EDAC
- **状态栏**：窗口内常驻 CPU / 内存 / 网络 / 磁盘 / 温度 / loadavg；可选置顶窄条（对标 iStat Menus）
- **记录**：GUI 开始/停止，采样追加到 `$AIDA_RECORD_LOG` 或 `~/.local/state/aida/history.jsonl`
- **存储与总线**：块设备 / MD / SCSI / iSCSI / NBD / zram / zswap，以及 rfkill、HID、GPIO、红外 `rc`、STM、PECI 等 leftover class（空 = 无硬件，不是失败）
- **内核与网络**：sysctl、cgroup、lockdown、conntrack、TCP/IPv6 knobs（缺权限标 `permission_denied`，不填假数据）
- **占用**：GUI 前台约 1Hz 只刷新传感器和速率；TCP 表 / sysctl / 挂载用量约每 8 秒才扫一次；窗口失焦降到约 2.5s
- **导出**：JSON（每个字段带 `access` / `source` / `hint`）和单文件 HTML
- **基准**：CPU / 内存 / 磁盘相对分（`--quick` 约 200ms）

刻意未做：GPU OpenCL/Vulkan 计算基准（会引入额外运行时，和可打包目标冲突）。

## 从源码安装（开发机）

需要 **Rust 1.88+**。无显示器请编 CLI（`--no-default-features`）。

```bash
cargo build --release
./target/release/aida --help
./target/release/aida gui          # 需要 X11 或 Wayland

# 装到用户目录（桌面文件 + 图标）
./scripts/install.sh
# 或：PREFIX=/usr/local sudo ./scripts/install.sh
```

只要采集 CLI：

```bash
cargo build --release --no-default-features
./scripts/build-cli.sh             # 优先 musl 静态
```

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

PolicyKit 策略：`packaging/polkit/com.aida.linux.policy`。细节见 [docs/PACKAGING.md](docs/PACKAGING.md)。

环境变量：

| 变量 | 默认 | 含义 |
| --- | --- | --- |
| `AIDA_ALERT_LOG` | `$XDG_STATE_HOME/aida/alerts.jsonl` | 传感器越限 JSONL |
| `AIDA_RECORD_LOG` | `$XDG_STATE_HOME/aida/history.jsonl` | 状态栏历史 JSONL |

## 打包约定

见 [docs/PACKAGING.md](docs/PACKAGING.md)。要点：

- **桌面版用 glibc AppImage**，不要把 egui/glow 链到 musl
- **CLI 可 musl 静态**，适合救援盘和容器
- 版本号只来自 `Cargo.toml`
- **每一轮开发都要打出可直接使用的包**：本地 `./scripts/package.sh`；CI 每个 PR 上传 Artifact `aida-linux`

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

约定：探测只读文件或 ioctl，结果进 `Sample<T>`；`ProbeCtx` 可替换 `/proc` `/sys` `/dev` 做夹具。GUI 快路径约 1Hz 更新传感器与速率，不全量重扫 PCI/USB/TCP 表。

内核 ABI 差异与「空 class 不是失败」见 [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。模块分层见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 开发

```bash
cargo test --lib
cargo test --test live_collect
cargo test --no-default-features --lib
./scripts/package.sh              # 每轮结束：产出 dist/ 可拷走的安装包
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
| `scripts/` `packaging/` | AppImage、CLI、安装包、桌面文件、polkit |

许可证：MIT。
