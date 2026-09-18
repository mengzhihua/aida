# AIDA Linux

开源 Linux 硬件检测与监控工具，对标 Windows [AIDA64](https://www.aida64.com/) 的常用能力：硬件信息、传感器监控、微基准、系统软件信息、报告导出。

**第二十一轮** 补齐 pty / inode-state / SysV shmall、io_uring、IPv6 RA/autoconf、conntrack 超时、CPU offline、device-tree model，以及 scsi_generic/wwan/ppp/phy 总线名。不写 io_uring sysctl。

| 模块 | 状态 |
| --- | --- |
| CPU / DMI / hwmon / NVMe / PCI / 块设备 / 软件 | 可读 sysfs/procfs；CPU `offline`；`ostype`；firmware device-tree `model` |
| CPU 拓扑 / cpuidle / 漏洞 / 每核利用率 / 缓存 / SMT | siblings + `cpuidle` + `smt/{active,control}` + `isolated` + cpufreq policy |
| GPU / 显示器 | DRM + 连接器 EDID（不调用 `edid-decode`/`xrandr`） |
| virtio / KVM | virtio `modalias`；`/dev/kvm` + `kvm_intel`/`kvm_amd` nested/EPT/NPT |
| PCIe 链路 / SR-IOV | `current_link_*` + MSI；`sriov_{num,total}vfs` |
| IOMMU | `/sys/kernel/iommu_groups`，不调用 `find` |
| 网络 | `/sys/class/net` + getifaddrs；snmp/softnet/TcpExt；IPv6 snmp6/路由/rt6_stats 第 6 列；TCP knobs/rmem/notsent；tcp6/udp6/raw/udplite/raw6；xfrm_stat；ptype；fib_triestat Leaves；igmp6 按接口去重；iptables 表名；busy_poll/busy_read/dev_weight；IPv6 accept_ra/autoconf/hop；conntrack 超时/buckets；tcp_max_tw_buckets；icmp_ratelimit；ip_default_ttl；protocols；conntrack；net.core；rp_filter per-iface |
| USB / 输入 / NUMA | sysfs / proc / nodeN |
| 内存 | meminfo + DirectMap + THP defrag + hugepages + zoneinfo + vmstat + KSM + zswap + memory_tier |
| zram | `/sys/block/zramN`（不调用 zramctl）；常规块设备表仍跳过 zram |
| EDAC / RAPL / 电源 / 睡眠 / 声卡 | power_supply；`/sys/power`；RAPL；无节点时说明 |
| 固件 | EFI / Secure Boot / ACPI 表名 / pm_profile / TPM / hwrng / firmware timeout / memmap |
| 文件系统 / 模块 / 时钟 | mountinfo + statvfs；ext4 sysfs；nfsd/fuse；modules；clocksource + RTC + PTP + clockevents |
| PSI / IRQ / taint / LSM / sysctl / cgroup / 安全 | pressure、interrupts、sysfs irq、lockdown/kptr、file-nr、aio/inotify、boot_id、panic/sysrq、keys、SysV IPC、fs.protected、sched_rt/OOM、printk/cfs/uffd、bpf_jit/binfmt_misc、dentry-state、inode-state、pty、io_uring、key-users、cgroup v1 enabled |
| ATA / MD / SCSI / iSCSI | ata_port；mdstat；scsi_host + scsi_device；iscsi_transport（不调用 iscsiadm）；dm name/uuid；BDI/BSG |
| 平台 / 总线 | watchdog/LED/I2C；rfkill/蓝牙/雷电/V4L/MMC/MEI；ttyS；misc；HID；GPIO/MTD/IB；MSR；vtconsole；`bus/platform/devices`；ieee80211/typec/udc/dax/wmi/spi/serio/ubi；scsi_generic/wwan/ppp/phy |
| DMA / PWM / IIO / nvmem / regulator / pci_bus | `/proc/dma`；`class/dma`；pwmchip npwm；IIO name；nvmem type；regulator 电压；devlink status；pci_bus cpulist |
| 磁盘 I/O / 分区 / 队列 / loop | diskstats 差分 + queue 参数；有 backing_file 的 loop |
| crypto / 命名空间 | `/proc/crypto`；`/proc/self/ns` + `max_*_namespaces` |
| 传感器告警 | hwmon 阈值，越限写 JSONL |
| 权限 / 提权 / GUI / 基准 / 导出 / AppImage | 同前几轮 |

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
   topology/idle  /dmi/id      /hwmon      ioctl       class 03    queues
                                  │
                                  ├── virtio / KVM / RAPL / PTP / buddyinfo / zoneinfo / vmstat / KSM / zswap
                                  ├── input / audio / power / firmware / EDAC / TPM / sysctl / cgroup / security / crypto / ns / pm
                                  ├── memory + iomem + modules + clocksource + PSI/IRQ / zram
                                  ├── gpio / mtd / infiniband / hidraw / virtio-ports / device-mapper
                                  └── fs: mountinfo / swaps / statvfs / ext4
```

约定：

1. **所有探测函数只读文件或发 ioctl**，把结果放进 `Sample<T>`，失败原因跟着字段走。
2. **`ProbeCtx` 把 `/proc` `/sys` `/dev` 做成可替换根**，单元测试用临时目录夹具，不 mock 整个操作系统。
3. **GUI 与 CLI 共用同一套 snapshot**，GUI 每 ~0.8s 刷新传感器、告警、网卡/磁盘速率、RAPL 瓦特、内存、zram/zswap、loadavg、clocksource/PTP、EDAC、PSI、IRQ/softirq、挂载用量、平台亮度、sysctl/cgroup/security/pm 和 `/proc/stat`，不全量重扫 PCI/USB/virtio/KVM/IOMMU/MD/SCSI/iSCSI/模块/iomem/ATA/crypto。

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

- JSON：完整 `HardwareSnapshot`（含每个字段的 `access` / `source` / `hint`，以及 `net` / `usb` / `input` / `numa` / `fs` / `modules` / `clock` / `edac` / `iomem` / `psi` / `irq` / `ata` / `virtio` / `rapl` / `alerts`）。
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
- 探测：`src/probes/`（含 `virtio` / `rapl` / `iommu` / `md` / `scsi` / `platform` / `fs` / `modules` / `clock` / `edac` / `iomem` / `psi` / `irq` / `ata` / `net` / `usb` / `input` / `numa` / `memory` / `power` / `audio` / `firmware`）
- 快照：`src/snapshot.rs`
- 导出：`src/export.rs`
- 基准：`src/bench.rs`
- 界面：`src/ui/app.rs`
- 提权：`src/elevate.rs`
- 告警：`src/alerts.rs`
- 打包：`scripts/build-appimage.sh`、`packaging/`

架构说明：[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
