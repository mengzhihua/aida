//! 汇总一次完整快照。采集层无副作用，可在 CLI 与 GUI 之间共享。

use serde::Serialize;

use crate::access::{Privilege, ProbeCtx};
use crate::alerts::{self, Alert};
use crate::probes::block::DiskSnap;
use crate::probes::net::NetSnap;
use crate::probes::rapl::RaplSnap;
use crate::probes::{
    ata, audio, block, buses, cgroup, clock, cpu, crypto, dmi, edac, firmware, fs, gpu, hwmon,
    input, iomem, iommu, irq, iscsi, kvm, md, memory, modules, net, ns, numa, nvme, pci, periph,
    platform, pm, power, psi, rapl, scsi, security, software, sysctl, usb, virtio, zmem,
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
    pub virtio: virtio::VirtioReport,
    pub kvm: kvm::KvmReport,
    pub iommu: iommu::IommuReport,
    pub gpu: gpu::GpuReport,
    pub net: net::NetReport,
    pub usb: usb::UsbReport,
    pub input: input::InputReport,
    pub audio: audio::AudioReport,
    pub power: power::PowerReport,
    pub pm: pm::PmReport,
    pub rapl: rapl::RaplReport,
    pub numa: numa::NumaReport,
    pub block: block::BlockReport,
    pub fs: fs::FsReport,
    pub modules: modules::ModulesReport,
    pub clock: clock::ClockReport,
    pub edac: edac::EdacReport,
    pub iomem: iomem::IomemReport,
    pub psi: psi::PsiReport,
    pub irq: irq::IrqReport,
    pub ata: ata::AtaReport,
    pub md: md::MdReport,
    pub scsi: scsi::ScsiReport,
    pub iscsi: iscsi::IscsiReport,
    pub zmem: zmem::ZmemReport,
    pub platform: platform::PlatformReport,
    pub periph: periph::PeriphReport,
    pub buses: buses::BusesReport,
    pub cgroup: cgroup::CgroupReport,
    pub sysctl: sysctl::SysctlReport,
    pub security: security::SecurityReport,
    pub crypto: crypto::CryptoReport,
    pub ns: ns::NsReport,
    pub software: software::SoftwareInfo,
}

impl HardwareSnapshot {
    pub fn collect(ctx: &ProbeCtx) -> Self {
        Self::collect_cpu_sample(ctx, true)
    }

    pub fn collect_cpu_sample(ctx: &ProbeCtx, sample_util: bool) -> Self {
        // 第一次计数必须记下真实墙钟：cpu::collect 自己还会 sleep 120ms，
        // 若再 sleep 一次却仍除以 0.12，RAPL/网卡/磁盘速率会被放大约一倍。
        let rate_t0 = if sample_util {
            Some(std::time::Instant::now())
        } else {
            None
        };
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
        let rapl_prev = if sample_util {
            Some(rapl::counters(&rapl::collect(ctx)))
        } else {
            None
        };
        let cpu = if sample_util {
            cpu::collect(ctx)
        } else {
            cpu::collect_with_util(ctx, None)
        };
        let (net, block, rapl) = if sample_util {
            let t0 = rate_t0.expect("sample_util 时已记录起点");
            let min = std::time::Duration::from_millis(120);
            if let Some(remain) = min.checked_sub(t0.elapsed()) {
                std::thread::sleep(remain);
            }
            let dt = t0.elapsed().as_secs_f64().max(1e-3);
            (
                net::collect_with_prev(ctx, net_prev.as_deref(), dt),
                block::collect_with_prev(ctx, disk_prev.as_deref(), dt),
                rapl::collect_with_prev(ctx, rapl_prev.as_deref(), dt),
            )
        } else {
            (net::collect(ctx), block::collect(ctx), rapl::collect(ctx))
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
            virtio: virtio::collect(ctx),
            kvm: kvm::collect(ctx),
            iommu: iommu::collect(ctx),
            gpu: gpu::collect(ctx),
            net,
            usb: usb::collect(ctx),
            input: input::collect(ctx),
            audio: audio::collect(ctx),
            power: power::collect(ctx),
            pm: pm::collect(ctx),
            rapl,
            numa: numa::collect(ctx),
            block,
            fs: fs::collect(ctx),
            modules: modules::collect(ctx),
            clock: clock::collect(ctx),
            edac: edac::collect(ctx),
            iomem: iomem::collect(ctx),
            psi: psi::collect(ctx),
            irq: irq::collect(ctx),
            ata: ata::collect(ctx),
            md: md::collect(ctx),
            scsi: scsi::collect(ctx),
            iscsi: iscsi::collect(ctx),
            zmem: zmem::collect(ctx),
            platform: platform::collect(ctx),
            periph: periph::collect(ctx),
            buses: buses::collect(ctx),
            cgroup: cgroup::collect(ctx),
            sysctl: sysctl::collect(ctx),
            security: security::collect(ctx),
            crypto: crypto::collect(ctx),
            ns: ns::collect(ctx),
            software: software::collect(ctx),
        }
    }

    pub fn refresh_live(
        &mut self,
        ctx: &ProbeCtx,
        prev_stat: &mut Option<cpu::CpuStatSnap>,
        prev_net: &mut Option<Vec<NetSnap>>,
        prev_disk: &mut Option<Vec<DiskSnap>>,
        prev_rapl: &mut Option<Vec<RaplSnap>>,
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
        self.pm = pm::collect(ctx);
        self.rapl = rapl::collect_with_prev(ctx, prev_rapl.as_deref(), dt_sec);
        *prev_rapl = Some(rapl::counters(&self.rapl));
        self.software = software::collect(ctx);
        self.clock = clock::collect(ctx);
        self.edac = edac::collect(ctx);
        self.fs = fs::collect(ctx);
        self.psi = psi::collect(ctx);
        self.irq = irq::collect(ctx);
        self.platform = platform::collect(ctx);
        self.zmem = zmem::collect(ctx);
        self.sysctl = sysctl::collect(ctx);
        self.cgroup = cgroup::collect(ctx);
        self.security = security::collect(ctx);
        self.collected_at_unix_ms = unix_ms();
    }
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
