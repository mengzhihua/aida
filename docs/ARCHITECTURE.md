# 模块架构（第一轮）

## 目标分层

| 层 | 职责 | 禁止事项 |
| --- | --- | --- |
| `access` | 读文件、翻译 `io::Error` 为 `AccessKind`、检测 uid/组 | 业务字段拼装 |
| `probes::*` | 一类内核 ABI 一个文件 | `Command::new("lspci")` 之类外部进程 |
| `snapshot` | 拼装 `HardwareSnapshot` | UI 字符串 |
| `export` / `bench` | 纯函数，方便 CLI/GUI/测试共用 | 弹窗 |
| `ui` | egui 布局与 1Hz 刷新 | 直接 `fs::read_to_string` |

## 数据采集逻辑

1. `ProbeCtx::live()` 指向真实 `/proc` `/sys` `/dev` `/etc`。
2. 每个 probe `collect(ctx) -> *Info`：
   - 枚举目录（`hwmonN`、PCI slot、`nvmeN`）
   - 对每个属性调用 `read_trimmed` / `read_bytes`
   - 数值字段在 probe 内换算（温度 m°C → °C，块设备 `size` 扇区 → 字节）
3. 失败不 panic：`Sample.value = None`，`hint` 写给人看的原因。
4. GUI 热路径：`HardwareSnapshot::refresh_live` 只更新 hwmon + `/proc/stat` 差分利用率 + cpufreq，避免每帧扫 PCI。

## 各 probe 内核接口

| Probe | 主路径 | 补充 |
| --- | --- | --- |
| CPU | `/proc/cpuinfo`，`/sys/devices/system/cpu/cpuN/` | `/proc/stat` 利用率；cache/index* |
| DMI | `/sys/class/dmi/id/*` | `/sys/firmware/dmi/tables/DMI` SMBIOS 结构 |
| hwmon | `/sys/class/hwmon/hwmonN/*_input` | `/sys/class/thermal/thermal_zoneN` |
| NVMe | `/sys/class/nvme/nvmeN/` | `NVME_IOCTL_ADMIN_CMD` Get Log Page 0x02 |
| PCI | `/sys/bus/pci/devices/*/vendor,device,class` | `pci.ids` + 内置厂商表 |
| Block | `/sys/block`（跳过 loop/ram/分区） | model/serial/scheduler |
| Software | `/etc/os-release`，`/proc/meminfo`，`osrelease` | `XDG_CURRENT_DESKTOP` |

## 界面

- 左：`SidePanel` 树（摘要 / CPU / DMI / 传感器 / 存储 / PCI / OS / 基准 / 导出）
- 右：对应面板；传感器用 `egui_plot` 保留约 120 个点
- 顶：权限条（root 绿色，普通用户橙色）
- 中文标签：若系统有 Noto/文泉驿等 CJK 字体则加载，否则回退英文，避免方块字

## 下一轮建议（刻意不做）

- GPU（DRM/`amdgpu`/`i915` sysfs，以及 NVIDIA 专有接口）
- 网络接口详细统计、USB 树、输入设备
- O_DIRECT 磁盘基准、多 NUMA 内存
- AppImage CI、polkit 提权 helper
- 传感器阈值告警与日志
