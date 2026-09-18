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
4. GUI 热路径：`HardwareSnapshot::refresh_live` 更新 hwmon + 告警 + GPU + `/proc/stat` + 网卡/磁盘差分 + RAPL + meminfo + zram/zswap + power + pm + 平台亮度 + loadavg + clocksource/PTP + EDAC + PSI + IRQ + 挂载用量 + sysctl + cgroup + security，避免每帧扫 PCI/USB/DMI/virtio/KVM/IOMMU/MD/SCSI/iSCSI/模块/iomem/ATA/crypto。

## 各 probe 内核接口

| Probe | 主路径 | 补充 |
| --- | --- | --- |
| CPU | `/proc/cpuinfo`，`/sys/devices/system/cpu/cpuN/` | topology；cpuidle；`/proc/stat`；cache；vulnerabilities；`smt/`；`isolated`；cpufreq `policyN`；schedstat |
| DMI | `/sys/class/dmi/id/*` | `/sys/firmware/dmi/tables/DMI` SMBIOS 结构 |
| hwmon | `/sys/class/hwmon/hwmonN/*_input` | thermal_zone + cooling_device |
| NVMe | `/sys/class/nvme/nvmeN/` | `NVME_IOCTL_ADMIN_CMD` Get Log Page 0x02 |
| GPU | `/sys/class/drm/cardN`，PCI class `0x03` | amdgpu busy/vram、i915/xe 频率、连接器 EDID、`/proc/driver/nvidia` |
| Net | `/sys/class/net/*/statistics` | getifaddrs；queues；sockstat；snmp；softnet；bridge/bond；conntrack；tcp congestion；`/proc/net/netstat`；snmp6；net.core；ipv6_route；if_inet6；tcp knobs/rmem；protocols；rt6_stats；rp_filter；igmp 只计接口头行 |
| USB | `/sys/bus/usb/devices`（跳过 `*:*.*` 接口节点） | `usb.ids` 名称 |
| Input | `/proc/bus/input/devices` | handlers → keyboard/mouse/js |
| NUMA | `/sys/devices/system/node/nodeN` | meminfo / cpulist / distance |
| Memory | `/proc/meminfo` | hugepages + THP defrag；buddyinfo；zoneinfo；vmstat；KSM；DirectMap |
| zmem | `/sys/block/zramN` + `module/zswap/parameters` | mm_stat；不调用 zramctl |
| EDAC | `/sys/devices/system/edac/mc/mcN` | ce_count / ue_count |
| Power | `/sys/class/power_supply` | 电池容量/能量、AC online |
| PM | `/sys/power` | state / mem_sleep / suspend_stats；wakeup |
| RAPL | `/sys/class/powercap/*/energy_uj` | 差分瓦特；回绕用 `max_energy_range_uj` |
| Audio | `/proc/asound/cards` | `/sys/class/sound/cardN/id` |
| Firmware | `/sys/firmware/efi` | SecureBoot；ACPI 表名；pm_profile；TPM；hwrng；pstore；`class/firmware/timeout`；memmap |
| Filesystems | `/proc/self/mountinfo` | `/proc/swaps`；`statvfs`；ext4 sysfs；xfs stats；nfsd；fuse connections |
| Modules | `/proc/modules` | 按名称排序 |
| Clock | `clocksource0/current_clocksource` | `/sys/class/rtc`；`/sys/class/ptp`；`/sys/class/pps`；`clockevents` |
| iomem | `/proc/iomem` | `/proc/ioports`；非 root 地址常为 0 |
| PSI | `/proc/pressure/{cpu,memory,io}` | some/full avg10/60/300 |
| IRQ | `/proc/interrupts` | `/proc/softirqs`；`smp_affinity_list`；`/sys/kernel/irq` |
| ATA | `/sys/class/ata_port` | link `sata_spd`；IDENTIFY 型号 |
| PCI | `/sys/bus/pci/devices/*/vendor,device,class` | `current_link_*`；MSI；`sriov_*vfs`；`pci.ids` |
| virtio | `/sys/bus/virtio/devices` | `modalias` → `virtio_ids.h` |
| KVM | `/dev/kvm` | `kvm_intel`/`kvm_amd` nested/EPT/NPT |
| IOMMU | `/sys/kernel/iommu_groups` | 组内 PCI 槽位名 |
| Block | `/sys/block`（跳过 ram/zram/分区） | diskstats 差分；`queue/*`；有 backing 的 loop；`dm-*` name/uuid；bdi；bsg |
| MD | `/proc/mdstat` | `/sys/block/mdN/md/{degraded,sync_action}` |
| SCSI | `/sys/class/scsi_host` | `scsi_device` vendor/model/type |
| iSCSI | `/sys/class/iscsi_{transport,host,session}` | 不调用 iscsiadm |
| Platform | watchdog / backlight / leds / i2c | ACPI/PnP 设备计数；workqueue；perf event_source；MSR；vtconsole；不调用 i2cdetect |
| Buses | rfkill / bluetooth / thunderbolt / V4L / MMC / MEI / ttyS / misc / hidraw / gpio / mtd / IB | `/proc/tty/drivers`；不调用 setserial；ttyS `type=0` 跳过 |
| Sysctl | `/proc/sys/{fs,vm,kernel}` | file-nr；pid_max；aio；inotify；boot_id；nmi_watchdog；panic（可为负）；sysrq；keys；SysV IPC；mqueue；consoles |
| Cgroup | `/sys/fs/cgroup` | v2 controllers / memory.current；第一层 `.slice`/`.scope` |
| Security | lockdown / yama / kptr / dmesg / FIPS / bpf / perf / fs.protected_* | 不调用 sysctl/aa-status |
| Crypto | `/proc/crypto` | 非 internal 截断 32 条 |
| Ns | `/proc/self/ns` | `max_*_namespaces` |
| Software | `/etc/os-release`，`/proc/meminfo` | loadavg / tainted / LSM / entropy / machine-id；`/proc/config.gz` 读字节长度；`/proc/locks`；oops/kexec；`/proc/filesystems` |

## 界面

- 左：`SidePanel` 树（摘要 / CPU / DMI / 内存 / GPU / 传感器 / 电源 / 存储 / 文件系统 / 网络 / USB / 输入 / 声卡 / PCI / 平台 / NUMA / OS / 基准 / 导出）
- 右：对应面板；温度、CPU 利用率、网卡/磁盘吞吐、RAPL 瓦特用 `egui_plot` 保留约 120 个点
- 顶：权限条 +「以管理员身份重启」（`elevate::reexec`）
- 告警：对照 `*_max`/`*_crit`/`*_min`，状态变化写入 JSONL（`$AIDA_ALERT_LOG` 或 `$XDG_STATE_HOME/aida/alerts.jsonl`）
- 中文标签：若系统有 Noto/文泉驿等 CJK 字体则加载，否则回退英文，避免方块字

## 提权

`src/elevate.rs` 只在用户点击或 `aida elevate` 时 `exec` 替换进程。采集函数不 spawn sudo。

## 磁盘基准

`bench::run_direct`：`posix_memalign` + `O_DIRECT`。失败则 `direct_error` 非空，buffered 仍可用。

## 刻意未做

- GPU 计算基准（OpenCL/Vulkan）：会引入额外运行时依赖，与「尽量少依赖、可静态/AppImage 打包」冲突。需要时再单独一轮。
