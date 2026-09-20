//! AIDA Linux：从 procfs/sysfs 采集硬件信息的核心库。
//!
//! - [`access`] 权限与读取原语
//! - [`probes`] CPU / DMI / GPU / virtio / KVM / IOMMU / 网络 / USB / 输入 / NUMA / 内存 / zram/zswap / 电源 / 睡眠 / RAPL / 声卡 / 固件 / 文件系统 / 模块 / 时钟 / EDAC / iomem / PSI / IRQ / ATA / MD / SCSI / iSCSI / 平台 / DMA/PWM/IIO / 总线 / GPIO / MTD / cgroup / sysctl / 安全 / crypto / 命名空间 / hwmon / NVMe / PCI / 块设备 / 软件
//! - [`snapshot`] 一次完整快照
//! - [`export`] JSON/HTML
//! - [`bench`] CPU / 内存 / 磁盘（buffered + O_DIRECT）
//! - [`doctor`] 发行版 / glibc / GUI 库体检（apt 与 dnf/yum）
//! - [`elevate`] pkexec/sudo 提权重启
//! - [`alerts`] hwmon 阈值告警与 JSONL 日志
//! - [`record`] CPU/内存/网络/磁盘/温度 JSONL 历史（对标 iStat Menus）

pub mod access;
pub mod alerts;
pub mod bench;
pub mod doctor;
pub mod elevate;
pub mod export;
pub mod probes;
pub mod record;
pub mod snapshot;

#[cfg(feature = "gui")]
pub mod ui;

pub use access::{Privilege, ProbeCtx, Sample};
pub use snapshot::HardwareSnapshot;
