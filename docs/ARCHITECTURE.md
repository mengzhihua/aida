# 技术方案

这份文档约定采集、展示和导出怎么分工。内核 ABI 的逐条例外在 [COMPATIBILITY.md](COMPATIBILITY.md)。打包、AppImage 和发版在 [PACKAGING.md](PACKAGING.md)。

## 范围

做的事：只读 `/proc` `/sys` `/dev` 和少量 ioctl，拼一份 `HardwareSnapshot`，GUI 与 CLI 共用。每个字段带 `Sample`（值、`access`、路径、hint）。缺权限、缺硬件、缺键都标原因，不填假数据。

不做的事：不调用 `dmidecode` / `lspci` / `smartctl` / `dpkg -l`。不做 OpenCL/Vulkan 计算基准。内存条厂商和料号只解析 SMBIOS Type 16/17，不扫 I2C SPD。

## 目标分层

| 层 | 职责 | 禁止事项 |
| --- | --- | --- |
| `access` | 读文件、把 `io::Error` 收成 `AccessKind`、检测 uid/组 | 业务字段拼装 |
| `probes::*` | 一类内核 ABI 一个文件 | `Command::new("lspci")` 之类外部进程 |
| `snapshot` | 拼装 `HardwareSnapshot`、`refresh_live`、环境摘要 | 直接画 UI |
| `export` / `bench` | 纯函数，CLI/GUI/测试共用同一份快照 | 弹窗；不要为文本/CSV 再扫一遍内核 |
| `record` / `alerts` | 任务栏 JSONL、阈值告警 JSONL | 采集时改硬件状态 |
| `elevate` | 用户点击或 `aida elevate` 时 `exec` 替换进程 | 采集路径里 spawn sudo |
| `doctor` | 读 os-release、扫 glibc 与 GUI `.so`，给出 apt 或 dnf/yum | 调用 `lsb_release` / `ldd` |
| `ui` | egui 布局。聚焦约 2 秒一拍，失焦约 10 秒 | 直接 `fs::read_to_string` 读硬件；悬停动画（会把软件渲染打满） |

## 数据模型

```text
Sample<T> { value, access, source, hint? }
```

| `AccessKind` | 含义 | JSON |
| --- | --- | --- |
| `Ok` | 读到了。空文件仍是 `Ok`，`value` 为空 | `ok` |
| `PermissionDenied` | 节点在，当前用户不能读 | `permission_denied` |
| `NotFound` | 文件或目录本身没有 | `not_found` |
| `Absent` | 文件在，里面没有这个键。Debian 的 `/etc/os-release` 没有 `ID_LIKE` 用这个，不用 `NotFound` | `absent` |
| `Unsupported` | 内核明确不提供该属性。网卡 `speed=-1` 或 `EINVAL`（os error 22）走这里 | `unsupported` |
| `Error` | 其它 IO 失败 | `error` |

构造：`Sample::ok` / `missing` / `denied` / `absent` / `unsupported` / `error`。`hint_for` 只给权限不足和「文件不存在」补一句原因；`Absent` 的 hint 由调用方写（例如「os-release 无 ID_LIKE」）。

## 给人看的文案

三套出口，不要混用：

| 函数 | 用在 | 空值 / 失败 |
| --- | --- | --- |
| `display()` | GUI 与 HTML 单元格 | 有值就显示值。`Ok` 但内容空是 `—`。`NotFound` 是 `[不存在]`。`Absent` 是 `[未设置]`。`PermissionDenied` 走 `compact()`，是 `[权限不足]`。`Unsupported` 用 hint（没有 hint 时 `[不支持]`）。`Error` 走 `friendly_io_error` |
| `compact()` | 文本 / CSV / Markdown 摘要 | `Ok` 空和 `NotFound` 都是 `—`。`Absent` 是 `[未设置]`。`PermissionDenied` 是 `[权限不足]`。`Unsupported` 是 `[不支持]`。`Error` 是 `[读取失败]`。不展开 hint |
| `access_label()` | 权限条、note | `NotFound` 只写 `[不存在]`，不拼路径。`PermissionDenied` 带 hint，没有 hint 时带 `source`。`Absent` / `Unsupported` 用 hint。JSON 另外保留原始 `value`、`source`、`hint` |

sysfs 开关不要把内核原文直接给用户。`cpu::display_sysfs_token` 把 `0`/`off` 显示成「关」，`1`/`on` 显示成「开」，`notsupported` 显示成「内核不支持」，`forceoff` 显示成「强制关闭」。JSON 里的 `value` 保持原文。

`friendly_io_error` 不把 `/proc` 路径和 errno 原文铺到界面上。`EINVAL` / os error 22 写成「内核不支持此读取（loopback / 虚拟设备常见）」。`ENODEV` / os error 19 写成「设备不存在」。其它失败是 `[读取失败]`。

## 缺数据时怎么呈现

云主机经常同时没有 DMI、GPU 和 hwmon。这是环境不适用，不是采集崩溃。

