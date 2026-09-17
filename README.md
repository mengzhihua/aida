# AIDA Linux

开源 Linux 硬件检测与监控工具，对标 Windows [AIDA64](https://www.aida64.com/) 的常用能力：硬件信息、传感器监控、微基准、系统软件信息、报告导出。

**第一轮（本仓库当前迭代）** 交付可编译的核心骨架，而不是一次性堆完整工程：

| 模块 | 状态 |
| --- | --- |
| CPU / DMI / hwmon / NVMe / PCI / 块设备 / 软件 | 可读 sysfs/procfs 的 demo |
| 权限模型 | 每个字段带 `ok / permission_denied / not_found` |
| egui 界面 | 左侧树 + 右侧详情 + 温度折线 |
| 微基准 | CPU / 内存带宽 / 磁盘顺序读写 |
| JSON / HTML 导出 | CLI + GUI |
| 静态编译 / AppImage | 文档中的下一轮路径，本轮先保证 glibc 动态链接可运行 |

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
        ┌─────────────┬───────────┼───────────┬────────────┐
        ▼             ▼           ▼           ▼            ▼
      CPU           DMI        hwmon        NVMe         PCI …
   /proc/cpuinfo  /sys/class   /sys/class  sysfs +     /sys/bus/pci
   /sys/.../cpu   /dmi/id      /hwmon      ioctl
                  SMBIOS blob              Get Log Page
```

约定：

1. **所有探测函数只读文件或发 ioctl**，把结果放进 `Sample<T>`，失败原因跟着字段走。
2. **`ProbeCtx` 把 `/proc` `/sys` `/dev` 做成可替换根**，单元测试用临时目录夹具，不 mock 整个操作系统。
3. **GUI 与 CLI 共用同一套 snapshot**，GUI 每 ~0.8s 只刷新传感器和 `/proc/stat`，不全量重扫 PCI。

## 运行

需要 **Rust 1.88+**（GUI 依赖树含 edition 2024 与较新的 `icu`/`image`）。无显示器时请用 `--no-default-features` 只编采集 CLI。

```bash
# 采集 JSON（无显示器的服务器/CI 可直接用）
cargo run --release -- collect

# HTML 报告
cargo run --release -- collect --html aida-report.html

# 微基准（--quick 约 200ms，适合测试）
cargo run --release -- bench --quick

# 桌面界面（需要 X11/Wayland）
cargo run --release -- gui
```

无 `DISPLAY`/`WAYLAND_DISPLAY` 时，裸跑 `aida` 会退化为 `collect`。

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
sudo -E ./target/release/aida gui
# 或 pkexec，便于保留 DISPLAY
```

## 基准测试思路

| 项目 | 做法 | 坑 |
| --- | --- | --- |
| CPU | 多线程整数 LCG + 浮点 `mul_add`/`sin`，按墙钟时间计 Mops/MFLOPS | Turbo、CPU 亲和性、同机后台负载 |
| 内存 | STREAM 风格 copy / scale / triad | 编译器优化（已 `black_box`）、缓存大小、NUMA |
| 磁盘 | 临时文件顺序写 + `fsync` + 读回 | **未开 O_DIRECT**，读常命中 page cache，数字会虚高 |

这些是相对分，不是 SPEC、也不是 `fio`。下一轮可加 `O_DIRECT`、可选测试路径、多线程顺序/随机。

## 报告导出

- JSON：完整 `HardwareSnapshot`（含每个字段的 `access` / `source` / `hint`）。
- HTML：单文件内嵌 CSS，表格展示摘要；字段值做了 `<>&` 转义。

```bash
aida collect --json out.json --html out.html
```

## 跨发行版

见 [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。要点：不要假设 `/sys/class/dmi` 存在（容器/部分云主机没有）；不要假设有 `pci.ids`；ARM/RISC-V 上 CPU 字段名与 x86 不同，解析按键名而不是位置。

## 打包（下一轮）

- **采集 CLI** 可以 `x86_64-unknown-linux-musl` 静态链接。
- **GUI** 依赖 OpenGL/X11，不适合硬 musl 静态。实用路径是 glibc + [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy) 打 AppImage。
- `Cargo.toml` 已打开 `lto = thin` 与 `strip`，减小 release 体积。

## 开发

```bash
cargo test --no-default-features
cargo test --features gui   # 不启动窗口，只编进 ui 模块
```

模块入口：

- 权限原语：`src/access.rs`
- 探测：`src/probes/`
- 快照：`src/snapshot.rs`
- 导出：`src/export.rs`
- 基准：`src/bench.rs`
- 界面：`src/ui/app.rs`

架构说明：[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
