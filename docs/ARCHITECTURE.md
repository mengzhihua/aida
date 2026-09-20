# 模块架构

## 目标分层

| 层 | 职责 | 禁止事项 |
| --- | --- | --- |
| `access` | 读文件、翻译 `io::Error` 为 `AccessKind`、检测 uid/组 | 业务字段拼装 |
| `probes::*` | 一类内核 ABI 一个文件 | `Command::new("lspci")` 之类外部进程 |
| `snapshot` | 拼装 `HardwareSnapshot` | UI 字符串 |
| `export` / `bench` | 纯函数，方便 CLI/GUI/测试共用 | 弹窗；不要为文本/CSV 再扫一遍内核 |
| `ui` | egui 布局与 1Hz 刷新 | 直接 `fs::read_to_string` |

## 数据采集逻辑

1. `ProbeCtx::live()` 指向真实 `/proc` `/sys` `/dev` `/etc`。
2. 每个 probe `collect(ctx) -> *Info`：
   - 枚举目录（`hwmonN`、PCI slot、`nvmeN`）
   - 对每个属性调用 `read_trimmed` / `read_bytes`
   - 数值字段在 probe 内换算（温度 m°C → °C，块设备 `size` 扇区 → 字节）
3. 失败不 panic：`Sample.value = None`，`hint` 写给人看的原因。
4. GUI 热路径：`refresh_live(..., full=false)` 约 1Hz（失焦 2.5s）只更新 hwmon/告警、CPU 利用率与当前频率/governor、GPU 忙闲/显存（不重读 EDID）、网卡/磁盘计数差分、RAPL、meminfo/vmstat、loadavg、PSI、电源、zram mm_stat、`buses.devcoredump`。`full=true` 每 8 拍才重扫 TCP 调优项、`/proc/net/tcp*`、sysctl、IRQ 亲和、挂载 statvfs、zoneinfo、cgroup、security、clock/EDAC/platform/pm。避免每帧扫 PCI/USB/DMI/virtio/KVM/IOMMU/MD/SCSI/iSCSI/模块/iomem/ATA/crypto。

## 各 probe 内核接口