- `environment_headline()`：顶栏一行，只拼有缺口的短语，例如「本环境：虚拟机 · 无 DMI · 无 GPU · 无温度传感器」。虚拟机看 `cpu.hypervisor`；「无 DMI」要 `sys_vendor` 和 `product_name` 都空；「无 GPU」是设备列表空；「无温度传感器」是 hwmon 与 thermal zone 都空。四项都没有则返回 `None`。
- `environment_cards()`：每条缺口一句原因，非 root 再加一句权限说明。放在「权限与环境说明」折叠里，以及 HTML 顶部的「本环境摘要」。不要在顶栏把这几段同时铺开。
- HTML 与文本 / CSV / Markdown 判定「无 DMI」更严：`sys_vendor`、`product_name`、`bios_vendor`、`board_name` 四个都空。HTML 用一个 `<details class="gap">`「本环境不可用」，提示只写一次。文本摘要清掉逐项 `—`，收成一行「本环境无 DMI/SMBIOS」。
- 无 GPU、无传感器：HTML 同样折叠。GUI 对应页给一句说明。顶栏 `TEMP —` 可点进传感器页。
- 普通用户：权限条只显示「普通用户 uid=…」，旁边是「提权后重新采集」。DMI 序列号、SMBIOS、SMART、iomem 真实地址可能被隐藏，写在折叠说明里。

## iomem 大小

`/proc/iomem` 在非 root 下经常是 `00000000-00000000`。这时 `start==end==0`，`size` 必须是 0。摘要按名字合并时，`count` 仍是条目数，`size` 保持 0。`format_iomem_size(0)` 显示 `—`。不要用 `end-start+1` 把 3 条 System RAM 显示成 `3.00 B`。

## 已装软件

对标 AIDA64 的软件清单，但不调用包管理器命令。

- Debian 系读 `/var/lib/dpkg/status`，只要 `Status: install ok installed`。
- Alpine 读 `/lib/apk/db/installed`。
- 夹具测试时，`ProbeCtx.etc` 不是 `/etc`，则到同级的 `var/lib` 或 `lib/apk/db`。
- `package_count` 是全量个数；`packages` 最多 256 条（按名字排序后截断）。
- OS 页先显示发行版和这份清单。内核 / 安全 / cgroup 原始项放在折叠「不是软件清单」里，默认收起。

## 数据采集逻辑

启动时 `HardwareSnapshot::collect` 走一遍全量 probe。之后 GUI 只调用 `refresh_live`，不重扫启动清单里的静态设备。

1. `ProbeCtx::live()` 指向真实 `/proc` `/sys` `/dev` `/etc` `/usr/share`。
2. 每个 probe `collect(ctx) -> *Info`：
   - 枚举目录（`hwmonN`、PCI slot、`nvmeN`）
   - 对每个属性调用 `read_trimmed` / `read_bytes`
   - 数值字段在 probe 内换算（温度 m°C → °C，块设备 `size` 扇区 → 字节）
3. 失败不 panic：`Sample.value = None`，`hint` 写给人看的原因。
4. GUI 轮询：窗口聚焦约 2s，失焦约 10s。每一拍都更新 hwmon 与告警、电源、RAPL 差分、PSI、`buses.devcoredump`。界面关掉悬停动画，避免 llvmpipe 按显示器刷新率空转。
5. 快路径（`full=false`；失焦时永远走这里）：CPU 利用率（`/proc/stat`）和 cpuidle 当前驱动。当前频率只对启动时已经读到 cpufreq 的核更新，虚拟机不再逐核打开 `scaling_*`。governor / online 不在这一拍读。GPU 忙闲、显存、时钟和连接器状态（不重读 EDID，不扫 PCI 回退）。网卡计数优先一次 `/proc/net/dev`，按名字查表，没有该文件才退回每块网卡的 `statistics/*_bytes`；同时读 sockstat、snmp、conntrack。不读 TCP 表、调优项、operstate。`/proc/diskstats` 吞吐（不重扫 queue / loop / mapper）。meminfo / vmstat。loadavg / uptime / entropy。已有 zram 的 `mm_stat`。
6. 慢路径：聚焦且 `poll_tick % 30 == 0` 时 `full=true`（约 60 秒一次），整份替换 CPU、GPU、网络、块设备、内存、电源管理、时钟、EDAC、文件系统、IRQ、平台、zram、sysctl、cgroup、security。软件只更新 taint / oops / load，不重读已装包和 `config.gz`。PCI / USB / DMI / virtio / KVM / IOMMU / MD / SCSI / iSCSI / 模块 / iomem / ATA / crypto 只在启动的 `collect` 里扫，不进 `refresh_live`。nfs / cifs / fuse 不调用 `statvfs`。IRQ 亲和只打开计数最高的 48 条。`conf/all` 的接口差异先看物理网卡，veth/docker 等靠后，最多比较 48 个接口。veth/cni/cali 等高基数接口不逐个打开 sysfs，计数来自同一次 `/proc/net/dev`（快路径用名字查表，不再线性扫）。docker0、`br-*`、virbr 仍读 sysfs，网桥不丢。`br0` 不是这个前缀。

