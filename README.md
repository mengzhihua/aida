# AIDA Linux

开源 Linux 硬件检测与监控，对标 Windows [AIDA64](https://www.aida64.com/) 和 macOS iStat Menus：硬件信息、传感器、状态栏、微基准、JSON/HTML 报告。

采集只走内核文件（`/proc` `/sys` `/dev`）和少量 ioctl，**不调用** `dmidecode`、`lspci`、`smartctl`、`nvme-cli`、`lshw`。GUI 与 CLI 共用同一套快照。

**不需要安装 Rust。** Ubuntu / Debian / CentOS / RHEL / Rocky / Fedora 解开 tar 就能用。每个 PR 和每次发版都会打出可直接运行的包。

## 下载即用

| 包 | 给谁 | 怎么跑 |
| --- | --- | --- |
| `aida-linux-<ver>-<arch>.tar.gz` | 拷到 U 盘 / 另一台机器 | 解压后 `./install.sh` 或看 `INSTALL.txt` |
| `AIDA_Linux-<ver>-<arch>.AppImage` | 较新 glibc 桌面（GUI + CLI） | `chmod +x` 后直接跑 |
| `aida-cli-<ver>-<arch>-musl`（或 `-gnu`） | **所有发行版** 服务器 / 无显示器 / 旧 glibc | `chmod +x` 后 `./aida-cli collect` |

去哪下：

1. **发版**：[GitHub Releases](https://github.com/mengzhihua/aida/releases)。合并到 `main` / 栈顶且 `Cargo.toml` 版本还没有 `v*` tag 时，`package` 工作流自测（单元测试 + 成品 smoke）通过后**自动**挂 tar.gz / AppImage / musl CLI。也可以 `./scripts/release.sh --push` 手工打 tag。
2. **每个 PR / 每次开发**：Actions 工作流 [package](https://github.com/mengzhihua/aida/actions/workflows/package.yml) → 最新成功的 run → Artifact **`aida-linux`**

```bash
tar -xzf aida-linux-*.tar.gz
cd aida-linux-*
chmod +x install.sh aida-cli
./aida-cli doctor                 # 识别 Ubuntu vs CentOS，给出 apt 或 dnf/yum
./install.sh                      # 装到 ~/.local，不需要 cargo
./install.sh --deps               # 再装 GUI 运行库

# 不安装也可以
./run-collect.sh --html report.html          # CentOS 7 也能采集（musl 静态）
APPIMAGE_EXTRACT_AND_RUN=1 ./run-gui.sh      # 无 FUSE 时必须加这个；缺 libegl/libxkbcommon-x11 GUI 起不来
```

### 发行版怎么选包

| 系统 | 采集 / 报告 | 桌面 GUI |
| --- | --- | --- |
| Ubuntu 24.04、Debian 13、Fedora 新版本 | musl CLI 或 AppImage | AppImage（构建机 glibc，常见 **2.39**） |
| Ubuntu 22.04 / 20.04、Rocky/Alma 8–9、CentOS Stream | musl CLI | 若 `aida doctor` 提示 glibc 偏低，只用 CLI |
| CentOS 7（glibc 2.17） | **musl CLI** | AppImage 起不来，这是预期 |

GUI 运行库（`./install.sh --deps` 会按 `ID`/`ID_LIKE` 选命令）：

```bash
# Debian / Ubuntu / Mint
sudo apt-get install -y libxkbcommon-x11-0 libegl1 libgl1 pkexec

# CentOS / RHEL / Rocky / Alma / Fedora
sudo dnf install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit
# 没有 dnf 时（CentOS 7）：
sudo yum install -y libxkbcommon-x11 mesa-libEGL mesa-libGL polkit
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
| 本机发行版 / glibc / GUI 库体检 | `aida doctor`（`--json` 可脚本化） |
| 桌面界面 | `aida gui` |
| 采集 JSON | `aida collect` |
| HTML 报告 | `aida collect --html aida-report.html` |
| 文本报告（对标 AIDA64 TXT） | `aida collect --format text` 或 `--text aida-report.txt` |
| CSV 表格 | `aida collect --csv aida-report.csv` |
| Markdown | `aida collect --md aida-report.md` |
| 微基准 | `aida bench --quick` |
| 以管理员重开 GUI | `aida elevate gui` |
| 版本 | `aida version` |

无图形会话（没有 `DISPLAY` / `WAYLAND_DISPLAY`）时，裸跑 `aida` 会变成 `collect`。

GUI 需要 OpenGL/EGL 和 `libxkbcommon`（X11 还要 `libxkbcommon-x11`）。musl CLI 无此依赖，旧发行版请用它。

## 能做什么

- **硬件与拓扑**：CPU（拓扑 / cpuidle / 漏洞 / 利用率）、DMI / 主板、SMBIOS Type 4 处理器插座、Type 7 缓存、Type 8 端口连接器、Type 9 系统插槽、Type 11 OEM 字符串、Type 12 配置选项、Type 13 BIOS 语言、Type 22 便携电池、Type 23 系统复位、Type 24 硬件安全、Type 26 电压探头、Type 27 冷却装置、Type 28 温度探头、Type 32 启动状态、Type 39 电源、Type 41 板载设备、Type 43 TPM、Type 0 BIOS ROM/Release、Type 16/17 内存阵列与 DIMM（容量、外形、额定/配置速度、位宽、rank、厂商/序列/料号；对标 AIDA64 CPU/主板/Memory/SPD，不扫 I2C）、PCI/PCIe、NVMe、GPU/DRM、USB、输入设备、NUMA、virtio / KVM / IOMMU
- **传感器**：hwmon + thermal，阈值告警写 JSONL；RAPL 瓦特、PSI、EDAC
- **任务栏**：窗口内常驻 CPU / 内存 / 网络 / 磁盘 / 温度 / loadavg；置顶无边框窄条可选，默认关闭（对标 iStat Menus，但不挡住旁边的浏览器）
- **历史记录**：默认写入 `$AIDA_RECORD_LOG` 或 `~/.local/state/aida/history.jsonl`；启动加载最近 1800 条（聚焦约 2 秒一条时大约一小时），左侧「历史记录」按本地时间画折线
- **存储与总线**：块设备 / MD / SCSI / iSCSI / NBD / zram / zswap，以及 rfkill、HID、GPIO、红外 `rc`、STM、PECI、wakeup、MSR、DPLL、FireWire、Greybus、RapidIO、ULPI、SPMI、`pci_epc`、PTP、PPS、TPM、`firmware_attributes`、`pci_epf`、Slimbus、Memory Stick、SIOX、HSI、AMBA、FSI、`ppdev` 等 leftover class（空 = 无硬件，不是失败）
- **内核与网络**：sysctl、cgroup、lockdown、conntrack、TCP/IPv6 knobs（缺权限标 `permission_denied`，不填假数据）
- **软件**：OS 页先给发行版和已装包（读 dpkg/apk 状态文件，不调用 `dpkg -l`，列表最多 256 条）。内核 / 安全 / cgroup 原始项收在折叠里，默认不铺开
- **占用**：GUI 聚焦约每 2 秒刷新利用率、温度和速率（没有 cpufreq 的虚拟机不再逐核打开 `scaling_*`；网卡计数读一次 `/proc/net/dev`；已有传感器只重读输入值，没有传感器/电源/RAPL 时不再扫目录）。TCP 表、per-iface sysctl、CPU 拓扑、内存块 online 状态只在启动时读。约每 60 秒补 governor、新网卡、新传感器，并扫挂载用量。失焦降到约 10 秒，并且不做这次慢扫描。已装包和 `config.gz` 只在启动时读。veth/cni 等高基数接口不逐个打开 sysfs，界面收进一个折叠；docker0 / `br-*` / virbr 仍读网桥。逻辑 CPU 超过 32 个时一行一个。悬停动画关掉。指针移动大约每 400ms 才重绘一次（拖拽约 50ms），避免软件 OpenGL 按鼠标事件铺满窗口。CJK 字体只做中文回退。形状羽化和像素抖动关掉。IRQ 亲和最多 48 条；per-iface sysctl 差异先看物理网卡，最多 48 个接口。nfs/cifs/fuse 不调用 `statvfs`
- **导出**：JSON（每个字段带 `access` / `source` / `hint`）、单文件 HTML、可读文本、CSV、Markdown。GUI 默认写到 `~/Documents`（`$AIDA_EXPORT_DIR` 或 XDG 文档目录优先），目录没有会创建，并显示完整路径；无 DMI 时文本/HTML 只留一句「本环境无 DMI」，不逐项打横线。CLI `--format` 打印到 stdout，FILE=`-` 也是 stdout
- **提权**：顶栏一行身份（普通用户 uid）和「提权后重新采集」；旁边一行「本环境：虚拟机 · 无 DMI · 无 GPU · 无温度传感器」。长说明在「权限与环境说明」里。CLI：`aida elevate gui`
- **基准**：CPU / 内存 / 磁盘相对分（`--quick` 约 200ms）；GUI 在后台线程跑，有「正在跑」提示
- **界面**：主窗口有边框、可缩放（最小约 800×560）。左侧导航和每个详情页各自滚动，详情页滚动条常显。置顶任务栏默认关。超线程等 sysfs 原文（`0` / `notsupported`）显示成「关 / 内核不支持」，JSON 仍保留原文

刻意未做：GPU OpenCL/Vulkan 计算基准（会引入额外运行时，和可打包目标冲突）。内存条级厂商/料号只靠 SMBIOS Type 16/17，不扫 I2C SPD。

## 从源码安装（开发机）

普通用户请走上面的 tar.gz，**不要先装 Rust**。开发机需要 **Rust 1.88+**。无显示器请编 CLI（`--no-default-features`）。

```bash
cargo build --release
./target/release/aida --help
./target/release/aida gui          # 需要 X11 或 Wayland
./target/release/aida doctor

# 优先装 dist/ 成品（不强制 cargo）：
./scripts/package.sh
./scripts/install.sh
./scripts/install.sh --deps
# 开发机现场编译 GUI：
./scripts/install.sh --from-source
# 系统级：sudo ./scripts/install.sh --prefix /usr
```

只要采集 CLI：

```bash
cargo build --release --no-default-features
./scripts/build-cli.sh             # 优先 musl 静态
```

## 权限

缺权限时字段标记为 `permission_denied` 并给出路径，**不会伪造数据**。文件在、键没有（例如 Debian 的 `os-release` 没有 `ID_LIKE`）是 `absent`，界面写 `[未设置]`，不要理解成文件丢了。

界面顶部是一行身份和「提权后重新采集」。云主机上还会有一行「本环境：虚拟机 · 无 DMI · 无 GPU · 无温度传感器」。TEMP — 点进去是传感器页，表示没有 hwmon，不是坏了。更长的说明在「权限与环境说明」里，默认收起，避免把详情页顶出窗口。

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
| `AIDA_RECORD_LOG` | `$XDG_STATE_HOME/aida/history.jsonl` | 任务栏历史 JSONL |
| `AIDA_EXPORT_DIR` | `~/Documents`（或 XDG 文档目录） | GUI 导出目录。没有这个目录时，第一次导出会创建 |

## 打包约定

见 [docs/PACKAGING.md](docs/PACKAGING.md)。要点：

- **桌面版用 glibc AppImage**，不要把 egui/glow 链到 musl；目标 glibc 见包内 `GLIBC_GUI`
- **CLI 可 musl 静态**，这是 CentOS / 旧 Ubuntu 的开箱采集路径
- 版本号只来自 `Cargo.toml`
- **每一轮开发都要打出可直接使用的包**：本地 `./scripts/package.sh`；CI 每个 PR 上传 Artifact `aida-linux`
- 用户安装：解压后 `./install.sh`（`--deps` 按 os-release 走 apt 或 dnf/yum）

## 采集架构

```
                  ┌──────────── GUI (egui) ────────────┐
                  │  树形菜单 / 任务栏 / 历史记录 / 详情表 / 折线图 / 导出  │
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

约定：探测只读文件或 ioctl，结果进 `Sample<T>`；`ProbeCtx` 可替换 `/proc` `/sys` `/dev` 做夹具。GUI 聚焦约每 2 秒更新传感器与速率，失焦约 10 秒；TCP/sysctl 约每 60 秒才重扫。

技术方案（数据模型、快慢路径、空字段、滚动、导出目录、命令行）见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。内核 ABI 差异与「空 class 不是失败」见 [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。

## 开发

```bash
cargo test --lib
cargo test --test live_collect
cargo test --no-default-features --lib
./scripts/package.sh              # 每轮结束：产出 dist/ 可拷走的安装包
./scripts/release.sh --push       # 无问题版本：打 v$VERSION tag，CI 自测后发 Release
```

| 路径 | 职责 |
| --- | --- |
| `src/access.rs` | 读文件 → `Sample` / `AccessKind` |
| `src/probes/` | 一类内核 ABI 一个文件 |
| `src/snapshot.rs` | 拼装快照与 `refresh_live` |
| `src/export.rs` / `src/bench.rs` | JSON/HTML、微基准 |
| `src/ui/app.rs` | egui |
| `src/record.rs` / `src/alerts.rs` | 任务栏历史 JSONL、阈值告警 |
| `src/elevate.rs` | pkexec / sudo |
| `src/doctor.rs` | 发行版 family / glibc / GUI `.so`，给出 apt 或 dnf/yum |
| `scripts/` `packaging/` | AppImage、CLI、`install.sh`、桌面文件、polkit |

许可证：MIT。
