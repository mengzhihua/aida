//! AIDA Linux：从 procfs/sysfs 采集硬件信息的核心库。
//!
//! - [`access`] 权限与读取原语
//! - [`probes`] CPU / DMI / GPU / hwmon / NVMe / PCI / 块设备 / 软件
//! - [`snapshot`] 一次完整快照
//! - [`export`] JSON/HTML
//! - [`bench`] CPU / 内存 / 磁盘（buffered + O_DIRECT）
//! - [`elevate`] pkexec/sudo 提权重启

pub mod access;
pub mod bench;
pub mod elevate;
pub mod export;
pub mod probes;
pub mod snapshot;

#[cfg(feature = "gui")]
pub mod ui;

pub use access::{Privilege, ProbeCtx, Sample};
pub use snapshot::HardwareSnapshot;
