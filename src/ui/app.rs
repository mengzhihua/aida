//! 桌面布局：模仿 AIDA64 的树形导航 + 详情表 + 传感器历史曲线。

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, RichText};
use egui_plot::{Line, Plot, PlotPoints};

use crate::access::{AccessKind, ProbeCtx};
use crate::alerts::{AlertLevel, AlertLogger};
use crate::bench::{self, BenchReport, BenchRequest};
use crate::export;
use crate::probes::block::DiskSnap;
use crate::probes::cpu::CpuStatSnap;
use crate::probes::hwmon;
use crate::probes::net::NetSnap;
use crate::probes::rapl::RaplSnap;
use crate::snapshot::HardwareSnapshot;

const HISTORY: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Summary,
    Cpu,
    Dmi,
    Memory,
    Gpu,
    Sensors,
    Power,
    Storage,
    Filesystems,
    Network,
    Usb,
    Input,
    Audio,
    Pci,
    Platform,
    Numa,
    Software,
    Bench,
    Export,
}

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AIDA Linux")
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "AIDA Linux",
        options,
        Box::new(|cc| Ok(Box::new(AidaApp::new(cc)))),
    )
    .map_err(|e| e.to_string())
}

struct AidaApp {
    ctx: ProbeCtx,
    snap: HardwareSnapshot,
    prev_stat: Option<CpuStatSnap>,
    prev_net: Option<Vec<NetSnap>>,
    prev_disk: Option<Vec<DiskSnap>>,
    prev_rapl: Option<Vec<RaplSnap>>,
    alert_log: AlertLogger,
    alert_log_path: PathBuf,
    alert_log_err: Option<String>,
    nav: Nav,
    cjk: bool,
    last_poll: Instant,
    temps: HashMap<String, VecDeque<[f64; 2]>>,
    cpu_hist: VecDeque<[f64; 2]>,
    net_hist: HashMap<String, VecDeque<[f64; 2]>>,
    disk_hist: HashMap<String, VecDeque<[f64; 2]>>,
    rapl_hist: HashMap<String, VecDeque<[f64; 2]>>,
    t0: Instant,
    bench: Option<BenchReport>,
    export_msg: Option<String>,
    elevate_msg: Option<String>,
}

impl AidaApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cjk = install_fonts(&cc.egui_ctx);
        let visuals = egui::Visuals::dark();
        cc.egui_ctx.set_visuals(visuals);
        let ctx = ProbeCtx::live();
        // GUI 不在启动时 sleep 测利用率，改为后续帧差分。
        let snap = HardwareSnapshot::collect_cpu_sample(&ctx, false);
        let prev_stat = crate::probes::cpu::read_proc_stat(&ctx);
        let prev_net = Some(crate::probes::net::counters(&snap.net));
        let prev_disk = Some(crate::probes::block::counters(&snap.block));
        let prev_rapl = Some(crate::probes::rapl::counters(&snap.rapl));
        Self {
            ctx,
            snap,
            prev_stat,
            prev_net,
            prev_disk,
            prev_rapl,
            alert_log: AlertLogger::default(),
            alert_log_path: crate::alerts::default_log_path(),
            alert_log_err: None,
            nav: Nav::Summary,
            cjk,
            last_poll: Instant::now(),
            temps: HashMap::new(),
            cpu_hist: VecDeque::new(),
            net_hist: HashMap::new(),
            disk_hist: HashMap::new(),
            rapl_hist: HashMap::new(),
            t0: Instant::now(),
            bench: None,
            export_msg: None,
            elevate_msg: None,
        }
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        tr(self.cjk, zh, en)
    }

    fn poll_sensors(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(800) {
            return;
        }
        let dt = self.last_poll.elapsed().as_secs_f64();
        self.last_poll = Instant::now();
        self.snap.refresh_live(
            &self.ctx,
            &mut self.prev_stat,
            &mut self.prev_net,
            &mut self.prev_disk,
            &mut self.prev_rapl,
            dt,
        );
        let events = self
            .alert_log
            .ingest(self.snap.collected_at_unix_ms, &self.snap.alerts);
        if let Err(e) = crate::alerts::append_jsonl(&self.alert_log_path, &events) {
            self.alert_log_err = Some(e);
        }
        let t = self.t0.elapsed().as_secs_f64();
        if let Some(u) = self.snap.cpu.utilization_pct {
            push_hist(&mut self.cpu_hist, t, u as f64);
        }
        for (key, value) in hwmon::temperature_series(&self.snap.sensors) {
            push_hist(self.temps.entry(key).or_default(), t, value);
        }
        for i in &self.snap.net.interfaces {
            if let Some(bps) = i.rx_bps {
                push_hist(
                    self.net_hist.entry(format!("{} RX", i.name)).or_default(),
                    t,
                    bps,
                );
            }
            if let Some(bps) = i.tx_bps {
                push_hist(
                    self.net_hist.entry(format!("{} TX", i.name)).or_default(),
                    t,
                    bps,
                );
            }
        }
        for d in &self.snap.block.devices {
            if let Some(bps) = d.rd_bps {
                push_hist(
                    self.disk_hist.entry(format!("{} rd", d.name)).or_default(),
                    t,
                    bps,
                );
            }
            if let Some(bps) = d.wr_bps {
                push_hist(
                    self.disk_hist.entry(format!("{} wr", d.name)).or_default(),
                    t,
                    bps,
                );
            }
        }
        for z in &self.snap.rapl.zones {
            if let Some(w) = z.power_w {
                let label = z
                    .label
                    .value
                    .clone()
                    .unwrap_or_else(|| z.name.clone());
                push_hist(self.rapl_hist.entry(label).or_default(), t, w);
            }
        }
    }
}

impl eframe::App for AidaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_sensors();
        ctx.request_repaint_after(Duration::from_millis(500));

        egui::TopBottomPanel::top("priv").show(ctx, |ui| {
            let color = if self.snap.privilege.is_root {
                Color32::from_rgb(120, 200, 140)
            } else {
                Color32::from_rgb(255, 179, 71)
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.t("权限", "Privilege")).strong());
                ui.colored_label(color, &self.snap.privilege.summary);
                if !self.snap.privilege.is_root {
                    if ui
                        .button(self.t("以管理员身份重启", "Restart as admin"))
                        .clicked()
                    {
                        if let Err(e) = crate::elevate::reexec(&["gui".into()]) {
                            self.elevate_msg = Some(e);
                        }
                    }
                }
            });
            if let Some(m) = &self.elevate_msg {
                ui.colored_label(Color32::from_rgb(255, 100, 100), m);
            }
            if !self.snap.alerts.is_empty() {
                ui.colored_label(
                    Color32::from_rgb(255, 120, 80),
                    format!(
                        "{} {}",
                        self.snap.alerts.len(),
                        self.t("条传感器告警", "sensor alert(s)")
                    ),
                );
            }
            ui.weak(crate::elevate::plan().summary);
        });

        egui::SidePanel::left("tree")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("AIDA");
                ui.label(RichText::new("Linux").italics().weak());
                ui.separator();
                let cjk = self.cjk;
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Summary,
                    tr(cjk, "计算机摘要", "Summary"),
                );
                nav_btn(ui, &mut self.nav, Nav::Cpu, tr(cjk, "处理器", "CPU"));
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Dmi,
                    tr(cjk, "主板 / DMI", "Motherboard / DMI"),
                );
                nav_btn(ui, &mut self.nav, Nav::Memory, tr(cjk, "内存", "Memory"));
                nav_btn(ui, &mut self.nav, Nav::Gpu, tr(cjk, "显示适配器", "GPU"));
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Sensors,
                    tr(cjk, "传感器", "Sensors"),
                );
                nav_btn(ui, &mut self.nav, Nav::Power, tr(cjk, "电源 / 电池", "Power"));
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Storage,
                    tr(cjk, "存储 / NVMe", "Storage / NVMe"),
                );
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Filesystems,
                    tr(cjk, "文件系统", "Filesystems"),
                );
                nav_btn(ui, &mut self.nav, Nav::Network, tr(cjk, "网络", "Network"));
                nav_btn(ui, &mut self.nav, Nav::Usb, tr(cjk, "USB", "USB"));
                nav_btn(ui, &mut self.nav, Nav::Input, tr(cjk, "输入设备", "Input"));
                nav_btn(ui, &mut self.nav, Nav::Audio, tr(cjk, "声卡", "Audio"));
                nav_btn(ui, &mut self.nav, Nav::Pci, tr(cjk, "PCI 设备", "PCI"));
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Platform,
                    tr(cjk, "平台 / 总线", "Platform"),
                );
                nav_btn(ui, &mut self.nav, Nav::Numa, tr(cjk, "NUMA 内存", "NUMA"));
                nav_btn(ui, &mut self.nav, Nav::Software, tr(cjk, "操作系统", "OS"));
                ui.separator();
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Bench,
                    tr(cjk, "基准测试", "Benchmark"),
                );
                nav_btn(
                    ui,
                    &mut self.nav,
                    Nav::Export,
                    tr(cjk, "导出报告", "Export"),
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.nav {
            Nav::Summary => self.ui_summary(ui),
            Nav::Cpu => self.ui_cpu(ui),
            Nav::Dmi => self.ui_dmi(ui),
            Nav::Memory => self.ui_memory(ui),
            Nav::Gpu => self.ui_gpu(ui),
            Nav::Sensors => self.ui_sensors(ui),
            Nav::Power => self.ui_power(ui),
            Nav::Storage => self.ui_storage(ui),
            Nav::Filesystems => self.ui_fs(ui),
            Nav::Network => self.ui_net(ui),
            Nav::Usb => self.ui_usb(ui),
            Nav::Input => self.ui_input(ui),
            Nav::Audio => self.ui_audio(ui),
            Nav::Pci => self.ui_pci(ui),
            Nav::Platform => self.ui_platform(ui),
            Nav::Numa => self.ui_numa(ui),
            Nav::Software => self.ui_software(ui),
            Nav::Bench => self.ui_bench(ui),
            Nav::Export => self.ui_export(ui),
        });
    }
}

