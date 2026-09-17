//! 硬件探测模块。每个子模块只负责一类内核接口，由 `snapshot` 汇总。

pub mod block;
pub mod cpu;
pub mod dmi;
pub mod hwmon;
pub mod nvme;
pub mod pci;
pub mod software;
