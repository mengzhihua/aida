# 跨发行版兼容、坑点与测试方案

## 发行版差异

| 点 | Debian/Ubuntu | Fedora/RHEL | Arch | 容器 / 部分云主机 |
| --- | --- | --- | --- | --- |
| DMI sysfs | 有（裸机/KVM） | 同左 | 同左 | 经常整棵 `/sys/class/dmi` 不存在 |
| `product_serial` 权限 | udev 常设 0400 | 同左 | 同左 | — |
| `pci.ids` | 包 `pci.ids` 或 `hwdata`，路径 `/usr/share/misc/pci.ids` | `hwdata`，`/usr/share/hwdata/pci.ids` | `hwdata` | 可能无 |
| `usb.ids` | 常随 `usbutils`/`hwdata` | `/usr/share/misc/usb.ids` 或 `hwdata` | 同 pci.ids | 可能无 |
| `/proc/bus/input/devices` | 桌面有 | 同 | 同 | 无头/容器常空 |
| NUMA sysfs | 多路服务器有 nodeN | 同 | 同 | 单节点或未启用 NUMA 只有 node0 |
| hwmon 驱动 | 需 `linux-modules-extra` 或自己加载 coretemp/k10temp | 内核包较全 | 较全 | 虚拟机常无 |
| NVMe 节点权限 | `root:disk` 0660 | 同左 | 同左 | 无 NVMe 时走 virtio `vd*` |
| 桌面 | GNOME 下 `pkexec` 保 DISPLAY 比裸 `sudo` 稳 | 同 | 同 | 无 GUI |
| DRM | `amdgpu`/`i915` 节点较稳 | 同，另有 `xe` | 同 | 常无 `/sys/class/drm` |
| NVIDIA procfs | 专有驱动才有 `/proc/driver/nvidia` | 同 | 同 | 云主机几乎没有 |

ARM 板子：`/proc/cpuinfo` 没有 `model name` / `physical id`，只有 `CPU part` 等。CPU probe 已按键名解析，缺键就 `not_found`，不要按 x86 行号切。

## 已知坑

