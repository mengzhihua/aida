//! 汇总一次完整快照。采集层无副作用，可在 CLI 与 GUI 之间共享。

use serde::Serialize;

use crate::access::{Privilege, ProbeCtx};
use crate::alerts::{self, Alert};
use crate::probes::block::DiskSnap;
use crate::probes::net::NetSnap;
use crate::probes::{
    audio, block, cpu, dmi, firmware, gpu, hwmon, input, memory, net, numa, nvme, pci, power,
    software, usb,
};

#[derive(Clone, Debug, Serialize)]
pub struct HardwareSnapshot {
    pub app: &'static str,
    pub version: &'static str,
    pub collected_at_unix_ms: u64,
    pub privilege: Privilege,
    pub cpu: cpu::CpuInfo,
    pub dmi: dmi::DmiInfo,
    pub firmware: firmware::FirmwareReport,
    pub memory: memory::MemoryReport,
    pub sensors: hwmon::SensorReport,
    pub alerts: Vec<Alert>,
    pub nvme: nvme::NvmeReport,
    pub pci: pci::PciReport,
    pub gpu: gpu::GpuReport,
    pub net: net::NetReport,
    pub usb: usb::UsbReport,
    pub input: input::InputReport,
    pub audio: audio::AudioReport,
    pub power: power::PowerReport,
    pub numa: numa::NumaReport,
    pub block: block::BlockReport,
    pub software: software::SoftwareInfo,
}

impl HardwareSnapshot {
    pub fn collect(ctx: &ProbeCtx) -> Self {
        Self::collect_cpu_sample(ctx, true)
    }

    pub fn collect_cpu_sample(ctx: &ProbeCtx, sample_util: bool) -> Self {
        let net_prev = if sample_util {
            Some(net::counters(&net::collect(ctx)))
        } else {
            None
        };
        let disk_prev = if sample_util {
            Some(block::counters(&block::collect(ctx)))
        } else {
            None
        };
        let cpu = if sample_util {
            cpu::collect(ctx)
        } else {
            cpu::collect_with_util(ctx, None)
        };
        let (net, block) = if sample_util {
            std::thread::sleep(std::time::Duration::from_millis(120));
            (
                net::collect_with_prev(ctx, net_prev.as_deref(), 0.12),
                block::collect_with_prev(ctx, disk_prev.as_deref(), 0.12),
            )
        } else {
            (net::collect(ctx), block::collect(ctx))
        };
        let sensors = hwmon::collect(ctx);
        let alerts = alerts::evaluate(&sensors);
        Self {
            app: "aida",
            version: env!("CARGO_PKG_VERSION"),
            collected_at_unix_ms: unix_ms(),
            privilege: Privilege::detect(),
            cpu,
            dmi: dmi::collect(ctx),
            firmware: firmware::collect(ctx),
            memory: memory::collect(ctx),
            sensors,
            alerts,
            nvme: nvme::collect(ctx),
            pci: pci::collect(ctx),
            gpu: gpu::collect(ctx),
            net,
            usb: usb::collect(ctx),
            input: input::collect(ctx),
            audio: audio::collect(ctx),
            power: power::collect(ctx),
            numa: numa::collect(ctx),
            block,
            software: software::collect(ctx),
        }
    }

    pub fn refresh_live(
        &mut self,
        ctx: &ProbeCtx,
        prev_stat: &mut Option<cpu::CpuStatSnap>,
        prev_net: &mut Option<Vec<NetSnap>>,
        prev_disk: &mut Option<Vec<DiskSnap>>,
        dt_sec: f64,
    ) {
        let now = cpu::read_proc_stat(ctx);
        self.cpu.utilization_pct = cpu::utilization(prev_stat, &now);
        let mut logical = cpu::collect_with_util(ctx, None).logical;
        cpu::apply_per_cpu(&mut logical, prev_stat, &now);
        self.cpu.logical = logical;
        *prev_stat = now;
        self.sensors = hwmon::collect(ctx);
        self.alerts = alerts::evaluate(&self.sensors);
        self.gpu = gpu::collect(ctx);
        self.net = net::collect_with_prev(ctx, prev_net.as_deref(), dt_sec);
        *prev_net = Some(net::counters(&self.net));
        self.block = block::collect_with_prev(ctx, prev_disk.as_deref(), dt_sec);
        *prev_disk = Some(block::counters(&self.block));
        self.memory = memory::collect(ctx);
        self.power = power::collect(ctx);
        self.collected_at_unix_ms = unix_ms();
    }
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