## 夹具

`ProbeCtx` 的五个根可以换成临时目录，单元测试不读本机。`etc` 不是 `/etc` 时，软件包清单改读同级 `var/lib/dpkg/status` 或 `lib/apk/db/installed`。`cargo test --test live_collect` 才读真实内核，用来确认报告里有包清单、空 DMI 不刷屏、iomem 隐藏区间不显示成若干字节。

## 各 probe 内核接口

| Probe | 主路径 | 补充 |
| --- | --- | --- |
| CPU | `/proc/cpuinfo`，`/sys/devices/system/cpu/cpuN/` | topology；cpuidle；全局 `cpuidle/current_driver`（`none` 合法）/`current_governor`/`available_governors`；GUI 快路径见上文 `refresh_live`，慢路径才替换整份 CPU 报告；cache；vulnerabilities；`smt/`；`isolated`；`online`/`offline`/`possible`/`present`/`kernel_max`/`enabled`；`nohz_full`（空或缺失=无）；`modalias`（界面截断）；cpufreq `policyN`；schedstat；不 dump `hotplug/states` |
| DMI | `/sys/class/dmi/id/*` | `/sys/firmware/dmi/tables/DMI` SMBIOS 结构；Type 0 BIOS ROM（`(n+1)*64` KiB，`0xFF`→扩展 WORD `0x18` bits13:0 数值、bits15:14 `00b`=MiB/`01b`=GiB）与 Release `0x14`/`0x15`；Type 4 处理器（插座/厂商/型号字符串号、最大/当前 MHz、Status bit6 已插入、核心/线程 BYTE `0xFF` 才读 3.0 WORD）；Type 7 缓存（级别=Config bits2:0+1，大小 `0xFFFF`→扩展 `0x17`）；Type 9 系统插槽（Usage `0x03` Available / `0x04` In use，类型 `0x09`=Proprietary / `0xB8`=PCIe Gen 4 / `0xBE`=PCIe Gen 5，PCI 段/总线/设备 length≥`0x11`，全 `FF`=无地址，最多 16 条）；Type 8 端口连接器（外部连接器优先，`0x0B`=RJ-45 / `0x12`=USB / `0x23`=USB-C，端口 `0x10`=USB / `0x1F`=Network）；Type 11 OEM 字符串（Count `0x04`）；Type 13 BIOS 语言（当前语言 `0x15`）；Type 32 启动状态（`0x0A`）；Type 41 板载设备（`0x05` bit7 启用）；Type 39 电源（最大功率 `0x0C`，仅 `0x8000` 未知）；Type 43 TPM（Vendor ID 4 ASCII、Spec `0x08`/`0x09`）；Type 12 系统配置选项（Count `0x04`）；Type 22 便携电池（化学 `0x09`，`0x02` 读 SBDS `0x14`；容量 `0x0A` 仅 `0` 未知、乘数 `0x15`；电压 `0x0C`）；Type 23 系统复位（Capabilities `0x04` bit0 启用 / bit5 看门狗，WORD `0xFFFF` 未知）；Type 24 硬件安全（Settings `0x04` 两比特一组）；Type 26 电压探头（毫伏，WORD `0x8000` 未知，Nominal `0x14`）；Type 27 冷却装置（类型/状态 `0x06`，Nominal Speed `0x0C` rpm `0x8000` 未知，Description `0x0E` length≥`0x0F`）；Type 28 温度探头（十分之一摄氏度，有符号，`0x8000` 未知）；Type 3 机箱（Type 字节 bit7 Lock、低 7 位类型 `0x17`=Rack Mount，高度 `0` 未指定，SKU 在 `0x15+n*m`）；Type 25 定时开机（BCD，月=`00` 未排程）；Type 29 电流探头（毫安，WORD `0x8000` 未知）；Type 38 IPMI（KCS/SMIC/BT/SSIF，NV `0xFF`=无，基址 QWORD bit0=`1` 为 I/O）；Type 18 32 位内存错误（类型/粒度/操作，Syndrome=`0` 未知，地址 DWORD 仅 `0x80000000` 未知）；Type 19 内存阵列映射（DWORD 单位 KB：起始×1024、结束×1024+1023，`0xFFFFFFFF` 读 2.7 扩展 QWORD 字节）；Type 21 板载指针（`0x03`=Mouse / `0x07`=Touch Pad，接口 `0x04`=PS/2 / `0xA2`=USB）；Type 20 内存设备映射（DWORD KB 同 Type 19，扩展 QWORD 在 `0x13`/`0x1B` 需 length≥`0x23`，行/交错 `0xFF` 未知或未交错）；Type 30 带外远程访问（Connections bit0 入站 / bit1 出站）；Type 33 64 位内存错误（地址 QWORD 仅 `0x8000000000000000` 未知，分辨率 DWORD 仅 `0x80000000` 未知）；Type 34 管理设备（类型 `0x04`=LM78 / `0x08`=ADM9240，地址 DWORD `0x06`，地址类型 `0x03`=I/O Port / `0x05`=SMBus）；Type 35 管理组件（阈值句柄 `0xFFFF`=无）；Type 37 内存通道（`0x03`=RamBus / `0x04`=SyncLink，设备条目 load+handle）；Type 36 管理阈值（WORD 仅 `0x8000` 未指定）；Type 40 附加信息（引用句柄/偏移/字符串）；Type 42 管理控制器主机接口（`<=0x3F`=MCTP / `0x40`=Network，设备 `0x00`=USB / `0x03`=PCI）；Type 16 Physical Memory Array（位置 `0x06`=PCI add-on / ECC / 最大容量 / 槽位数）；Type 17 按 DSP0134：速度 `0x15`（`0xFFFF`→扩展 `0x54`）、类型 `0x12`、外形偏移 `0x0E`（`0x09`=DIMM）、配置速度 `0x20`（`0xFFFF`→扩展 `0x58`）、Attributes `0x1B` rank、Size=`0` 空槽 / `0xFFFF` 已装未知容量；厂商/序列/料号用字符串号（`0`=未用）；不调用 dmidecode，不扫 I2C SPD |
| hwmon | `/sys/class/hwmon/hwmonN/*_input` | thermal_zone + cooling_device |
| NVMe | `/sys/class/nvme/nvmeN/` | `NVME_IOCTL_ADMIN_CMD` Get Log Page 0x02（ioctl request `as _`，兼容 musl `c_int` / glibc `c_ulong`） |
| GPU | `/sys/class/drm/cardN`，PCI class `0x03` | amdgpu busy/vram、i915/xe 频率、连接器 EDID、`/proc/driver/nvidia` |
| Net | `/sys/class/net/*/statistics` | getifaddrs；queues；sockstat；snmp；softnet；bridge/bond；conntrack count/max/buckets/established timeout；tcp congestion；`/proc/net/netstat`；snmp6；net.core busy_poll/busy_read/dev_weight/rps_sock_flow_entries/netdev_tstamp_prequeue/message_cost/message_burst；ipv6_route；if_inet6；tcp knobs/rmem/notsent_lowat/adv_win_scale/max_tw_buckets/tcp_mem/udp_mem/orphans/dsack/autocorking/retries1/early_retrans/frto/min_tso/pacing_ss/pacing_ca/invalid_ratelimit/orphan_retries/rfc1337/ecn_fallback/abort_overflow/no_metrics/challenge_ack（INT_MAX 不额外收紧）/thin_linear/limit_output/comp_sack/fwmark/early_demux/app_win/base_mss/min_snd_mss/reordering/recovery/tfo_blackhole/max_reordering/tso_win_divisor/udp_early_demux/syn_linear/fwd_pmtu/no_ssthresh/min_rtt_wlen/mtu_probe_floor/tso_rtt_log/shrink_window/l3mdev_accept/migrate_req/reflect_tos/rto_min_us/plb_enabled/backlog_ack_defer；udp_rmem_min/udp_wmem_min；udp_l3mdev_accept；fwmark_reflect；tcp_workaround_signed_windows（`0` RFC）/tcp_stdurg（`0` BSD）/tcp_available_ulp/tcp_plb_cong_thresh/tcp_allowed_congestion_control/tcp_plb_idle_rehash_rounds/tcp_plb_rehash_rounds/tcp_plb_suspend_rto_sec/tcp_pingpong_thresh/tcp_retrans_collapse/tcp_probe_interval/tcp_probe_threshold/tcp_ehash_entries/tcp_child_ehash_entries（`0` 沿用父表）/udp_hash_entries/ip_autobind_reuse（`0` 不复用 TIME_WAIT）/tcp_fack（`0` 关）/tcp_low_latency（`0` 吞吐）；ping_group_range；icmp_ratemask；icmp_errors_use_inbound_ifaddr（`0` 出接口）；tcp6/udp6/raw/udplite/raw6/udplite6；xfrm_stat；ptype（function 取末 token）；fib_triestat 主表 Leaves（不读 fib_trie）；igmp6 按接口名去重；ip_tables_names（空=未加载，denied≠空）；connector 名；protocols；rt6_stats 第 6 列 dst cache；rp_filter/use_tempaddr/accept_dad/addr_gen_mode/accept_ra_defrtr/router_solicitations/dad_transmits/ndisc_notify/accept_ra_pinfo/enhanced_dad/accept_ra_mtu/keep_addr_on_down/accept_ra_min_hop_limit/accept_ra_min_lft/accept_ra_rt_info_min_plen/accept_ra_rt_info_max_plen/accept_ra_rtr_pref/accept_ra_from_local/accept_redirects/drop_unsolicited_na/drop_unicast_in_l2_multicast/force_tllao/accept_untracked_na/proxy_ndp/ndisc_tclass/suppress_frag_ndisc/optimistic_dad/accept_source_route/use_optimistic/ignore_routes_with_linkdown/ndisc_evict_nocarrier/disable_policy/mc_forwarding/force_forwarding/temp_valid_lft/temp_prefered_lft/disable_xfrm/skip_notify_on_dev_down/regen_max_retry/max_desync_factor/ioam6_enabled/seg6_enabled/rpl_seg_enabled/ra_honor_pio_life 含与 `conf/all` 不同的接口；igmp 只计接口头行；IPv6 accept_ra/autoconf/hop_limit/accept_dad/addr_gen_mode/max_addresses/force_mld_version/enhanced_dad/auto_flowlabels/flowlabel_consistency/idgen_retries/idgen_delay/ip6frag_time；ip6frag high/low；ipfrag high/low/time/max_dist；ip_no_pmtu_disc；fib_multipath_hash_policy；fib_notify_on_flag_change；icmp_ratelimit；icmp_echo_ignore_all/enable_probe；ip_default_ttl；ip_unprivileged_port_start；bindv6only；ip_nonlocal_bind；ip_dynaddr；ip_forward_update_priority；不 dump `tcp_fastopen_key`/`netdev_rss_key`/`stable_secret` |
| USB | `/sys/bus/usb/devices`（跳过 `*:*.*` 接口节点） | `usb.ids` 名称 |
| Input | `/proc/bus/input/devices` | handlers → keyboard/mouse/js |
| NUMA | `/sys/devices/system/node/nodeN` | meminfo / cpulist / distance |
| Memory | `/proc/meminfo` | hugepages + THP defrag；buddyinfo；zoneinfo；vmstat；KSM；DirectMap；`bus/memory_tiering/devices`；GUI/HTML 内存页同时展示 DMI Type 16/17 DIMM（对标 AIDA64 Memory/SPD） |
| zmem | `/sys/block/zramN` + `module/zswap/parameters` | mm_stat；不调用 zramctl |
| EDAC | `/sys/devices/system/edac/mc/mcN` | ce_count / ue_count |
| Power | `/sys/class/power_supply` | 电池容量/能量、AC online |
| PM | `/sys/power` | state / mem_sleep / suspend_stats；wakeup |
| RAPL | `/sys/class/powercap/*/energy_uj` | 差分瓦特；回绕用 `max_energy_range_uj` |
| Audio | `/proc/asound/cards` | `/sys/class/sound/cardN/id` |
| Firmware | `/sys/firmware/efi` | SecureBoot；ACPI 表名；pm_profile；TPM；hwrng；pstore；`class/firmware/timeout`；memmap；devicetree `model`（去尾 NUL；x86 常无） |
| Filesystems | `/proc/self/mountinfo` | `/proc/swaps`；`statvfs`；ext4 sysfs；xfs stats；nfsd；fuse connections |
| Modules | `/proc/modules` | 按名称排序 |
| Clock | `clocksource0/current_clocksource` | `/sys/class/rtc`；`/sys/class/ptp`；`/sys/class/pps`；`clockevents` |
| iomem | `/proc/iomem` | `/proc/ioports`；非 root 起止常为 `0-0`，此时 `size=0`，界面和 HTML 显示 `—`，不用条目数冒充字节 |
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
| iSCSI | `/sys/class/iscsi_{transport,host,session,endpoint,iface,connection,flashnode}`；旧内核回退 `bus/iscsi_flashnode/devices` | 不调用 iscsiadm |
| Platform | watchdog / backlight / leds / i2c | ACPI/PnP 设备计数；workqueue；perf event_source；MSR；vtconsole；`bus/platform/devices` 名（最多 16）；`class/wakeup` 只计数不展开 wakeupN；不调用 i2cdetect |
| Periph | `/proc/dma`；`/sys/class/dma` | PWM `pwmchipN/npwm`；IIO `name`；nvmem `type`（不读二进制）；regulator 电压；devlink `status`；`pci_bus` cpulist；不调用 `devlink` |
| Buses | rfkill / bluetooth / thunderbolt / V4L / MMC / MEI / ttyS / misc / hidraw / gpio / mtd / IB | ieee80211 / typec / udc / dax / wmi / spi / serio / ubi / scsi_generic / wwan / ppp / phy / remoteproc / extcon / tee / mdio_bus / spi_master / i2c-dev / nvme-subsystem / w1 / macvtap / nvme-generic / iscsi_endpoint / iscsi_iface / iscsi_connection / container / iscsi_flashnode / nd / dma_heap / cxl（`bus/cxl/devices`，回退 `class/cxl`）/ devfreq / fpga（`fpga_manager`/`fpga_bridge`/`fpga_region`）/ gnss / rpmsg / devcoredump / scsi_disk / scsi_tape / graphics / cec / media / nbd（`class/block/nbdN`，回退 `sys/block`）/ vfio（合并 `class/vfio` 与 `class/vfio-dev`）/ mdev（`bus/mdev/devices`，回退 `class/mdev`）/ vhost（`class/misc/vhost-*` 与 `/dev/vhost-*`，没有 `class/vhost`）/ fc（`fc_host`/`fc_remote_ports`/`fc_vports`，没有统一 `class/fc`）/ accel（`class/accel`）/ vdpa（`bus/vdpa/devices`，回退 `class/vdpa`）/ uio（`class/uio`）/ auxiliary（`bus/auxiliary/devices`，回退 `class/auxiliary`）/ usbmon（`class/usbmon`）/ counter（`bus/counter/devices`，回退 `class/counter`）/ drm_dp_aux_dev（`class/drm_dp_aux_dev`）/ mhi（`bus/mhi/devices`，回退 `class/mhi`）/ ipmi（合并 `class/ipmi` 与 `class/ipmi_bmc`）/ usb_role（`class/usb_role`）/ i3c（`bus/i3c/devices`，回退 `class/i3c`）/ vduse（`class/vduse`）/ mux（`class/mux`）/ soundwire（`bus/soundwire/devices`，回退 `class/soundwire`）/ rc（`class/rc`）/ stm（合并 `class/stm` 与 `class/stm_source`）/ peci（`bus/peci/devices`，回退 `class/peci`）/ wakeup（`class/wakeup`，只列名）/ msr（`class/msr`，只列名，不 dump `/dev/cpu/N/msr`）/ dpll（`class/dpll`）/ iommu（`class/iommu`，与 `iommu_groups` 不同）/ hid（`bus/hid/devices`，回退 `class/hid`）/ memory（`bus/memory/devices`，回退 `class/memory`，最多 8 个名）/ firewire（`bus/firewire/devices`，回退 `class/firewire`）/ greybus（`bus/greybus/devices`，回退 `class/greybus`）/ rapidio（`bus/rapidio/devices`，回退 `class/rapidio`）/ ulpi（`bus/ulpi/devices`，回退 `class/ulpi`）/ spmi（`bus/spmi/devices`，回退 `class/spmi`）/ pci_epc（`class/pci_epc`）/ ptp（`class/ptp`）/ pps（`class/pps`）/ tpm（合并 `class/tpm` 与 `class/tpmrm`）/ firmware_attributes（`class/firmware-attributes`）/ pci_epf（`bus/pci-epf/devices`，回退 `class/pci_epf`）/ slimbus（`bus/slimbus/devices`，回退 `class/slimbus`）/ memstick（合并 `bus/memstick/devices` 与 `class/memstick_host`）/ siox（`bus/siox/devices`，回退 `class/siox`）/ hsi（`bus/hsi/devices`，回退 `class/hsi`）/ amba（`bus/amba/devices`，回退 `class/amba`）/ fsi（合并 `bus/fsi/devices` 与 `class/fsi-master`）/ ppdev（`class/ppdev`）/ pcmcia（`bus/pcmcia/devices`，回退 `class/pcmcia`）/ vmbus（`bus/vmbus/devices`，回退 `class/vmbus`）/ bcma（`bus/bcma/devices`，回退 `class/bcma`）/ intel_th（`bus/intel_th/devices`，回退 `class/intel_th`）/ coresight（`bus/coresight/devices`，回退 `class/coresight`）/ ntb（`bus/ntb/devices`，回退 `class/ntb`）/ xen（`bus/xen/devices`，回退 `class/xen`）/ gameport（`bus/gameport/devices`，回退 `class/gameport`）/ dfl（`bus/dfl/devices`，回退 `class/dfl`）/ ssb（`bus/ssb/devices`，回退 `class/ssb`）/ fsl-mc（`bus/fsl-mc/devices`，回退 `class/fsl-mc`）/ mcb（`bus/mcb/devices`，回退 `class/mcb`）/ usb4（`bus/usb4/devices`，回退 `class/usb4_port`）/ ishtp（`bus/ishtp/devices`，回退 `class/ishtp`）/ scmi（`bus/scmi/devices`，回退 `bus/scmi_protocol/devices`）名（缺类合并一条 note）；`tun` / `nvme-fabrics` 看 `class/misc` 与 `/dev` 节点；`/proc/tty/drivers`；不调用 setserial/`iw`；ttyS `type=0` 跳过；不 dump `serial-base` |
| Sysctl | `/proc/sys/{fs,vm,kernel}` | file-nr；pid_max；aio；inotify；boot_id；nmi_watchdog；unknown_nmi_panic；panic（可为负）；sysrq；keys；SysV IPC（含 shmall/msgmnb/msgmni）；mqueue；consoles；sched_rt（`-1` 不限）；SCHED_DEADLINE period min/max；OOM/laptop/kexec_load_disabled；printk_ratelimit；cfs bandwidth；oops_limit；uffd；dentry-state（一次读取两列）；inode-state（inuse=nr_inodes-nr_unused）；pty max/nr；overflowuid/gid；vsyscall32；ldisc_autoload；io_uring_disabled/group（`-1` 未绑定组）；dirty_bytes/dirty_background_bytes（`0` 用 ratio）；overcommit_kbytes（`0` 用 ratio）；pipe-user-pages-soft/hard（hard=`0` 不限）；compact_unevictable_allowed；compaction_proactiveness；page_lock_unfairness；min_slab_ratio/min_unmapped_ratio；watermark_boost_factor；core_pipe_limit（`0` 不限）；printk_devkmsg；task_delayacct；acct 三 token；zone_reclaim_mode；mount-max；RNG write_wakeup_threshold / urandom_min_reseed_secs（不 dump `uuid`）；shm_rmid_forced；memfd_noexec；dirtytime_expire_seconds；soft_watchdog；watchdog_cpumask；panic_on_rcu_stall；warn_limit（`0` 不限）；kexec_load_limit_panic/reboot（`-1` 不限）；split_lock_mitigate；hung_task_warnings（有符号）；hung_task_check_count；hung_task_check_interval_secs（`0` 用 timeout）；hung_task_all_cpu_backtrace；hardlockup_all_cpu_backtrace；print-fatal-signals（连字符路径）；bpf_stats_enabled；core_sort_vma；max_rcu_stall_to_panic（`0` 不升级）；panic_print 位图；panic_on_io_nmi；panic_on_unrecovered_nmi；oops_all_cpu_backtrace；softlockup_all_cpu_backtrace；io_delay_type；extfrag_threshold；stat_interval；printk_delay（`0` 无额外延迟）；max_lock_depth；perf_event_mlock_kb；perf_event_max_stack；perf_event_max_contexts_per_stack；hugetlb_optimize_vmemmap（`0` 关）；percpu_pagelist_high_fraction（`0` 用默认）；numa_stat；numa_balancing_promote_rate_limit_MBps（文件名大写 MBps）；legacy_va_layout（`0` 新布局）；hugetlb_shm_group（`0` 无 gid）；core_file_note_size_limit；auto_msgmni（`0` 不自动重算）；numa_zonelist_order；lowmem_reserve_ratio；nr_overcommit_hugepages（`0` 不额外 overcommit）；nr_hugepages_mempolicy（`0` 不按 mempolicy 拆）；nr_hugepages（`0` 无静态预留）；acpi_video_flags（`0` 无特殊标志）；bootloader_type/version（x86 启动协议，`type=0` 未声明）；firmware_config force/ignore_sysfs_fallback；real-root-dev（`0` 未设）；sched_schedstats（无 CONFIG_SCHEDSTATS 时 NotFound）；traceoff_on_warning（无 tracing 时 NotFound）；kernel.arch（内核自称架构）；`/proc/key-users` 计数（失败不是 0）；不读 `compact_memory`/`cad_pid`/`stat_refresh`；不 dump `mmap_rnd_bits` |
| Cgroup | `/sys/fs/cgroup` | v2 controllers / memory.current；第一层 `.slice`/`.scope`；`/proc/cgroups` enabled=1 |
| Security | lockdown / yama / kptr / dmesg / FIPS / bpf / perf / fs.protected_* | bpf_jit_enable/harden；binfmt_misc status（不写 register）；seccomp `actions_avail`（不 dump `actions_logged`）；不调用 sysctl/aa-status |
| Crypto | `/proc/crypto` | 非 internal 截断 32 条 |
| Ns | `/proc/self/ns` | `max_*_namespaces` |
| Software | `/etc/os-release`，`/proc/meminfo` | loadavg / tainted / LSM / entropy / machine-id；`/proc/config.gz` 读字节长度；`/proc/locks`；oops/kexec；`/proc/filesystems`；`cpu_byteorder`/`address_bits`/`profiling`；`ostype`；os-release 缺键为 `Absent`；已装包来自 dpkg status 或 apk db，列表最多 256。GUI 周期刷新不重读已装包和 config.gz |

