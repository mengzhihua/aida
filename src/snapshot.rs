//! 汇总一次完整快照。采集层无副作用，可在 CLI 与 GUI 之间共享。

use serde::Serialize;

use crate::access::{Privilege, ProbeCtx};
use crate::probes::{block, cpu, dmi, gpu, hwmon, nvme, pci, software};

#[derive(Clone, Debug, Serialize)]
pub struct HardwareSnapshot {
    pub app: &'static str,
    pub version: &'static str,
    pub collected_at_unix_ms: u64,
    pub privilege: Privilege,
    pub cpu: cpu::CpuInfo,
    pub dmi: dmi::DmiInfo,
    pub sensors: hwmon::SensorReport,
    pub nvme: nvme::NvmeReport,
    pub pci: pci::PciReport,
    pub gpu: gpu::GpuReport,
    pub block: block::BlockReport,
    pub software: software::SoftwareInfo,
}

impl HardwareSnapshot {
    pub fn collect(ctx: &ProbeCtx) -> Self {
        Self::collect_cpu_sample(ctx, true)
    }

    pub fn collect_cpu_sample(ctx: &ProbeCtx, sample_util: bool) -> Self {
        let cpu = if sample_util {
            cpu::collect(ctx)
        } else {
            cpu::collect_with_util(ctx, None)
        };
        Self {
            app: "aida",
            version: env!("CARGO_PKG_VERSION"),
            collected_at_unix_ms: unix_ms(),
            privilege: Privilege::detect(),
            cpu,
            dmi: dmi::collect(ctx),
            sensors: hwmon::collect(ctx),
            nvme: nvme::collect(ctx),
            pci: pci::collect(ctx),
            gpu: gpu::collect(ctx),
            block: block::collect(ctx),
            software: software::collect(ctx),
        }
    }

    pub fn refresh_live(&mut self, ctx: &ProbeCtx, prev_stat: &mut Option<cpu::CpuStatSnap>) {
        let now = cpu::read_proc_stat(ctx);
        self.cpu.utilization_pct = cpu::utilization(prev_stat, &now);
        *prev_stat = now;
        self.sensors = hwmon::collect(ctx);
        self.gpu = gpu::collect(ctx);
        self.cpu.logical = cpu::collect_with_util(ctx, None).logical;
        self.collected_at_unix_ms = unix_ms();
    }
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
