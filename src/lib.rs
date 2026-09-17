//! AIDA Linux：从 procfs/sysfs 采集硬件信息的核心库。
//!
//! 第一轮模块划分：
//! - [`access`] 权限与读取原语
//! - [`probes`] CPU / DMI / hwmon / NVMe / PCI / 块设备 / 软件
//! - [`snapshot`] 一次完整快照
//! - [`export`] JSON/HTML
//! - [`bench`] CPU / 内存 / 磁盘微基准

pub mod access;
pub mod bench;
pub mod export;
pub mod probes;
pub mod snapshot;

#[cfg(feature = "gui")]
pub mod ui;

pub use access::{Privilege, ProbeCtx, Sample};
pub use snapshot::HardwareSnapshot;