## 界面

主窗口有边框、可缩放，最小约 800×560。置顶 Dock 窄条默认关，勾选「置顶任务栏」才弹出。

布局四块：

| 区域 | 做法 |
| --- | --- |
| 顶：权限 | 一行身份 +「提权后重新采集」。`environment_headline()` 有内容时再加一行橙色摘要。长说明放在折叠里 |
| 顶：任务栏 | CPU / 内存 / 网络 / 磁盘 / 温度 / loadavg，`horizontal_wrapped`，不用固定 36px 把右侧裁掉。`TEMP —` 点进传感器页 |
| 左：导航 | `SidePanel` 自己一层纵向 `ScrollArea`（`id_salt=nav-scroll`）。项：摘要、处理器、主板/DMI、内存、显示适配器、传感器、电源、存储、文件系统、网络、USB、输入、声卡、PCI、平台、NUMA、操作系统、历史记录、基准、导出 |
| 右：详情 | 每个 `Nav` 一层纵向 `ScrollArea`，`id_salt` 带页面 id，切页不带着上一页的滚动位置。`scroll_bar_visibility=AlwaysVisible`。内容宽度用 `set_width(available_width())` |

滚动约束：详情区不要用 `ScrollArea::both()`。横向滚动里 `available_width()` 是无限的，再 `set_min_width` 会把内容高度算坏，滚不到页底。折线图 `allow_scroll(false)`，滚轮留给外层页面。CPU / OS 页不要再套内层 `ScrollArea`。