1. **容器看不到主机 SMBIOS/hwmon。** 这不是 bug。提示里写了路径。若要在容器里测真实硬件，需要 `--privileged` 且挂载对应 sysfs，生产工具仍应在主机跑。
2. **`cpu MHz` 在虚拟机里是恒定值**，且常常没有 `cpufreq`。界面会退回 cpuinfo 频率并加 note。
3. **virtio 块设备 `queue/rotational` 经常是 1**，不能据此判断“这是机械硬盘”。类型列优先看名字（`vd*` / `nvme*`）。
4. **PCI class `0xffff00`** 出现在部分 virtio 设备上，内核用兜底类码。名称解析只能靠 vendor/device + pci.ids。
5. **NVMe ioctl 结构体大小必须是 72 字节（64-bit）**。搞错 `_IOWR` 会 `ENOTTY`。本仓库用 `assert!(size_of::<NvmeAdminCmd>() == 72)`。
6. **不要把 SMART 的 Kelvin 当成摄氏度。** 规范是绝对温度，代码减 273.15。
7. **HTML 导出必须转义 DMI 字符串。** 厂商自定义字段可能含 `<`。
8. **AppImage + musl + egui/glow 基本不现实。** CLI musl、GUI glibc+linuxdeploy。
9. **sudo 掉 DISPLAY。** GUI 提权用 `pkexec` 或 `sudo -E`，并检查 `xhost`。
10. **`/sys/class/nvme/nvme0n1` 是命名空间不是控制器。** 枚举要过滤 `nvme\d+n\d+`。
11. **NVIDIA 占用率不在公开 sysfs。** 不要用 `nvidia-smi` 填这个洞；界面标 `unsupported`。
12. **`O_DIRECT` 在 tmpfs 上基本必失败**（EINVAL）。测试文件放当前目录或 ext4 数据盘。缓冲必须 512/4K 对齐，用 `posix_memalign`，不要 `Vec<u8>`。
13. **pkexec 与 polkitd 不是同一个包。** 只有 `polkitd` 时 `aida elevate` 会落到 sudo，无 TTY 的 GUI 按钮会失败。
14. **AppImage 在无 FUSE 容器里** 要用 `APPIMAGE_EXTRACT_AND_RUN=1`，脚本已默认导出该变量。
15. **virtio / docker0 的 `speed` 经常是 -1。** 不是解析失败，标 `unsupported`。
16. **不要调用 `ip`/`ifconfig`/`lsusb`。** 地址用 `getifaddrs`；USB 枚举 `/sys/bus/usb/devices`，接口节点名含 `:` 要跳过。
17. **`/proc/bus/input/devices` 在无头虚拟机里经常不存在。** 提示路径即可。
18. **NUMA `meminfo` 行是 `Node 0 MemTotal:`**，不能当 `/proc/meminfo` 的 `MemTotal:` 去切。
19. **告警日志只在状态变化时写一行 JSONL。** 持续越限不会刷盘；恢复再写 `ok`。
20. **`/proc/stat` 的 `cpu` 行与 `cpu0` 行不要混用。** 整机利用率用前者，每核用后者。
21. **`/proc/diskstats` 的扇区按 512 字节计**（即使盘是 4K）。差分前先乘 512。
22. **Secure Boot efivar 前 4 字节是属性。** 真正的开关是第 5 个字节。
23. **无 `power_supply` 不是 bug。** 多数服务器/容器没有电池节点。
24. **`/proc/self/mountinfo` 用 ` - ` 切开 optional fields 与 fs type。** 不要按空格数切，overlay 选项里会有逗号。
25. **`/proc/modules` 在容器里经常是空文件。** 内置内核或未加载可卸载模块时正常。
26. **无 RTC / 无 EDAC 在虚拟机上常见。** 不要用 `hwclock`/`edac-util` 兜底。
27. **`/proc/iomem` 非 root 时起止地址常被写成 `00000000`。** 只信区域名称；完整范围要 root。
28. **Wi-Fi 接口未必是 `type=803`。** 有 `/sys/class/net/<if>/wireless` 就标 wireless。
29. **分区枚举看 `/sys/block/<disk>/<disk>N`，不要把 `sda1` 当独立盘。** NVMe 分区是 `nvme0n1p1`。
30. **EDID 只解析前 128 字节。** 扩展块/CEA 不读；无显示器或虚拟机无 DRM 时连接器列表为空。
31. **挂载用量用 `statvfs`。** overlay 的 `f_blocks` 可能是下层文件系统大小，不是镜像“真实”容量。
32. **无 `/proc/pressure` 不是 bug。** 需 `CONFIG_PSI`；容器有时不挂该节点。
33. **`/proc/interrupts` 的 NMI/ERR 行 per-cpu 列数可能少于 CPU 数。** 按能解析到的整数求和。
34. **virtio/NVMe 没有 `ata_port`。** 不要用空列表当探测失败。
35. **不要 dump ACPI 表二进制。** 只列 `/sys/firmware/acpi/tables` 下的表名。
36. **tainted=0 就是干净内核。** 按位解码，不要把十进制当“错误码”。
37. **无 `/sys/class/powercap` 不是 bug。** 虚拟机、ARM、未开 RAPL 时常见；不要调用 `turbostat`。
38. **`energy_uj` 会回绕。** 功率用差分，回绕时加上 `max_energy_range_uj`。
39. **virtio `modalias` 是 `virtio:d{device}v{vendor}`。** ID 对照 `linux/virtio_ids.h`（16=gpu、18=input、20=crypto、23=iommu、26=fs），不要靠 PCI class 猜。
40. **`block_size_bytes` 是十六进制。** `8000000` = 128 MiB，不是十进制 8e6。
41. **KSM `run=0` 表示未启用合并。** 不是采集失败。
42. **PTP 在 KVM 上经常是 `KVM virtual PTP`。** 无 `/sys/class/ptp` 时列表为空。
43. **`/proc/softirqs` 与 interrupts 同格式。** 第一列是名字不是数字 IRQ。
44. **网卡 `queues/rx-*`/`tx-*` 在旧内核或某些虚拟接口上可能没有。** 计数为 0 即可。
45. **virtio PCI 通常没有 `current_link_speed`。** 那是 PCIe 链路属性，不是采集失败。
46. **空的 `/sys/kernel/iommu_groups` 表示未启用 IOMMU。** 不要当权限错误。
47. **不要调用 `mdadm`/`lsscsi`/`i2cdetect`。** mdstat、scsi_host、i2c sysfs 足够列清单。
48. **`/sys/kernel/security/lsm` 是逗号列表。** SELinux `enforce` 只在 selinuxfs 挂上时存在。
49. **zram 不进常规块设备表。** 压缩统计走 `/sys/block/zramN/mm_stat`，不要调用 `zramctl`。
50. **zswap `enabled=N` 表示未启用。** 模块在、开关关都正常；无 `module/zswap` 才是未编译。
51. **`/dev/kvm` 常为 `root:kvm`。** 存在即可说明宿主机支持；ioctl 建 VM 仍可能要组权限。
52. **SMT `control=notsupported` 在虚拟机上常见。** 不是采集失败。
53. **不要调用 `iscsiadm`/`rfkill`/`bluetoothctl`/`v4l2-ctl`。** class 目录为空就写 note。
54. **`sriov_totalvfs` 只在 PF 上存在。** virtio/普通端点没有该节点。
55. **平台/总线 class 的 `PermissionDenied` 不是「无设备」。** 用 `access_label`，不要写成空列表。
56. **不要调用 `sysctl`/`systemd-cgls`/`brctl`/`ss`/`netstat`。** snmp、softnet、cgroup v2、bridge sysfs 足够。
57. **`/proc/net/snmp` 两行一组。** 先字段名后数值；`Tcp.MaxConn=-1` 表示无限，不要当错误。
58. **`softnet_stat` 是十六进制。** 每行一个 CPU，列 0/1/2 = processed/dropped/time_squeeze。
59. **网桥看 `class/net/<if>/bridge`。** docker0 这类 type=1 的也是桥，不要只信 `type`。
60. **cgroup v2 只扫根和第一层 `.slice`/`.scope`。** 不要递归整个树。
61. **`pagetypeinfo` 常要 root。** zoneinfo 对普通用户通常可读。
62. **IRQ 亲和只读数字 IRQ 的 `smp_affinity_list`。** NMI/ERR/LOC 没有该目录。
63. **不要调用 `sysctl`/`aa-status`/`lsns`/`losetup`/`setserial`/`netstat`。** lockdown、crypto、ns、loop、tty、conntrack 都从文件读。
64. **lockdown 当前模式在方括号里。** `none [integrity] confidentiality` 表示 integrity。
65. **`/proc/crypto` 的 `internal : yes` 是内核内部算法。** 界面只列非 internal，总数仍统计全部。
66. **`/proc/net/netstat` 与 snmp 一样两行一组。** 不要用用户态 `netstat` 填 TcpExt。
67. **无 backing_file 的 loopN 视为空闲。** 虚拟机常有 loop0–7 且 size=0。
68. **串口只列 `ttyS*`/`ttyUSB*`/`ttyACM*`/`ttyAMA*`。** 不要把 `tty0`–`tty63` 或 `hvc*` 当 UART。`ttyS` 的 `type=0` 是 8250 空槽。
69. **无 `scsi_device` 不是采集失败。** virtio-blk 没有 SCSI LUN。
70. **`/sys/power/state` 只有 `disk` 不代表不能读。** 云 VM 常无 mem/freeze；不要调用 `systemctl suspend`。
71. **`/proc/net/snmp6` 是「键 值」每行一项。** 不要用 IPv4 snmp 的两行组去切。
72. **`ioports` 与 iomem 一样非 root 地址常为 0。** 只信区域名称。
73. **`unprivileged_bpf_disabled=2` 表示默认禁止非特权 BPF。** 不是采集失败。
74. **`modules_disabled=1` 之后不能再加载模块。** 加固云镜像可能为 1，不要当采集失败。
75. **THP `defrag`/`shmem_enabled` 也是方括号标当前策略。** 与 `enabled` 分开读。
76. **misc 只列 `/sys/class/misc` 名字。** kvm/tun/fuse 的细节仍在各自 probe。
77. **cgroup `groups` 只收第一层 `.slice`/`.scope`。** `docker` 这类无点号目录不算。
78. **loop 读 `backing_file` 权限不足时不要算空闲。** 只把 NotFound / 空内容当未使用。
79. **GPIO / MTD / InfiniBand 在云 VM 上经常没有 class。** 写 note，不要当采集崩溃。
80. **空的 `/sys/devices/system/cpu/cpufreq` 不是读失败。** 虚拟机常无 policyN。
81. **无 `/proc/config.gz` 很常见。** 需要 `CONFIG_IKCONFIG_PROC`；不要解压 gzip 加依赖。procfs inode size 常为 0，必须读字节长度。
82. **`nmi_watchdog=0` 在虚拟机上正常。** 不是采集失败。
83. **`/proc/net/if_inet6` 和 `ipv6_route` 没有表头。** 不要像 unix/packet 那样 skip 第一行。
84. **`/proc/net/protocols` 第三列才是 sockets。** 不要把 size 当连接数。
85. **nfsd `threads` 不存在表示未加载 nfsd。** 空 `/proc/fs/nfsd` 目录不是失败。
86. **BDI 名字是主:次设备号。** 不要调用 `dmsetup`/`lsblk` 去解析。
87. **`/proc/locks` 为空表示当前无文件锁。** 权限不足或读失败时写 note，不要当成 0 把锁。
88. **`class/vtconsole` 的 dummy device 在无真实 VT 的云 VM 上常见。** `bind=1` 仍可能是 dummy。目录 PermissionDenied 不要当成没有 VT。
89. **`class/msr` 每个逻辑 CPU 一个 `msrN`。** 只计数，不要 ioctl 读 MSR。读目录失败写 note，不要当成 0 个设备。
90. **`rt6_stats` 是十六进制 7 列。** 第 6 列才是 destination cache entries，第一列是 `fib_nodes`。IPv4-only 主机仍可能有该文件。
91. **`tcp_fastopen` 可读即可。** 不要把 IPv6/unix 表当 live 测试前置条件。
92. **`/sys/kernel/irq` 计数与 `/proc/interrupts` 行数不必相等。** 后者含 NMI/ERR。目录读失败写 note，不要当成 0。
93. **`kernel.panic` 允许负数。** `-1` 表示立即重启，不要用无符号解析标成读取失败。
94. **`/proc/net/igmp` 只计接口头行。** 组记录定时器含冒号，不能当接口。
95. **`shmmax` 在 64 位上常接近 `u64::MAX`。** 不是溢出错误。
96. **sysvipc 表只有表头表示当前无对象。** 不要把表头当一条 shm。
97. **`protected_fifos=1` / `protected_regular=2` 是发行版默认加固。** 不是采集失败。
98. **`kexec_loaded=0` 表示未加载 crash/kexec 内核。** 云 VM 常见。
99. **`/proc/net/tcp` 行数含 TIME_WAIT。** 不是 established-only。
100. **`firmware/memmap` 编号目录是 e820 段。** 不要展开每一段的 type/start（长度已够）。
101. **`rp_filter` / `use_tempaddr` / `accept_dad` / `addr_gen_mode` 是 per-iface。** 只读 `conf/all` 会漏掉 `eth0:1` 或 `lo:-1`。不要用 `default` 代替现有接口。
102. **`clockevents` 只列 `broadcast` 与 `clockeventN`。** 目录 PermissionDenied 不要当成没有时钟事件设备。
103. **`/proc/dma` 的 `4: cascade` 在 PC 上正常。** 这是 ISA DMA 级联，不是采集错误。
104. **空的 `/sys/class/dma` 表示没有 dmaengine 通道。** 云 VM 常见，不要当读失败。
105. **不要写 PWM `export`/`unexport`。** 只读 `pwmchipN/npwm`。
106. **不要读取 nvmem 的 `nvmem` 二进制属性。** 只读 `type`。
107. **空的 `/sys/class/devlink` 表示没有设备链路。** class 存在但无条目不是失败。
108. **`pci_bus` 的 `cpulistaffinity` 是 host bridge 的 CPU 掩码。** 不要写 `rescan`。
109. **`sched_rt_runtime_us` 是有符号 i64。** `-1` 表示 RT 运行时不限（占满 period），不要当读取失败。
110. **不要读巨大的 `/proc/net/fib_trie`。** 只取 `fib_triestat` 第一段 `Leaves:`（主表）。
111. **`/proc/net/ptype` 的 Device 列经常为空。** function 取最后一个空白分隔 token。
112. **空的 `bus/memory_tiering/devices` 表示没有 CXL/HMAT 分层。** 不是采集失败。
113. **`kexec_load_disabled=0` 表示仍允许 kexec。** 云 VM 常见。
114. **`panic_on_oom=0` 是默认：OOM killer 而不 panic。** 不要当采集失败。
115. **`/proc/net/tcp6`/`udp6`/`raw`/`udplite` 与 tcp 一样跳过表头。** IPv4-only 主机文件可能只有表头或缺失。
116. **`xfrm_stat` 是每行 `Key Value`。** 不要用 snmp 两行组解析。
117. **空的 ieee80211/typec/udc/dax/wmi/spi/serio/ubi 表示没有对应硬件。** 云 VM 常见；`PermissionDenied` 仍写 note，不要当成空列表。
118. **不要写 `binfmt_misc` 的 `register`。** 只读 `status`；目录在但无 status 就是未挂载。
119. **`bpf_jit_enable` 经常不存在。** 未开 JIT 时 `NotFound`，不是采集失败。
120. **`/proc/net/igmp6` 按接口名去重。** 不要把组播组行数当接口数。
121. **空的 `/proc/net/ip_tables_names` 表示未加载 iptables。** 不是读失败，也不要跑 `iptables-save`。`PermissionDenied` 必须写 note，不要当成空表。
122. **`tcp_notsent_lowat=4294967295` 表示不限制。** 用 u64 解析。
123. **不要读 `/proc/sys/vm/drop_caches` 或 `compact_memory`。** 它们是只写触发器。
124. **不要转储 `/sys/kernel/notes` 二进制。**
125. **不要 dump `/proc/keys`。** 只计 `/proc/key-users` 行数。读取失败或 `CONFIG_KEYS` 未开是 `NotFound`/`PermissionDenied`，不是 0。
126. **`/proc/cgroups` 最后一列才是 enabled。** `0` 表示该 v1 子系统未启用，不要列进去。
127. **`dentry-state` 第一列是 nr_dentry，第二列 nr_unused。** 一次读取再拆两列，不要把整行当单个整数。
128. **不要读 `/sys/kernel/vmcoreinfo` 当文本。** 那是二进制地址范围。
129. **`/proc/net/connector` 跳过表头。** 只取 Name 列。
130. **`sched_cfs_bandwidth_slice_us` 在未开 `CONFIG_CFS_BANDWIDTH` 时不存在。** `NotFound` 不是采集失败。
131. **空的 `/sys/devices/system/cpu/offline` 表示没有离线 CPU。** 不是读失败。
132. **`inode-state` 第一列是 nr_inodes（已分配），第二列 nr_unused。** `inode_inuse` 是两者之差，不要把第一列当正在使用。空闲大于已分配记为读取失败。
133. **`io_uring_disabled`：`0` 允许，`1` 仅特权，`2` 全关。** `io_uring_group=-1` 表示未绑定组，用有符号解析。不要写这些 sysctl。
134. **`shmall` 与 shmmax 一样在 64 位上常接近 `u64::MAX`。** 用字符串保留。
135. **空的 scsi_generic/wwan/ppp/phy 表示没有对应硬件。** 云 VM 常见；`PermissionDenied` 仍写 note。
136. **无 device-tree `model` 在 x86/云主机上常见。** 先读 sysfs 再读 `/proc/device-tree`，不要当采集失败。字符串属性以 NUL 结尾，导出前要去掉。
137. **`nf_conntrack_tcp_timeout_established` 单位是秒。** 未加载 conntrack 时 `NotFound`。
138. **`dirty_bytes=0` 表示改用 `dirty_ratio`。** `dirty_background_bytes=0` 同理。不要把 0 当成采集失败。
139. **`overcommit_kbytes=0` 表示改用 `overcommit_ratio`。**
140. **空的 remoteproc/extcon/tee/mdio_bus 表示没有对应硬件。** 云 VM 常见；`PermissionDenied` 仍写 note。
141. **`tcp_mem` / `udp_mem` 是三个页数（min / pressure / max）。** 保留整行字符串，不要拆成单个整数。
142. **IPv6 `addr_gen_mode`：`0` EUI64，`1` none，`2` stable-privacy，`3` random。** 与 `accept_dad` 一样列出和 `conf/all` 不同的接口。`lo` 上 `accept_dad=-1` 常见。
143. **`pipe-user-pages-hard=0` 表示不限制。** soft 默认常为 16384 页。
144. **`kernel_max` 是内核编译时的最大 CPU 下标，不是在线数量。** `possible`/`present` 是掩码列表。
145. **`/sys/class/wakeup` 只计数。** 不要展开每个 `wakeupN`。`PermissionDenied` 必须写 note。
146. **`core_pipe_limit=0` 表示不限制 core dump 管道。** 不要当采集失败。
147. **`printk_devkmsg` 为 `on` / `off` / `ratelimit`。**
148. **`kernel.acct` 是三个 token（highwater / lowwater / frequency）。** 保留整行。
149. **不要读 `kernel.cad_pid`。** 常无权限或为空。
150. **空的 spi_master / i2c-dev / nvme-subsystem / w1 表示没有对应硬件。** 云 VM 常见；`PermissionDenied` 仍写 note。
151. **`cpu/enabled` 在较新内核才有。** `NotFound` 不是采集失败。空的 `nohz_full` 表示没有 nohz_full CPU。
152. **不要 dump `cpu/hotplug/states`。** 那是内部 CPUHP 回调表。
153. **不要 dump seccomp `actions_logged`。** 只读 `actions_avail`。
154. **`fib_multipath_hash_policy` 在未开多路径时可能不存在。** `NotFound` 不是采集失败。
155. **不要 dump `/proc/sys/kernel/random/uuid`。** 每次读取都会变；boot_id 才是稳定的。
156. **`router_solicitations` 是有符号 i64。** `-1` 表示使用 RFC 默认次数，不要当读取失败。
157. **`memfd_noexec`：`0` 不限制，`1` 仅 dumpable，`2` 一律禁止。** 旧内核可能不存在。
158. **空的 macvtap / nvme-generic 表示没有对应硬件。** `tun` / `nvme-fabrics` 是 misc 设备（`class/misc/tun`、`/dev/net/tun`），不是独立 class；未加载才写 note。`PermissionDenied` 仍写 note，不要当成缺失。
159. **`cpu/modalias` 可能很长。** JSON 保留全文，界面与 HTML 截断前缀。
160. **`message_cost=0` 表示关闭内核网络 printk 限速。** 不是采集失败。
161. **`cpuidle/current_driver=none` 在虚拟机上合法。** 不是采集失败；同时读 `current_governor`。
162. **`warn_limit=0` 表示不限制 warn 次数。**
163. **`kexec_load_limit_panic` 是有符号 i64。** `-1` 表示 panic 路径不限制 kexec load。
164. **`hung_task_warnings` 是有符号 i64。** 不要当无符号解析。
165. **`split_lock_mitigate` 在非 x86 上可能不存在。** `NotFound` 不是采集失败。
166. **空的 iscsi_endpoint / iscsi_iface / iscsi_connection / bus/container 表示没有对应硬件。** `PermissionDenied` 仍写 note，不要当成缺失。
167. **`dad_transmits` 只列出与 `conf/all` 不同的接口。** 不要用 `default` 顶替已有 iface。
168. **`hung_task_check_interval_secs=0` 表示沿用 `hung_task_timeout_secs`。** 不是关闭检测。
169. **`kexec_load_limit_reboot` 是有符号 i64。** `-1` 表示 reboot 路径不限制 kexec load。
170. **`max_rcu_stall_to_panic=0` 表示不把 RCU stall 升级为 panic。**
171. **`tcp_challenge_ack_limit=2147483647`（INT_MAX）是默认上限，表示不额外收紧。** 不要 dump `tcp_fastopen_key`。
172. **空的 iscsi_flashnode / nd / dma_heap 表示没有对应硬件。** flashnode 先看 `class/iscsi_flashnode`，没有再看 `bus/iscsi_flashnode/devices`（旧内核仍是 bus）。两边都缺失才写 leftover note；`PermissionDenied` 仍写 note。不 dump `serial-base` 设备名。
173. **GUI `refresh_live` 必须替换整份 CPU 报告。** 只写回 `logical` 会让 cpuidle governor 停留在启动值。
174. **空的 cxl / devfreq / fpga / gnss 表示没有对应硬件。** CXL 先看 `bus/cxl/devices`，没有再看 `class/cxl`。FPGA 分别看 `fpga_manager` / `fpga_bridge` / `fpga_region`（没有统一的 `class/fpga`），三个 class 都缺失才记 leftover。`PermissionDenied` 仍写 note。
175. **`ndisc_notify` 只列出与 `conf/all` 不同的接口。** 不要用 `default` 顶替已有 iface。
176. **不要读 `compact_memory`。** 那是一次性触发器，不是状态。
177. **空的 rpmsg / devcoredump 表示没有对应硬件。** `PermissionDenied` 仍写 note。
178. **`accept_ra_pinfo` 只列出与 `conf/all` 不同的接口。** 不要用 `default` 顶替已有 iface。
179. **`print-fatal-signals` 路径带连字符。** 不要写成 `print_fatal_signals`。
180. **`bpf_stats_enabled=0` 表示不采集 BPF 运行统计。** 不是采集失败。
181. **`core_sort_vma=0` 表示 core dump VMA 按插入顺序。** `1` 按地址排序。
182. **`min_slab_ratio` / `min_unmapped_ratio` 是 zone reclaim 百分比阈值。** 不是采集失败。
183. **GUI `refresh_live` 必须更新 `buses.devcoredump`。** 这是设备崩溃后才出现、读完或超时即消失的瞬时 class；不要为此重扫整份 buses。不更新会让界面和后续导出停在启动清单。