| Probe | 主路径 | 补充 |
| --- | --- | --- |
| CPU | `/proc/cpuinfo`，`/sys/devices/system/cpu/cpuN/` | topology；cpuidle；全局 `cpuidle/current_driver`（`none` 合法）/`current_governor`/`available_governors`；GUI 快路径 `cpu::refresh_runtime` 只更新 `/proc/stat` 利用率、当前频率与 governor；慢路径才替换整份 CPU 报告；cache；vulnerabilities；`smt/`；`isolated`；`online`/`offline`/`possible`/`present`/`kernel_max`/`enabled`；`nohz_full`（空或缺失=无）；`modalias`（界面截断）；cpufreq `policyN`；schedstat；不 dump `hotplug/states` |
| DMI | `/sys/class/dmi/id/*` | `/sys/firmware/dmi/tables/DMI` SMBIOS 结构；Type 0 BIOS ROM（`(n+1)*64` KiB，`0xFF`→扩展 WORD `0x18` bits13:0 数值、bits15:14 `00b`=MiB/`01b`=GiB）与 Release `0x14`/`0x15`；Type 4 处理器（插座/厂商/型号字符串号、最大/当前 MHz、Status bit6 已插入、核心/线程 BYTE `0xFF` 才读 3.0 WORD）；Type 7 缓存（级别=Config bits2:0+1，大小 `0xFFFF`→扩展 `0x17`）；Type 9 系统插槽（Usage `0x03` Available / `0x04` In use，类型 `0x09`=Proprietary / `0xB8`=PCIe Gen 4 / `0xBE`=PCIe Gen 5，PCI 段/总线/设备 length≥`0x11`，全 `FF`=无地址，最多 16 条）；Type 8 端口连接器（外部连接器优先，`0x0B`=RJ-45 / `0x12`=USB / `0x23`=USB-C，端口 `0x10`=USB / `0x1F`=Network）；Type 41 板载设备（`0x05` bit7 启用）；Type 39 电源（最大功率 `0x0C`，仅 `0x8000` 未知）；Type 16 Physical Memory Array（位置 `0x06`=PCI add-on / ECC / 最大容量 / 槽位数）；Type 17 按 DSP0134：速度 `0x15`（`0xFFFF`→扩展 `0x54`）、类型 `0x12`、外形偏移 `0x0E`（`0x09`=DIMM）、配置速度 `0x20`（`0xFFFF`→扩展 `0x58`）、Attributes `0x1B` rank、Size=`0` 空槽 / `0xFFFF` 已装未知容量；厂商/序列/料号用字符串号（`0`=未用）；不调用 dmidecode，不扫 I2C SPD |
| hwmon | `/sys/class/hwmon/hwmonN/*_input` | thermal_zone + cooling_device |
| NVMe | `/sys/class/nvme/nvmeN/` | `NVME_IOCTL_ADMIN_CMD` Get Log Page 0x02（ioctl request `as _`，兼容 musl `c_int` / glibc `c_ulong`） |
| GPU | `/sys/class/drm/cardN`，PCI class `0x03` | amdgpu busy/vram、i915/xe 频率、连接器 EDID、`/proc/driver/nvidia` |
| Net | `/sys/class/net/*/statistics` | getifaddrs；queues；sockstat；snmp；softnet；bridge/bond；conntrack count/max/buckets/established timeout；tcp congestion；`/proc/net/netstat`；snmp6；net.core busy_poll/busy_read/dev_weight/rps_sock_flow_entries/netdev_tstamp_prequeue/message_cost/message_burst；ipv6_route；if_inet6；tcp knobs/rmem/notsent_lowat/adv_win_scale/max_tw_buckets/tcp_mem/udp_mem/orphans/dsack/autocorking/retries1/early_retrans/frto/min_tso/pacing_ss/pacing_ca/invalid_ratelimit/orphan_retries/rfc1337/ecn_fallback/abort_overflow/no_metrics/challenge_ack（INT_MAX 不额外收紧）/thin_linear/limit_output/comp_sack/fwmark/early_demux/app_win/base_mss/min_snd_mss/reordering/recovery/tfo_blackhole/max_reordering/tso_win_divisor/udp_early_demux/syn_linear/fwd_pmtu/no_ssthresh/min_rtt_wlen/mtu_probe_floor/tso_rtt_log/shrink_window/l3mdev_accept/migrate_req/reflect_tos/rto_min_us/plb_enabled/backlog_ack_defer；udp_rmem_min/udp_wmem_min；udp_l3mdev_accept；fwmark_reflect；tcp_workaround_signed_windows（`0` RFC）/tcp_stdurg（`0` BSD）/tcp_available_ulp/tcp_plb_cong_thresh/tcp_allowed_congestion_control/tcp_plb_idle_rehash_rounds/tcp_plb_rehash_rounds/tcp_plb_suspend_rto_sec/tcp_pingpong_thresh/tcp_retrans_collapse/tcp_probe_interval/tcp_probe_threshold/tcp_ehash_entries/tcp_child_ehash_entries（`0` 沿用父表）/udp_hash_entries/ip_autobind_reuse（`0` 不复用 TIME_WAIT）/tcp_fack（`0` 关）/tcp_low_latency（`0` 吞吐）；ping_group_range；icmp_ratemask；icmp_errors_use_inbound_ifaddr（`0` 出接口）；tcp6/udp6/raw/udplite/raw6/udplite6；xfrm_stat；ptype（function 取末 token）；fib_triestat 主表 Leaves（不读 fib_trie）；igmp6 按接口名去重；ip_tables_names（空=未加载，denied≠空）；connector 名；protocols；rt6_stats 第 6 列 dst cache；rp_filter/use_tempaddr/accept_dad/addr_gen_mode/accept_ra_defrtr/router_solicitations/dad_transmits/ndisc_notify/accept_ra_pinfo/enhanced_dad/accept_ra_mtu/keep_addr_on_down/accept_ra_min_hop_limit/accept_ra_min_lft/accept_ra_rt_info_min_plen/accept_ra_rt_info_max_plen/accept_ra_rtr_pref/accept_ra_from_local/accept_redirects/drop_unsolicited_na/drop_unicast_in_l2_multicast/force_tllao/accept_untracked_na/proxy_ndp/ndisc_tclass/suppress_frag_ndisc/optimistic_dad/accept_source_route 含与 `conf/all` 不同的接口；igmp 只计接口头行；IPv6 accept_ra/autoconf/hop_limit/accept_dad/addr_gen_mode/max_addresses/force_mld_version/enhanced_dad/auto_flowlabels/flowlabel_consistency/idgen_retries/idgen_delay/ip6frag_time；ip6frag high/low；ipfrag high/low/time/max_dist；ip_no_pmtu_disc；fib_multipath_hash_policy；fib_notify_on_flag_change；icmp_ratelimit；icmp_echo_ignore_all/enable_probe；ip_default_ttl；ip_unprivileged_port_start；bindv6only；ip_nonlocal_bind；ip_dynaddr；ip_forward_update_priority；不 dump `tcp_fastopen_key`/`netdev_rss_key`/`stable_secret` |
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
| iSCSI | `/sys/class/iscsi_{transport,host,session,endpoint,iface,connection,flashnode}`；旧内核回退 `bus/iscsi_flashnode/devices` | 不调用 iscsiadm |
| Platform | watchdog / backlight / leds / i2c | ACPI/PnP 设备计数；workqueue；perf event_source；MSR；vtconsole；`bus/platform/devices` 名（最多 16）；`class/wakeup` 只计数不展开 wakeupN；不调用 i2cdetect |
| Periph | `/proc/dma`；`/sys/class/dma` | PWM `pwmchipN/npwm`；IIO `name`；nvmem `type`（不读二进制）；regulator 电压；devlink `status`；`pci_bus` cpulist；不调用 `devlink` |
| Buses | rfkill / bluetooth / thunderbolt / V4L / MMC / MEI / ttyS / misc / hidraw / gpio / mtd / IB | ieee80211 / typec / udc / dax / wmi / spi / serio / ubi / scsi_generic / wwan / ppp / phy / remoteproc / extcon / tee / mdio_bus / spi_master / i2c-dev / nvme-subsystem / w1 / macvtap / nvme-generic / iscsi_endpoint / iscsi_iface / iscsi_connection / container / iscsi_flashnode / nd / dma_heap / cxl（`bus/cxl/devices`，回退 `class/cxl`）/ devfreq / fpga（`fpga_manager`/`fpga_bridge`/`fpga_region`）/ gnss / rpmsg / devcoredump / scsi_disk / scsi_tape / graphics / cec / media / nbd（`class/block/nbdN`，回退 `sys/block`）/ vfio（合并 `class/vfio` 与 `class/vfio-dev`）/ mdev（`bus/mdev/devices`，回退 `class/mdev`）/ vhost（`class/misc/vhost-*` 与 `/dev/vhost-*`，没有 `class/vhost`）/ fc（`fc_host`/`fc_remote_ports`/`fc_vports`，没有统一 `class/fc`）/ accel（`class/accel`）/ vdpa（`bus/vdpa/devices`，回退 `class/vdpa`）/ uio（`class/uio`）/ auxiliary（`bus/auxiliary/devices`，回退 `class/auxiliary`）/ usbmon（`class/usbmon`）/ counter（`bus/counter/devices`，回退 `class/counter`）/ drm_dp_aux_dev（`class/drm_dp_aux_dev`）/ mhi（`bus/mhi/devices`，回退 `class/mhi`）/ ipmi（合并 `class/ipmi` 与 `class/ipmi_bmc`）/ usb_role（`class/usb_role`）/ i3c（`bus/i3c/devices`，回退 `class/i3c`）/ vduse（`class/vduse`）/ mux（`class/mux`）/ soundwire（`bus/soundwire/devices`，回退 `class/soundwire`）/ rc（`class/rc`）/ stm（合并 `class/stm` 与 `class/stm_source`）/ peci（`bus/peci/devices`，回退 `class/peci`）/ wakeup（`class/wakeup`，只列名）/ msr（`class/msr`，只列名，不 dump `/dev/cpu/N/msr`）/ dpll（`class/dpll`）/ iommu（`class/iommu`，与 `iommu_groups` 不同）/ hid（`bus/hid/devices`，回退 `class/hid`）/ memory（`bus/memory/devices`，回退 `class/memory`，最多 8 个名）/ firewire（`bus/firewire/devices`，回退 `class/firewire`）/ greybus（`bus/greybus/devices`，回退 `class/greybus`）/ rapidio（`bus/rapidio/devices`，回退 `class/rapidio`）/ ulpi（`bus/ulpi/devices`，回退 `class/ulpi`）/ spmi（`bus/spmi/devices`，回退 `class/spmi`）/ pci_epc（`class/pci_epc`）/ ptp（`class/ptp`）/ pps（`class/pps`）/ tpm（合并 `class/tpm` 与 `class/tpmrm`）名（缺类合并一条 note）；`tun` / `nvme-fabrics` 看 `class/misc` 与 `/dev` 节点；`/proc/tty/drivers`；不调用 setserial/`iw`；ttyS `type=0` 跳过；不 dump `serial-base` |
| Sysctl | `/proc/sys/{fs,vm,kernel}` | file-nr；pid_max；aio；inotify；boot_id；nmi_watchdog；unknown_nmi_panic；panic（可为负）；sysrq；keys；SysV IPC（含 shmall/msgmnb/msgmni）；mqueue；consoles；sched_rt（`-1` 不限）；SCHED_DEADLINE period min/max；OOM/laptop/kexec_load_disabled；printk_ratelimit；cfs bandwidth；oops_limit；uffd；dentry-state（一次读取两列）；inode-state（inuse=nr_inodes-nr_unused）；pty max/nr；overflowuid/gid；vsyscall32；ldisc_autoload；io_uring_disabled/group（`-1` 未绑定组）；dirty_bytes/dirty_background_bytes（`0` 用 ratio）；overcommit_kbytes（`0` 用 ratio）；pipe-user-pages-soft/hard（hard=`0` 不限）；compact_unevictable_allowed；compaction_proactiveness；page_lock_unfairness；min_slab_ratio/min_unmapped_ratio；watermark_boost_factor；core_pipe_limit（`0` 不限）；printk_devkmsg；task_delayacct；acct 三 token；zone_reclaim_mode；mount-max；RNG write_wakeup_threshold / urandom_min_reseed_secs（不 dump `uuid`）；shm_rmid_forced；memfd_noexec；dirtytime_expire_seconds；soft_watchdog；watchdog_cpumask；panic_on_rcu_stall；warn_limit（`0` 不限）；kexec_load_limit_panic/reboot（`-1` 不限）；split_lock_mitigate；hung_task_warnings（有符号）；hung_task_check_count；hung_task_check_interval_secs（`0` 用 timeout）；hung_task_all_cpu_backtrace；hardlockup_all_cpu_backtrace；print-fatal-signals（连字符路径）；bpf_stats_enabled；core_sort_vma；max_rcu_stall_to_panic（`0` 不升级）；panic_print 位图；panic_on_io_nmi；panic_on_unrecovered_nmi；oops_all_cpu_backtrace；softlockup_all_cpu_backtrace；io_delay_type；extfrag_threshold；stat_interval；printk_delay（`0` 无额外延迟）；max_lock_depth；perf_event_mlock_kb；perf_event_max_stack；perf_event_max_contexts_per_stack；hugetlb_optimize_vmemmap（`0` 关）；percpu_pagelist_high_fraction（`0` 用默认）；numa_stat；numa_balancing_promote_rate_limit_MBps（文件名大写 MBps）；legacy_va_layout（`0` 新布局）；hugetlb_shm_group（`0` 无 gid）；core_file_note_size_limit；auto_msgmni（`0` 不自动重算）；numa_zonelist_order；lowmem_reserve_ratio；nr_overcommit_hugepages（`0` 不额外 overcommit）；nr_hugepages_mempolicy（`0` 不按 mempolicy 拆）；nr_hugepages（`0` 无静态预留）；acpi_video_flags（`0` 无特殊标志）；bootloader_type/version（x86 启动协议，`type=0` 未声明）；firmware_config force/ignore_sysfs_fallback；real-root-dev（`0` 未设）；sched_schedstats（无 CONFIG_SCHEDSTATS 时 NotFound）；traceoff_on_warning（无 tracing 时 NotFound）；kernel.arch（内核自称架构）；`/proc/key-users` 计数（失败不是 0）；不读 `compact_memory`/`cad_pid`/`stat_refresh`；不 dump `mmap_rnd_bits` |
| Cgroup | `/sys/fs/cgroup` | v2 controllers / memory.current；第一层 `.slice`/`.scope`；`/proc/cgroups` enabled=1 |
| Security | lockdown / yama / kptr / dmesg / FIPS / bpf / perf / fs.protected_* | bpf_jit_enable/harden；binfmt_misc status（不写 register）；seccomp `actions_avail`（不 dump `actions_logged`）；不调用 sysctl/aa-status |
| Crypto | `/proc/crypto` | 非 internal 截断 32 条 |
| Ns | `/proc/self/ns` | `max_*_namespaces` |
| Software | `/etc/os-release`，`/proc/meminfo` | loadavg / tainted / LSM / entropy / machine-id；`/proc/config.gz` 读字节长度；`/proc/locks`；oops/kexec；`/proc/filesystems`；`cpu_byteorder`/`address_bits`/`profiling`；`ostype` |