impl AidaApp {
    fn ui_summary(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("计算机摘要", "Summary"));
        kv(
            ui,
            self.t("CPU", "CPU"),
            &self.snap.cpu.model_name.display(),
        );
        kv(
            ui,
            self.t("逻辑 CPU", "Logical CPUs"),
            &self.snap.cpu.logical_cpus.to_string(),
        );
        if let Some(u) = self.snap.cpu.utilization_pct {
            kv(ui, self.t("利用率", "Utilization"), &format!("{u:.1}%"));
        }
        kv(
            ui,
            "loadavg",
            &format!(
                "{}  {}  {}",
                self.snap.software.load_1.display(),
                self.snap.software.load_5.display(),
                self.snap.software.load_15.display()
            ),
        );
        if let Some(cpu) = &self.snap.psi.cpu {
            kv(
                ui,
                "PSI",
                &format!(
                    "cpu {:.2}  mem {}  io {}",
                    cpu.some.avg10,
                    self.snap
                        .psi
                        .memory
                        .as_ref()
                        .map(|m| format!("{:.2}", m.some.avg10))
                        .unwrap_or_else(|| "—".into()),
                    self.snap
                        .psi
                        .io
                        .as_ref()
                        .map(|m| format!("{:.2}", m.some.avg10))
                        .unwrap_or_else(|| "—".into()),
                ),
            );
        }
        kv(
            ui,
            self.t("系统厂商", "Vendor"),
            &self.snap.dmi.sys_vendor.display(),
        );
        kv(
            ui,
            self.t("产品", "Product"),
            &self.snap.dmi.product_name.display(),
        );
        kv(
            ui,
            self.t("操作系统", "OS"),
            &self.snap.software.os_name.display(),
        );
        kv(
            ui,
            self.t("内核", "Kernel"),
            &self.snap.software.kernel_release.display(),
        );
        if let Some(kb) = self.snap.memory.total_kb.value {
            let avail = self
                .snap
                .memory
                .available_kb
                .value
                .map(|v| crate::export::format_bytes(v * 1024))
                .unwrap_or_else(|| "—".into());
            kv(
                ui,
                self.t("内存", "Memory"),
                &format!("{}  (avail {avail})", crate::export::format_bytes(kb * 1024)),
            );
        }
        kv(
            ui,
            self.t("固件", "Firmware"),
            &self
                .snap
                .firmware
                .interface
                .value
                .clone()
                .unwrap_or_else(|| self.snap.firmware.interface.access_label()),
        );
        kv(
            ui,
            self.t("PCI 设备数", "PCI devices"),
            &self.snap.pci.devices.len().to_string(),
        );
        if !self.snap.virtio.devices.is_empty() {
            kv(
                ui,
                "virtio",
                &self
                    .snap
                    .virtio
                    .devices
                    .iter()
                    .map(|d| format!("{} {}", d.name, d.kind))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        kv(
            ui,
            "KVM",
            &format!(
                "{}  {}  nested {}",
                self.snap.kvm.device.display(),
                self.snap.kvm.vendor.display(),
                self.snap.kvm.nested.display()
            ),
        );
        kv(
            ui,
            self.t("GPU", "GPU"),
            &if self.snap.gpu.devices.is_empty() {
                self.snap
                    .gpu
                    .notes
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "none".into())
            } else {
                self.snap
                    .gpu
                    .devices
                    .iter()
                    .map(|g| format!("{} ({})", g.id, g.driver))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        );
        kv(
            ui,
            self.t("块设备", "Block devices"),
            &self.snap.block.devices.len().to_string(),
        );
        kv(
            ui,
            self.t("网卡", "NICs"),
            &self.snap.net.interfaces.len().to_string(),
        );
        kv(ui, "USB", &self.snap.usb.devices.len().to_string());
        kv(
            ui,
            self.t("输入设备", "Input devices"),
            &self.snap.input.devices.len().to_string(),
        );
        kv(ui, "NUMA", &self.snap.numa.nodes.len().to_string());
        if !self.snap.alerts.is_empty() {
            kv(
                ui,
                self.t("传感器告警", "Sensor alerts"),
                &self.snap.alerts.len().to_string(),
            );
            for a in &self.snap.alerts {
                ui.colored_label(alert_color(a.level), &a.message);
            }
        }
        ui.separator();
        ui.label(self.t(
            "缺字段时请看橙色权限条：普通用户看不到序列号/SMART 是预期行为。",
            "Missing fields usually mean the current user cannot read that sysfs node.",
        ));
    }

    fn ui_cpu(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("处理器", "CPU"));
        kv(
            ui,
            self.t("型号", "Model"),
            &self.snap.cpu.model_name.display(),
        );
        kv(
            ui,
            self.t("厂商", "Vendor"),
            &self.snap.cpu.vendor.display(),
        );
        kv(
            ui,
            self.t("封装 / 每封装核心", "Packages / cores"),
            &format!(
                "{} / {}",
                self.snap.cpu.physical_packages,
                self.snap.cpu.cores_per_package.display()
            ),
        );
        kv(
            ui,
            "hypervisor",
            if self.snap.cpu.hypervisor {
                "yes"
            } else {
                "no"
            },
        );
        kv(
            ui,
            "SMT",
            &format!(
                "active {}  control {}",
                self.snap.cpu.smt_active.display(),
                self.snap.cpu.smt_control.display()
            ),
        );
        kv(ui, "online", &self.snap.cpu.online.display());
        kv(
            ui,
            "offline",
            &match (
                self.snap.cpu.offline.access,
                self.snap.cpu.offline.value.as_deref(),
            ) {
                (AccessKind::Ok, Some(s)) if !s.is_empty() => s.to_string(),
                (AccessKind::Ok, _) => "—".into(),
                _ => self.snap.cpu.offline.access_label(),
            },
        );
        if self.snap.cpu.isolated.access == AccessKind::Ok {
            kv(
                ui,
                "isolated",
                &self
                    .snap
                    .cpu
                    .isolated
                    .value
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("—"),
            );
        }
        kv(ui, "microcode", &self.snap.cpu.microcode.display());
        if !self.cpu_hist.is_empty() {
            Plot::new("cpu_util_plot")
                .height(140.0)
                .legend(egui_plot::Legend::default())
                .show(ui, |plot| {
                    let pts: PlotPoints = self.cpu_hist.iter().copied().map(|p| [p[0], p[1]]).collect();
                    plot.line(Line::new(pts).name("%"));
                });
        }
        for n in &self.snap.cpu.notes {
            ui.colored_label(Color32::YELLOW, n);
        }
        ui.separator();
        ui.strong("KVM");
        for n in &self.snap.kvm.notes {
            ui.weak(n);
        }
        kv(ui, "/dev/kvm", &self.snap.kvm.device.display());
        kv(ui, "module", &self.snap.kvm.module.display());
        kv(ui, "vendor", &self.snap.kvm.vendor.display());
        kv(ui, "nested", &self.snap.kvm.nested.display());
        if self.snap.kvm.ept.access == AccessKind::Ok {
            kv(ui, "EPT", &self.snap.kvm.ept.display());
        }
        if self.snap.kvm.npt.access == AccessKind::Ok {
            kv(ui, "NPT", &self.snap.kvm.npt.display());
        }
        kv(ui, "nx_huge_pages", &self.snap.kvm.nx_huge_pages.display());
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("cpu_grid").striped(true).show(ui, |ui| {
                ui.strong("#");
                ui.strong("core");
                ui.strong("MHz");
                ui.strong("%");
                ui.strong("governor");
                ui.strong("smt");
                ui.end_row();
                for l in &self.snap.cpu.logical {
                    ui.label(l.processor.to_string());
                    ui.label(
                        l.core_id
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "-".into()),
                    );
                    let mhz = l
                        .scaling_cur_khz
                        .value
                        .map(|k| format!("{:.0}", k as f64 / 1000.0))
                        .or_else(|| l.mhz_from_cpuinfo.map(|m| format!("{m:.0}")))
                        .unwrap_or_else(|| l.scaling_cur_khz.access_label());
                    ui.label(mhz);
                    ui.label(
                        l.utilization_pct
                            .map(|u| format!("{u:.0}"))
                            .unwrap_or_else(|| "—".into()),
                    );
                    ui.label(l.governor.display());
                    ui.label(l.thread_siblings.display());
                    ui.end_row();
                }
            });
            ui.separator();
            ui.strong(self.t("缓存 (cpu0)", "Caches (cpu0)"));
            for c in &self.snap.cpu.caches {
                ui.label(format!(
                    "L{} {} {}  line {}  assoc {}  shared {}",
                    c.level.display(),
                    c.kind.display(),
                    c.size.display(),
                    c.line_size.display(),
                    c.associativity.display(),
                    c.shared_cpu_list.display()
                ));
            }
            ui.collapsing("flags", |ui| {
                ui.label(self.snap.cpu.flags.join(" "));
            });
            if !self.snap.cpu.vulnerabilities.is_empty() {
                ui.collapsing(self.t("CPU 漏洞缓解", "CPU vulnerabilities"), |ui| {
                    for v in &self.snap.cpu.vulnerabilities {
                        kv(ui, &v.name, &v.status.display());
                    }
                });
            }
            if !self.snap.cpu.idle_states.is_empty() {
                ui.collapsing("cpuidle", |ui| {
                    for s in &self.snap.cpu.idle_states {
                        kv(
                            ui,
                            &s.name.display(),
                            &format!(
                                "{}  lat {}  res {}",
                                s.desc.display(),
                                s.latency_us.display(),
                                s.residency_us.display()
                            ),
                        );
                    }
                });
            }
            if !self.snap.cpu.freq_policies.is_empty() {
                ui.collapsing(
                    format!("cpufreq ({})", self.snap.cpu.freq_policies.len()),
                    |ui| {
                        for p in &self.snap.cpu.freq_policies {
                            kv(
                                ui,
                                &p.name,
                                &format!(
                                    "{}  {}  {}-{} kHz  cpus {}",
                                    p.driver.display(),
                                    p.governor.display(),
                                    p.scaling_min_khz.display(),
                                    p.scaling_max_khz.display(),
                                    p.affected_cpus.display()
                                ),
                            );
                        }
                    },
                );
            }
            if self.snap.cpu.schedstat_cpus > 0 {
                kv(ui, "schedstat cpus", &self.snap.cpu.schedstat_cpus.to_string());
            }
        });
    }

    fn ui_memory(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("内存", "Memory"));
        for n in &self.snap.memory.notes {
            ui.weak(n);
        }
        let m = &self.snap.memory;
        kv(
            ui,
            self.t("物理", "Physical"),
            &kb_pair(&m.total_kb, &m.available_kb),
        );
        kv(ui, "MemFree", &kb_disp(&m.free_kb));
        kv(
            ui,
            "Buffers / Cached",
            &format!("{} / {}", kb_disp(&m.buffers_kb), kb_disp(&m.cached_kb)),
        );
        kv(
            ui,
            "Anon / Shmem",
            &format!("{} / {}", kb_disp(&m.anon_kb), kb_disp(&m.shmem_kb)),
        );
        kv(
            ui,
            "Dirty / Mapped",
            &format!("{} / {}", kb_disp(&m.dirty_kb), kb_disp(&m.mapped_kb)),
        );
        kv(
            ui,
            "Swap",
            &format!("{} / {}", kb_disp(&m.swap_total_kb), kb_disp(&m.swap_free_kb)),
        );
        kv(
            ui,
            "Committed",
            &format!(
                "{} / limit {}",
                kb_disp(&m.committed_as_kb),
                kb_disp(&m.commit_limit_kb)
            ),
        );
        kv(ui, "THP", &m.thp_enabled.display());
        kv(
            ui,
            "THP defrag/shmem",
            &format!("{}  {}", m.thp_defrag.display(), m.thp_shmem.display()),
        );
        kv(
            ui,
            "DirectMap",
            &format!(
                "4k {}  2M {}  1G {}",
                kb_disp(&m.directmap_4k_kb),
                kb_disp(&m.directmap_2m_kb),
                kb_disp(&m.directmap_1g_kb)
            ),
        );
        kv(ui, "VmallocUsed", &kb_disp(&m.vmalloc_used_kb));
        kv(
            ui,
            "zswap",
            &format!(
                "enabled {}  {} / {}  pool {}%",
                self.snap.zmem.zswap.enabled.display(),
                self.snap.zmem.zswap.compressor.display(),
                self.snap.zmem.zswap.zpool.display(),
                self.snap.zmem.zswap.max_pool_percent.display()
            ),
        );
        kv(
            ui,
            "KSM",
            &format!(
                "run {}  shared {}  sharing {}  scans {}",
                m.ksm.run.display(),
                m.ksm.pages_shared.display(),
                m.ksm.pages_sharing.display(),
                m.ksm.full_scans.display()
            ),
        );
        if !m.memory_tiers.is_empty() {
            kv(ui, "memory tiers", &m.memory_tiers.join(" "));
        }
        if m.mem_blocks.total > 0 {
            kv(
                ui,
                self.t("热插拔块", "memory blocks"),
                &format!(
                    "{} online / {}  block {}",
                    m.mem_blocks.online,
                    m.mem_blocks.total,
                    m.mem_blocks
                        .block_size_bytes
                        .value
                        .map(crate::export::format_bytes)
                        .unwrap_or_else(|| m.mem_blocks.block_size_bytes.access_label())
                ),
            );
        }
        kv(
            ui,
            "vmstat",
            &format!(
                "fault {}  maj {}  in {}  out {}  oom {}",
                m.vmstat.pgfault.display(),
                m.vmstat.pgmajfault.display(),
                m.vmstat.pgpgin.display(),
                m.vmstat.pgpgout.display(),
                m.vmstat.oom_kill.display()
            ),
        );
        if !m.buddy.is_empty() {
            ui.separator();
            ui.strong("buddyinfo");
            for z in &m.buddy {
                kv(
                    ui,
                    &format!("N{} {}", z.node, z.zone),
                    &z.free_counts
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
        if !m.zones.is_empty() {
            ui.separator();
            ui.strong("zoneinfo");
            for z in &m.zones {
                kv(
                    ui,
                    &format!("N{} {}", z.node, z.zone),
                    &format!(
                        "free {}  present {}  managed {}",
                        z.free.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                        z.present
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "—".into()),
                        z.managed
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "—".into())
                    ),
                );
            }
        }
        if !m.hugepages.is_empty() {
            ui.separator();
            ui.strong("hugepages");
            for p in &m.hugepages {
                kv(
                    ui,
                    &format!("{} KiB", p.size_kb),
                    &format!(
                        "nr {}  free {}  surplus {}",
                        p.nr.display(),
                        p.free.display(),
                        p.surplus.display()
                    ),
                );
            }
        }
        for n in &self.snap.zmem.notes {
            ui.weak(n);
        }
        if !self.snap.zmem.zram.is_empty() {
            ui.separator();
            ui.strong("zram");
            for z in &self.snap.zmem.zram {
                kv(
                    ui,
                    &z.name,
                    &format!(
                        "disk {}  {}  orig {}  compr {}  mem {}",
                        z.disksize
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| z.disksize.access_label()),
                        z.algorithm.display(),
                        z.orig_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| z.orig_bytes.access_label()),
                        z.compr_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| z.compr_bytes.access_label()),
                        z.mem_used
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| z.mem_used.access_label())
                    ),
                );
            }
        }
        for n in &self.snap.edac.notes {
            ui.weak(n);
        }
        if !self.snap.edac.controllers.is_empty() {
            ui.separator();
            ui.strong("EDAC");
            for c in &self.snap.edac.controllers {
                kv(
                    ui,
                    &c.name,
                    &format!(
                        "{}  {} MiB  CE {}  UE {}",
                        c.mc_name.display(),
                        c.size_mb.display(),
                        c.ce_count.display(),
                        c.ue_count.display()
                    ),
                );
            }
        }
    }

    fn ui_power(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("电源 / 电池", "Power"));
        for n in &self.snap.power.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        for s in &self.snap.power.supplies {
            ui.separator();
            ui.strong(&s.name);
            kv(ui, self.t("类型", "type"), &s.kind.display());
            kv(ui, self.t("状态", "status"), &s.status.display());
            if s.capacity_pct.access == AccessKind::Ok {
                kv(
                    ui,
                    self.t("电量", "capacity"),
                    &s.capacity_pct
                        .value
                        .map(|v| format!("{v}%"))
                        .unwrap_or_else(|| s.capacity_pct.access_label()),
                );
            }
            if s.online.access == AccessKind::Ok {
                kv(ui, "online", &s.online.display());
            }
            if s.voltage_v.access == AccessKind::Ok {
                kv(
                    ui,
                    "V",
                    &s.voltage_v
                        .value
                        .map(|v| format!("{v:.2}"))
                        .unwrap_or_else(|| s.voltage_v.access_label()),
                );
            }
            if s.energy_now_wh.access == AccessKind::Ok {
                kv(
                    ui,
                    "Wh",
                    &format!(
                        "{} / {}",
                        s.energy_now_wh.display(),
                        s.energy_full_wh.display()
                    ),
                );
            }
            field_row(ui, "serial", &s.serial);
            kv(ui, "model", &s.model.display());
            kv(ui, "cycles", &s.cycle_count.display());
        }
        ui.separator();
        ui.strong("Sleep / PM");
        for n in &self.snap.pm.notes {
            ui.weak(n);
        }
        kv(ui, "state", &self.snap.pm.state.display());
        kv(ui, "mem_sleep", &self.snap.pm.mem_sleep.display());
        kv(ui, "disk", &self.snap.pm.disk.display());
        kv(
            ui,
            "suspend",
            &format!(
                "ok {}  fail {}  wakeup_count {}  sources {}",
                self.snap.pm.suspend_success.display(),
                self.snap.pm.suspend_fail.display(),
                self.snap.pm.wakeup_count.display(),
                self.snap.pm.wakeups
            ),
        );
        ui.separator();
        ui.strong("RAPL / powercap");
        for n in &self.snap.rapl.notes {
            ui.weak(n);
        }
        if !self.rapl_hist.is_empty() {
            plot_lines(ui, "rapl_plot", &self.rapl_hist, 120.0);
        }
        for z in &self.snap.rapl.zones {
            kv(
                ui,
                z.label.value.as_deref().unwrap_or(&z.name),
                &format!(
                    "{}  limit {}  max {}",
                    z.power_w
                        .map(|w| format!("{w:.2} W"))
                        .unwrap_or_else(|| "n/a".into()),
                    z.power_limit_uw
                        .value
                        .map(|u| format!("{:.1} W", u as f64 / 1_000_000.0))
                        .unwrap_or_else(|| z.power_limit_uw.access_label()),
                    z.max_power_uw
                        .value
                        .map(|u| format!("{:.1} W", u as f64 / 1_000_000.0))
                        .unwrap_or_else(|| z.max_power_uw.access_label())
                ),
            );
        }
    }

    fn ui_audio(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("声卡", "Audio"));
        for n in &self.snap.audio.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        for c in &self.snap.audio.cards {
            ui.separator();
            ui.strong(format!("#{} {}", c.index, c.id));
            kv(ui, self.t("名称", "name"), &c.name);
            if let Some(e) = &c.extra {
                ui.weak(e);
            }
            kv(ui, "sys id", &c.sys_id.display());
        }
    }

    fn ui_dmi(&self, ui: &mut egui::Ui) {
        ui.heading("DMI / SMBIOS");
        kv(
            ui,
            "BIOS",
            &format!(
                "{} {}",
                self.snap.dmi.bios_vendor.display(),
                self.snap.dmi.bios_version.display()
            ),
        );
        kv(
            ui,
            self.t("厂商", "Vendor"),
            &self.snap.dmi.sys_vendor.display(),
        );
        kv(
            ui,
            self.t("产品", "Product"),
            &self.snap.dmi.product_name.display(),
        );
        field_row(
            ui,
            self.t("序列号", "Serial"),
            &self.snap.dmi.product_serial,
        );
        field_row(ui, "UUID", &self.snap.dmi.product_uuid);
        kv(
            ui,
            self.t("主板", "Board"),
            &format!(
                "{} {}",
                self.snap.dmi.board_vendor.display(),
                self.snap.dmi.board_name.display()
            ),
        );
        for n in &self.snap.dmi.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        if !self.snap.dmi.memory_devices.is_empty() {
            ui.separator();
            ui.strong(self.t("内存插槽 (SMBIOS Type 17)", "Memory devices"));
            for m in &self.snap.dmi.memory_devices {
                ui.label(format!(
                    "{} {} {:?} MB {:?} {:?}",
                    m.locator.as_deref().unwrap_or("?"),
                    m.r#type.as_deref().unwrap_or("?"),
                    m.size_mb,
                    m.speed_mts,
                    m.part
                ));
            }
        }
        if !self.snap.dmi.smbios_records.is_empty() {
            ui.collapsing("SMBIOS records", |ui| {
                for r in &self.snap.dmi.smbios_records {
                    ui.label(format!("[{}] {} {:?}", r.kind_name, r.handle, r.strings));
                }
            });
        }
    }

    fn ui_gpu(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("显示适配器", "GPU"));
        for n in &self.snap.gpu.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        if self.snap.gpu.nvidia_kernel.access == AccessKind::Ok {
            kv(ui, "NVIDIA kernel", &self.snap.gpu.nvidia_kernel.display());
        }
        for g in &self.snap.gpu.devices {
            ui.separator();
            ui.strong(format!("{}  [{}]", g.id, g.driver));
            kv(ui, "PCI", &g.pci_slot.display());
            kv(
                ui,
                "ID",
                &format!("{}:{}", g.vendor_id.display(), g.device_id.display()),
            );
            kv(
                ui,
                self.t("占用", "Busy"),
                &g.busy_percent
                    .value
                    .map(|v| format!("{v}%"))
                    .unwrap_or_else(|| g.busy_percent.access_label()),
            );
            kv(
                ui,
                self.t("显存", "VRAM"),
                &match (g.vram_used_bytes.value, g.vram_total_bytes.value) {
                    (Some(u), Some(t)) => format!(
                        "{} / {}",
                        crate::export::format_bytes(u),
                        crate::export::format_bytes(t)
                    ),
                    _ => g.vram_total_bytes.access_label(),
                },
            );
            kv(ui, "VBIOS", &g.vbios.display());
            for c in &g.clocks {
                kv(
                    ui,
                    &format!("{} MHz", c.name),
                    &c.current_mhz
                        .value
                        .map(|v| {
                            format!(
                                "{v} (min {} max {})",
                                c.min_mhz.display(),
                                c.max_mhz.display()
                            )
                        })
                        .unwrap_or_else(|| c.current_mhz.access_label()),
                );
            }
            for conn in &g.connectors {
                let edid = conn
                    .edid
                    .as_ref()
                    .map(|e| {
                        format!(
                            "{} {} {}x{}",
                            e.manufacturer,
                            e.name.as_deref().unwrap_or(""),
                            e.h_active.unwrap_or(0),
                            e.v_active.unwrap_or(0)
                        )
                    })
                    .unwrap_or_default();
                kv(
                    ui,
                    &conn.name,
                    &format!(
                        "{} / {}  {}",
                        conn.status.display(),
                        conn.enabled.display(),
                        edid
                    ),
                );
            }
            for (k, v) in &g.extra {
                kv(ui, k, &v.display());
            }
        }
    }

    fn ui_sensors(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("传感器", "Sensors"));
        if !self.snap.alerts.is_empty() {
            ui.colored_label(
                Color32::from_rgb(255, 120, 80),
                self.t("当前越限", "Threshold alerts"),
            );
            for a in &self.snap.alerts {
                ui.colored_label(alert_color(a.level), &a.message);
            }
            ui.separator();
        }
        ui.weak(format!(
            "{}: {}",
            self.t("告警日志", "Alert log"),
            self.alert_log_path.display()
        ));
        if let Some(e) = &self.alert_log_err {
            ui.colored_label(Color32::from_rgb(255, 100, 100), e);
        }
        for n in &self.snap.sensors.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        if !self.temps.is_empty() {
            Plot::new("temp_plot")
                .height(240.0)
                .legend(egui_plot::Legend::default())
                .show(ui, |plot| {
                    for (name, q) in &self.temps {
                        let pts: PlotPoints = q.iter().copied().map(|p| [p[0], p[1]]).collect();
                        plot.line(Line::new(pts).name(name));
                    }
                });
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for chip in &self.snap.sensors.chips {
                ui.strong(chip.name.display());
                for ch in &chip.channels {
                    let val = ch
                        .value
                        .map(|v| format!("{v:.3} {}", ch.unit))
                        .unwrap_or_else(|| ch.raw.access_label());
                    let limits = [
                        ch.min.map(|v| format!("min {v:.1}")),
                        ch.max.map(|v| format!("max {v:.1}")),
                        ch.crit.map(|v| format!("crit {v:.1}")),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("  ");
                    let shown = if limits.is_empty() {
                        val
                    } else {
                        format!("{val}  ({limits})")
                    };
                    kv(ui, &ch.label, &shown);
                }
            }
            for tz in &self.snap.sensors.thermal_zones {
                kv(
                    ui,
                    &tz.r#type.display(),
                    &tz.temp_c
                        .value
                        .map(|v| format!("{v:.1} °C"))
                        .unwrap_or_else(|| tz.temp_c.access_label()),
                );
            }
            if !self.snap.sensors.cooling.is_empty() {
                ui.separator();
                ui.strong(self.t("冷却设备", "Cooling"));
                for c in &self.snap.sensors.cooling {
                    kv(
                        ui,
                        &format!("{} ({})", c.name, c.r#type.display()),
                        &format!("{}/{}", c.cur_state.display(), c.max_state.display()),
                    );
                }
            }
        });
    }

    fn ui_fs(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("文件系统", "Filesystems"));
        for n in &self.snap.fs.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        kv(
            ui,
            "NFS / FUSE",
            &format!(
                "nfsd {}  volumes {}  fuse {}",
                self.snap.fs.nfsd_threads.display(),
                self.snap.fs.nfs_volumes,
                self.snap.fs.fuse_conns
            ),
        );
        if !self.snap.fs.swaps.is_empty() {
            ui.strong("swap");
            for s in &self.snap.fs.swaps {
                kv(
                    ui,
                    &s.filename,
                    &format!(
                        "{}  {} / {}",
                        s.kind,
                        crate::export::format_bytes(s.used_kb * 1024),
                        crate::export::format_bytes(s.size_kb * 1024)
                    ),
                );
            }
            ui.separator();
        }
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("fs").striped(true).show(ui, |ui| {
                ui.strong(self.t("挂载点", "target"));
                ui.strong("fstype");
                ui.strong(self.t("源", "source"));
                ui.strong("kind");
                ui.strong(self.t("用量", "usage"));
                ui.end_row();
                for m in self.snap.fs.mounts.iter().filter(|m| m.kind != "virtual") {
                    ui.label(&m.target);
                    ui.label(&m.fstype);
                    ui.label(&m.source);
                    ui.label(m.kind);
                    ui.label(match (m.used_bytes, m.total_bytes) {
                        (Some(u), Some(t)) => format!(
                            "{} / {}",
                            crate::export::format_bytes(u),
                            crate::export::format_bytes(t)
                        ),
                        _ => "—".into(),
                    });
                    ui.end_row();
                }
            });
            let virt: Vec<_> = self
                .snap
                .fs
                .mounts
                .iter()
                .filter(|m| m.kind == "virtual")
                .collect();
            if !virt.is_empty() {
                ui.collapsing(format!("virtual ({})", virt.len()), |ui| {
                    for m in virt {
                        ui.weak(format!("{}  {}  {}", m.target, m.fstype, m.source));
                    }
                });
            }
        });
        if !self.snap.fs.ext4.is_empty() {
            ui.separator();
            ui.strong("ext4 sysfs");
            for e in &self.snap.fs.ext4 {
                kv(
                    ui,
                    &e.name,
                    &format!(
                        "lifetime {}  session {}  errors {}",
                        e.lifetime_write_kbytes
                            .value
                            .map(|v| crate::export::format_bytes(v * 1024))
                            .unwrap_or_else(|| e.lifetime_write_kbytes.access_label()),
                        e.session_write_kbytes
                            .value
                            .map(|v| crate::export::format_bytes(v * 1024))
                            .unwrap_or_else(|| e.session_write_kbytes.access_label()),
                        e.errors_count.display()
                    ),
                );
            }
        }
        if self.snap.fs.xfs_stats.access == AccessKind::Ok {
            kv(ui, "xfs rw", &self.snap.fs.xfs_stats.display());
        }
    }

    fn ui_storage(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("存储", "Storage"));
        if !self.disk_hist.is_empty() {
            plot_lines(ui, "disk_io_plot", &self.disk_hist, 140.0);
        }
        ui.strong("Block");
        egui::Grid::new("blk").striped(true).show(ui, |ui| {
            ui.strong("name");
            ui.strong("type");
            ui.strong("size");
            ui.strong("model");
            ui.strong("rd");
            ui.strong("wr");
            ui.end_row();
            for b in &self.snap.block.devices {
                ui.label(&b.name);
                ui.label(&b.r#type);
                ui.label(
                    b.size_bytes
                        .value
                        .map(crate::export::format_bytes)
                        .unwrap_or_else(|| b.size_bytes.access_label()),
                );
                ui.label(b.model.display());
                ui.label(
                    b.rd_bps
                        .map(crate::export::format_bps)
                        .unwrap_or_else(|| {
                            b.rd_bytes
                                .value
                                .map(crate::export::format_bytes)
                                .unwrap_or_else(|| b.rd_bytes.access_label())
                        }),
                );
                ui.label(
                    b.wr_bps
                        .map(crate::export::format_bps)
                        .unwrap_or_else(|| {
                            b.wr_bytes
                                .value
                                .map(crate::export::format_bytes)
                                .unwrap_or_else(|| b.wr_bytes.access_label())
                        }),
                );
                ui.end_row();
            }
        });
        for b in &self.snap.block.devices {
            ui.weak(format!(
                "{}  phys/log {}/{}  nr {}  dax {}  cache {}  discard {}",
                b.name,
                b.physical_block_size.display(),
                b.logical_block_size.display(),
                b.nr_requests.display(),
                b.dax.display(),
                b.write_cache.display(),
                b.discard_max_bytes
                    .value
                    .map(crate::export::format_bytes)
                    .unwrap_or_else(|| b.discard_max_bytes.access_label())
            ));
        }
        for b in &self.snap.block.devices {
            if !b.partitions.is_empty() {
                ui.weak(format!(
                    "{}: {}",
                    b.name,
                    b.partitions
                        .iter()
                        .map(|p| format!(
                            "{} {}",
                            p.name,
                            p.size_bytes
                                .value
                                .map(crate::export::format_bytes)
                                .unwrap_or_else(|| p.size_bytes.access_label())
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        for n in &self.snap.block.notes {
            ui.weak(n);
        }
        if !self.snap.block.loops.is_empty() {
            ui.separator();
            ui.strong("loop");
            for l in &self.snap.block.loops {
                kv(
                    ui,
                    &l.name,
                    &format!(
                        "{}  {}",
                        l.size_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| l.size_bytes.access_label()),
                        l.backing_file.display()
                    ),
                );
            }
        }
        if !self.snap.block.mapper.is_empty() {
            ui.separator();
            ui.strong("device-mapper");
            for d in &self.snap.block.mapper {
                kv(
                    ui,
                    &d.name,
                    &format!(
                        "{}  {}  susp {}",
                        d.mapper_name.display(),
                        d.uuid.display(),
                        d.suspended.display()
                    ),
                );
            }
        }
        if !self.snap.block.bdi.is_empty() || !self.snap.block.bsg.is_empty() {
            ui.separator();
            ui.strong("BDI / BSG");
            if !self.snap.block.bsg.is_empty() {
                kv(ui, "bsg", &self.snap.block.bsg.join(" "));
            }
            for b in self.snap.block.bdi.iter().take(8) {
                kv(
                    ui,
                    &format!("bdi {}", b.name),
                    &format!(
                        "ra {}  max_ratio {}",
                        b.read_ahead_kb.display(),
                        b.max_ratio.display()
                    ),
                );
            }
        }
        ui.separator();
        ui.strong("ATA / SATA");
        for n in &self.snap.ata.notes {
            ui.weak(n);
        }
        for p in &self.snap.ata.ports {
            for l in &p.links {
                kv(
                    ui,
                    &format!("{} {}", p.name, l.name),
                    &format!(
                        "spd {}  limit {}",
                        l.sata_spd.display(),
                        l.sata_spd_limit.display()
                    ),
                );
                for d in &l.devices {
                    kv(
                        ui,
                        &d.name,
                        &format!(
                            "{}  {}  trim {}",
                            d.model.display(),
                            d.class.display(),
                            d.trim.display()
                        ),
                    );
                }
            }
        }
        ui.separator();
        ui.strong("MD RAID");
        for n in &self.snap.md.notes {
            ui.weak(n);
        }
        if !self.snap.md.personalities.is_empty() {
            kv(ui, "personalities", &self.snap.md.personalities);
        }
        for a in &self.snap.md.arrays {
            kv(
                ui,
                &a.name,
                &format!(
                    "{} {}  {}  deg {}  {}",
                    a.state,
                    a.level,
                    a.members,
                    a.degraded.display(),
                    a.detail
                ),
            );
        }
        ui.separator();
        ui.strong("SCSI host");
        for n in &self.snap.scsi.notes {
            ui.weak(n);
        }
        for h in &self.snap.scsi.hosts {
            kv(
                ui,
                &h.name,
                &format!(
                    "{}  queue {}  {}",
                    h.proc_name.display(),
                    h.can_queue.display(),
                    h.state.display()
                ),
            );
        }
        for d in &self.snap.scsi.devices {
            kv(
                ui,
                &format!("LUN {}", d.name),
                &format!(
                    "{} {}  type {}  {}",
                    d.vendor.display(),
                    d.model.display(),
                    d.type_code.display(),
                    d.state.display()
                ),
            );
        }
        ui.separator();
        ui.strong("iSCSI");
        for n in &self.snap.iscsi.notes {
            ui.weak(n);
        }
        for t in &self.snap.iscsi.transports {
            kv(
                ui,
                &format!("transport {}", t.name),
                &format!("handle {}  caps {}", t.handle.display(), t.caps.display()),
            );
        }
        for h in &self.snap.iscsi.hosts {
            kv(
                ui,
                &h.name,
                &format!(
                    "{}  {}  {}",
                    h.netdev.display(),
                    h.ipaddress.display(),
                    h.port_state.display()
                ),
            );
        }
        for s in &self.snap.iscsi.sessions {
            kv(
                ui,
                &s.name,
                &format!("{}  {}", s.state.display(), s.targetname.display()),
            );
        }
        ui.separator();
        ui.strong("NVMe");
        if self.snap.nvme.controllers.is_empty() {
            for n in &self.snap.nvme.notes {
                ui.colored_label(Color32::from_rgb(255, 179, 71), n);
            }
        }
        for c in &self.snap.nvme.controllers {
            kv(ui, "model", &c.model.display());
            field_row(ui, "serial", &c.serial);
            kv(ui, "firmware", &c.firmware.display());
            match &c.smart {
                s if s.access == AccessKind::Ok => {
                    if let Some(sm) = &s.value {
                        kv(
                            ui,
                            "SMART",
                            &format!(
                                "used {}% spare {}% temp {:?}°C hours {}",
                                sm.percentage_used,
                                sm.available_spare_pct,
                                sm.temperature_c.map(|t| format!("{t:.1}")),
                                sm.power_on_hours
                            ),
                        );
                    }
                }
                s => {
                    ui.colored_label(Color32::from_rgb(255, 179, 71), s.access_label());
                }
            }
        }
    }

    fn ui_net(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("网络", "Network"));
        ui.weak(self.t(
            "来自 /sys/class/net 与 getifaddrs，不调用 ip/ifconfig。",
            "From /sys/class/net and getifaddrs; no ip/ifconfig.",
        ));
        if !self.net_hist.is_empty() {
            plot_lines(ui, "net_rate_plot", &self.net_hist, 140.0);
        }
        for n in &self.snap.net.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("net").striped(true).show(ui, |ui| {
                ui.strong(self.t("接口", "iface"));
                ui.strong(self.t("状态", "state"));
                ui.strong(self.t("类型", "type"));
                ui.strong("Mb/s");
                ui.strong("RX");
                ui.strong("TX");
                ui.strong(self.t("地址", "addr"));
                ui.end_row();
                for i in &self.snap.net.interfaces {
                    ui.label(&i.name);
                    ui.label(i.operstate.display());
                    ui.label(if i.wireless {
                        format!("{} / Wi-Fi", i.kind)
                    } else {
                        i.kind.clone()
                    });
                    ui.label(
                        i.speed_mbps
                            .value
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| i.speed_mbps.access_label()),
                    );
                    let rx = i.rx_bps.map(crate::export::format_bps).unwrap_or_else(|| {
                        i.rx_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| i.rx_bytes.access_label())
                    });
                    let tx = i.tx_bps.map(crate::export::format_bps).unwrap_or_else(|| {
                        i.tx_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| i.tx_bytes.access_label())
                    });
                    ui.label(rx);
                    ui.label(tx);
                    ui.label(i.addresses.join(", "));
                    ui.end_row();
                }
            });
            for i in &self.snap.net.interfaces {
                ui.collapsing(&i.name, |ui| {
                    kv(ui, "MAC", &i.mac.display());
                    kv(ui, "MTU", &i.mtu.display());
                    kv(ui, "duplex", &i.duplex.display());
                    kv(ui, "carrier", &i.carrier.display());
                    kv(ui, "driver", &i.driver.display());
                    kv(ui, "wireless", if i.wireless { "yes" } else { "no" });
                    kv(
                        ui,
                        self.t("队列", "queues"),
                        &format!("rx {}  tx {}", i.rx_queues, i.tx_queues),
                    );
                    kv(
                        ui,
                        self.t("累计 RX", "RX bytes"),
                        &i.rx_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| i.rx_bytes.access_label()),
                    );
                    kv(
                        ui,
                        self.t("累计 TX", "TX bytes"),
                        &i.tx_bytes
                            .value
                            .map(crate::export::format_bytes)
                            .unwrap_or_else(|| i.tx_bytes.access_label()),
                    );
                    kv(ui, "RX err", &i.rx_errors.display());
                    kv(ui, "TX err", &i.tx_errors.display());
                });
            }
        });
        let ss = &self.snap.net.sockstat;
        kv(
            ui,
            "sockstat",
            &format!(
                "sockets {}  TCP {} (TIME_WAIT {})  UDP {}",
                ss.sockets_used
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                ss.tcp_inuse
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                ss.tcp_tw
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                    ss.udp_inuse
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into())
            ),
        );
        let sn = &self.snap.net.snmp;
        kv(
            ui,
            "TCP/IP",
            &format!(
                "estab {}  in {}  out {}  retrans {}  udp {}/{}",
                sn.tcp_curr_estab
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                sn.tcp_in_segs
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                sn.tcp_out_segs
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                sn.tcp_retrans
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                sn.udp_in.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                sn.udp_out.map(|v| v.to_string()).unwrap_or_else(|| "—".into())
            ),
        );
        kv(
            ui,
            "softnet",
            &format!(
                "processed {}  dropped {}  squeeze {}  cpus {}",
                self.snap.net.softnet.processed,
                self.snap.net.softnet.dropped,
                self.snap.net.softnet.time_squeeze,
                self.snap.net.softnet.cpus
            ),
        );
        kv(
            ui,
            "conntrack",
            &format!(
                "{} / {}  cong {} ({})  est {}s  buckets {}  tw {}",
                self.snap.net.conntrack_count.display(),
                self.snap.net.conntrack_max.display(),
                self.snap.net.tcp_congestion.display(),
                self.snap.net.tcp_available_congestion.display(),
                self.snap.net.conntrack_tcp_established.display(),
                self.snap.net.conntrack_buckets.display(),
                self.snap.net.tcp_max_tw_buckets.display()
            ),
        );
        let te = &self.snap.net.tcpext;
        kv(
            ui,
            "TcpExt",
            &format!(
                "TW {}  listen {}/{}  timeout {}  octets {}/{}",
                te.timewait.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                te.listen_overflows
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                te.listen_drops
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                te.timeouts.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                te.in_octets
                    .map(crate::export::format_bytes)
                    .unwrap_or_else(|| "—".into()),
                te.out_octets
                    .map(crate::export::format_bytes)
                    .unwrap_or_else(|| "—".into())
            ),
        );
        let s6 = &self.snap.net.snmp6;
        kv(
            ui,
            "IPv6",
            &format!(
                "in {}  out {}  octets {}/{}  TCP6 {} UDP6 {}",
                s6.in_receives.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                s6.out_requests.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                s6.in_octets
                    .map(crate::export::format_bytes)
                    .unwrap_or_else(|| "—".into()),
                s6.out_octets
                    .map(crate::export::format_bytes)
                    .unwrap_or_else(|| "—".into()),
                self.snap
                    .net
                    .sockstat6
                    .tcp_inuse
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                self.snap
                    .net
                    .sockstat6
                    .udp_inuse
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into())
            ),
        );
        kv(
            ui,
            "net.core",
            &format!(
                "somaxconn {}  backlog {}  budget {}/{}us  r/wmem {}/{}  busy_poll {}  busy_read {}  weight {}",
                self.snap.net.somaxconn.display(),
                self.snap.net.netdev_max_backlog.display(),
                self.snap.net.netdev_budget.display(),
                self.snap.net.netdev_budget_usecs.display(),
                self.snap.net.rmem_max.display(),
                self.snap.net.wmem_max.display(),
                self.snap.net.busy_poll.display(),
                self.snap.net.busy_read.display(),
                self.snap.net.dev_weight.display()
            ),
        );
        kv(
            ui,
            "tables",
            &format!(
                "arp {}  route {}  unix {}  inet6 {}  ipv6_route {}  packet {}  netlink {}  tcp {}  udp {}  tcp6 {}  udp6 {}  raw {}  udplite {}  raw6 {}  udplite6 {}",
                self.snap.net.arp_entries,
                self.snap.net.route_entries,
                self.snap.net.unix_sockets,
                self.snap.net.inet6_addrs,
                self.snap.net.ipv6_routes,
                self.snap.net.packet_sockets,
                self.snap.net.netlink_sockets,
                self.snap.net.tcp_socks,
                self.snap.net.udp_socks,
                self.snap.net.tcp6_socks,
                self.snap.net.udp6_socks,
                self.snap.net.raw_socks,
                self.snap.net.udplite_socks,
                self.snap.net.raw6_socks,
                self.snap.net.udplite6_socks
            ),
        );
        kv(
            ui,
            "tcp knobs",
            &format!(
                "fastopen {}  syncookies {}  ports {}  ka {}/{}x{}  fin {}  syn {}  sack {} ts {} wscale {} ecn {} tw {} syn/synack {}/{} retries2 {} slow_start {}",
                self.snap.net.tcp_fastopen.display(),
                self.snap.net.tcp_syncookies.display(),
                self.snap.net.ip_local_port_range.display(),
                self.snap.net.tcp.keepalive_time.display(),
                self.snap.net.tcp.keepalive_probes.display(),
                self.snap.net.tcp.keepalive_intvl.display(),
                self.snap.net.tcp.fin_timeout.display(),
                self.snap.net.tcp.max_syn_backlog.display(),
                self.snap.net.tcp.sack.display(),
                self.snap.net.tcp.timestamps.display(),
                self.snap.net.tcp.window_scaling.display(),
                self.snap.net.tcp.ecn.display(),
                self.snap.net.tcp.tw_reuse.display(),
                self.snap.net.tcp.syn_retries.display(),
                self.snap.net.tcp.synack_retries.display(),
                self.snap.net.tcp.retries2.display(),
                self.snap.net.tcp.slow_start_after_idle.display()
            ),
        );
        kv(
            ui,
            "tcp mem",
            &format!(
                "rmem {}  wmem {}  mtu_probing {}  optmem {}  notsent {}  unix_dgram {}  adv_win {}  moderate_rcvbuf {}",
                self.snap.net.tcp.rmem.display(),
                self.snap.net.tcp.wmem.display(),
                self.snap.net.tcp.mtu_probing.display(),
                self.snap.net.optmem_max.display(),
                self.snap.net.tcp.notsent_lowat.display(),
                self.snap.net.unix_max_dgram_qlen.display(),
                self.snap.net.tcp.adv_win_scale.display(),
                self.snap.net.tcp.moderate_rcvbuf.display()
            ),
        );
        kv(
            ui,
            "qdisc / IPv6",
            &format!(
                "qdisc {}  disable_ipv6 {}  fwd {}  tempaddr {}  accept_ra {}  autoconf {}  hop {}  ttl {}  igmp {}  igmp6 {}  rt6 {}",
                self.snap.net.default_qdisc.display(),
                self.snap.net.ipv6_disable.display(),
                self.snap.net.ipv6_forwarding.display(),
                self.snap.net.ipv6_use_tempaddr.display(),
                self.snap.net.ipv6_accept_ra.display(),
                self.snap.net.ipv6_autoconf.display(),
                self.snap.net.ipv6_hop_limit.display(),
                self.snap.net.ip_default_ttl.display(),
                self.snap.net.igmp_ifaces,
                self.snap.net.igmp6_ifaces,
                self.snap.net.rt6_entries.display()
            ),
        );
        kv(
            ui,
            "netsec",
            &format!(
                "rp_filter {}  icmp_echo_ignore_broadcasts {}  icmp_ratelimit {}  redirects {}  srcrt {}  martians {}  bogus {}",
                self.snap.net.rp_filter.display(),
                self.snap.net.icmp_echo_ignore_broadcasts.display(),
                self.snap.net.icmp_ratelimit.display(),
                self.snap.net.accept_redirects.display(),
                self.snap.net.accept_source_route.display(),
                self.snap.net.log_martians.display(),
                self.snap.net.icmp_ignore_bogus.display()
            ),
        );
        if !self.snap.net.rp_filter_dev.is_empty() {
            kv(ui, "rp_filter iface", &self.snap.net.rp_filter_dev.join("  "));
        }
        if !self.snap.net.ipv6_use_tempaddr_dev.is_empty() {
            kv(
                ui,
                "use_tempaddr iface",
                &self.snap.net.ipv6_use_tempaddr_dev.join("  "),
            );
        }
        kv(
            ui,
            "xfrm",
            &format!(
                "in_no_states {}  out_no_states {}",
                self.snap
                    .net
                    .xfrm_in_no_states
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                self.snap
                    .net
                    .xfrm_out_no_states
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into())
            ),
        );
        if !self.snap.net.ptypes.is_empty() {
            kv(ui, "ptype", &self.snap.net.ptypes.join("  "));
        }
        if let Some(leaves) = self.snap.net.fib_trie_leaves {
            kv(ui, "fib leaves", &leaves.to_string());
        }
        if !self.snap.net.protocols.is_empty() {
            kv(ui, "protocols", &self.snap.net.protocols.join("  "));
        }
        kv_name_list(
            ui,
            "iptables",
            &self.snap.net.iptables,
            &self.snap.net.notes,
            "ip_tables_names",
        );
        kv_name_list(
            ui,
            "ip6tables",
            &self.snap.net.ip6tables,
            &self.snap.net.notes,
            "ip6_tables_names",
        );
        if !self.snap.net.connectors.is_empty() {
            kv(ui, "connector", &self.snap.net.connectors.join(" "));
        }
        for b in &self.snap.net.bridges {
            kv(
                ui,
                &format!("bridge {}", b.name),
                &format!(
                    "{}  stp {}  members {}",
                    b.bridge_id.display(),
                    b.stp_state.display(),
                    b.members.join(",")
                ),
            );
        }
        for b in &self.snap.net.bonds {
            kv(
                ui,
                &format!("bond {}", b.name),
                &format!(
                    "mode {}  slaves {}",
                    b.mode.display(),
                    b.slaves.display()
                ),
            );
        }
    }

    fn ui_usb(&self, ui: &mut egui::Ui) {
        ui.heading("USB");
        for n in &self.snap.usb.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("usb").striped(true).show(ui, |ui| {
                ui.strong(self.t("节点", "node"));
                ui.strong(self.t("父", "parent"));
                ui.strong("ID");
                ui.strong(self.t("产品", "product"));
                ui.strong(self.t("速度", "speed"));
                ui.strong("serial");
                ui.end_row();
                for d in &self.snap.usb.devices {
                    let indent = if d.sys_name.starts_with("usb") {
                        0
                    } else {
                        1 + d.sys_name.bytes().filter(|b| *b == b'.').count()
                    };
                    ui.label(format!("{}{}", "  ".repeat(indent), d.sys_name));
                    ui.label(d.parent.as_deref().unwrap_or("—"));
                    ui.label(format!(
                        "{}:{}",
                        d.vendor_id.display(),
                        d.product_id.display()
                    ));
                    let product = d
                        .product
                        .value
                        .clone()
                        .or_else(|| d.product_name.clone())
                        .or_else(|| d.vendor_name.clone())
                        .unwrap_or_else(|| d.product.access_label());
                    ui.label(product);
                    ui.label(d.speed.display());
                    field_row_cell(ui, &d.serial);
                    ui.end_row();
                }
            });
        });
    }

    fn ui_input(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("输入设备", "Input"));
        for n in &self.snap.input.notes {
            ui.colored_label(Color32::from_rgb(255, 179, 71), n);
        }
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("input").striped(true).show(ui, |ui| {
                ui.strong(self.t("名称", "name"));
                ui.strong(self.t("类型", "kind"));
                ui.strong("handlers");
                ui.strong("phys");
                ui.end_row();
                for d in &self.snap.input.devices {
                    ui.label(&d.name);
                    ui.label(d.kinds.join(", "));
                    ui.label(d.handlers.join(" "));
                    ui.label(d.phys.as_deref().unwrap_or("—"));
                    ui.end_row();
                }
            });
        });
    }

    fn ui_numa(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("NUMA 内存", "NUMA"));
        for n in &self.snap.numa.notes {
            ui.weak(n);
        }
        egui::Grid::new("numa").striped(true).show(ui, |ui| {
            ui.strong("node");
            ui.strong("cpulist");
            ui.strong(self.t("内存", "memory"));
            ui.strong(self.t("空闲", "free"));
            ui.strong("distance");
            ui.end_row();
            for n in &self.snap.numa.nodes {
                ui.label(n.id.to_string());
                ui.label(n.cpulist.display());
                ui.label(
                    n.mem_total_kb
                        .value
                        .map(|v| crate::export::format_bytes(v * 1024))
                        .unwrap_or_else(|| n.mem_total_kb.access_label()),
                );
                ui.label(
                    n.mem_free_kb
                        .value
                        .map(|v| crate::export::format_bytes(v * 1024))
                        .unwrap_or_else(|| n.mem_free_kb.access_label()),
                );
                ui.label(n.distance.display());
                ui.end_row();
            }
        });
    }

    fn ui_pci(&self, ui: &mut egui::Ui) {
        ui.heading("PCI");
        for n in &self.snap.pci.notes {
            ui.colored_label(Color32::from_rgb(180, 180, 180), n);
        }
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("pci").striped(true).show(ui, |ui| {
                ui.strong("slot");
                ui.strong("id");
                ui.strong("name");
                ui.strong("class");
                ui.strong("driver");
                ui.strong("link");
                ui.end_row();
                for d in &self.snap.pci.devices {
                    ui.label(&d.slot);
                    ui.label(format!("{}:{}", d.vendor_id, d.device_id));
                    let name = match (&d.vendor_name, &d.device_name) {
                        (Some(v), Some(n)) => format!("{v} {n}"),
                        (Some(v), None) => v.clone(),
                        _ => "—".into(),
                    };
                    ui.label(name);
                    ui.label(&d.class_name);
                    ui.label(d.driver.display());
                    ui.label(d.link_label());
                    ui.end_row();
                }
            });
        });
        let sriov: Vec<_> = self
            .snap
            .pci
            .devices
            .iter()
            .filter(|d| d.sriov_totalvfs.access == AccessKind::Ok)
            .collect();
        if !sriov.is_empty() {
            ui.separator();
            ui.strong("SR-IOV");
            for d in sriov {
                kv(
                    ui,
                    &d.slot,
                    &format!(
                        "num {} / total {}",
                        d.sriov_numvfs.display(),
                        d.sriov_totalvfs.display()
                    ),
                );
            }
        }
        ui.separator();
        ui.strong("virtio");
        for n in &self.snap.virtio.notes {
            ui.weak(n);
        }
        for d in &self.snap.virtio.devices {
            kv(
                ui,
                &d.name,
                &format!(
                    "{}  {}  status {}",
                    d.kind,
                    d.driver.display(),
                    d.status.display()
                ),
            );
        }
        ui.separator();
        ui.strong("IOMMU");
        for n in &self.snap.iommu.notes {
            ui.weak(n);
        }
        for g in &self.snap.iommu.groups {
            kv(
                ui,
                &format!("group {}", g.id),
                &g.devices.join(" "),
            );
        }
    }

    fn ui_platform(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("平台 / 总线", "Platform"));
        for n in &self.snap.platform.notes {
            ui.weak(n);
        }
        kv(
            ui,
            "ACPI / PnP",
            &format!(
                "acpi {}  pnp {}  msr {}",
                self.snap.platform.acpi_devices,
                self.snap.platform.pnp_devices,
                self.snap.platform.msr_devices
            ),
        );
        if !self.snap.platform.platform_devices.is_empty() {
            kv(
                ui,
                "platform",
                &self.snap.platform.platform_devices.join(" "),
            );
        }
        if !self.snap.platform.workqueues.is_empty() {
            kv(ui, "workqueue", &self.snap.platform.workqueues.join(" "));
        }
        if !self.snap.platform.event_sources.is_empty() {
            kv(ui, "perf events", &self.snap.platform.event_sources.join(" "));
        }
        for v in &self.snap.platform.vtconsoles {
            kv(
                ui,
                &format!("vt {}", v.name),
                &format!("{}  bind {}", v.device.display(), v.bind.display()),
            );
        }
        if !self.snap.platform.watchdogs.is_empty() {
            ui.strong("watchdog");
            for w in &self.snap.platform.watchdogs {
                kv(
                    ui,
                    &w.name,
                    &format!(
                        "{}  timeout {}  left {}",
                        w.identity.display(),
                        w.timeout.display(),
                        w.timeleft.display()
                    ),
                );
            }
        }
        if !self.snap.platform.backlights.is_empty() {
            ui.strong("backlight");
            for b in &self.snap.platform.backlights {
                kv(
                    ui,
                    &b.name,
                    &format!(
                        "{}  {} / {}",
                        b.kind.display(),
                        b.actual.display(),
                        b.max.display()
                    ),
                );
            }
        }
        if !self.snap.platform.leds.is_empty() {
            ui.collapsing(format!("LED ({})", self.snap.platform.leds.len()), |ui| {
                for l in &self.snap.platform.leds {
                    kv(
                        ui,
                        &l.name,
                        &format!(
                            "{} / {}  {}",
                            l.brightness.display(),
                            l.max_brightness.display(),
                            l.trigger.display()
                        ),
                    );
                }
            });
        }
        ui.separator();
        ui.strong("I2C");
        for a in &self.snap.platform.i2c_adapters {
            kv(
                ui,
                &a.name,
                &format!("{}  clients {}", a.adapter_name.display(), a.clients),
            );
        }
        ui.separator();
        ui.strong(self.t("DMA / 模拟外设", "DMA / analog"));
        for n in &self.snap.periph.notes {
            ui.weak(n);
        }
        if !self.snap.periph.dma_isa.is_empty() {
            kv(
                ui,
                "/proc/dma",
                &self
                    .snap
                    .periph
                    .dma_isa
                    .iter()
                    .map(|c| format!("{}:{}", c.channel, c.name))
                    .collect::<Vec<_>>()
                    .join("  "),
            );
        }
        for d in &self.snap.periph.dmaengine {
            kv(
                ui,
                &format!("dma {}", d.name),
                &format!("in_use {}", d.in_use.display()),
            );
        }
        for p in &self.snap.periph.pwm_chips {
            kv(ui, &p.name, &format!("npwm {}", p.npwm.display()));
        }
        for i in &self.snap.periph.iio {
            kv(ui, &format!("IIO {}", i.name), &i.iio_name.display());
        }
        for n in &self.snap.periph.nvmem {
            kv(ui, &format!("nvmem {}", n.name), &n.typ.display());
        }
        for r in &self.snap.periph.regulators {
            kv(
                ui,
                &format!("reg {}", r.name),
                &format!(
                    "{}  {}  {} uV",
                    r.regulator_name.display(),
                    r.state.display(),
                    r.microvolts.display()
                ),
            );
        }
        for d in &self.snap.periph.devlinks {
            kv(ui, &format!("devlink {}", d.name), &d.status.display());
        }
        for b in &self.snap.periph.pci_buses {
            kv(
                ui,
                &format!("pci_bus {}", b.name),
                &format!("cpulist {}", b.cpulist.display()),
            );
        }
        ui.separator();
        ui.strong(self.t("无线 / 外设总线", "Radios / extra buses"));
        for n in &self.snap.buses.notes {
            ui.weak(n);
        }
        for r in &self.snap.buses.rfkill {
            kv(
                ui,
                &format!("rfkill {}", r.name),
                &format!(
                    "{}  state {}  hard {} soft {}",
                    r.kind.display(),
                    r.state.display(),
                    r.hard.display(),
                    r.soft.display()
                ),
            );
        }
        for b in &self.snap.buses.bluetooth {
            kv(
                ui,
                &format!("BT {}", b.name),
                &format!("{}  {}", b.dev_name.display(), b.address.display()),
            );
        }
        for t in &self.snap.buses.thunderbolt {
            kv(
                ui,
                &format!("TB {}", t.name),
                &format!(
                    "{} {}  auth {}",
                    t.vendor.display(),
                    t.device.display(),
                    t.authorized.display()
                ),
            );
        }
        for v in &self.snap.buses.video {
            kv(
                ui,
                &v.name,
                &format!("{}  index {}", v.dev_name.display(), v.index.display()),
            );
        }
        for m in &self.snap.buses.mmc {
            kv(
                ui,
                &m.name,
                &format!(
                    "{}  {}  {}",
                    m.card.as_deref().unwrap_or("—"),
                    m.name_tag.display(),
                    m.r#type.display()
                ),
            );
        }
        for mei in &self.snap.buses.mei {
            kv(
                ui,
                &format!("MEI {}", mei.name),
                &mei.fw_status.display(),
            );
        }
        if !self.snap.buses.serial.is_empty() || !self.snap.buses.tty_drivers.is_empty() {
            ui.separator();
            ui.strong("Serial / TTY");
            for d in &self.snap.buses.tty_drivers {
                kv(
                    ui,
                    &d.name,
                    &format!("{}  {} {}  {}", d.device, d.major, d.minors, d.kind),
                );
            }
            for s in &self.snap.buses.serial {
                kv(
                    ui,
                    &s.name,
                    &format!(
                        "irq {}  uartclk {}  type {}",
                        s.irq.display(),
                        s.uartclk.display(),
                        s.typ.display()
                    ),
                );
            }
        }
        for h in &self.snap.buses.hidraw {
            kv(ui, &h.name, &h.hid_name.display());
        }
        for v in &self.snap.buses.virtio_ports {
            kv(
                ui,
                &v.name,
                &format!(
                    "{}  guest {} host {}",
                    v.port_name.display(),
                    v.guest_connected.display(),
                    v.host_connected.display()
                ),
            );
        }
        for g in &self.snap.buses.gpio {
            kv(
                ui,
                &g.name,
                &format!("{}  ngpio {}  base {}", g.label.display(), g.ngpio.display(), g.base.display()),
            );
        }
        for m in &self.snap.buses.mtd {
            kv(
                ui,
                &m.name,
                &format!(
                    "{}  {}  erase {}",
                    m.mtd_name.display(),
                    m.size
                        .value
                        .map(crate::export::format_bytes)
                        .unwrap_or_else(|| m.size.access_label()),
                    m.erasesize.display()
                ),
            );
        }
        if !self.snap.buses.infiniband.is_empty() {
            for ib in &self.snap.buses.infiniband {
                kv(
                    ui,
                    &format!("IB {}", ib.name),
                    &format!(
                        "{}  {}  ports {}",
                        ib.node_type.display(),
                        ib.node_guid.display(),
                        ib.ports
                    ),
                );
            }
        }
        for (label, names) in [
            ("ieee80211", &self.snap.buses.ieee80211),
            ("typec", &self.snap.buses.typec),
            ("udc", &self.snap.buses.udc),
            ("dax", &self.snap.buses.dax),
            ("wmi", &self.snap.buses.wmi),
            ("spi", &self.snap.buses.spi),
            ("serio", &self.snap.buses.serio),
            ("ubi", &self.snap.buses.ubi),
            ("scsi_generic", &self.snap.buses.scsi_generic),
            ("wwan", &self.snap.buses.wwan),
            ("ppp", &self.snap.buses.ppp),
            ("phy", &self.snap.buses.phy),
        ] {
            if !names.is_empty() {
                kv(ui, label, &names.join(" "));
            }
        }
        if !self.snap.buses.misc.is_empty() {
            kv(ui, "misc", &self.snap.buses.misc.join(" "));
        }
    }

    fn ui_software(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("操作系统", "OS"));
        kv(ui, "OS", &self.snap.software.os_name.display());
        kv(ui, "ID", &self.snap.software.os_id.display());
        kv(
            ui,
            self.t("内核", "Kernel"),
            &self.snap.software.kernel_release.display(),
        );
        kv(ui, "ostype", &self.snap.software.ostype.display());
        kv(
            ui,
            "arch",
            &format!(
                "{}  {}-bit  profiling {}",
                self.snap.software.cpu_byteorder.display(),
                self.snap.software.address_bits.display(),
                self.snap.software.profiling.display()
            ),
        );
        kv(ui, "hostname", &self.snap.software.hostname.display());
        kv(
            ui,
            self.t("内存", "Memory"),
            &self
                .snap
                .software
                .mem_total_kb
                .value
                .map(|v| crate::export::format_bytes(v * 1024))
                .unwrap_or_else(|| self.snap.software.mem_total_kb.access_label()),
        );
        kv(ui, "desktop", &self.snap.software.desktop.display());
        kv(
            ui,
            "tainted",
            &match self.snap.software.tainted.value {
                Some(0) => "0".into(),
                Some(v) => format!("{}  {}", v, self.snap.software.taint_flags.join(", ")),
                None => self.snap.software.tainted.access_label(),
            },
        );
        kv(ui, "LSM", &self.snap.software.lsm.display());
        if self.snap.software.selinux_enforce.access == AccessKind::Ok {
            kv(ui, "SELinux enforce", &self.snap.software.selinux_enforce.display());
        }
        kv(ui, "lockdown", &self.snap.security.lockdown.display());
        kv(
            ui,
            "yama/kptr/dmesg",
            &format!(
                "ptrace {}  kptr {}  dmesg {}",
                self.snap.security.ptrace_scope.display(),
                self.snap.security.kptr_restrict.display(),
                self.snap.security.dmesg_restrict.display()
            ),
        );
        kv(
            ui,
            "fs.protected",
            &format!(
                "hardlinks {}  symlinks {}  fifos {}  regular {}",
                self.snap.security.protected_hardlinks.display(),
                self.snap.security.protected_symlinks.display(),
                self.snap.security.protected_fifos.display(),
                self.snap.security.protected_regular.display()
            ),
        );
        if self.snap.security.fips_enabled.access == AccessKind::Ok
            || self.snap.security.apparmor_enabled.access == AccessKind::Ok
        {
            kv(
                ui,
                "FIPS/AppArmor",
                &format!(
                    "fips {}  apparmor {}",
                    self.snap.security.fips_enabled.display(),
                    self.snap.security.apparmor_enabled.display()
                ),
            );
        }
        kv(
            ui,
            "crypto",
            &format!(
                "{} algs ({} internal, {} selftest≠passed)",
                self.snap.crypto.total,
                self.snap.crypto.internal,
                self.snap.crypto.failed_selftest
            ),
        );
        if !self.snap.crypto.types.is_empty() {
            ui.collapsing(
                format!("crypto types ({})", self.snap.crypto.types.len()),
                |ui| {
                    for t in &self.snap.crypto.types {
                        kv(ui, &t.type_name, &t.count.to_string());
                    }
                },
            );
        }
        if !self.snap.crypto.algs.is_empty() {
            ui.collapsing(
                format!("crypto algs ({})", self.snap.crypto.algs.len()),
                |ui| {
                    for a in &self.snap.crypto.algs {
                        kv(
                            ui,
                            &a.name,
                            &format!("{}  {}  {}", a.type_name, a.driver, a.selftest),
                        );
                    }
                },
            );
        }
        if !self.snap.ns.self_ns.is_empty() {
            kv(
                ui,
                "namespaces",
                &self
                    .snap
                    .ns
                    .self_ns
                    .iter()
                    .map(|n| {
                        format!(
                            "{}:{}",
                            n.kind,
                            n.inode
                                .map(|i| i.to_string())
                                .unwrap_or_else(|| "—".into())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("  "),
            );
        }
        kv(
            ui,
            "max_user_namespaces",
            &self.snap.ns.max_user.display(),
        );
        kv(ui, "entropy", &self.snap.software.entropy_avail.display());
        kv(ui, "boot_id", &self.snap.sysctl.boot_id.display());
        kv(ui, "machine-id", &self.snap.software.machine_id.display());
        kv(ui, "domainname", &self.snap.software.domainname.display());
        kv(ui, "config.gz", &self.snap.software.config_gz.display());
        kv(ui, "file locks", &self.snap.software.file_locks.to_string());
        for n in &self.snap.software.notes {
            ui.weak(n);
        }
        kv(
            ui,
            "oops / kexec",
            &format!(
                "oops {}  warn {}  kexec {}  fscaps {}  uevent {}",
                self.snap.software.oops_count.display(),
                self.snap.software.warn_count.display(),
                self.snap.software.kexec_loaded.display(),
                self.snap.software.fscaps.display(),
                self.snap.software.uevent_seqnum.display()
            ),
        );
        if !self.snap.software.filesystems.is_empty() {
            kv(ui, "filesystems", &self.snap.software.filesystems.join(" "));
        }
        kv(
            ui,
            "bpf fs / pstore",
            &format!(
                "bpf {}  pstore {}",
                self.snap.software.bpf_fs_entries, self.snap.firmware.pstore_files
            ),
        );
        kv(
            ui,
            "nmi/watchdog",
            &format!(
                "nmi {}  wd {}  thresh {}  file-max {}  panic {}  sysrq {}  min_free {}  vfs_cache {}  hung {}  oops_panic {}",
                self.snap.sysctl.nmi_watchdog.display(),
                self.snap.sysctl.watchdog.display(),
                self.snap.sysctl.watchdog_thresh.display(),
                self.snap.sysctl.file_max.display(),
                self.snap.sysctl.panic.display(),
                self.snap.sysctl.sysrq.display(),
                self.snap.sysctl.min_free_kbytes.display(),
                self.snap.sysctl.vfs_cache_pressure.display(),
                self.snap.sysctl.hung_task_timeout_secs.display(),
                self.snap.sysctl.panic_on_oops.display()
            ),
        );
        kv(
            ui,
            "sched / oom",
            &format!(
                "rt {}/{}us  rr {}ms  numa {}  tmig {}  panic_oom {}  oom_alloc {}  laptop {}  kexec_off {}  hung_panic {}",
                self.snap.sysctl.sched_rt_runtime_us.display(),
                self.snap.sysctl.sched_rt_period_us.display(),
                self.snap.sysctl.sched_rr_timeslice_ms.display(),
                self.snap.sysctl.numa_balancing.display(),
                self.snap.sysctl.timer_migration.display(),
                self.snap.sysctl.panic_on_oom.display(),
                self.snap.sysctl.oom_kill_allocating_task.display(),
                self.snap.sysctl.laptop_mode.display(),
                self.snap.sysctl.kexec_load_disabled.display(),
                self.snap.sysctl.hung_task_panic.display()
            ),
        );
        kv(
            ui,
            "printk / cfs / uffd",
            &format!(
                "ratelimit {}/{}  cfs {}us  oops_limit {}  hard {}  soft {}  oom_dump {}  user_reserve {}  uffd {}  ngroups {}",
                self.snap.sysctl.printk_ratelimit.display(),
                self.snap.sysctl.printk_ratelimit_burst.display(),
                self.snap.sysctl.sched_cfs_bandwidth_slice_us.display(),
                self.snap.sysctl.oops_limit.display(),
                self.snap.sysctl.hardlockup_panic.display(),
                self.snap.sysctl.softlockup_panic.display(),
                self.snap.sysctl.oom_dump_tasks.display(),
                self.snap.sysctl.user_reserve_kbytes.display(),
                self.snap.sysctl.unprivileged_userfaultfd.display(),
                self.snap.sysctl.ngroups_max.display()
            ),
        );
        kv(
            ui,
            "keys / dumpable",
            &format!(
                "maxkeys {}  maxbytes {}  gc {}  key-users {}  cap_last {}  dumpable {}  autogroup {}  cad {}  leases {}  poolsize {}",
                self.snap.sysctl.keys_maxkeys.display(),
                self.snap.sysctl.keys_maxbytes.display(),
                self.snap.sysctl.keys_gc_delay.display(),
                self.snap.sysctl.key_users,
                self.snap.sysctl.cap_last_cap.display(),
                self.snap.sysctl.suid_dumpable.display(),
                self.snap.sysctl.sched_autogroup.display(),
                self.snap.sysctl.ctrl_alt_del.display(),
                self.snap.sysctl.leases_enable.display(),
                self.snap.sysctl.random_poolsize.display()
            ),
        );
        kv(
            ui,
            "aio/inotify",
            &format!(
                "aio {} / {}  watches {}  inst {}  queued {}  epoll {}  dentry {}/{}  inode {}/{}  pty {}/{}  overflowuid {}  overflowgid {}  dir_notify {}  lease_break {}  writes_strict {}  vsyscall32 {}  ldisc {}  io_uring {}/{}",
                self.snap.sysctl.aio_nr.display(),
                self.snap.sysctl.aio_max_nr.display(),
                self.snap.sysctl.inotify_max_user_watches.display(),
                self.snap.sysctl.inotify_max_user_instances.display(),
                self.snap.sysctl.inotify_max_queued_events.display(),
                self.snap.sysctl.epoll_max_user_watches.display(),
                self.snap.sysctl.dentry_nr.display(),
                self.snap.sysctl.dentry_unused.display(),
                self.snap.sysctl.inode_inuse.display(),
                self.snap.sysctl.inode_free.display(),
                self.snap.sysctl.pty_max.display(),
                self.snap.sysctl.pty_nr.display(),
                self.snap.sysctl.overflowuid.display(),
                self.snap.sysctl.overflowgid.display(),
                self.snap.sysctl.dir_notify_enable.display(),
                self.snap.sysctl.lease_break_time.display(),
                self.snap.sysctl.sysctl_writes_strict.display(),
                self.snap.sysctl.vsyscall32.display(),
                self.snap.sysctl.ldisc_autoload.display(),
                self.snap.sysctl.io_uring_disabled.display(),
                self.snap.sysctl.io_uring_group.display()
            ),
        );
        kv(
            ui,
            "ipc",
            &format!(
                "shmmax {}  shmall {}  shmmni {}  msgmax {}  msgmnb {}  msgmni {}  sem {}  mqueue {}  sysvipc shm/sem/msg {}/{}/{}",
                self.snap.sysctl.shmmax.display(),
                self.snap.sysctl.shmall.display(),
                self.snap.sysctl.shmmni.display(),
                self.snap.sysctl.msgmax.display(),
                self.snap.sysctl.msgmnb.display(),
                self.snap.sysctl.msgmni.display(),
                self.snap.sysctl.sem.display(),
                self.snap.sysctl.mqueue_queues_max.display(),
                self.snap.sysctl.sysvipc_shm,
                self.snap.sysctl.sysvipc_sem,
                self.snap.sysctl.sysvipc_msg
            ),
        );
        kv(
            ui,
            "bpf/modules/perf",
            &format!(
                "unpriv_bpf {}  jit {}/{}  binfmt {}  modules_disabled {}  perf {}  sample_rate {}  cpu% {}",
                self.snap.security.unprivileged_bpf_disabled.display(),
                self.snap.security.bpf_jit_enable.display(),
                self.snap.security.bpf_jit_harden.display(),
                self.snap.security.binfmt_misc_status.display(),
                self.snap.security.modules_disabled.display(),
                self.snap.security.perf_event_paranoid.display(),
                self.snap.sysctl.perf_event_max_sample_rate.display(),
                self.snap.sysctl.perf_cpu_time_max_percent.display()
            ),
        );
        kv(
            ui,
            "file-nr",
            &format!(
                "{} / {}",
                self.snap.sysctl.file_nr_alloc.display(),
                self.snap.sysctl.file_nr_max.display()
            ),
        );
        kv(ui, "pid_max", &self.snap.sysctl.pid_max.display());
        kv(
            ui,
            "vm",
            &format!(
                "swappiness {}  overcommit {}  dirty {}/{}  watermark {}  dirty_expire {}  writeback {}  page-cluster {}  admin_reserve {}",
                self.snap.sysctl.swappiness.display(),
                self.snap.sysctl.overcommit_memory.display(),
                self.snap.sysctl.dirty_ratio.display(),
                self.snap.sysctl.dirty_background_ratio.display(),
                self.snap.sysctl.watermark_scale_factor.display(),
                self.snap.sysctl.dirty_expire_centisecs.display(),
                self.snap.sysctl.dirty_writeback_centisecs.display(),
                self.snap.sysctl.page_cluster.display(),
                self.snap.sysctl.admin_reserve_kbytes.display()
            ),
        );
        kv(ui, "ASLR", &self.snap.sysctl.aslr.display());
        kv(ui, "core_pattern", &self.snap.sysctl.core_pattern.display());
        kv(ui, "ip_forward", &self.snap.sysctl.ip_forward.display());
        kv(
            ui,
            "cgroup",
            &format!(
                "{}  mem {}  cpu {} us  v1 {}",
                self.snap.cgroup.controllers.display(),
                self.snap
                    .cgroup
                    .memory_current
                    .value
                    .map(crate::export::format_bytes)
                    .unwrap_or_else(|| self.snap.cgroup.memory_current.access_label()),
                self.snap.cgroup.cpu_usage_usec.display(),
                if self.snap.cgroup.v1_enabled.is_empty() {
                    "—".into()
                } else {
                    self.snap.cgroup.v1_enabled.join(" ")
                }
            ),
        );
        if !self.snap.cgroup.groups.is_empty() {
            ui.collapsing(
                format!("cgroup slices ({})", self.snap.cgroup.groups.len()),
                |ui| {
                    for g in &self.snap.cgroup.groups {
                        kv(
                            ui,
                            &g.name,
                            &format!(
                                "procs {}  mem {}",
                                g.procs.display(),
                                g.memory_current
                                    .value
                                    .map(crate::export::format_bytes)
                                    .unwrap_or_else(|| g.memory_current.access_label())
                            ),
                        );
                    }
                },
            );
        }
        if !self.snap.sysctl.consoles.is_empty() {
            kv(
                ui,
                "consoles",
                &self
                    .snap
                    .sysctl
                    .consoles
                    .iter()
                    .map(|c| format!("{} {}", c.name, c.flags))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        for n in &self.snap.sysctl.notes {
            ui.weak(n);
        }
        for n in &self.snap.cgroup.notes {
            ui.weak(n);
        }
        for n in &self.snap.security.notes {
            ui.weak(n);
        }
        for n in &self.snap.crypto.notes {
            ui.weak(n);
        }
        for n in &self.snap.ns.notes {
            ui.weak(n);
        }
        if let Some(cpu) = &self.snap.psi.cpu {
            kv(
                ui,
                "PSI cpu",
                &format!(
                    "some {:.2}/{:.2}/{:.2}",
                    cpu.some.avg10, cpu.some.avg60, cpu.some.avg300
                ),
            );
        }
        if let Some(mem) = &self.snap.psi.memory {
            kv(
                ui,
                "PSI memory",
                &format!("some {:.2}  full {:.2}", mem.some.avg10, mem.full.as_ref().map(|f| f.avg10).unwrap_or(0.0)),
            );
        }
        if let Some(io) = &self.snap.psi.io {
            kv(
                ui,
                "PSI io",
                &format!("some {:.2}  full {:.2}", io.some.avg10, io.full.as_ref().map(|f| f.avg10).unwrap_or(0.0)),
            );
        }
        for n in &self.snap.psi.notes {
            ui.weak(n);
        }
        kv(
            ui,
            "loadavg",
            &format!(
                "{}  {}  {}  {}",
                self.snap.software.load_1.display(),
                self.snap.software.load_5.display(),
                self.snap.software.load_15.display(),
                self.snap.software.procs.display()
            ),
        );
        kv(ui, "clocksource", &self.snap.clock.current.display());
        kv(ui, "available", &self.snap.clock.available.display());
        if !self.snap.clock.clockevents.is_empty() {
            kv(ui, "clockevents", &self.snap.clock.clockevents.join(" "));
        }
        for n in &self.snap.clock.notes {
            ui.weak(n);
        }
        for r in &self.snap.clock.rtcs {
            kv(ui, &format!("RTC {}", r.name), &r.rtc_name.display());
        }
        for p in &self.snap.clock.ptps {
            kv(
                ui,
                &format!("PTP {}", p.name),
                &format!(
                    "{}  pps {}",
                    p.clock_name.display(),
                    p.pps_available.display()
                ),
            );
        }
        for p in &self.snap.clock.pps {
            kv(
                ui,
                &format!("PPS {}", p.name),
                &format!("{}  mode {}", p.path.display(), p.mode.display()),
            );
        }
        kv(
            ui,
            self.t("模块", "modules"),
            &self.snap.modules.modules.len().to_string(),
        );
        for n in &self.snap.modules.notes {
            ui.weak(n);
        }
        if !self.snap.modules.modules.is_empty() {
            ui.collapsing("lsmod", |ui| {
                for m in &self.snap.modules.modules {
                    ui.label(format!(
                        "{}  {}  refs {}  {}",
                        m.name,
                        crate::export::format_bytes(m.size_bytes),
                        m.refcount,
                        m.state
                    ));
                }
            });
        }
        if !self.snap.iomem.summaries.is_empty() {
            ui.collapsing("iomem", |ui| {
                for n in &self.snap.iomem.notes {
                    ui.weak(n);
                }
                for s in &self.snap.iomem.summaries {
                    kv(
                        ui,
                        &s.name,
                        &format!(
                            "×{}  {}",
                            s.count,
                            crate::export::format_bytes(s.size)
                        ),
                    );
                }
            });
        }
        if !self.snap.iomem.ioports.is_empty() {
            ui.collapsing(
                format!("ioports ({})", self.snap.iomem.ioports.len()),
                |ui| {
                    for r in self.snap.iomem.ioports.iter().take(24) {
                        ui.label(&r.name);
                    }
                },
            );
        }
        kv(
            ui,
            self.t("固件", "Firmware"),
            &self
                .snap
                .firmware
                .interface
                .value
                .clone()
                .unwrap_or_else(|| self.snap.firmware.interface.access_label()),
        );
        kv(ui, "Secure Boot", &self.snap.firmware.secure_boot.display());
        kv(
            ui,
            "ACPI",
            &if self.snap.firmware.acpi_tables.is_empty() {
                "—".into()
            } else {
                self.snap.firmware.acpi_tables.join(" ")
            },
        );
        kv(ui, "ACPI pm_profile", &self.snap.firmware.acpi_pm_profile.display());
        kv(ui, "pstore", &self.snap.firmware.pstore_files.to_string());
        kv(ui, "firmware timeout", &self.snap.firmware.firmware_timeout.display());
        kv(ui, "memmap", &self.snap.firmware.memmap_entries.to_string());
        if self.snap.firmware.dt_model.access == AccessKind::Ok {
            kv(ui, "device-tree", &self.snap.firmware.dt_model.display());
        }
        kv(ui, "hwrng", &self.snap.firmware.rng_current.display());
        for t in &self.snap.firmware.tpms {
            kv(
                ui,
                &format!("TPM {}", t.name),
                &format!(
                    "v{}  {}",
                    t.version_major.display(),
                    t.pcr_banks.join(",")
                ),
            );
        }
        for n in &self.snap.firmware.notes {
            ui.weak(n);
        }
        kv(ui, "sysfs irq", &self.snap.irq.sysfs_irqs.to_string());
        for n in &self.snap.irq.notes {
            ui.weak(n);
        }
        if !self.snap.irq.lines.is_empty() {
            ui.collapsing(
                format!(
                    "IRQ top ({}, sysfs {})",
                    self.snap.irq.lines.len(),
                    self.snap.irq.sysfs_irqs
                ),
                |ui| {
                    for l in self.snap.irq.lines.iter().take(16) {
                        kv(
                            ui,
                            &l.irq,
                            &format!(
                                "{}  {}  {}",
                                l.total,
                                l.extra,
                                l.affinity.value.as_deref().unwrap_or("")
                            ),
                        );
                    }
                },
            );
        }
        if !self.snap.irq.softirqs.is_empty() {
            ui.collapsing("softirq", |ui| {
                for l in self.snap.irq.softirqs.iter().take(16) {
                    kv(ui, &l.irq, &l.total.to_string());
                }
            });
        }
        ui.collapsing("cmdline", |ui| {
            ui.label(self.snap.software.cmdline.display());
        });
        ui.collapsing("/proc/version", |ui| {
            ui.label(self.snap.software.kernel_version_banner.display());
        });
    }

    fn ui_bench(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("基准测试", "Benchmark"));
        ui.label(self.t(
            "用户态微基准。磁盘先 buffered，再尝试 O_DIRECT（绕过 page cache）。",
            "In-process microbenchmarks. Disk tries O_DIRECT after buffered.",
        ));
        if ui
            .button(self.t("运行快速测试", "Run quick bench"))
            .clicked()
        {
            self.bench = Some(bench::run(&BenchRequest::quick()));
        }
        if ui
            .button(self.t("运行标准测试 (~2s)", "Run standard bench"))
            .clicked()
        {
            self.bench = Some(bench::run(&BenchRequest::default()));
        }
        if let Some(r) = &self.bench {
            if let Some(c) = &r.cpu {
                kv(
                    ui,
                    "CPU",
                    &format!(
                        "threads {}  int {:.1} Mops  fp {:.1} MFLOPS  score {:.1}",
                        c.threads, c.integer_mops, c.float_mflops, c.score
                    ),
                );
            }
            if let Some(m) = &r.memory {
                kv(
                    ui,
                    self.t("内存带宽", "Memory"),
                    &format!(
                        "copy {:.2} GB/s  scale {:.2}  triad {:.2}",
                        m.copy_gbs, m.scale_gbs, m.triad_gbs
                    ),
                );
            }
            if let Some(d) = &r.disk {
                if let Some(b) = &d.buffered {
                    kv(
                        ui,
                        self.t("磁盘 buffered", "Disk buffered"),
                        &format!(
                            "write {:.1} MB/s  read {:.1} MB/s  fsync {} ms",
                            b.write_mbs, b.read_mbs, b.fsync_ms
                        ),
                    );
                }
                if let Some(dir) = &d.direct {
                    kv(
                        ui,
                        self.t("磁盘 O_DIRECT", "Disk O_DIRECT"),
                        &format!(
                            "write {:.1} MB/s  read {:.1} MB/s  fsync {} ms",
                            dir.write_mbs, dir.read_mbs, dir.fsync_ms
                        ),
                    );
                } else if let Some(err) = &d.direct_error {
                    ui.colored_label(Color32::from_rgb(255, 179, 71), format!("O_DIRECT: {err}"));
                }
            }
            for n in &r.notes {
                ui.weak(n);
            }
        }
    }

    fn ui_export(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("导出", "Export"));
        if ui.button("JSON → aida-report.json").clicked() {
            match export::to_json_pretty(&self.snap) {
                Ok(s) => match std::fs::write("aida-report.json", s) {
                    Ok(()) => self.export_msg = Some("wrote aida-report.json".into()),
                    Err(e) => self.export_msg = Some(e.to_string()),
                },
                Err(e) => self.export_msg = Some(e),
            }
        }
        if ui.button("HTML → aida-report.html").clicked() {
            match std::fs::write("aida-report.html", export::to_html(&self.snap)) {
                Ok(()) => self.export_msg = Some("wrote aida-report.html".into()),
                Err(e) => self.export_msg = Some(e.to_string()),
            }
        }
        if let Some(m) = &self.export_msg {
            ui.label(m);
        }
        ui.label(self.t(
            "也可在终端: aida collect --html report.html",
            "CLI: aida collect --html report.html",
        ));
    }
}

fn tr<'a>(cjk: bool, zh: &'a str, en: &'a str) -> &'a str {
    if cjk {
        zh
    } else {
        en
    }
}

fn push_hist(q: &mut VecDeque<[f64; 2]>, t: f64, v: f64) {
    q.push_back([t, v]);
    while q.len() > HISTORY {
        q.pop_front();
    }
}

fn plot_lines(
    ui: &mut egui::Ui,
    id: &str,
    series: &HashMap<String, VecDeque<[f64; 2]>>,
    height: f32,
) {
    Plot::new(id)
        .height(height)
        .legend(egui_plot::Legend::default())
        .show(ui, |plot| {
            for (name, q) in series {
                let pts: PlotPoints = q.iter().copied().map(|p| [p[0], p[1]]).collect();
                plot.line(Line::new(pts).name(name));
            }
        });
}

fn kb_disp(s: &crate::Sample<u64>) -> String {
    s.value
        .map(|v| crate::export::format_bytes(v * 1024))
        .unwrap_or_else(|| s.access_label())
}

fn kb_pair(total: &crate::Sample<u64>, avail: &crate::Sample<u64>) -> String {
    format!("{}  avail {}", kb_disp(total), kb_disp(avail))
}

fn nav_btn(ui: &mut egui::Ui, current: &mut Nav, id: Nav, label: &str) {
    if ui.selectable_label(*current == id, label).clicked() {
        *current = id;
    }
}

fn kv_name_list(
    ui: &mut egui::Ui,
    label: &str,
    names: &[String],
    notes: &[String],
    path_frag: &str,
) {
    if !names.is_empty() {
        kv(ui, label, &names.join(" "));
    } else if let Some(n) = notes.iter().find(|s| s.contains(path_frag)) {
        kv(ui, label, n);
    }
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.strong(format!("{k}:"));
        ui.label(v);
    });
}