## 测试方案

### 自动化（本仓库）

```bash
cargo test --no-default-features
```

- 夹具：伪造 hwmon、cpuinfo、pci.ids、os-release（不依赖本机硬件）
- 权限：临时文件 `chmod 000`，非 root 断言 `permission_denied`（root 环境会跳过该断言）
- 现场：`tests/live_collect.rs` 对真实 `/proc` `/sys` 采集一次，只要求不 panic，允许大量 `not_found`
- 基准：`BenchRequest::quick()` 小缓冲，避免 CI 超时

### 手工矩阵（发布前）

| 环境 | 看什么 |
| --- | --- |
| x86_64 裸机 Fedora/Ubuntu 普通用户 | CPU/PCI 有值；DMI serial 为权限不足 |
| 同上 root | serial/UUID/SMART 有值 |
| AMD + k10temp、Intel + coretemp | 温度折线 |
| NVMe 笔记本 | sysfs 型号 + SMART percentage_used |
| QEMU virtio 虚拟机 | 无 hwmon/DMI/GPU 时提示正确；块设备为 `vd*`；O_DIRECT 看文件系统 |
| AMD amdgpu / Intel i915 | GPU 页有 busy 或 gt 频率 |
| NVIDIA 专有驱动 | 有 model/vbios；busy 为 unsupported |
| aarch64 树莓派/ARM 云 | cpuinfo 缺字段不崩 |
| Wayland + X11 各测一次 GUI | 字体、折线、导出按钮 |

### 回归命令

```bash
# 对比普通用户 vs root 的 access 统计
aida collect > /tmp/u.json
sudo aida collect > /tmp/r.json
# 抽查 product_serial.access / nvme.smart.access
```

不要用“字段是否为空”当回归标准，要用 `access` 枚举：root 下 `permission_denied` 才是回归失败。
