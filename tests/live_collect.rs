//! 对真实 /proc /sys 做一次采集：虚拟机里大量 not_found 是正常的，只要不 panic。

use aida::{export, HardwareSnapshot, ProbeCtx};

#[test]
fn live_snapshot_json_and_html() {
    let snap = HardwareSnapshot::collect(&ProbeCtx::live());
    assert!(
        snap.cpu.logical_cpus >= 1,
        "应至少看到一个逻辑 CPU，实际 {}",
        snap.cpu.logical_cpus
    );
    assert!(
        snap.software.os_name.value.is_some() || snap.software.kernel_release.value.is_some(),
        "os-release 或 osrelease 至少有一个可读"
    );
    let json = export::to_json_pretty(&snap).expect("json");
    assert!(json.contains("\"app\": \"aida\""));
    assert!(json.contains("\"gpu\""));
    assert!(json.contains("\"net\""));
    assert!(json.contains("\"usb\""));
    assert!(json.contains("\"input\""));
    assert!(json.contains("\"numa\""));
    assert!(json.contains("\"alerts\""));
    assert!(json.contains("\"memory\""));
    assert!(json.contains("\"power\""));
    assert!(json.contains("\"audio\""));
    assert!(json.contains("\"firmware\""));
    assert!(json.contains("\"fs\""));
    assert!(json.contains("\"modules\""));
    assert!(json.contains("\"clock\""));
    assert!(json.contains("\"edac\""));
    assert!(json.contains("\"iomem\""));
    assert!(json.contains("\"psi\""));
    assert!(json.contains("\"irq\""));
    assert!(json.contains("\"ata\""));
    assert!(json.contains("\"virtio\""));
    assert!(json.contains("\"rapl\""));
    assert!(json.contains("\"iommu\""));
    assert!(json.contains("\"watchdogs\""));
    assert!(json.contains("\"scsi\""));
    assert!(json.contains("\"lsm\""));
    assert!(json.contains("\"softirqs\""));
    assert!(json.contains("\"ptps\""));
    assert!(json.contains("\"zswap\""));
    assert!(json.contains("\"kvm\""));
    assert!(json.contains("\"iscsi\""));
    assert!(json.contains("\"rfkill\""));
    assert!(json.contains("\"smt_control\""));
    assert!(json.contains("\"snmp\""));
    assert!(json.contains("\"softnet\""));
    assert!(json.contains("\"sysctl\""));
    assert!(json.contains("\"cgroup\""));
    assert!(json.contains("\"zones\""));
    assert!(json.contains("\"ext4\""));
    assert!(json.contains("\"lockdown\""));
    assert!(json.contains("\"crypto\""));
    assert!(json.contains("\"conntrack_count\""));
    assert!(json.contains("\"tcp_congestion\""));
    assert!(json.contains("\"self_ns\""));
    assert!(json.contains("\"snmp6\""));
    assert!(json.contains("\"boot_id\""));
    assert!(json.contains("\"directmap_2m_kb\""));
    assert!(json.contains("\"suspend_success\""));
    assert!(json.contains("\"freq_policies\""));
    assert!(json.contains("\"tcp_fastopen\""));
    assert!(json.contains("\"gpio\""));
    assert!(json.contains("\"mapper\""));
    assert!(json.contains("\"nmi_watchdog\""));
    assert!(json.contains("\"tcp\""));
    assert!(json.contains("\"protocols\""));
    assert!(json.contains("\"nfsd_threads\""));
    assert!(json.contains("\"bdi\""));
    assert!(!snap.irq.softirqs.is_empty(), "/proc/softirqs 应至少有一行");
    assert!(
        snap.memory.vmstat.pgfault.value.is_some(),
        "vmstat pgfault 应可读"
    );
    assert!(!snap.memory.buddy.is_empty(), "buddyinfo 应至少有一个 zone");
    assert!(
        snap.net
            .interfaces
            .iter()
            .any(|i| i.rx_queues + i.tx_queues > 0)
            || snap.net.interfaces.iter().any(|i| i.name == "lo"),
        "网卡应能看到 queues 或至少 lo"
    );
    assert!(
        snap.psi.cpu.is_some() || !snap.psi.notes.is_empty(),
        "PSI 应可读或给出说明"
    );
    assert!(!snap.irq.lines.is_empty(), "/proc/interrupts 应至少有一行");
    assert!(
        snap.software.tainted.value.is_some(),
        "kernel tainted 应可读"
    );
    assert!(
        snap.fs.mounts.iter().any(|m| m.total_bytes.is_some()),
        "至少有一个挂载点能 statvfs"
    );
    assert!(snap.software.load_1.value.is_some(), "loadavg 应可读");
    assert!(
        snap.cpu.smt_control.access != aida::access::AccessKind::Error,
        "SMT control 不应是读取失败"
    );
    assert!(
        snap.zmem.zswap.enabled.access != aida::access::AccessKind::Error,
        "zswap enabled 不应是读取失败"
    );
    assert!(
        snap.kvm.device.value.is_some() || !snap.kvm.notes.is_empty(),
        "/dev/kvm 应存在或给出说明"
    );
    assert!(
        snap.net.snmp.tcp_in_segs.is_some() || snap.net.snmp.ip_in_receives.is_some(),
        "snmp Tcp/Ip 计数应可读"
    );
    assert!(snap.net.softnet.cpus >= 1, "softnet_stat 应至少有一行");
    assert!(snap.sysctl.pid_max.value.is_some(), "pid_max 应可读");
    assert!(snap.sysctl.file_nr_alloc.value.is_some(), "file-nr 应可读");
    assert!(
        snap.cgroup.controllers.value.is_some() || !snap.cgroup.notes.is_empty(),
        "cgroup v2 应可读或给出说明"
    );
    assert!(snap.crypto.total >= 1, "/proc/crypto 应至少有一个算法");
    assert!(
        snap.net.tcp_congestion.value.is_some(),
        "tcp_congestion_control 应可读"
    );
    assert!(
        !snap.ns.self_ns.is_empty(),
        "/proc/self/ns 应至少有一个命名空间"
    );
    assert!(
        snap.net.tcp_fastopen.access != aida::access::AccessKind::Error,
        "tcp_fastopen 不应是读取失败（IPv4-only 主机也必须通过）"
    );
    assert!(
        snap.sysctl.keys_maxkeys.access != aida::access::AccessKind::Error,
        "keys/maxkeys 不应是读取失败"
    );
    assert!(
        snap.sysctl.cap_last_cap.value.is_some(),
        "cap_last_cap 应可读"
    );
    assert!(json.contains("\"keys_maxkeys\""));
    assert!(json.contains("\"netdev_budget\""));
    assert!(json.contains("\"firmware_timeout\""));
    assert!(json.contains("\"clockevents\""));
    assert!(json.contains("\"sysfs_irqs\""));
    assert!(json.contains("\"file_locks\""));
    assert!(json.contains("\"rp_filter_dev\""));
    assert!(json.contains("\"ipv6_use_tempaddr_dev\""));
    assert!(json.contains("\"sysvipc_shm\""));
    assert!(json.contains("\"protected_hardlinks\""));
    assert!(json.contains("\"kexec_loaded\""));
    assert!(json.contains("\"shmmax\""));
    assert!(json.contains("\"dma_isa\""));
    assert!(json.contains("\"pci_buses\""));
    assert!(json.contains("\"pwm_chips\""));
    assert!(json.contains("\"sched_rt_runtime_us\""));
    assert!(json.contains("\"tcp6_socks\""));
    assert!(json.contains("\"platform_devices\""));
    assert!(json.contains("\"memory_tiers\""));
    assert!(json.contains("\"xfrm_in_no_states\""));
    assert!(json.contains("\"ptypes\""));
    assert!(json.contains("\"fib_trie_leaves\""));
    assert!(json.contains("\"ieee80211\""));
    assert!(json.contains("\"printk_ratelimit\""));
    assert!(json.contains("\"igmp6_ifaces\""));
    assert!(json.contains("\"bpf_jit_enable\""));
    assert!(json.contains("\"binfmt_misc_status\""));
    assert!(json.contains("\"busy_poll\""));
    assert!(json.contains("\"cpu_byteorder\""));
    assert!(json.contains("\"v1_enabled\""));
    assert!(json.contains("\"key_users\""));
    assert!(json.contains("\"dentry_nr\""));
    assert!(json.contains("\"connectors\""));
    assert!(json.contains("\"pty_max\""));
    assert!(json.contains("\"io_uring_disabled\""));
    assert!(json.contains("\"ipv6_accept_ra\""));
    assert!(json.contains("\"scsi_generic\""));
    assert!(json.contains("\"dt_model\""));
    assert!(json.contains("\"ostype\""));
    assert!(json.contains("\"kernel_max\""));
    assert!(json.contains("\"dirty_bytes\""));
    assert!(json.contains("\"tcp_mem\""));
    assert!(json.contains("\"ipv6_addr_gen_mode\""));
    assert!(json.contains("\"ipv6_accept_dad_dev\""));
    assert!(json.contains("\"remoteproc\""));
    assert!(json.contains("\"wakeup_sources\""));
    assert!(json.contains("\"core_pipe_limit\""));
    assert!(json.contains("\"ipfrag_high_thresh\""));
    assert!(json.contains("\"spi_master\""));
    assert!(json.contains("\"seccomp_actions_avail\""));
    assert!(json.contains("\"write_wakeup_threshold\""));
    assert!(json.contains("\"tcp_frto\""));
    assert!(json.contains("\"ipv6_router_solicitations\""));
    assert!(json.contains("\"macvtap\""));
    assert!(json.contains("\"modalias\""));
    assert!(json.contains("\"cpuidle_driver\""));
    assert!(json.contains("\"kexec_load_limit_panic\""));
    assert!(json.contains("\"ip_unprivileged_port_start\""));
    assert!(json.contains("\"iscsi_endpoint\""));
    assert!(json.contains("\"cpuidle_available_governors\""));
    assert!(json.contains("\"kexec_load_limit_reboot\""));
    assert!(json.contains("\"tcp_ecn_fallback\""));
    assert!(json.contains("\"iscsi_flashnode\""));
    assert!(json.contains("\"compaction_proactiveness\""));
    assert!(json.contains("\"tcp_comp_sack_nr\""));
    assert!(json.contains("\"ipv6_ndisc_notify\""));
    assert!(json.contains("\"cxl\""));
    assert!(json.contains("\"hardlockup_all_cpu_backtrace\""));
    assert!(json.contains("\"tcp_early_demux\""));
    assert!(json.contains("\"ipv6_accept_ra_pinfo\""));
    assert!(json.contains("\"rpmsg\""));
    assert!(json.contains("\"softlockup_all_cpu_backtrace\""));
    assert!(json.contains("\"tcp_base_mss\""));
    assert!(json.contains("\"ipv6_enhanced_dad\""));
    assert!(json.contains("\"scsi_disk\""));
    assert!(json.contains("\"printk_delay\""));
    assert!(json.contains("\"tcp_max_reordering\""));
    assert!(json.contains("\"ipv6_accept_ra_mtu\""));
    assert!(json.contains("\"cec\""));
    assert!(json.contains("\"perf_event_max_stack\""));
    assert!(json.contains("\"tcp_min_rtt_wlen\""));
    assert!(json.contains("\"ipv6_keep_addr_on_down\""));
    assert!(json.contains("\"vfio\""));
    assert!(json.contains("\"fc\""));
    assert!(json.contains("\"accel\""));
    assert!(json.contains("\"vdpa\""));
    assert!(json.contains("\"uio\""));
    assert!(json.contains("\"auxiliary\""));
    assert!(json.contains("\"usbmon\""));
    assert!(json.contains("\"counter\""));
    assert!(json.contains("\"drm_dp_aux_dev\""));
    assert!(json.contains("\"mhi\""));
    assert!(json.contains("\"tcp_stdurg\""));
    assert!(json.contains("\"nr_hugepages_mempolicy\""));
    assert!(json.contains("\"ipmi\""));
    assert!(json.contains("\"usb_role\""));
    assert!(json.contains("\"i3c\""));
    assert!(json.contains("\"tcp_allowed_congestion\""));
    assert!(json.contains("\"tcp_plb_idle_rehash_rounds\""));
    assert!(json.contains("\"nr_hugepages\""));
    assert!(json.contains("\"vduse\""));
    assert!(json.contains("\"mux\""));
    assert!(json.contains("\"soundwire\""));
    assert!(json.contains("\"tcp_pingpong_thresh\""));
    assert!(json.contains("\"acpi_video_flags\""));
    assert!(json.contains("\"rc\""));
    assert!(json.contains("\"stm\""));
    assert!(json.contains("\"peci\""));
    assert!(json.contains("\"tcp_ehash_entries\""));
    assert!(json.contains("\"bootloader_type\""));
    assert!(json.contains("\"tcp_fack\""));
    assert!(json.contains("\"tcp_low_latency\""));
    assert!(json.contains("\"ipv6_force_tllao\""));
    assert!(json.contains("\"sched_schedstats\""));
    assert!(json.contains("\"traceoff_on_warning\""));
    assert!(json.contains("\"memory_arrays\""));
    assert!(json.contains("\"firewire\""));
    assert!(json.contains("\"ipv6_accept_untracked_na\""));
    assert!(json.contains("\"ipv6_proxy_ndp\""));
    assert!(json.contains("\"kernel_arch\""));
    assert!(json.contains("\"processors\""));
    assert!(json.contains("\"slots\""));
    assert!(json.contains("\"bios_rom_kb\""));
    assert!(json.contains("\"ulpi\""));
    assert!(json.contains("\"spmi\""));
    assert!(json.contains("\"pci_epc\""));
    assert!(json.contains("\"ipv6_ndisc_tclass\""));
    assert!(json.contains("\"ipv6_suppress_frag_ndisc\""));
    assert!(
        !snap.periph.pci_buses.is_empty()
            || snap.periph.dma_isa.iter().any(|c| c.name == "cascade"),
        "x86 应有 pci_bus 或 /proc/dma cascade"
    );
    assert!(
        snap.sysctl.panic.access != aida::access::AccessKind::Error,
        "kernel.panic 不应是读取失败（负数也是合法值）"
    );
    assert!(
        snap.sysctl.sched_rt_runtime_us.access != aida::access::AccessKind::Error,
        "sched_rt_runtime_us 不应是读取失败（-1 表示不限）"
    );
    assert!(
        snap.sysctl.panic_on_oom.access != aida::access::AccessKind::Error,
        "panic_on_oom 不应是读取失败"
    );
    assert!(
        snap.sysctl.printk_ratelimit.access != aida::access::AccessKind::Error,
        "printk_ratelimit 不应是读取失败"
    );
    assert!(
        snap.sysctl.sched_cfs_bandwidth_slice_us.access != aida::access::AccessKind::Error,
        "sched_cfs_bandwidth_slice_us 不应是读取失败（无 CONFIG_CFS_BANDWIDTH 时为 NotFound）"
    );
    assert!(
        snap.net.igmp6_ifaces >= 1 || snap.net.igmp_ifaces >= 1,
        "igmp 或 igmp6 应至少看到一个接口（IPv4-only 主机用 igmp）"
    );
    assert!(
        snap.software.cpu_byteorder.access != aida::access::AccessKind::Error,
        "cpu_byteorder 不应是读取失败"
    );
    assert!(
        !snap.cgroup.v1_enabled.is_empty() || snap.cgroup.controllers.value.is_some(),
        "cgroup v1 enabled 或 v2 controllers 应至少有一个"
    );
    assert!(snap.sysctl.dentry_nr.value.is_some(), "dentry-state 应可读");
    assert!(
        snap.sysctl.key_users.access != aida::access::AccessKind::Error,
        "key-users 不应是读取失败（无 CONFIG_KEYS 时为 NotFound，不是 0）"
    );
    assert!(snap.sysctl.pty_max.value.is_some(), "pty/max 应可读");
    assert!(
        snap.sysctl.io_uring_disabled.access != aida::access::AccessKind::Error,
        "io_uring_disabled 不应是读取失败"
    );
    assert!(
        snap.cpu.offline.access != aida::access::AccessKind::Error,
        "cpu offline 不应是读取失败（空文件表示无离线 CPU）"
    );
    assert!(
        snap.software.ostype.value.as_deref() == Some("Linux")
            || snap.software.ostype.access != aida::access::AccessKind::Error,
        "ostype 应为 Linux 或至少不是读取失败"
    );
    assert!(
        snap.sysctl.dirty_bytes.access != aida::access::AccessKind::Error,
        "dirty_bytes 不应是读取失败（0 表示改用 dirty_ratio）"
    );
    assert!(
        snap.sysctl.overcommit_kbytes.access != aida::access::AccessKind::Error,
        "overcommit_kbytes 不应是读取失败（0 表示改用 overcommit_ratio）"
    );
    assert!(
        snap.cpu.possible.access != aida::access::AccessKind::Error,
        "cpu possible 不应是读取失败"
    );
    assert!(
        snap.net.tcp_mem.access != aida::access::AccessKind::Error,
        "tcp_mem 不应是读取失败（三个页数 token）"
    );
    assert!(
        snap.sysctl.core_pipe_limit.access != aida::access::AccessKind::Error,
        "core_pipe_limit 不应是读取失败（0 表示不限制）"
    );
    assert!(
        snap.net.ipfrag_high_thresh.access != aida::access::AccessKind::Error,
        "ipfrag_high_thresh 不应是读取失败"
    );
    assert!(
        snap.net.tcp.retries1.access != aida::access::AccessKind::Error,
        "tcp_retries1 不应是读取失败"
    );
    assert!(
        snap.cpu.nohz_full.access != aida::access::AccessKind::Error,
        "nohz_full 不应是读取失败（缺失或空表示无 nohz_full CPU）"
    );
    assert!(
        snap.sysctl.write_wakeup_threshold.access != aida::access::AccessKind::Error,
        "write_wakeup_threshold 不应是读取失败"
    );
    assert!(
        snap.net.tcp_frto.access != aida::access::AccessKind::Error,
        "tcp_frto 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_router_solicitations.access != aida::access::AccessKind::Error,
        "router_solicitations 不应是读取失败（-1 表示 RFC 默认次数）"
    );
    assert!(
        snap.sysctl.memfd_noexec.access != aida::access::AccessKind::Error,
        "memfd_noexec 不应是读取失败（旧内核可为 NotFound）"
    );
    assert!(
        snap.cpu.modalias.access != aida::access::AccessKind::Error,
        "cpu modalias 不应是读取失败（无该文件时为 NotFound）"
    );
    assert!(
        snap.cpu.cpuidle_driver.access != aida::access::AccessKind::Error,
        "cpuidle current_driver 不应是读取失败（none 在虚拟机上合法）"
    );
    assert!(
        snap.sysctl.kexec_load_limit_panic.access != aida::access::AccessKind::Error,
        "kexec_load_limit_panic 不应是读取失败（-1 表示不限）"
    );
    assert!(
        snap.sysctl.warn_limit.access != aida::access::AccessKind::Error,
        "warn_limit 不应是读取失败（0 表示不限）"
    );
    assert!(
        snap.sysctl.split_lock_mitigate.access != aida::access::AccessKind::Error,
        "split_lock_mitigate 不应是读取失败（非 x86 可为 NotFound）"
    );
    assert!(
        snap.net.ip_unprivileged_port_start.access != aida::access::AccessKind::Error,
        "ip_unprivileged_port_start 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_dad_transmits.access != aida::access::AccessKind::Error,
        "dad_transmits 不应是读取失败"
    );
    assert!(
        snap.cpu.cpuidle_available_governors.access != aida::access::AccessKind::Error,
        "cpuidle available_governors 不应是读取失败（无 cpuidle 时为 NotFound）"
    );
    assert!(
        snap.sysctl.kexec_load_limit_reboot.access != aida::access::AccessKind::Error,
        "kexec_load_limit_reboot 不应是读取失败（-1 表示不限）"
    );
    assert!(
        snap.sysctl.hung_task_check_interval_secs.access != aida::access::AccessKind::Error,
        "hung_task_check_interval_secs 不应是读取失败（0 表示沿用 timeout）"
    );
    assert!(
        snap.net.tcp_ecn_fallback.access != aida::access::AccessKind::Error,
        "tcp_ecn_fallback 不应是读取失败"
    );
    assert!(
        snap.net.tcp_challenge_ack_limit.access != aida::access::AccessKind::Error,
        "tcp_challenge_ack_limit 不应是读取失败（INT_MAX 表示不额外收紧）"
    );
    assert!(
        snap.sysctl.compaction_proactiveness.access != aida::access::AccessKind::Error,
        "compaction_proactiveness 不应是读取失败"
    );
    assert!(
        snap.net.tcp_comp_sack_nr.access != aida::access::AccessKind::Error,
        "tcp_comp_sack_nr 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_ndisc_notify.access != aida::access::AccessKind::Error,
        "ndisc_notify 不应是读取失败"
    );
    assert!(
        snap.sysctl.hardlockup_all_cpu_backtrace.access != aida::access::AccessKind::Error,
        "hardlockup_all_cpu_backtrace 不应是读取失败"
    );
    assert!(
        snap.net.tcp_early_demux.access != aida::access::AccessKind::Error,
        "tcp_early_demux 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_accept_ra_pinfo.access != aida::access::AccessKind::Error,
        "accept_ra_pinfo 不应是读取失败"
    );
    assert!(
        snap.sysctl.softlockup_all_cpu_backtrace.access != aida::access::AccessKind::Error,
        "softlockup_all_cpu_backtrace 不应是读取失败"
    );
    assert!(
        snap.net.tcp_base_mss.access != aida::access::AccessKind::Error,
        "tcp_base_mss 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_enhanced_dad.access != aida::access::AccessKind::Error,
        "enhanced_dad 不应是读取失败"
    );
    assert!(
        snap.sysctl.printk_delay.access != aida::access::AccessKind::Error,
        "printk_delay 不应是读取失败"
    );
    assert!(
        snap.net.tcp_max_reordering.access != aida::access::AccessKind::Error,
        "tcp_max_reordering 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_accept_ra_mtu.access != aida::access::AccessKind::Error,
        "accept_ra_mtu 不应是读取失败"
    );
    assert!(
        snap.sysctl.perf_event_max_stack.access != aida::access::AccessKind::Error,
        "perf_event_max_stack 不应是读取失败"
    );
    assert!(
        snap.net.tcp_min_rtt_wlen.access != aida::access::AccessKind::Error,
        "tcp_min_rtt_wlen 不应是读取失败"
    );
    assert!(
        snap.net.ipv6_keep_addr_on_down.access != aida::access::AccessKind::Error,
        "keep_addr_on_down 不应是读取失败"
    );
    assert!(
        snap.sysctl.sched_deadline_period_max_us.access != aida::access::AccessKind::Error,
        "sched_deadline_period_max_us 不应是读取失败"
    );
    assert!(
        !snap.buses.tun.is_empty()
            || snap
                .buses
                .notes
                .iter()
                .any(|n| n.contains("tun") && (n.contains("无") || n.contains("权限"))),
        "tun 应来自 misc/tun 或 /dev/net/tun；缺失或权限不足才写 note"
    );
    assert!(
        snap.software.kexec_loaded.access != aida::access::AccessKind::Error,
        "kexec_loaded 不应是读取失败"
    );
    assert!(snap.sysctl.boot_id.value.is_some(), "boot_id 应可读");
    assert!(
        snap.pm.state.access != aida::access::AccessKind::Error,
        "sys/power/state 不应是读取失败"
    );
    assert!(!snap.memory.zones.is_empty(), "zoneinfo 应至少有一个 zone");
    assert!(!snap.fs.mounts.is_empty(), "mountinfo 应至少有一个挂载点");
    assert!(!snap.memory.total_kb.value.is_none(), "MemTotal 应可读");
    assert!(
        !snap.cpu.vulnerabilities.is_empty(),
        "应至少有 CPU vulnerability 节点"
    );
    assert!(
        snap.block
            .devices
            .iter()
            .any(|d| d.rd_bytes.value.is_some()),
        "diskstats 应能对上至少一个块设备"
    );
    assert!(
        !snap.net.interfaces.is_empty(),
        "至少应有 lo 或其它 /sys/class/net 接口"
    );
    assert!(
        !snap.numa.nodes.is_empty(),
        "至少应有 NUMA node0（非 NUMA 内核也会导出 node0）"
    );
    let html = export::to_html(&snap);
    assert!(html.contains("AIDA Linux"));
    assert!(html.contains("GPU"));
    assert!(html.contains("网络"));
    assert!(html.contains("USB"));
    assert!(html.contains("NUMA"));
    assert!(html.contains("告警"));
    assert!(html.contains("内存"));
    assert!(html.contains("电源"));
    assert!(html.contains("声卡"));
    assert!(html.contains("固件"));
    assert!(html.contains("文件系统"));
    assert!(html.contains("loadavg"));
    assert!(html.contains("clocksource"));
    assert!(html.contains("PSI") || html.contains("tainted"));
    assert!(html.contains("virtio"));
    assert!(html.contains("sockstat"));
    assert!(html.contains("vmstat"));
    assert!(html.contains("IOMMU"));
    assert!(html.contains("平台"));
    assert!(html.contains("LSM"));
    assert!(html.contains("KVM"));
    assert!(html.contains("zswap"));
    assert!(html.contains("TCP") || html.contains("softnet"));
    assert!(html.contains("cgroup") || html.contains("file-nr"));
    assert!(html.contains("conntrack") || html.contains("crypto") || html.contains("lockdown"));
    assert!(html.contains("IPv6") || html.contains("sleep") || html.contains("DirectMap"));
    assert!(html.contains("fastopen") || html.contains("gpio") || html.contains("nmi"));
    assert!(html.contains("protocols") || html.contains("nfsd") || html.contains("qdisc"));
    assert!(html.contains("somaxconn"));
    assert!(html.contains("syn/synack") || html.contains("retries"));
    assert!(html.contains("maxkeys") || html.contains("watermark") || html.contains("vtcon"));
    assert!(html.contains("shmmax") || html.contains("kexec") || html.contains("protected"));
    assert!(html.contains("pci_bus") || html.contains("/proc/dma") || html.contains("cascade"));
    assert!(html.contains("sched_rt") || html.contains("tcp6") || html.contains("xfrm"));
    assert!(
        html.contains("igmp6")
            || html.contains("printk")
            || html.contains("binfmt")
            || html.contains("ieee80211")
    );
    assert!(
        html.contains("byteorder")
            || html.contains("dentry")
            || html.contains("key-users")
            || html.contains("connector")
    );
    assert!(
        html.contains("pty")
            || html.contains("io_uring")
            || html.contains("accept_ra")
            || html.contains("ostype")
    );
    assert!(
        html.contains("tcp_mem")
            || html.contains("dirty_bytes")
            || html.contains("addr_gen")
            || html.contains("remoteproc")
            || html.contains("wakeup_sources")
    );
    assert!(
        html.contains("core_pipe")
            || html.contains("ipfrag")
            || html.contains("seccomp")
            || html.contains("spi_master")
    );
    assert!(
        html.contains("frto")
            || html.contains("rng_wake")
            || html.contains("macvtap")
            || html.contains("modalias")
    );
    assert!(
        html.contains("cpuidle")
            || html.contains("kexec_limit")
            || html.contains("unpriv_port")
            || html.contains("iscsi_endpoint")
    );
    assert!(
        html.contains("available")
            || html.contains("kexec_reboot")
            || html.contains("ecn_fb")
            || html.contains("iscsi_flashnode")
            || html.contains("challenge_ack")
    );
    assert!(
        html.contains("comp_sack")
            || html.contains("compact_proact")
            || html.contains("ndisc_notify")
            || html.contains("cxl")
            || html.contains("hung_bt")
    );
    assert!(
        html.contains("early_demux")
            || html.contains("min_slab")
            || html.contains("rpmsg")
            || html.contains("ra_pinfo")
            || html.contains("hardlockup_bt")
    );
    assert!(
        html.contains("softlockup_bt")
            || html.contains("base_mss")
            || html.contains("scsi_disk")
            || html.contains("enhanced_dad")
            || html.contains("extfrag")
    );
    assert!(
        html.contains("printk_delay")
            || html.contains("max_reorder")
            || html.contains("cec")
            || html.contains("ra_mtu")
            || html.contains("hugetlb_vmemmap")
    );
    assert!(
        html.contains("max_stack")
            || html.contains("min_rtt_wlen")
            || html.contains("vfio")
            || html.contains("keep_addr")
            || html.contains("numa_stat")
    );
    assert!(
        html.contains("ra_min_hop")
            || html.contains("numa_promote")
            || html.contains("udp_wmem_min")
            || html.contains("legacy_va")
            || html.contains("vdpa")
            || html.contains("accel")
    );
    assert!(
        html.contains("ra_min_lft")
            || html.contains("plb")
            || html.contains("uio")
            || html.contains("usbmon")
            || html.contains("zonelist")
            || html.contains("auto_msgmni")
    );
    assert!(
        html.contains("stdurg")
            || html.contains("signed_win")
            || html.contains("counter")
            || html.contains("drm_dp_aux")
            || html.contains("hugepages_mempolicy")
            || html.contains("ra_rt_min_plen")
    );
    assert!(!html.contains("<script"));
}

#[test]
fn pci_scan_does_not_require_pci_ids() {
    let snap = HardwareSnapshot::collect_cpu_sample(&ProbeCtx::live(), false);
    // 本 CI 虚拟机有 virtio PCI 设备；即使没有也不应 panic。
    for d in &snap.pci.devices {
        assert_eq!(d.vendor_id.len(), 4);
        assert_eq!(d.device_id.len(), 4);
    }
}
