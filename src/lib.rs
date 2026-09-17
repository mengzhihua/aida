//! AIDA Linux：从 procfs/sysfs 采集硬件信息的核心库。
//!
//! - [`access`] 权限与读取原语
//! - [`probes`] CPU / DMI / GPU / virtio / KVM / IOMMU / 网络 / USB / 输入 / NUMA / 内存 / zram/zswap / 电源 / RAPL / 声卡 / 固件 / 文件系统 / 模块 / 时钟 / EDAC / iomem / PSI / IRQ / ATA / MD / SCSI / iSCSI / 平台 / 总线 / cgroup / sysctl / 安全 / crypto / 命名空间 / hwmon / NVMe / PCI / 块设备 / 软件
//! - [`snapshot`] 一次完整快照
//! - [`export`] JSON/HTML
//! - [`bench`] CPU / 内存 / 磁盘（buffered + O_DIRECT）
//! - [`elevate`] pkexec/sudo 提权重启
//! - [`alerts`] hwmon 阈值告警与 JSONL 日志

pub mod access;
pub mod alerts;
pub mod bench;
pub mod elevate;
pub mod export;
pub mod probes;
pub mod snapshot;

#[cfg(feature = "gui")]
pub mod ui;

pub use access::{Privilege, ProbeCtx, Sample};
pub use snapshot::HardwareSnapshot;