fn alert_color(level: AlertLevel) -> Color32 {
    match level {
        AlertLevel::Ok => Color32::from_rgb(120, 200, 140),
        AlertLevel::Low => Color32::from_rgb(120, 180, 255),
        AlertLevel::High => Color32::from_rgb(255, 179, 71),
        AlertLevel::Crit => Color32::from_rgb(255, 100, 100),
    }
}

fn field_row_cell<T: serde::Serialize + std::fmt::Display>(
    ui: &mut egui::Ui,
    s: &crate::Sample<T>,
) {
    match s.access {
        AccessKind::Ok => {
            if let Some(v) = &s.value {
                ui.label(v.to_string());
            } else {
                ui.weak("empty");
            }
        }
        AccessKind::PermissionDenied => {
            ui.colored_label(Color32::from_rgb(255, 179, 71), s.access_label());
        }
        _ => {
            ui.weak(s.access_label());
        }
    }
}

fn field_row<T: serde::Serialize + std::fmt::Display>(
    ui: &mut egui::Ui,
    k: &str,
    s: &crate::Sample<T>,
) {
    ui.horizontal(|ui| {
        ui.strong(format!("{k}:"));
        match s.access {
            AccessKind::Ok => {
                if let Some(v) = &s.value {
                    ui.label(v.to_string());
                } else {
                    ui.weak("empty");
                }
            }
            AccessKind::PermissionDenied => {
                ui.colored_label(Color32::from_rgb(255, 179, 71), s.access_label());
            }
            _ => {
                ui.weak(s.access_label());
            }
        }
    });
}

fn install_fonts(ctx: &egui::Context) -> bool {
    let candidates = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/opentype/source-han-sans/SourceHanSansCN-Regular.otf",
        "/usr/share/fonts/truetype/arphic/uming.ttc",
    ];
    for p in candidates {
        if let Ok(bytes) = std::fs::read(p) {
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".into(), FontData::from_owned(bytes));
            if let Some(fam) = fonts.families.get_mut(&FontFamily::Proportional) {
                fam.insert(0, "cjk".into());
            }
            if let Some(fam) = fonts.families.get_mut(&FontFamily::Monospace) {
                fam.push("cjk".into());
            }
            ctx.set_fonts(fonts);
            return true;
        }
    }
    false
}