其它：

- 曲线：温度、CPU 利用率、网卡/磁盘吞吐、RAPL 各保留约 120 个点。历史页横轴是采样的 unix 秒，刻度格式化成本地 `HH:MM:SS`。记录文件路径放在折叠里。
- 重绘与采集同拍：聚焦约 2s，失焦约 10s。悬停动画时间为 0，形状羽化和像素抖动关掉。指针移动最多约 2.5 次/秒触发重绘（拖拽约 20 次/秒）；点击、滚轮、按键不封顶。egui 在移动后再要的那一帧也等 400ms，避免事件变密后空转。提示不在鼠标移动时每帧请求重绘。`third_party/egui` 和 `third_party/egui-winit` 是这两处的同版本补丁。基准在后台线程跑，界面显示「正在跑」，跑完一次更新数字。
- 大机界面：网络页把只记计数的接口收进「虚拟接口」折叠，默认不建每口控件，曲线合成 virtual RX/TX。docker0、`br-*`、virbr 留在主表。逻辑 CPU 超过 32 个时一行一个，不再每核六个格子。
- 历史：默认记录。启动只读 JSONL 尾部最多 1800 条并裁掉更旧的磁盘内容；录满后每隔 256 条再裁回 cap。路径 `$AIDA_RECORD_LOG` 或 `$XDG_STATE_HOME/aida/history.jsonl`。
- 告警：对照 `*_max` / `*_crit` / `*_min`，状态变化写入 `$AIDA_ALERT_LOG` 或 `$XDG_STATE_HOME/aida/alerts.jsonl`。
- 中文：有 Noto / 文泉驿等 CJK 字体就作为回退。拉丁字母仍用内置字体。不要把 CJK 插到字体列表最前，否则每个英文标签都走那份大字体，鼠标移动时会占满一核。

