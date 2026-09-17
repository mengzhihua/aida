//! 硬件探测模块。每个子模块只负责一类内核接口，由 `snapshot` 汇总。

pub mod audio;
pub mod block;
pub mod cpu;
pub mod dmi;
pub mod firmware;
pub mod gpu;
pub mod hwmon;
pub mod input;
pub mod memory;
pub mod net;
pub mod numa;
pub mod nvme;
pub mod pci;
pub mod power;
pub mod software;
pub mod usb;
