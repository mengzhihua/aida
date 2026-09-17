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
        Self {
            ctx,
            snap,
            prev_stat,
            prev_net,
            prev_disk,
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
                    "L{} {} {} shared {}",
                    c.level.display(),
                    c.kind.display(),
                    c.size.display(),
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
                    ui.end_row();
                }
            });
        });
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
        for n in &self.snap.clock.notes {
            ui.weak(n);
        }
        for r in &self.snap.clock.rtcs {
            kv(ui, &format!("RTC {}", r.name), &r.rtc_name.display());
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
        if !self.snap.irq.lines.is_empty() {
            ui.collapsing(
                format!("IRQ top ({})", self.snap.irq.lines.len()),
                |ui| {
                    for l in self.snap.irq.lines.iter().take(16) {
                        kv(ui, &l.irq, &format!("{}  {}", l.total, l.extra));
                    }
                },
            );
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
