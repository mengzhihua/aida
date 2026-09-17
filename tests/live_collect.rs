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
