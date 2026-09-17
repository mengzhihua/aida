# 模块架构

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
4. GUI 热路径：`HardwareSnapshot::refresh_live` 更新 hwmon + 告警 + GPU busy/vram + `/proc/stat` 整机/每核利用率 + 网卡/磁盘字节差分 + meminfo + power_supply + cpufreq，避免每帧扫 PCI/USB/DMI。

## 各 probe 内核接口

| Probe | 主路径 | 补充 |
| --- | --- | --- |
| CPU | `/proc/cpuinfo`，`/sys/devices/system/cpu/cpuN/` | `/proc/stat` 整机+每核利用率；cache；vulnerabilities |
| DMI | `/sys/class/dmi/id/*` | `/sys/firmware/dmi/tables/DMI` SMBIOS 结构 |
| hwmon | `/sys/class/hwmon/hwmonN/*_input` | thermal_zone + cooling_device |
| NVMe | `/sys/class/nvme/nvmeN/` | `NVME_IOCTL_ADMIN_CMD` Get Log Page 0x02 |
| GPU | `/sys/class/drm/cardN`，PCI class `0x03` | amdgpu busy/vram、i915/xe 频率、`/proc/driver/nvidia` |
| Net | `/sys/class/net/*/statistics` | `getifaddrs` 地址；`speed=-1` → unsupported |
| USB | `/sys/bus/usb/devices`（跳过 `*:*.*` 接口节点） | `usb.ids` 名称 |
| Input | `/proc/bus/input/devices` | handlers → keyboard/mouse/js |
| NUMA | `/sys/devices/system/node/nodeN` | meminfo / cpulist / distance |
| Memory | `/proc/meminfo` | hugepages + THP |
| Power | `/sys/class/power_supply` | 电池容量/能量、AC online |
| Audio | `/proc/asound/cards` | `/sys/class/sound/cardN/id` |
| Firmware | `/sys/firmware/efi` | SecureBoot efivar（跳过 4 字节属性） |
| PCI | `/sys/bus/pci/devices/*/vendor,device,class` | `pci.ids` + 内置厂商表 |
| Block | `/sys/block`（跳过 loop/ram/分区） | `/proc/diskstats` 差分 I/O |
| Software | `/etc/os-release`，`/proc/meminfo`，`osrelease` | `XDG_CURRENT_DESKTOP` |

## 界面

- 左：`SidePanel` 树（摘要 / CPU / DMI / 内存 / GPU / 传感器 / 电源 / 存储 / 网络 / USB / 输入 / 声卡 / PCI / NUMA / OS / 基准 / 导出）
- 右：对应面板；温度、CPU 利用率、网卡/磁盘吞吐用 `egui_plot` 保留约 120 个点
- 顶：权限条 +「以管理员身份重启」（`elevate::reexec`）
- 告警：对照 `*_max`/`*_crit`/`*_min`，状态变化写入 JSONL（`$AIDA_ALERT_LOG` 或 `$XDG_STATE_HOME/aida/alerts.jsonl`）
- 中文标签：若系统有 Noto/文泉驿等 CJK 字体则加载，否则回退英文，避免方块字

## 提权

`src/elevate.rs` 只在用户点击或 `aida elevate` 时 `exec` 替换进程。采集函数不 spawn sudo。

## 磁盘基准

`bench::run_direct`：`posix_memalign` + `O_DIRECT`。失败则 `direct_error` 非空，buffered 仍可用。

## 刻意未做

- GPU 计算基准（OpenCL/Vulkan）：会引入额外运行时依赖，与「尽量少依赖、可静态/AppImage 打包」冲突。需要时再单独一轮。