## 报告导出

`export` 只读已经采好的 `HardwareSnapshot`，五种格式同一次采集：

| 格式 | 内容 |
| --- | --- |
| JSON | 全字段 + `access` / `source` / `hint`。`absent` 表示文件在但键空 |
| HTML | 单文件深色页。表格 `overflow-wrap`。缺 DMI / GPU / 传感器时用 `<details>`，长提示不按字段复制 |
| 文本 / CSV / Markdown | 同一份 `report_sections` 摘要。单元格用 `compact()`。CSV 表头 `section,key,value` |

GUI 写出目录，按顺序取第一个非空值：

1. `$AIDA_EXPORT_DIR`
2. `$XDG_DOCUMENTS_DIR`
3. `~/.config/user-dirs.dirs` 里的 `XDG_DOCUMENTS_DIR`
4. `~/Documents`

目录可以还不存在，点导出时 `create_dir_all`。成功或失败都显示完整路径。没有家目录时才退回当前目录。不要因为 `~/Documents` 还没建好就改写到 `~/.local/share/aida` 或安装目录。

CLI：`aida collect --format text` 打 stdout；`--text` / `--csv` / `--md` / `--html` / `--json` 写文件，FILE=`-` 也是 stdout。可以和 `--format` 并存，按出现顺序追加。不要为换格式再采集一次。`collect` 只调用一次 `HardwareSnapshot::collect`，不走 GUI 的 `refresh_live`。