## 界面

- 左：`SidePanel` 树（摘要 / CPU / DMI / 内存 / GPU / 传感器 / 电源 / 存储 / 文件系统 / 网络 / USB / 输入 / 声卡 / PCI / 平台 / NUMA / OS / 基准 / 导出）
- 右：对应面板；温度、CPU 利用率、网卡/磁盘吞吐、RAPL 瓦特用 `egui_plot` 保留约 120 个点；界面重绘间隔与采集一致（前台 1s，失焦 2.5s），避免空转 500ms 帧
- 顶：权限条 +「以管理员身份重启」（`elevate::reexec`）
- 状态栏：对标 iStat Menus，窗口内常驻 CPU/内存用量/网络/磁盘/温度/loadavg；可选 always-on-top 窄条（无托盘 crate）；有底层盘时磁盘合计跳过 Device Mapper，只有 `dm-*` 时保留 mapper 速率
- 记录：GUI 开始/停止，把每次 live 采样写成 JSONL（`$AIDA_RECORD_LOG` 或 `$XDG_STATE_HOME/aida/history.jsonl`）
- 告警：对照 `*_max`/`*_crit`/`*_min`，状态变化写入 JSONL（`$AIDA_ALERT_LOG` 或 `$XDG_STATE_HOME/aida/alerts.jsonl`）
- 中文标签：若系统有 Noto/文泉驿等 CJK 字体则加载，否则回退英文，避免方块字

