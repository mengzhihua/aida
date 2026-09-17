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
    assert!(
        !snap.irq.softirqs.is_empty(),
        "/proc/softirqs 应至少有一行"
    );
    assert!(
        snap.memory.vmstat.pgfault.value.is_some(),
        "vmstat pgfault 应可读"
    );
    assert!(
        !snap.memory.buddy.is_empty(),
        "buddyinfo 应至少有一个 zone"
    );
    assert!(
        snap.net.interfaces.iter().any(|i| i.rx_queues + i.tx_queues > 0)
            || snap.net.interfaces.iter().any(|i| i.name == "lo"),
        "网卡应能看到 queues 或至少 lo"
    );
    assert!(
        snap.psi.cpu.is_some() || !snap.psi.notes.is_empty(),
        "PSI 应可读或给出说明"
    );
    assert!(
        !snap.irq.lines.is_empty(),
        "/proc/interrupts 应至少有一行"
    );
    assert!(
        snap.software.tainted.value.is_some(),
        "kernel tainted 应可读"
    );
    assert!(
        snap.fs.mounts.iter().any(|m| m.total_bytes.is_some()),
        "至少有一个挂载点能 statvfs"
    );
    assert!(
        snap.software.load_1.value.is_some(),
        "loadavg 应可读"
    );
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
    assert!(
        snap.net.softnet.cpus >= 1,
        "softnet_stat 应至少有一行"
    );
    assert!(
        snap.sysctl.pid_max.value.is_some(),
        "pid_max 应可读"
    );
    assert!(
        snap.sysctl.file_nr_alloc.value.is_some(),
        "file-nr 应可读"
    );
    assert!(
        snap.cgroup.controllers.value.is_some() || !snap.cgroup.notes.is_empty(),
        "cgroup v2 应可读或给出说明"
    );
    assert!(
        snap.crypto.total >= 1,
        "/proc/crypto 应至少有一个算法"
    );
    assert!(
        snap.net.tcp_congestion.value.is_some(),
        "tcp_congestion_control 应可读"
    );
    assert!(
        !snap.ns.self_ns.is_empty(),
        "/proc/self/ns 应至少有一个命名空间"
    );
    assert!(
        !snap.memory.zones.is_empty(),
        "zoneinfo 应至少有一个 zone"
    );
    assert!(
        !snap.fs.mounts.is_empty(),
        "mountinfo 应至少有一个挂载点"
    );
    assert!(
        !snap.memory.total_kb.value.is_none(),
        "MemTotal 应可读"
    );
    assert!(
        !snap.cpu.vulnerabilities.is_empty(),
        "应至少有 CPU vulnerability 节点"
    );
    assert!(
        snap.block.devices.iter().any(|d| d.rd_bytes.value.is_some()),
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