## 命令行

| 命令 | 行为 |
| --- | --- |
| `aida` | 有 `DISPLAY` 或 `WAYLAND_DISPLAY` 且编进 `gui` feature 则开界面，否则打印 JSON |
| `aida gui` | 桌面界面 |
| `aida collect` | 一次全量采集 |
| `aida doctor` | 发行版 family、glibc、GUI `.so`；`--json` 打机器可读结果 |
| `aida bench` | `--quick` / `--cpu` / `--memory` / `--disk` / `--no-direct` |
| `aida elevate` | 见下一节。参数里再出现 `elevate` 直接拒绝 |
| `aida version` | 打印 `Cargo.toml` 版本 |

`AIDA_ALERT_LOG`、`AIDA_RECORD_LOG` 见界面一节。`AIDA_EXPORT_DIR` 只影响 GUI 写出目录，不影响 `collect` 的 stdout。

## 提权

`src/elevate.rs` 只在用户点击或 `aida elevate` 时 `exec` 替换进程。采集函数不 spawn sudo。

## 磁盘基准

`bench::run_direct`：`posix_memalign` + `O_DIRECT`。失败则 `direct_error` 非空，buffered 仍可用。

## 刻意未做

- GPU 计算基准（OpenCL/Vulkan）：会引入额外运行时，和可打包目标冲突。
- I2C SPD：内存条级厂商/料号只靠 SMBIOS Type 16/17。云主机没有 SMBIOS 时看不到料号，这是数据源没有，不是解析漏了。

## 打包

`./scripts/package.sh` 产出：

- `dist/aida-cli-<ver>-<arch>-{musl|gnu}`：`--no-default-features`，有 musl 工具链则静态
- `dist/AIDA_Linux-<ver>-<arch>.AppImage`：glibc + linuxdeploy 收集 OpenGL/xkb `.so`
- `dist/SHA256SUMS`

版本号只写在 `Cargo.toml`。本机菜单：`./scripts/install.sh`（默认 `~/.local`）。细节见 [PACKAGING.md](PACKAGING.md)。