## 报告导出

`export` 在同一份 `HardwareSnapshot` 上生成：

- JSON：全字段 + `access`/`source`/`hint`
- HTML：单文件深色报告
- 文本 / CSV / Markdown：同一份摘要清单（CPU/DMI/内存/GPU/传感器/存储/网络/PCI/USB/OS），对标 AIDA64 TXT/CSV。CSV 表头 `section,key,value`

CLI：`aida collect --format text` 打 stdout；`--text`/`--csv`/`--md` 写文件。不要为换格式再采集一次。

## 提权

`src/elevate.rs` 只在用户点击或 `aida elevate` 时 `exec` 替换进程。采集函数不 spawn sudo。

## 磁盘基准

`bench::run_direct`：`posix_memalign` + `O_DIRECT`。失败则 `direct_error` 非空，buffered 仍可用。

## 刻意未做

- GPU 计算基准（OpenCL/Vulkan）：会引入额外运行时依赖，与「尽量少依赖、可静态/AppImage 打包」冲突。需要时再单独一轮。

## 打包

`./scripts/package.sh` 产出：

- `dist/aida-cli-<ver>-<arch>-{musl|gnu}`：`--no-default-features`，有 musl 工具链则静态
- `dist/AIDA_Linux-<ver>-<arch>.AppImage`：glibc + linuxdeploy 收集 OpenGL/xkb `.so`
- `dist/SHA256SUMS`

版本号只写在 `Cargo.toml`。本机菜单：`./scripts/install.sh`（默认 `~/.local`）。细节见 [PACKAGING.md](PACKAGING.md)。
