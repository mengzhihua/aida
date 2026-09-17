//! 桌面布局：模仿 AIDA64 的树形导航 + 详情表 + 传感器历史曲线。

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, RichText};
use egui_plot::{Line, Plot, PlotPoints};

use crate::access::{AccessKind, ProbeCtx};
use crate::bench::{self, BenchReport, BenchRequest};
use crate::export;
use crate::probes::cpu::CpuStatSnap;
use crate::probes::hwmon;
use crate::snapshot::HardwareSnapshot;

const HISTORY: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Summary,
    Cpu,
    Dmi,
    Sensors,
    Storage,
    Pci,
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
    nav: Nav,
    cjk: bool,
    last_poll: Instant,
    temps: HashMap<String, VecDeque<[f64; 2]>>,
    t0: Instant,
    bench: Option<BenchReport>,
    export_msg: Option<String>,
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
        Self {
            ctx,
            snap,
            prev_stat,
            nav: Nav::Summary,
            cjk,
            last_poll: Instant::now(),
            temps: HashMap::new(),
            t0: Instant::now(),
            bench: None,
            export_msg: None,
        }
    }

    fn t<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        tr(self.cjk, zh, en)
    }

    fn poll_sensors(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(800) {
            return;
        }
        self.last_poll = Instant::now();
        self.snap.refresh_live(&self.ctx, &mut self.prev_stat);
        let t = self.t0.elapsed().as_secs_f64();
        for (key, value) in hwmon::temperature_series(&self.snap.sensors) {
            let q = self.temps.entry(key).or_default();
            q.push_back([t, value]);
            while q.len() > HISTORY {
                q.pop_front();
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
            });
        });

        egui::SidePanel::left("tree")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("AIDA");
                ui.label(RichText::new("Linux").italics().weak());
                ui.separator();
                let cjk = self.cjk;
                nav_btn(ui, &mut self.nav, Nav::Summary, tr(cjk, "计算机摘要", "Summary"));
                nav_btn(ui, &mut self.nav, Nav::Cpu, tr(cjk, "处理器", "CPU"));
                nav_btn(ui, &mut self.nav, Nav::Dmi, tr(cjk, "主板 / DMI", "Motherboard / DMI"));
                nav_btn(ui, &mut self.nav, Nav::Sensors, tr(cjk, "传感器", "Sensors"));
                nav_btn(ui, &mut self.nav, Nav::Storage, tr(cjk, "存储 / NVMe", "Storage / NVMe"));
                nav_btn(ui, &mut self.nav, Nav::Pci, tr(cjk, "PCI 设备", "PCI"));
                nav_btn(ui, &mut self.nav, Nav::Software, tr(cjk, "操作系统", "OS"));
                ui.separator();
                nav_btn(ui, &mut self.nav, Nav::Bench, tr(cjk, "基准测试", "Benchmark"));
                nav_btn(ui, &mut self.nav, Nav::Export, tr(cjk, "导出报告", "Export"));
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.nav {
            Nav::Summary => self.ui_summary(ui),
            Nav::Cpu => self.ui_cpu(ui),
            Nav::Dmi => self.ui_dmi(ui),
            Nav::Sensors => self.ui_sensors(ui),
            Nav::Storage => self.ui_storage(ui),
            Nav::Pci => self.ui_pci(ui),
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
        kv(ui, self.t("系统厂商", "Vendor"), &self.snap.dmi.sys_vendor.display());
        kv(ui, self.t("产品", "Product"), &self.snap.dmi.product_name.display());
        kv(ui, self.t("操作系统", "OS"), &self.snap.software.os_name.display());
        kv(
            ui,
            self.t("内核", "Kernel"),
            &self.snap.software.kernel_release.display(),
        );
        if let Some(kb) = self.snap.software.mem_total_kb.value {
            kv(ui, self.t("内存", "Memory"), &crate::export::format_bytes(kb * 1024));
        }
        kv(
            ui,
            self.t("PCI 设备数", "PCI devices"),
            &self.snap.pci.devices.len().to_string(),
        );
        kv(
            ui,
            self.t("块设备", "Block devices"),
            &self.snap.block.devices.len().to_string(),
        );
        ui.separator();
        ui.label(self.t(
            "缺字段时请看橙色权限条：普通用户看不到序列号/SMART 是预期行为。",
            "Missing fields usually mean the current user cannot read that sysfs node.",
        ));
    }

    fn ui_cpu(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("处理器", "CPU"));
        kv(ui, self.t("型号", "Model"), &self.snap.cpu.model_name.display());
        kv(ui, self.t("厂商", "Vendor"), &self.snap.cpu.vendor.display());
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
            if self.snap.cpu.hypervisor { "yes" } else { "no" },
        );
        for n in &self.snap.cpu.notes {
            ui.colored_label(Color32::YELLOW, n);
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("cpu_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("#");
                    ui.strong("core");
                    ui.strong("MHz");
                    ui.strong("governor");
                    ui.end_row();
                    for l in &self.snap.cpu.logical {
                        ui.label(l.processor.to_string());
                        ui.label(l.core_id.map(|c| c.to_string()).unwrap_or_else(|| "-".into()));
                        let mhz = l
                            .scaling_cur_khz
                            .value
                            .map(|k| format!("{:.0}", k as f64 / 1000.0))
                            .or_else(|| l.mhz_from_cpuinfo.map(|m| format!("{m:.0}")))
                            .unwrap_or_else(|| l.scaling_cur_khz.access_label());
                        ui.label(mhz);
                        ui.label(l.governor.display());
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
        });
    }

    fn ui_dmi(&self, ui: &mut egui::Ui) {
        ui.heading("DMI / SMBIOS");
        kv(ui, "BIOS", &format!("{} {}", self.snap.dmi.bios_vendor.display(), self.snap.dmi.bios_version.display()));
        kv(ui, self.t("厂商", "Vendor"), &self.snap.dmi.sys_vendor.display());
        kv(ui, self.t("产品", "Product"), &self.snap.dmi.product_name.display());
        field_row(ui, self.t("序列号", "Serial"), &self.snap.dmi.product_serial);
        field_row(ui, "UUID", &self.snap.dmi.product_uuid);
        kv(ui, self.t("主板", "Board"), &format!("{} {}", self.snap.dmi.board_vendor.display(), self.snap.dmi.board_name.display()));
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

    fn ui_sensors(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("传感器", "Sensors"));
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
                    kv(ui, &ch.label, &val);
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
        });
    }

    fn ui_storage(&self, ui: &mut egui::Ui) {
        ui.heading(self.t("存储", "Storage"));
        ui.strong("Block");
        egui::Grid::new("blk").striped(true).show(ui, |ui| {
            ui.strong("name");
            ui.strong("type");
            ui.strong("size");
            ui.strong("model");
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
                ui.end_row();
            }
        });
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
        kv(ui, self.t("内核", "Kernel"), &self.snap.software.kernel_release.display());
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
            "用户态微基准，结果只适合本机前后对比。磁盘写临时文件，非 O_DIRECT。",
            "In-process microbenchmarks for relative comparison only.",
        ));
        if ui.button(self.t("运行快速测试", "Run quick bench")).clicked() {
            self.bench = Some(bench::run(&BenchRequest::quick()));
        }
        if ui.button(self.t("运行标准测试 (~2s)", "Run standard bench")).clicked() {
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
                kv(
                    ui,
                    self.t("磁盘", "Disk"),
                    &format!(
                        "write {:.1} MB/s  read {:.1} MB/s  fsync {} ms",
                        d.write_mbs, d.read_mbs, d.fsync_ms
                    ),
                );
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

fn field_row<T: serde::Serialize + std::fmt::Display>(ui: &mut egui::Ui, k: &str, s: &crate::Sample<T>) {
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
