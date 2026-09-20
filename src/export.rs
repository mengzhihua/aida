//! JSON / HTML / 文本 / CSV / Markdown 报告导出。
//! HTML 为单文件，无外部资源。文本/CSV/Markdown 共用同一份摘要清单
//!（对标 AIDA64 的 TXT/CSV 报告；完整字段仍走 JSON）。

use crate::access::AccessKind;
use crate::alerts::AlertLevel;
use crate::probes::hwmon::SensorKind;
use crate::snapshot::HardwareSnapshot;

pub fn to_json_pretty(snap: &HardwareSnapshot) -> Result<String, String> {
    serde_json::to_string_pretty(snap).map_err(|e| e.to_string())
}

/// CLI `--format` / 文件后缀用的报告种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFormat {
    Json,
    Html,
    Text,
    Csv,
    Markdown,
}

impl ReportFormat {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "html" | "htm" => Some(Self::Html),
            "text" | "txt" => Some(Self::Text),
            "csv" => Some(Self::Csv),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    pub fn render(self, snap: &HardwareSnapshot) -> Result<String, String> {
        match self {
            Self::Json => to_json_pretty(snap),
            Self::Html => Ok(to_html(snap)),
            Self::Text => Ok(to_text(snap)),
            Self::Csv => Ok(to_csv(snap)),
            Self::Markdown => Ok(to_markdown(snap)),
        }
    }
}

pub fn to_text(snap: &HardwareSnapshot) -> String {
    let mut out = String::new();
    out.push_str("AIDA Linux 硬件报告\n");
    out.push_str(&format!(
        "version: {}\nunix_ms: {}\nprivilege: {}\n",
        snap.version, snap.collected_at_unix_ms, snap.privilege.summary
    ));
    for sec in report_sections(snap) {
        out.push('\n');
        out.push_str(&format!("[{}]\n", sec.title));
        for (k, v) in sec.rows {
            out.push_str(&format!("{k}: {v}\n"));
        }
    }
    out
}

pub fn to_csv(snap: &HardwareSnapshot) -> String {
    let mut out = String::from("section,key,value\n");
    out.push_str(&format!(
        "{},{},{}\n",
        csv_escape("meta"),
        csv_escape("version"),
        csv_escape(snap.version)
    ));
    out.push_str(&format!(
        "{},{},{}\n",
        csv_escape("meta"),
        csv_escape("unix_ms"),
        csv_escape(&snap.collected_at_unix_ms.to_string())
    ));
    out.push_str(&format!(
        "{},{},{}\n",
        csv_escape("meta"),
        csv_escape("privilege"),
        csv_escape(&snap.privilege.summary)
    ));
    for sec in report_sections(snap) {
        for (k, v) in sec.rows {
            out.push_str(&format!(
                "{},{},{}\n",
                csv_escape(&sec.title),
                csv_escape(&k),
                csv_escape(&v)
            ));
        }
    }
    out
}

pub fn to_markdown(snap: &HardwareSnapshot) -> String {
    let mut out = String::new();
    out.push_str("# AIDA Linux 硬件报告\n\n");
    out.push_str(&format!(
        "- version: `{}`\n- unix_ms: {}\n- privilege: {}\n",
        snap.version, snap.collected_at_unix_ms, snap.privilege.summary
    ));
    for sec in report_sections(snap) {
        out.push_str(&format!("\n## {}\n\n| 项 | 值 |\n| --- | --- |\n", sec.title));
        for (k, v) in sec.rows {
            out.push_str(&format!("| {} | {} |\n", md_cell(&k), md_cell(&v)));
        }
    }
    out
}

pub fn to_html(snap: &HardwareSnapshot) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\">");
    html.push_str("<title>AIDA Linux 硬件报告</title><style>");
    html.push_str(
        "body{font-family:sans-serif;background:#12141a;color:#e6e6e6;margin:0;padding:24px;}",
    );
    html.push_str(
        "h1,h2{color:#7ec8ff;} table{border-collapse:collapse;width:100%;margin:12px 0;}",
    );
    html.push_str(
        "td,th{border:1px solid #333;padding:6px 8px;text-align:left;vertical-align:top;}",
    );
    html.push_str("th{background:#1c2230;} .muted{color:#aaa;font-size:12px;} .warn{color:#ffb347;} .crit{color:#ff6b6b;} .ok{color:#78c88c;}");
    html.push_str("</style></head><body>");
    html.push_str(&format!(
        "<h1>AIDA Linux 硬件报告</h1><p class=\"muted\">v{} · unix_ms {}</p>",
        snap.version, snap.collected_at_unix_ms
    ));
    html.push_str(&format!("<p>{}</p>", esc(&snap.privilege.summary)));

    section(&mut html, "CPU");
    kv(
        &mut html,
        &[
            ("型号", snap.cpu.model_name.display()),
            ("厂商", snap.cpu.vendor.display()),
            ("逻辑 CPU", snap.cpu.logical_cpus.to_string()),
            ("封装数", snap.cpu.physical_packages.to_string()),
            (
                "利用率",
                snap.cpu
                    .utilization_pct
                    .map(|v| format!("{v:.1}%"))
                    .unwrap_or_else(|| "n/a".into()),
            ),
            ("虚拟化", snap.cpu.hypervisor.to_string()),
            (
                "SMT",
                format!(
                    "active {} control {}",
                    snap.cpu.smt_active.display(),
                    snap.cpu.smt_control.display()
                ),
            ),
            ("online", snap.cpu.online.display()),
            (
                "offline",
                match (snap.cpu.offline.access, snap.cpu.offline.value.as_deref()) {
                    (crate::access::AccessKind::Ok, Some(s)) if !s.is_empty() => s.to_string(),
                    (crate::access::AccessKind::Ok, _) => "—".into(),
                    _ => snap.cpu.offline.access_label(),
                },
            ),
            ("possible", snap.cpu.possible.display()),
            ("present", snap.cpu.present.display()),
            ("kernel_max", snap.cpu.kernel_max.display()),
            ("enabled", snap.cpu.enabled.display()),
            (
                "nohz_full",
                match (
                    snap.cpu.nohz_full.access,
                    snap.cpu.nohz_full.value.as_deref(),
                ) {
                    (crate::access::AccessKind::Ok, Some(s)) if !s.is_empty() => s.to_string(),
                    (crate::access::AccessKind::Ok | crate::access::AccessKind::NotFound, _) => {
                        "—".into()
                    }
                    _ => snap.cpu.nohz_full.access_label(),
                },
            ),
            (
                "modalias",
                crate::probes::cpu::display_modalias(&snap.cpu.modalias),
            ),
            (
                "cpuidle",
                format!(
                    "driver {} governor {} available {}",
                    snap.cpu.cpuidle_driver.display(),
                    snap.cpu.cpuidle_governor.display(),
                    snap.cpu.cpuidle_available_governors.display()
                ),
            ),
            ("KVM", snap.kvm.device.display()),
            ("nested", snap.kvm.nested.display()),
            ("microcode", snap.cpu.microcode.display()),
        ],
    );
    html_dmi_board(&mut html, snap);
    if !snap.cpu.vulnerabilities.is_empty() {
        html.push_str("<table><tr><th>漏洞</th><th>状态</th></tr>");
        for v in &snap.cpu.vulnerabilities {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                esc(&v.name),
                esc(&v.status.display())
            ));
        }
        html.push_str("</table>");
    }
    if !snap.cpu.idle_states.is_empty() {
        html.push_str("<table><tr><th>cpuidle</th><th>说明</th><th>latency</th></tr>");
        for s in &snap.cpu.idle_states {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&s.name.display()),
                esc(&s.desc.display()),
                esc(&s.latency_us.display())
            ));
        }
        html.push_str("</table>");
    }
    if !snap.cpu.freq_policies.is_empty() {
        html.push_str(
            "<table><tr><th>cpufreq</th><th>driver</th><th>governor</th><th>kHz</th></tr>",
        );
        for p in &snap.cpu.freq_policies {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}-{}</td></tr>",
                esc(&p.name),
                esc(&p.driver.display()),
                esc(&p.governor.display()),
                esc(&p.scaling_min_khz.display()),
                esc(&p.scaling_max_khz.display())
            ));
        }
        html.push_str("</table>");
    }

    section(&mut html, "DMI / 主板");
    kv(
        &mut html,
        &[
            ("系统厂商", snap.dmi.sys_vendor.display()),
            ("产品", snap.dmi.product_name.display()),
            ("序列号", snap.dmi.product_serial.display()),
            ("UUID", snap.dmi.product_uuid.display()),
            (
                "主板",
                format!(
                    "{} {}",
                    snap.dmi.board_vendor.display(),
                    snap.dmi.board_name.display()
                ),
            ),
            (
                "BIOS",
                format!(
                    "{} {} rom {} rel {}",
                    snap.dmi.bios_vendor.display(),
                    snap.dmi.bios_version.display(),
                    snap.dmi
                        .bios_rom_kb
                        .map(|n| format!("{n} KiB"))
                        .unwrap_or_else(|| "—".into()),
                    snap.dmi.bios_release.as_deref().unwrap_or("—")
                ),
            ),
        ],
    );
    for n in &snap.dmi.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }
    html_dmi_memory(&mut html, snap);
    html_dmi_board(&mut html, snap);

    section(&mut html, "固件");
    kv(
        &mut html,
        &[
            ("接口", snap.firmware.interface.display()),
            ("Secure Boot", snap.firmware.secure_boot.display()),
            ("fw_platform_size", snap.firmware.fw_platform_size.display()),
            (
                "ACPI",
                if snap.firmware.acpi_tables.is_empty() {
                    "—".into()
                } else {
                    snap.firmware.acpi_tables.join(" ")
                },
            ),
            ("ACPI pm_profile", snap.firmware.acpi_pm_profile.display()),
            ("pstore", snap.firmware.pstore_files.to_string()),
            ("firmware timeout", snap.firmware.firmware_timeout.display()),
            ("memmap", snap.firmware.memmap_entries.to_string()),
            ("device-tree", snap.firmware.dt_model.display()),
            ("hwrng", snap.firmware.rng_current.display()),
        ],
    );
    for t in &snap.firmware.tpms {
        html.push_str(&format!(
            "<p>TPM {} version {} banks {}</p>",
            esc(&t.name),
            esc(&t.version_major.display()),
            esc(&t.pcr_banks.join(","))
        ));
    }
    for n in &snap.firmware.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "内存");
    kv(
        &mut html,
        &[
            ("物理", kb_html(&snap.memory.total_kb)),
            ("可用", kb_html(&snap.memory.available_kb)),
            ("空闲", kb_html(&snap.memory.free_kb)),
            (
                "Buffers / Cached",
                format!(
                    "{} / {}",
                    kb_html(&snap.memory.buffers_kb),
                    kb_html(&snap.memory.cached_kb)
                ),
            ),
            (
                "Swap",
                format!(
                    "{} / {}",
                    kb_html(&snap.memory.swap_total_kb),
                    kb_html(&snap.memory.swap_free_kb)
                ),
            ),
            ("THP", snap.memory.thp_enabled.display()),
            (
                "memory tiers",
                if snap.memory.memory_tiers.is_empty() {
                    "—".into()
                } else {
                    snap.memory.memory_tiers.join(" ")
                },
            ),
            (
                "DirectMap",
                format!(
                    "4k {} 2M {} 1G {}",
                    kb_html(&snap.memory.directmap_4k_kb),
                    kb_html(&snap.memory.directmap_2m_kb),
                    kb_html(&snap.memory.directmap_1g_kb)
                ),
            ),
            (
                "zswap",
                format!(
                    "enabled {} {} {}",
                    snap.zmem.zswap.enabled.display(),
                    snap.zmem.zswap.compressor.display(),
                    snap.zmem.zswap.zpool.display()
                ),
            ),
            (
                "KSM",
                format!(
                    "run {} shared {} sharing {}",
                    snap.memory.ksm.run.display(),
                    snap.memory.ksm.pages_shared.display(),
                    snap.memory.ksm.pages_sharing.display()
                ),
            ),
            (
                "vmstat",
                format!(
                    "pgfault {} maj {} oom {}",
                    snap.memory.vmstat.pgfault.display(),
                    snap.memory.vmstat.pgmajfault.display(),
                    snap.memory.vmstat.oom_kill.display()
                ),
            ),
        ],
    );
    html_dmi_memory(&mut html, snap);
    if !snap.zmem.zram.is_empty() {
        html.push_str("<table><tr><th>zram</th><th>disksize</th><th>algo</th><th>orig</th><th>compr</th></tr>");
        for z in &snap.zmem.zram {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&z.name),
                esc(&z
                    .disksize
                    .value
                    .map(format_bytes)
                    .unwrap_or_else(|| z.disksize.access_label())),
                esc(&z.algorithm.display()),
                esc(&z
                    .orig_bytes
                    .value
                    .map(format_bytes)
                    .unwrap_or_else(|| z.orig_bytes.access_label())),
                esc(&z
                    .compr_bytes
                    .value
                    .map(format_bytes)
                    .unwrap_or_else(|| z.compr_bytes.access_label()))
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.zmem.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    if snap.memory.mem_blocks.total > 0 {
        html.push_str(&format!(
            "<p class=\"muted\">memory blocks {}/{} online, size {}</p>",
            snap.memory.mem_blocks.online,
            snap.memory.mem_blocks.total,
            esc(&snap
                .memory
                .mem_blocks
                .block_size_bytes
                .value
                .map(format_bytes)
                .unwrap_or_else(|| snap.memory.mem_blocks.block_size_bytes.access_label()))
        ));
    }
    if !snap.memory.buddy.is_empty() {
        html.push_str("<table><tr><th>node</th><th>zone</th><th>buddy free</th></tr>");
        for z in &snap.memory.buddy {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                z.node,
                esc(&z.zone),
                esc(&z
                    .free_counts
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(" "))
            ));
        }
        html.push_str("</table>");
    }
    if !snap.memory.zones.is_empty() {
        html.push_str("<p class=\"muted\">zoneinfo</p>");
        html.push_str("<table><tr><th>node</th><th>zone</th><th>free</th><th>present</th><th>managed</th></tr>");
        for z in &snap.memory.zones {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                z.node,
                esc(&z.zone),
                z.free.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                z.present
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into()),
                z.managed
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into())
            ));
        }
        html.push_str("</table>");
    }
    if !snap.memory.hugepages.is_empty() {
        html.push_str("<table><tr><th>页大小</th><th>nr</th><th>free</th></tr>");
        for p in &snap.memory.hugepages {
            html.push_str(&format!(
                "<tr><td>{} KiB</td><td>{}</td><td>{}</td></tr>",
                p.size_kb,
                esc(&p.nr.display()),
                esc(&p.free.display())
            ));
        }
        html.push_str("</table>");
    }

    if !snap.edac.controllers.is_empty() {
        html.push_str("<table><tr><th>EDAC</th><th>名称</th><th>CE</th><th>UE</th></tr>");
        for c in &snap.edac.controllers {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&c.name),
                esc(&c.mc_name.display()),
                esc(&c.ce_count.display()),
                esc(&c.ue_count.display())
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.edac.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "告警");
    if snap.alerts.is_empty() {
        html.push_str(
            "<p class=\"muted\">当前无越限传感器（对照 hwmon *_max / *_crit / *_min）。</p>",
        );
    } else {
        html.push_str(
            "<table><tr><th>级别</th><th>通道</th><th>值</th><th>阈值</th><th>说明</th></tr>",
        );
        for a in &snap.alerts {
            html.push_str(&format!(
                "<tr><td class=\"{}\">{}</td><td>{}</td><td>{:.3} {}</td><td>{:.3}</td><td>{}</td></tr>",
                if matches!(a.level, AlertLevel::Crit) {
                    "crit"
                } else {
                    "warn"
                },
                alert_level_label(a.level),
                esc(&a.label),
                a.value,
                esc(&a.unit),
                a.threshold,
                esc(&a.message)
            ));
        }
        html.push_str("</table>");
    }

    section(&mut html, "传感器");
    html.push_str("<table><tr><th>芯片</th><th>通道</th><th>值</th><th>min</th><th>max</th><th>crit</th><th>状态</th></tr>");
    for chip in &snap.sensors.chips {
        for ch in &chip.channels {
            let val = ch
                .value
                .map(|v| format!("{v:.3} {}", ch.unit))
                .unwrap_or_else(|| ch.raw.access_label());
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(chip.name.value.as_deref().unwrap_or("?")),
                esc(&ch.label),
                esc(&val),
                esc(&opt_f(ch.min)),
                esc(&opt_f(ch.max)),
                esc(&opt_f(ch.crit)),
                access_cell(ch.raw.access)
            ));
        }
    }
    if snap.sensors.chips.is_empty() {
        html.push_str("<tr><td colspan=\"7\" class=\"warn\">无 hwmon 数据</td></tr>");
    }
    html.push_str("</table>");

    if !snap.sensors.cooling.is_empty() {
        html.push_str("<table><tr><th>冷却</th><th>类型</th><th>状态</th></tr>");
        for c in &snap.sensors.cooling {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}/{}</td></tr>",
                esc(&c.name),
                esc(&c.r#type.display()),
                esc(&c.cur_state.display()),
                esc(&c.max_state.display())
            ));
        }
        html.push_str("</table>");
    }

    section(&mut html, "电源");
    if snap.power.supplies.is_empty() {
        html.push_str("<p class=\"muted\">无电源类设备</p>");
    }
    for s in &snap.power.supplies {
        kv(
            &mut html,
            &[
                ("名称", s.name.clone()),
                ("类型", s.kind.display()),
                ("状态", s.status.display()),
                (
                    "电量",
                    s.capacity_pct
                        .value
                        .map(|v| format!("{v}%"))
                        .unwrap_or_else(|| s.capacity_pct.access_label()),
                ),
                ("电压", s.voltage_v.display()),
            ],
        );
    }
    for n in &snap.power.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }
    html.push_str(&format!(
        "<p class=\"muted\">sleep state {} mem_sleep {} suspend ok {} fail {} wakeups {}</p>",
        esc(&snap.pm.state.display()),
        esc(&snap.pm.mem_sleep.display()),
        esc(&snap.pm.suspend_success.display()),
        esc(&snap.pm.suspend_fail.display()),
        snap.pm.wakeups
    ));
    if !snap.rapl.zones.is_empty() {
        html.push_str("<table><tr><th>RAPL</th><th>功率</th><th>限制</th><th>energy_uj</th></tr>");
        for z in &snap.rapl.zones {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(z.label.value.as_deref().unwrap_or(&z.name)),
                esc(&z
                    .power_w
                    .map(|w| format!("{w:.2} W"))
                    .unwrap_or_else(|| "n/a".into())),
                esc(&z
                    .power_limit_uw
                    .value
                    .map(|u| format!("{:.1} W", u as f64 / 1_000_000.0))
                    .unwrap_or_else(|| z.power_limit_uw.access_label())),
                esc(&z.energy_uj.display())
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.rapl.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "声卡");
    if snap.audio.cards.is_empty() {
        html.push_str("<p class=\"muted\">无 ALSA 声卡</p>");
    }
    html.push_str("<table><tr><th>#</th><th>id</th><th>名称</th></tr>");
    for c in &snap.audio.cards {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            c.index,
            esc(&c.id),
            esc(&c.name)
        ));
    }
    html.push_str("</table>");
    for n in &snap.audio.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "平台");
    html.push_str(&format!(
        "<p class=\"muted\">ACPI {} PnP {} MSR {} platform {} wakeup_sources {} workqueue {} events {}</p>",
        snap.platform.acpi_devices,
        snap.platform.pnp_devices,
        snap.platform.msr_devices,
        esc(&if snap.platform.platform_devices.is_empty() {
            "—".into()
        } else {
            snap.platform.platform_devices.join(" ")
        }),
        snap.platform.wakeup_sources,
        esc(&snap.platform.workqueues.join(" ")),
        esc(&snap.platform.event_sources.join(" "))
    ));
    for v in &snap.platform.vtconsoles {
        html.push_str(&format!(
            "<p>vt {} {} bind {}</p>",
            esc(&v.name),
            esc(&v.device.display()),
            esc(&v.bind.display())
        ));
    }
    if !snap.platform.watchdogs.is_empty() {
        html.push_str("<table><tr><th>watchdog</th><th>identity</th><th>timeout</th></tr>");
        for w in &snap.platform.watchdogs {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&w.name),
                esc(&w.identity.display()),
                esc(&w.timeout.display())
            ));
        }
        html.push_str("</table>");
    }
    for b in &snap.platform.backlights {
        html.push_str(&format!(
            "<p>backlight {} {} / {}</p>",
            esc(&b.name),
            esc(&b.actual.display()),
            esc(&b.max.display())
        ));
    }
    for a in &snap.platform.i2c_adapters {
        html.push_str(&format!(
            "<p>I2C {} {} clients {}</p>",
            esc(&a.name),
            esc(&a.adapter_name.display()),
            a.clients
        ));
    }
    for n in &snap.platform.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    if !snap.periph.dma_isa.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">/proc/dma {}</p>",
            esc(&snap
                .periph
                .dma_isa
                .iter()
                .map(|c| format!("{}:{}", c.channel, c.name))
                .collect::<Vec<_>>()
                .join(" "))
        ));
    }
    if !snap.periph.dmaengine.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">dmaengine {}</p>",
            esc(&snap
                .periph
                .dmaengine
                .iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>()
                .join(" "))
        ));
    }
    for p in &snap.periph.pwm_chips {
        html.push_str(&format!(
            "<p>pwm {} npwm {}</p>",
            esc(&p.name),
            esc(&p.npwm.display())
        ));
    }
    for i in &snap.periph.iio {
        html.push_str(&format!(
            "<p>IIO {} {}</p>",
            esc(&i.name),
            esc(&i.iio_name.display())
        ));
    }
    for n in &snap.periph.nvmem {
        html.push_str(&format!(
            "<p>nvmem {} {}</p>",
            esc(&n.name),
            esc(&n.typ.display())
        ));
    }
    for r in &snap.periph.regulators {
        html.push_str(&format!(
            "<p>regulator {} {} {} {} uV</p>",
            esc(&r.name),
            esc(&r.regulator_name.display()),
            esc(&r.state.display()),
            esc(&r.microvolts.display())
        ));
    }
    for d in &snap.periph.devlinks {
        html.push_str(&format!(
            "<p>devlink {} {}</p>",
            esc(&d.name),
            esc(&d.status.display())
        ));
    }
    for b in &snap.periph.pci_buses {
        html.push_str(&format!(
            "<p>pci_bus {} {}</p>",
            esc(&b.name),
            esc(&b.cpulist.display())
        ));
    }
    for n in &snap.periph.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }
    for r in &snap.buses.rfkill {
        html.push_str(&format!(
            "<p>rfkill {} {} state {}</p>",
            esc(&r.name),
            esc(&r.kind.display()),
            esc(&r.state.display())
        ));
    }
    for b in &snap.buses.bluetooth {
        html.push_str(&format!(
            "<p>BT {} {} {}</p>",
            esc(&b.name),
            esc(&b.dev_name.display()),
            esc(&b.address.display())
        ));
    }
    for v in &snap.buses.video {
        html.push_str(&format!(
            "<p>V4L {} {}</p>",
            esc(&v.name),
            esc(&v.dev_name.display())
        ));
    }
    for m in &snap.buses.mmc {
        html.push_str(&format!(
            "<p>MMC {} {} {}</p>",
            esc(&m.name),
            esc(&m.name_tag.display()),
            esc(&m.r#type.display())
        ));
    }
    for mei in &snap.buses.mei {
        html.push_str(&format!(
            "<p>MEI {} {}</p>",
            esc(&mei.name),
            esc(&mei.fw_status.display())
        ));
    }
    for s in &snap.buses.serial {
        html.push_str(&format!(
            "<p>serial {} irq {} uartclk {}</p>",
            esc(&s.name),
            esc(&s.irq.display()),
            esc(&s.uartclk.display())
        ));
    }
    for h in &snap.buses.hidraw {
        html.push_str(&format!(
            "<p>hidraw {} {}</p>",
            esc(&h.name),
            esc(&h.hid_name.display())
        ));
    }
    for g in &snap.buses.gpio {
        html.push_str(&format!(
            "<p>gpio {} {} ngpio {}</p>",
            esc(&g.name),
            esc(&g.label.display()),
            esc(&g.ngpio.display())
        ));
    }
    for m in &snap.buses.mtd {
        html.push_str(&format!(
            "<p>mtd {} {}</p>",
            esc(&m.name),
            esc(&m.mtd_name.display())
        ));
    }
    for ib in &snap.buses.infiniband {
        html.push_str(&format!(
            "<p>IB {} {} ports {}</p>",
            esc(&ib.name),
            esc(&ib.node_guid.display()),
            ib.ports
        ));
    }
    for (label, names) in [
        ("ieee80211", &snap.buses.ieee80211),
        ("typec", &snap.buses.typec),
        ("udc", &snap.buses.udc),
        ("dax", &snap.buses.dax),
        ("wmi", &snap.buses.wmi),
        ("spi", &snap.buses.spi),
        ("serio", &snap.buses.serio),
        ("ubi", &snap.buses.ubi),
        ("scsi_generic", &snap.buses.scsi_generic),
        ("wwan", &snap.buses.wwan),
        ("ppp", &snap.buses.ppp),
        ("phy", &snap.buses.phy),
        ("remoteproc", &snap.buses.remoteproc),
        ("extcon", &snap.buses.extcon),
        ("tee", &snap.buses.tee),
        ("mdio_bus", &snap.buses.mdio_bus),
        ("spi_master", &snap.buses.spi_master),
        ("i2c-dev", &snap.buses.i2c_dev),
        ("nvme-subsystem", &snap.buses.nvme_subsystem),
        ("w1", &snap.buses.w1),
        ("macvtap", &snap.buses.macvtap),
        ("tun", &snap.buses.tun),
        ("nvme-generic", &snap.buses.nvme_generic),
        ("nvme-fabrics", &snap.buses.nvme_fabrics),
        ("iscsi_endpoint", &snap.buses.iscsi_endpoint),
        ("iscsi_iface", &snap.buses.iscsi_iface),
        ("iscsi_connection", &snap.buses.iscsi_connection),
        ("container", &snap.buses.container),
        ("iscsi_flashnode", &snap.buses.iscsi_flashnode),
        ("nd", &snap.buses.nd),
        ("dma_heap", &snap.buses.dma_heap),
        ("cxl", &snap.buses.cxl),
        ("devfreq", &snap.buses.devfreq),
        ("fpga", &snap.buses.fpga),
        ("gnss", &snap.buses.gnss),
        ("rpmsg", &snap.buses.rpmsg),
        ("devcoredump", &snap.buses.devcoredump),
        ("scsi_disk", &snap.buses.scsi_disk),
        ("scsi_tape", &snap.buses.scsi_tape),
        ("graphics", &snap.buses.graphics),
        ("cec", &snap.buses.cec),
        ("media", &snap.buses.media),
        ("nbd", &snap.buses.nbd),
        ("vfio", &snap.buses.vfio),
        ("mdev", &snap.buses.mdev),
        ("vhost", &snap.buses.vhost),
        ("fc", &snap.buses.fc),
        ("accel", &snap.buses.accel),
        ("vdpa", &snap.buses.vdpa),
        ("uio", &snap.buses.uio),
        ("auxiliary", &snap.buses.auxiliary),
        ("usbmon", &snap.buses.usbmon),
        ("counter", &snap.buses.counter),
        ("drm_dp_aux_dev", &snap.buses.drm_dp_aux_dev),
        ("mhi", &snap.buses.mhi),
        ("ipmi", &snap.buses.ipmi),
        ("usb_role", &snap.buses.usb_role),
        ("i3c", &snap.buses.i3c),
        ("vduse", &snap.buses.vduse),
        ("mux", &snap.buses.mux),
        ("soundwire", &snap.buses.soundwire),
        ("rc", &snap.buses.rc),
        ("stm", &snap.buses.stm),
        ("peci", &snap.buses.peci),
        ("wakeup", &snap.buses.wakeup),
        ("msr", &snap.buses.msr),
        ("dpll", &snap.buses.dpll),
        ("iommu", &snap.buses.iommu),
        ("hid", &snap.buses.hid),
        ("memory", &snap.buses.memory),
        ("firewire", &snap.buses.firewire),
        ("greybus", &snap.buses.greybus),
        ("rapidio", &snap.buses.rapidio),
        ("ulpi", &snap.buses.ulpi),
        ("spmi", &snap.buses.spmi),
        ("pci_epc", &snap.buses.pci_epc),
    ] {
        if !names.is_empty() {
            html.push_str(&format!(
                "<p class=\"muted\">{} {}</p>",
                esc(label),
                esc(&names.join(" "))
            ));
        }
    }
    if !snap.buses.misc.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">misc {}</p>",
            esc(&snap.buses.misc.join(" "))
        ));
    }
    for n in &snap.buses.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "NVMe");
    if snap.nvme.controllers.is_empty() {
        html.push_str("<p class=\"warn\">未发现 NVMe 控制器</p>");
    }
    for c in &snap.nvme.controllers {
        kv(
            &mut html,
            &[
                ("名称", c.name.clone()),
                ("型号", c.model.display()),
                ("序列号", c.serial.display()),
                ("固件", c.firmware.display()),
                (
                    "SMART",
                    c.smart
                        .value
                        .as_ref()
                        .map(|s| {
                            format!(
                                "used {}%, spare {}%, temp {:?}",
                                s.percentage_used, s.available_spare_pct, s.temperature_c
                            )
                        })
                        .unwrap_or_else(|| c.smart.access_label()),
                ),
            ],
        );
    }

    section(&mut html, "PCI");
    html.push_str(
        "<table><tr><th>槽位</th><th>ID</th><th>名称</th><th>类别</th><th>驱动</th><th>链路</th></tr>",
    );
    for d in &snap.pci.devices {
        let name = match (&d.vendor_name, &d.device_name) {
            (Some(v), Some(n)) => format!("{v} {n}"),
            (Some(v), None) => v.clone(),
            _ => String::from("—"),
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}:{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&d.slot),
            esc(&d.vendor_id),
            esc(&d.device_id),
            esc(&name),
            esc(&d.class_name),
            esc(&d.driver.display()),
            esc(&d.link_label())
        ));
    }
    html.push_str("</table>");
    let sriov: Vec<_> = snap
        .pci
        .devices
        .iter()
        .filter(|d| d.sriov_totalvfs.access == AccessKind::Ok)
        .collect();
    if !sriov.is_empty() {
        html.push_str("<p>SR-IOV ");
        for d in sriov {
            html.push_str(&format!(
                "{} {}/{} ",
                esc(&d.slot),
                esc(&d.sriov_numvfs.display()),
                esc(&d.sriov_totalvfs.display())
            ));
        }
        html.push_str("</p>");
    }

    section(&mut html, "virtio");
    if snap.virtio.devices.is_empty() {
        html.push_str("<p class=\"muted\">无 virtio 设备</p>");
    } else {
        html.push_str("<table><tr><th>节点</th><th>类型</th><th>驱动</th><th>status</th></tr>");
        for d in &snap.virtio.devices {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&d.name),
                esc(&d.kind),
                esc(&d.driver.display()),
                esc(&d.status.display())
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.virtio.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "KVM");
    kv(
        &mut html,
        &[
            ("/dev/kvm", snap.kvm.device.display()),
            ("module", snap.kvm.module.display()),
            ("vendor", snap.kvm.vendor.display()),
            ("nested", snap.kvm.nested.display()),
            ("EPT", snap.kvm.ept.display()),
            ("NPT", snap.kvm.npt.display()),
            ("nx_huge_pages", snap.kvm.nx_huge_pages.display()),
        ],
    );
    for n in &snap.kvm.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "IOMMU");
    if snap.iommu.groups.is_empty() {
        html.push_str("<p class=\"muted\">无 IOMMU 分组</p>");
    } else {
        html.push_str("<table><tr><th>group</th><th>devices</th></tr>");
        for g in &snap.iommu.groups {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                esc(&g.id),
                esc(&g.devices.join(" "))
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.iommu.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "GPU");
    if snap.gpu.devices.is_empty() {
        html.push_str("<p class=\"warn\">未发现 GPU / DRM 设备</p>");
    }
    for g in &snap.gpu.devices {
        kv(
            &mut html,
            &[
                ("节点", g.id.clone()),
                ("驱动", g.driver.clone()),
                ("PCI", g.pci_slot.display()),
                (
                    "ID",
                    format!("{}:{}", g.vendor_id.display(), g.device_id.display()),
                ),
                (
                    "占用",
                    g.busy_percent
                        .value
                        .map(|v| format!("{v}%"))
                        .unwrap_or_else(|| g.busy_percent.access_label()),
                ),
                (
                    "显存",
                    match (g.vram_used_bytes.value, g.vram_total_bytes.value) {
                        (Some(u), Some(t)) => format!("{} / {}", format_bytes(u), format_bytes(t)),
                        _ => g.vram_total_bytes.access_label(),
                    },
                ),
                ("VBIOS", g.vbios.display()),
            ],
        );
        for c in &g.connectors {
            let edid = c
                .edid
                .as_ref()
                .map(|e| {
                    format!(
                        "{} {} {}x{} {}cm",
                        e.manufacturer,
                        e.name.as_deref().unwrap_or("—"),
                        e.h_active.unwrap_or(0),
                        e.v_active.unwrap_or(0),
                        e.width_cm.unwrap_or(0)
                    )
                })
                .unwrap_or_else(|| "no EDID".into());
            html.push_str(&format!(
                "<p>{} {} / {} · {}</p>",
                esc(&c.name),
                esc(&c.status.display()),
                esc(&c.enabled.display()),
                esc(&edid)
            ));
        }
    }
    for n in &snap.gpu.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "网络");
    html.push_str(
        "<table><tr><th>接口</th><th>状态</th><th>类型</th><th>速率</th><th>队列</th><th>地址</th><th>RX / TX</th></tr>",
    );
    for i in &snap.net.interfaces {
        let speed = i
            .speed_mbps
            .value
            .map(|v| format!("{v} Mb/s"))
            .unwrap_or_else(|| i.speed_mbps.access_label());
        let rx_tx = match (i.rx_bytes.value, i.tx_bytes.value) {
            (Some(r), Some(t)) => format!("{} / {}", format_bytes(r), format_bytes(t)),
            _ => i.rx_bytes.access_label(),
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>rx {} tx {}</td><td>{}</td><td>{}</td></tr>",
            esc(&i.name),
            esc(&i.operstate.display()),
            esc(&if i.wireless {
                format!("{} / Wi-Fi", i.kind)
            } else {
                i.kind.clone()
            }),
            esc(&speed),
            i.rx_queues,
            i.tx_queues,
            esc(&i.addresses.join(", ")),
            esc(&rx_tx)
        ));
    }
    if snap.net.interfaces.is_empty() {
        html.push_str("<tr><td colspan=\"7\" class=\"warn\">无网卡</td></tr>");
    }
    html.push_str("</table>");
    html.push_str(&format!(
        "<p class=\"muted\">sockstat sockets {} TCP {} TIME_WAIT {} UDP {}</p>",
        snap.net
            .sockstat
            .sockets_used
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .sockstat
            .tcp_inuse
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .sockstat
            .tcp_tw
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .sockstat
            .udp_inuse
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    ));
    html.push_str(&format!(
        "<p class=\"muted\">TCP estab {} in {} out {} retrans {} UDP {}/{} softnet proc {} drop {}</p>",
        snap.net.snmp.tcp_curr_estab.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
        snap.net.snmp.tcp_in_segs.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
        snap.net.snmp.tcp_out_segs.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
        snap.net.snmp.tcp_retrans.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
        snap.net.snmp.udp_in.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
        snap.net.snmp.udp_out.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
        snap.net.softnet.processed,
        snap.net.softnet.dropped
    ));
    html.push_str(&format!(
        "<p class=\"muted\">conntrack {} / {} cong {} allowed {} TcpExt TW {} timeout {} octets {}/{}</p>",
        snap.net.conntrack_count.display(),
        snap.net.conntrack_max.display(),
        snap.net.tcp_congestion.display(),
        snap.net.tcp_allowed_congestion.display(),
        snap.net
            .tcpext
            .timewait
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .tcpext
            .timeouts
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .tcpext
            .in_octets
            .map(format_bytes)
            .unwrap_or_else(|| "—".into()),
        snap.net
            .tcpext
            .out_octets
            .map(format_bytes)
            .unwrap_or_else(|| "—".into())
    ));
    html.push_str(&format!(
        "<p class=\"muted\">IPv6 in {} out {} octets {}/{} TCP6 {} unix {} inet6 {} ipv6_route {} fastopen {} somaxconn {} ka {} sack {} syn/synack {}/{} retries {}/{} qdisc {} budget {} rp_filter {} redirects {} tcp/udp {}/{} tcp6/udp6 {}/{} raw {} udplite {} raw6 {} udplite6 {} igmp6 {} busy_poll {} weight {} notsent {}</p>",
        snap.net
            .snmp6
            .in_receives
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .snmp6
            .out_requests
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .snmp6
            .in_octets
            .map(format_bytes)
            .unwrap_or_else(|| "—".into()),
        snap.net
            .snmp6
            .out_octets
            .map(format_bytes)
            .unwrap_or_else(|| "—".into()),
        snap.net
            .sockstat6
            .tcp_inuse
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net.unix_sockets,
        snap.net.inet6_addrs,
        snap.net.ipv6_routes,
        snap.net.tcp_fastopen.display(),
        snap.net.somaxconn.display(),
        snap.net.tcp.keepalive_time.display(),
        snap.net.tcp.sack.display(),
        snap.net.tcp.syn_retries.display(),
        snap.net.tcp.synack_retries.display(),
        snap.net.tcp.retries1.display(),
        snap.net.tcp.retries2.display(),
        snap.net.default_qdisc.display(),
        snap.net.netdev_budget.display(),
        snap.net.rp_filter.display(),
        snap.net.accept_redirects.display(),
        snap.net.tcp_socks,
        snap.net.udp_socks,
        snap.net.tcp6_socks,
        snap.net.udp6_socks,
        snap.net.raw_socks,
        snap.net.udplite_socks,
        snap.net.raw6_socks,
        snap.net.udplite6_socks,
        snap.net.igmp6_ifaces,
        snap.net.busy_poll.display(),
        snap.net.dev_weight.display(),
        snap.net.tcp.notsent_lowat.display()
    ));
    html.push_str(&format!(
        "<p class=\"muted\">tcp_mem {} udp_mem {} orphans {} dsack {} autocorking {} rps {} ipfrag {}/{} early_retrans {} no_pmtu {} fib_mp {}</p>",
        snap.net.tcp_mem.display(),
        snap.net.udp_mem.display(),
        snap.net.tcp_max_orphans.display(),
        snap.net.tcp_dsack.display(),
        snap.net.tcp_autocorking.display(),
        snap.net.rps_sock_flow_entries.display(),
        snap.net.ipfrag_high_thresh.display(),
        snap.net.ipfrag_low_thresh.display(),
        snap.net.tcp_early_retrans.display(),
        snap.net.ip_no_pmtu_disc.display(),
        snap.net.fib_multipath_hash_policy.display()
    ));
    html.push_str(&format!(
        "<p class=\"muted\">frto {} invalid_ratelimit {} min_tso {} pacing_ss {} pacing_ca {} tstamp_prequeue {} message_cost {} burst {} ip6frag_low {}</p>",
        snap.net.tcp_frto.display(),
        snap.net.tcp_invalid_ratelimit.display(),
        snap.net.tcp_min_tso_segs.display(),
        snap.net.tcp_pacing_ss_ratio.display(),
        snap.net.tcp_pacing_ca_ratio.display(),
        snap.net.netdev_tstamp_prequeue.display(),
        snap.net.message_cost.display(),
        snap.net.message_burst.display(),
        snap.net.ip6frag_low_thresh.display()
    ));
    html.push_str(&format!(
        "<p class=\"muted\">orphan_retries {} rfc1337 {} unpriv_port {} bindv6only {} ipfrag_time {} dad_tx {} ecn_fb {} nonlocal {} echo_ignore_all {} ipfrag_max_dist {} abort_ovf {} no_metrics {} challenge_ack {} dynaddr {}</p>",
        snap.net.tcp_orphan_retries.display(),
        snap.net.tcp_rfc1337.display(),
        snap.net.ip_unprivileged_port_start.display(),
        snap.net.bindv6only.display(),
        snap.net.ipfrag_time.display(),
        snap.net.ipv6_dad_transmits.display(),
        snap.net.tcp_ecn_fallback.display(),
        snap.net.ip_nonlocal_bind.display(),
        snap.net.icmp_echo_ignore_all.display(),
        snap.net.ipfrag_max_dist.display(),
        snap.net.tcp_abort_on_overflow.display(),
        snap.net.tcp_no_metrics_save.display(),
        match snap.net.tcp_challenge_ack_limit.value {
            Some(2_147_483_647) => "2147483647 不限".into(),
            _ => snap.net.tcp_challenge_ack_limit.display(),
        },
        snap.net.ip_dynaddr.display()
    ));
    html.push_str(&format!(
        "<p class=\"muted\">thin_linear {} limit_out {} comp_sack {} fwd_prio {} fib_notify {} echo_probe {} fwmark {} ndisc_notify {} early_demux {}/{} sack_delay {}ns sack_slack {}ns app_win {} tfo_blackhole {}s base_mss {} min_snd_mss {} reorder {} recovery {} max_reorder {} tso_div {} udp_demux {} syn_linear {} fwd_pmtu {} no_ssthresh {} min_rtt_wlen {} mtu_floor {} tso_rtt_log {} udp_rmem_min {} udp_wmem_min {} shrink_win {} l3mdev {} migrate_req {} reflect_tos {} rto_min {}us plb {} udp_l3mdev {} backlog_ack {} fwmark_reflect {} signed_win {} stdurg {} ulp {} plb_cong {} plb_idle {} plb_rehash {} plb_rto {}s pingpong {} retrans_collapse {} probe_int {} probe_th {} ehash {} child_ehash {} udp_hash {} autobind {} fack {} low_lat {}</p>",
        snap.net.tcp_thin_linear_timeouts.display(),
        snap.net.tcp_limit_output_bytes.display(),
        snap.net.tcp_comp_sack_nr.display(),
        snap.net.ip_forward_update_priority.display(),
        snap.net.fib_notify_on_flag_change.display(),
        snap.net.icmp_echo_enable_probe.display(),
        snap.net.tcp_fwmark_accept.display(),
        snap.net.ipv6_ndisc_notify.display(),
        snap.net.tcp_early_demux.display(),
        snap.net.ip_early_demux.display(),
        snap.net.tcp_comp_sack_delay_ns.display(),
        snap.net.tcp_comp_sack_slack_ns.display(),
        snap.net.tcp_app_win.display(),
        match snap.net.tcp_fastopen_blackhole_timeout_sec.value {
            Some(0) => "0 关".into(),
            _ => snap.net.tcp_fastopen_blackhole_timeout_sec.display(),
        },
        snap.net.tcp_base_mss.display(),
        snap.net.tcp_min_snd_mss.display(),
        snap.net.tcp_reordering.display(),
        snap.net.tcp_recovery.display(),
        snap.net.tcp_max_reordering.display(),
        snap.net.tcp_tso_win_divisor.display(),
        snap.net.udp_early_demux.display(),
        snap.net.tcp_syn_linear_timeouts.display(),
        snap.net.ip_forward_use_pmtu.display(),
        snap.net.tcp_no_ssthresh_metrics_save.display(),
        snap.net.tcp_min_rtt_wlen.display(),
        snap.net.tcp_mtu_probe_floor.display(),
        snap.net.tcp_tso_rtt_log.display(),
        snap.net.udp_rmem_min.display(),
        snap.net.udp_wmem_min.display(),
        match snap.net.tcp_shrink_window.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.tcp_shrink_window.display(),
        },
        match snap.net.tcp_l3mdev_accept.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.tcp_l3mdev_accept.display(),
        },
        match snap.net.tcp_migrate_req.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.tcp_migrate_req.display(),
        },
        match snap.net.tcp_reflect_tos.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.tcp_reflect_tos.display(),
        },
        snap.net.tcp_rto_min_us.display(),
        match snap.net.tcp_plb_enabled.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.tcp_plb_enabled.display(),
        },
        match snap.net.udp_l3mdev_accept.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.udp_l3mdev_accept.display(),
        },
        snap.net.tcp_backlog_ack_defer.display(),
        match snap.net.fwmark_reflect.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.fwmark_reflect.display(),
        },
        match snap.net.tcp_workaround_signed_windows.value.as_deref() {
            Some("0") => "0 RFC".into(),
            _ => snap.net.tcp_workaround_signed_windows.display(),
        },
        match snap.net.tcp_stdurg.value.as_deref() {
            Some("0") => "0 BSD".into(),
            _ => snap.net.tcp_stdurg.display(),
        },
        snap.net.tcp_available_ulp.display(),
        snap.net.tcp_plb_cong_thresh.display(),
        snap.net.tcp_plb_idle_rehash_rounds.display(),
        snap.net.tcp_plb_rehash_rounds.display(),
        snap.net.tcp_plb_suspend_rto_sec.display(),
        snap.net.tcp_pingpong_thresh.display(),
        match snap.net.tcp_retrans_collapse.value.as_deref() {
            Some("1") => "1 合并".into(),
            _ => snap.net.tcp_retrans_collapse.display(),
        },
        snap.net.tcp_probe_interval.display(),
        snap.net.tcp_probe_threshold.display(),
        snap.net.tcp_ehash_entries.display(),
        match snap.net.tcp_child_ehash_entries.value {
            Some(0) => "0 沿用".into(),
            _ => snap.net.tcp_child_ehash_entries.display(),
        },
        snap.net.udp_hash_entries.display(),
        match snap.net.ip_autobind_reuse.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.ip_autobind_reuse.display(),
        },
        match snap.net.tcp_fack.value.as_deref() {
            Some("0") => "0 关".into(),
            _ => snap.net.tcp_fack.display(),
        },
        match snap.net.tcp_low_latency.value.as_deref() {
            Some("0") => "0 吞吐".into(),
            _ => snap.net.tcp_low_latency.display(),
        }
    ));
    html.push_str(&format!(
        "<p class=\"muted\">accept_ra {} autoconf {} hop {} ttl {} dad {} addr_gen {} ip6frag {}/{} max_addrs {} ra_defrtr {} rs {} ct_est {} buckets {} tw {} busy_read {} icmp_ratelimit {} force_mld {} ra_pinfo {} enhanced_dad {} auto_flowlabels {} icmp_msgs {}/{} flowlabel {} idgen {} ra_mtu {} idgen_delay {} ip6frag_time {} keep_addr {} ping_group {} icmp_ratemask {} ra_min_hop {} icmp_inbound_ifaddr {} ra_min_lft {} ra_rt_min_plen {} ra_rt_max_plen {} ra_rtr_pref {} ra_from_local {} v6_redir {} drop_una {} drop_l2mcast {} force_tllao {} untracked_na {} proxy_ndp {} ndisc_tclass {} frag_ndisc {}</p>",
        snap.net.ipv6_accept_ra.display(),
        snap.net.ipv6_autoconf.display(),
        snap.net.ipv6_hop_limit.display(),
        snap.net.ip_default_ttl.display(),
        snap.net.ipv6_accept_dad.display(),
        snap.net.ipv6_addr_gen_mode.display(),
        snap.net.ip6frag_high_thresh.display(),
        snap.net.ip6frag_low_thresh.display(),
        snap.net.ipv6_max_addresses.display(),
        snap.net.ipv6_accept_ra_defrtr.display(),
        snap.net.ipv6_router_solicitations.display(),
        snap.net.conntrack_tcp_established.display(),
        snap.net.conntrack_buckets.display(),
        snap.net.tcp_max_tw_buckets.display(),
        snap.net.busy_read.display(),
        snap.net.icmp_ratelimit.display(),
        snap.net.ipv6_force_mld_version.display(),
        snap.net.ipv6_accept_ra_pinfo.display(),
        snap.net.ipv6_enhanced_dad.display(),
        snap.net.ipv6_auto_flowlabels.display(),
        snap.net.icmp_msgs_per_sec.display(),
        snap.net.icmp_msgs_burst.display(),
        snap.net.ipv6_flowlabel_consistency.display(),
        snap.net.ipv6_idgen_retries.display(),
        snap.net.ipv6_accept_ra_mtu.display(),
        snap.net.ipv6_idgen_delay.display(),
        snap.net.ipv6_ip6frag_time.display(),
        snap.net.ipv6_keep_addr_on_down.display(),
        crate::probes::net::ping_group_range_display(&snap.net.ping_group_range),
        snap.net.icmp_ratemask.display(),
        snap.net.ipv6_accept_ra_min_hop_limit.display(),
        match snap.net.icmp_errors_use_inbound_ifaddr.value.as_deref() {
            Some("0") => "0 出接口".into(),
            Some("1") => "1 入接口".into(),
            _ => snap.net.icmp_errors_use_inbound_ifaddr.display(),
        },
        snap.net.ipv6_accept_ra_min_lft.display(),
        snap.net.ipv6_accept_ra_rt_info_min_plen.display(),
        snap.net.ipv6_accept_ra_rt_info_max_plen.display(),
        match snap.net.ipv6_accept_ra_rtr_pref.value.as_deref() {
            Some("0") => "0 忽略".into(),
            Some("1") => "1 接受".into(),
            _ => snap.net.ipv6_accept_ra_rtr_pref.display(),
        },
        match snap.net.ipv6_accept_ra_from_local.value.as_deref() {
            Some("0") => "0 拒本机".into(),
            Some("1") => "1 接受".into(),
            _ => snap.net.ipv6_accept_ra_from_local.display(),
        },
        match snap.net.ipv6_accept_redirects.value.as_deref() {
            Some("0") => "0 忽略".into(),
            Some("1") => "1 接受".into(),
            _ => snap.net.ipv6_accept_redirects.display(),
        },
        match snap.net.ipv6_drop_unsolicited_na.value.as_deref() {
            Some("0") => "0 留".into(),
            Some("1") => "1 丢".into(),
            _ => snap.net.ipv6_drop_unsolicited_na.display(),
        },
        match snap.net.ipv6_drop_unicast_in_l2_multicast.value.as_deref() {
            Some("0") => "0 留".into(),
            Some("1") => "1 丢".into(),
            _ => snap.net.ipv6_drop_unicast_in_l2_multicast.display(),
        },
        match snap.net.ipv6_force_tllao.value.as_deref() {
            Some("0") => "0 关".into(),
            Some("1") => "1 强制".into(),
            _ => snap.net.ipv6_force_tllao.display(),
        },
        match snap.net.ipv6_accept_untracked_na.value.as_deref() {
            Some("0") => "0 关".into(),
            Some("1") => "1 接受".into(),
            _ => snap.net.ipv6_accept_untracked_na.display(),
        },
        match snap.net.ipv6_proxy_ndp.value.as_deref() {
            Some("0") => "0 关".into(),
            Some("1") => "1 代理".into(),
            _ => snap.net.ipv6_proxy_ndp.display(),
        },
        snap.net.ipv6_ndisc_tclass.display(),
        match snap.net.ipv6_suppress_frag_ndisc.value.as_deref() {
            Some("0") => "0 允许".into(),
            Some("1") => "1 丢分片".into(),
            _ => snap.net.ipv6_suppress_frag_ndisc.display(),
        }
    ));
    if !snap.net.rp_filter_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">rp_filter iface {}</p>",
            esc(&snap.net.rp_filter_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_use_tempaddr_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">use_tempaddr iface {}</p>",
            esc(&snap.net.ipv6_use_tempaddr_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_dad_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_dad iface {}</p>",
            esc(&snap.net.ipv6_accept_dad_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_addr_gen_mode_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">addr_gen iface {}</p>",
            esc(&snap.net.ipv6_addr_gen_mode_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_defrtr_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ra_defrtr iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_defrtr_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_router_solicitations_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">router_solicitations iface {}</p>",
            esc(&snap.net.ipv6_router_solicitations_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_dad_transmits_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">dad_transmits iface {}</p>",
            esc(&snap.net.ipv6_dad_transmits_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_ndisc_notify_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ndisc_notify iface {}</p>",
            esc(&snap.net.ipv6_ndisc_notify_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_pinfo_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_pinfo iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_pinfo_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_enhanced_dad_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">enhanced_dad iface {}</p>",
            esc(&snap.net.ipv6_enhanced_dad_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_mtu_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_mtu iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_mtu_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_keep_addr_on_down_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">keep_addr_on_down iface {}</p>",
            esc(&snap.net.ipv6_keep_addr_on_down_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_min_hop_limit_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_min_hop iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_min_hop_limit_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_min_lft_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_min_lft iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_min_lft_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_rt_info_min_plen_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_rt_info_min_plen iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_rt_info_min_plen_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_rt_info_max_plen_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_rt_info_max_plen iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_rt_info_max_plen_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_rtr_pref_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_rtr_pref iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_rtr_pref_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_ra_from_local_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">accept_ra_from_local iface {}</p>",
            esc(&snap.net.ipv6_accept_ra_from_local_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_redirects_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ipv6_accept_redirects iface {}</p>",
            esc(&snap.net.ipv6_accept_redirects_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_drop_unsolicited_na_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">drop_unsolicited_na iface {}</p>",
            esc(&snap.net.ipv6_drop_unsolicited_na_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_drop_unicast_in_l2_multicast_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">drop_unicast_l2mcast iface {}</p>",
            esc(&snap.net.ipv6_drop_unicast_in_l2_multicast_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_force_tllao_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">force_tllao iface {}</p>",
            esc(&snap.net.ipv6_force_tllao_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_accept_untracked_na_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">untracked_na iface {}</p>",
            esc(&snap.net.ipv6_accept_untracked_na_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_proxy_ndp_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">proxy_ndp iface {}</p>",
            esc(&snap.net.ipv6_proxy_ndp_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_ndisc_tclass_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ndisc_tclass iface {}</p>",
            esc(&snap.net.ipv6_ndisc_tclass_dev.join(" "))
        ));
    }
    if !snap.net.ipv6_suppress_frag_ndisc_dev.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">suppress_frag_ndisc iface {}</p>",
            esc(&snap.net.ipv6_suppress_frag_ndisc_dev.join(" "))
        ));
    }
    if !snap.net.protocols.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">protocols {}</p>",
            esc(&snap.net.protocols.join(" "))
        ));
    }
    html.push_str(&format!(
        "<p class=\"muted\">xfrm in_no_states {} out_no_states {} fib leaves {}</p>",
        snap.net
            .xfrm_in_no_states
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .xfrm_out_no_states
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into()),
        snap.net
            .fib_trie_leaves
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    ));
    if !snap.net.ptypes.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ptype {}</p>",
            esc(&snap.net.ptypes.join(" "))
        ));
    }
    html_name_list(
        &mut html,
        "iptables",
        &snap.net.iptables,
        &snap.net.notes,
        "ip_tables_names",
    );
    html_name_list(
        &mut html,
        "ip6tables",
        &snap.net.ip6tables,
        &snap.net.notes,
        "ip6_tables_names",
    );
    if !snap.net.connectors.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">connector {}</p>",
            esc(&snap.net.connectors.join(" "))
        ));
    }
    for b in &snap.net.bridges {
        html.push_str(&format!(
            "<p>bridge {} {} members {}</p>",
            esc(&b.name),
            esc(&b.bridge_id.display()),
            esc(&b.members.join(","))
        ));
    }
    for b in &snap.net.bonds {
        html.push_str(&format!(
            "<p>bond {} {} {}</p>",
            esc(&b.name),
            esc(&b.mode.display()),
            esc(&b.slaves.display())
        ));
    }
    for n in &snap.net.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "USB");
    html.push_str("<table><tr><th>节点</th><th>父</th><th>ID</th><th>产品</th><th>速度</th></tr>");
    for d in &snap.usb.devices {
        let id = format!("{}:{}", d.vendor_id.display(), d.product_id.display());
        let product = d
            .product
            .value
            .clone()
            .or_else(|| d.product_name.clone())
            .unwrap_or_else(|| d.product.access_label());
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&d.sys_name),
            esc(d.parent.as_deref().unwrap_or("—")),
            esc(&id),
            esc(&product),
            esc(&d.speed.display())
        ));
    }
    if snap.usb.devices.is_empty() {
        html.push_str("<tr><td colspan=\"5\" class=\"warn\">无 USB 设备</td></tr>");
    }
    html.push_str("</table>");
    for n in &snap.usb.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "输入设备");
    html.push_str("<table><tr><th>名称</th><th>类型</th><th>handlers</th><th>phys</th></tr>");
    for d in &snap.input.devices {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&d.name),
            esc(&d.kinds.join(", ")),
            esc(&d.handlers.join(" ")),
            esc(d.phys.as_deref().unwrap_or("—"))
        ));
    }
    if snap.input.devices.is_empty() {
        html.push_str("<tr><td colspan=\"4\" class=\"warn\">无输入设备</td></tr>");
    }
    html.push_str("</table>");
    for n in &snap.input.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "NUMA");
    html.push_str(
        "<table><tr><th>节点</th><th>CPU</th><th>内存</th><th>空闲</th><th>distance</th></tr>",
    );
    for n in &snap.numa.nodes {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            n.id,
            esc(&n.cpulist.display()),
            esc(&n
                .mem_total_kb
                .value
                .map(|v| format_bytes(v * 1024))
                .unwrap_or_else(|| n.mem_total_kb.access_label())),
            esc(&n
                .mem_free_kb
                .value
                .map(|v| format_bytes(v * 1024))
                .unwrap_or_else(|| n.mem_free_kb.access_label())),
            esc(&n.distance.display())
        ));
    }
    html.push_str("</table>");
    for n in &snap.numa.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "存储");
    html.push_str(
        "<table><tr><th>设备</th><th>类型</th><th>容量</th><th>型号</th><th>读</th><th>写</th></tr>",
    );
    for b in &snap.block.devices {
        let size = b
            .size_bytes
            .value
            .map(format_bytes)
            .unwrap_or_else(|| b.size_bytes.access_label());
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&b.name),
            esc(&b.r#type),
            esc(&size),
            esc(&b.model.display()),
            esc(&b
                .rd_bytes
                .value
                .map(format_bytes)
                .unwrap_or_else(|| b.rd_bytes.access_label())),
            esc(&b
                .wr_bytes
                .value
                .map(format_bytes)
                .unwrap_or_else(|| b.wr_bytes.access_label()))
        ));
    }
    html.push_str("</table>");
    for b in &snap.block.devices {
        html.push_str(&format!(
            "<p class=\"muted\">{} queue phys/log {}/{} nr {} cache {} discard {}</p>",
            esc(&b.name),
            esc(&b.physical_block_size.display()),
            esc(&b.logical_block_size.display()),
            esc(&b.nr_requests.display()),
            esc(&b.write_cache.display()),
            esc(&b
                .discard_max_bytes
                .value
                .map(format_bytes)
                .unwrap_or_else(|| b.discard_max_bytes.access_label()))
        ));
    }
    if !snap.ata.ports.is_empty() {
        html.push_str("<table><tr><th>ATA</th><th>链路</th><th>设备</th></tr>");
        for p in &snap.ata.ports {
            for l in &p.links {
                let devs = l
                    .devices
                    .iter()
                    .map(|d| format!("{} {} {}", d.name, d.class.display(), d.model.display()))
                    .collect::<Vec<_>>()
                    .join("; ");
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{} {}</td><td>{}</td></tr>",
                    esc(&p.name),
                    esc(&l.name),
                    esc(&l.sata_spd.display()),
                    esc(&devs)
                ));
            }
        }
        html.push_str("</table>");
    }
    for n in &snap.ata.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    if !snap.md.arrays.is_empty() {
        html.push_str("<table><tr><th>MD</th><th>级别</th><th>状态</th><th>成员</th></tr>");
        for a in &snap.md.arrays {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&a.name),
                esc(&a.level),
                esc(&a.state),
                esc(&a.members)
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.md.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    if !snap.scsi.hosts.is_empty() {
        html.push_str("<table><tr><th>SCSI</th><th>驱动</th><th>can_queue</th><th>状态</th></tr>");
        for h in &snap.scsi.hosts {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&h.name),
                esc(&h.proc_name.display()),
                esc(&h.can_queue.display()),
                esc(&h.state.display())
            ));
        }
        html.push_str("</table>");
    }
    if !snap.scsi.devices.is_empty() {
        html.push_str(
            "<table><tr><th>LUN</th><th>vendor</th><th>model</th><th>type</th><th>状态</th></tr>",
        );
        for d in &snap.scsi.devices {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&d.name),
                esc(&d.vendor.display()),
                esc(&d.model.display()),
                esc(&d.type_code.display()),
                esc(&d.state.display())
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.scsi.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    if !snap.iscsi.transports.is_empty() || !snap.iscsi.sessions.is_empty() {
        html.push_str("<table><tr><th>iSCSI</th><th>详情</th></tr>");
        for t in &snap.iscsi.transports {
            html.push_str(&format!(
                "<tr><td>transport {}</td><td>handle {}</td></tr>",
                esc(&t.name),
                esc(&t.handle.display())
            ));
        }
        for s in &snap.iscsi.sessions {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{} {}</td></tr>",
                esc(&s.name),
                esc(&s.state.display()),
                esc(&s.targetname.display())
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.iscsi.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    for b in &snap.block.devices {
        if b.partitions.is_empty() {
            continue;
        }
        html.push_str(&format!(
            "<p class=\"muted\">{}: {}</p>",
            esc(&b.name),
            esc(&b
                .partitions
                .iter()
                .map(|p| format!(
                    "{} {}",
                    p.name,
                    p.size_bytes
                        .value
                        .map(format_bytes)
                        .unwrap_or_else(|| p.size_bytes.access_label())
                ))
                .collect::<Vec<_>>()
                .join(", "))
        ));
    }
    for l in &snap.block.loops {
        html.push_str(&format!(
            "<p>loop {} {} {}</p>",
            esc(&l.name),
            esc(&l
                .size_bytes
                .value
                .map(format_bytes)
                .unwrap_or_else(|| l.size_bytes.access_label())),
            esc(&l.backing_file.display())
        ));
    }
    for d in &snap.block.mapper {
        html.push_str(&format!(
            "<p>dm {} {} {}</p>",
            esc(&d.name),
            esc(&d.mapper_name.display()),
            esc(&d.uuid.display())
        ));
    }
    if !snap.block.bdi.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">bdi {}</p>",
            esc(&snap
                .block
                .bdi
                .iter()
                .take(8)
                .map(|b| format!("{} ra {}", b.name, b.read_ahead_kb.display()))
                .collect::<Vec<_>>()
                .join(", "))
        ));
    }
    for n in &snap.block.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }

    section(&mut html, "文件系统");
    html.push_str(&format!(
        "<p class=\"muted\">nfsd {} volumes {} fuse {}</p>",
        esc(&snap.fs.nfsd_threads.display()),
        snap.fs.nfs_volumes,
        snap.fs.fuse_conns
    ));
    html.push_str(
        "<table><tr><th>挂载点</th><th>fstype</th><th>源</th><th>kind</th><th>用量</th></tr>",
    );
    for m in snap.fs.mounts.iter().filter(|m| m.kind != "virtual") {
        let usage = match (m.used_bytes, m.total_bytes) {
            (Some(u), Some(t)) => format!("{} / {}", format_bytes(u), format_bytes(t)),
            _ => "—".into(),
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&m.target),
            esc(&m.fstype),
            esc(&m.source),
            m.kind,
            esc(&usage)
        ));
    }
    html.push_str("</table>");
    for s in &snap.fs.swaps {
        html.push_str(&format!(
            "<p>swap {} {} {} / {}</p>",
            esc(&s.filename),
            esc(&s.kind),
            esc(&format_bytes(s.used_kb * 1024)),
            esc(&format_bytes(s.size_kb * 1024))
        ));
    }
    for e in &snap.fs.ext4 {
        html.push_str(&format!(
            "<p>ext4 {} lifetime {} errors {}</p>",
            esc(&e.name),
            esc(&e
                .lifetime_write_kbytes
                .value
                .map(|v| format_bytes(v * 1024))
                .unwrap_or_else(|| e.lifetime_write_kbytes.access_label())),
            esc(&e.errors_count.display())
        ));
    }

    section(&mut html, "系统");
    kv(
        &mut html,
        &[
            ("操作系统", snap.software.os_name.display()),
            ("ID", snap.software.os_id.display()),
            ("ID_LIKE", snap.software.os_like.display()),
            ("VERSION_ID", snap.software.os_version.display()),
            ("内核", snap.software.kernel_release.display()),
            ("ostype", snap.software.ostype.display()),
            ("主机名", snap.software.hostname.display()),
            (
                "内存",
                snap.software
                    .mem_total_kb
                    .value
                    .map(|v| format_bytes(v * 1024))
                    .unwrap_or_else(|| snap.software.mem_total_kb.access_label()),
            ),
            (
                "loadavg",
                format!(
                    "{} {} {}",
                    snap.software.load_1.display(),
                    snap.software.load_5.display(),
                    snap.software.load_15.display()
                ),
            ),
            ("clocksource", snap.clock.current.display()),
            ("available_clocksource", snap.clock.available.display()),
            (
                "clockevents",
                if snap.clock.clockevents.is_empty() {
                    "—".into()
                } else {
                    snap.clock.clockevents.join(" ")
                },
            ),
            (
                "PPS",
                if snap.clock.pps.is_empty() {
                    "—".into()
                } else {
                    snap.clock
                        .pps
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>()
                        .join(" ")
                },
            ),
            (
                "PTP",
                if snap.clock.ptps.is_empty() {
                    "—".into()
                } else {
                    snap.clock
                        .ptps
                        .iter()
                        .map(|p| {
                            format!(
                                "{} {}",
                                p.name,
                                p.clock_name.display()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            ),
            ("模块数", snap.modules.modules.len().to_string()),
            (
                "tainted",
                match snap.software.tainted.value {
                    Some(0) => "0".into(),
                    Some(v) => format!("{} ({})", v, snap.software.taint_flags.join(", ")),
                    None => snap.software.tainted.access_label(),
                },
            ),
            ("LSM", snap.software.lsm.display()),
            ("lockdown", snap.security.lockdown.display()),
            (
                "yama/kptr/dmesg",
                format!(
                    "ptrace {} kptr {} dmesg {}",
                    snap.security.ptrace_scope.display(),
                    snap.security.kptr_restrict.display(),
                    snap.security.dmesg_restrict.display()
                ),
            ),
            (
                "fs.protected",
                format!(
                    "hardlinks {} symlinks {} fifos {} regular {} seccomp {}",
                    snap.security.protected_hardlinks.display(),
                    snap.security.protected_symlinks.display(),
                    snap.security.protected_fifos.display(),
                    snap.security.protected_regular.display(),
                    snap.security.seccomp_actions_avail.display()
                ),
            ),
            (
                "crypto",
                format!(
                    "{} algs ({} internal)",
                    snap.crypto.total, snap.crypto.internal
                ),
            ),
            (
                "namespaces",
                if snap.ns.self_ns.is_empty() {
                    "—".into()
                } else {
                    snap.ns
                        .self_ns
                        .iter()
                        .map(|n| n.kind.clone())
                        .collect::<Vec<_>>()
                        .join(" ")
                },
            ),
            ("max_user_namespaces", snap.ns.max_user.display()),
            ("entropy", snap.software.entropy_avail.display()),
            ("boot_id", snap.sysctl.boot_id.display()),
            ("machine-id", snap.software.machine_id.display()),
            ("config.gz", snap.software.config_gz.display()),
            (
                "arch",
                format!(
                    "{} {} {}-bit profiling {}",
                    snap.sysctl.kernel_arch.display(),
                    snap.software.cpu_byteorder.display(),
                    snap.software.address_bits.display(),
                    snap.software.profiling.display()
                ),
            ),
            ("file locks", snap.software.file_locks.to_string()),
            (
                "oops / kexec",
                format!(
                    "oops {} warn {} kexec {} fscaps {}",
                    snap.software.oops_count.display(),
                    snap.software.warn_count.display(),
                    snap.software.kexec_loaded.display(),
                    snap.software.fscaps.display()
                ),
            ),
            (
                "filesystems",
                if snap.software.filesystems.is_empty() {
                    "—".into()
                } else {
                    snap.software.filesystems.join(" ")
                },
            ),
            (
                "nmi/watchdog",
                format!(
                    "nmi {} wd {} thresh {} unknown_nmi_panic {} panic {} sysrq {} min_free {} hung {} core_pipe {} printk_devkmsg {} delayacct {} acct {} mount_max {} rng_wake {} urandom_reseed {} soft_wd {} wd_mask {} rcu_stall {} warn {} kexec_limit {} split_lock {} hung_warn {} hung_check {} hung_interval {} kexec_reboot {} rcu_stall_max {} panic_print {} io_nmi {} hung_bt {} unrecovered_nmi {} oops_bt {} hardlockup_bt {} fatal_signals {} softlockup_bt {}",
                    snap.sysctl.nmi_watchdog.display(),
                    snap.sysctl.watchdog.display(),
                    snap.sysctl.watchdog_thresh.display(),
                    snap.sysctl.unknown_nmi_panic.display(),
                    snap.sysctl.panic.display(),
                    snap.sysctl.sysrq.display(),
                    snap.sysctl.min_free_kbytes.display(),
                    snap.sysctl.hung_task_timeout_secs.display(),
                    snap.sysctl.core_pipe_limit.display(),
                    snap.sysctl.printk_devkmsg.display(),
                    snap.sysctl.task_delayacct.display(),
                    snap.sysctl.acct.display(),
                    snap.sysctl.mount_max.display(),
                    snap.sysctl.write_wakeup_threshold.display(),
                    snap.sysctl.urandom_min_reseed_secs.display(),
                    snap.sysctl.soft_watchdog.display(),
                    snap.sysctl.watchdog_cpumask.display(),
                    snap.sysctl.panic_on_rcu_stall.display(),
                    match snap.sysctl.warn_limit.value {
                        Some(0) => "0 不限".into(),
                        _ => snap.sysctl.warn_limit.display(),
                    },
                    match snap.sysctl.kexec_load_limit_panic.value {
                        Some(-1) => "-1 不限".into(),
                        _ => snap.sysctl.kexec_load_limit_panic.display(),
                    },
                    snap.sysctl.split_lock_mitigate.display(),
                    snap.sysctl.hung_task_warnings.display(),
                    snap.sysctl.hung_task_check_count.display(),
                    match snap.sysctl.hung_task_check_interval_secs.value {
                        Some(0) => "0 用 timeout".into(),
                        _ => snap.sysctl.hung_task_check_interval_secs.display(),
                    },
                    match snap.sysctl.kexec_load_limit_reboot.value {
                        Some(-1) => "-1 不限".into(),
                        _ => snap.sysctl.kexec_load_limit_reboot.display(),
                    },
                    match snap.sysctl.max_rcu_stall_to_panic.value {
                        Some(0) => "0 不升级".into(),
                        _ => snap.sysctl.max_rcu_stall_to_panic.display(),
                    },
                    snap.sysctl.panic_print.display(),
                    snap.sysctl.panic_on_io_nmi.display(),
                    snap.sysctl.hung_task_all_cpu_backtrace.display(),
                    snap.sysctl.panic_on_unrecovered_nmi.display(),
                    snap.sysctl.oops_all_cpu_backtrace.display(),
                    snap.sysctl.hardlockup_all_cpu_backtrace.display(),
                    snap.sysctl.print_fatal_signals.display(),
                    snap.sysctl.softlockup_all_cpu_backtrace.display()
                ),
            ),
            (
                "sched / oom",
                format!(
                    "rt {}/{}us rr {}ms numa {} numa_promote {}MBps tmig {} panic_oom {} oom_alloc {} laptop {} kexec_off {} hung_panic {} dl {}/{}us",
                    snap.sysctl.sched_rt_runtime_us.display(),
                    snap.sysctl.sched_rt_period_us.display(),
                    snap.sysctl.sched_rr_timeslice_ms.display(),
                    snap.sysctl.numa_balancing.display(),
                    snap.sysctl.numa_balancing_promote_rate_limit_mbps.display(),
                    snap.sysctl.timer_migration.display(),
                    snap.sysctl.panic_on_oom.display(),
                    snap.sysctl.oom_kill_allocating_task.display(),
                    snap.sysctl.laptop_mode.display(),
                    snap.sysctl.kexec_load_disabled.display(),
                    snap.sysctl.hung_task_panic.display(),
                    snap.sysctl.sched_deadline_period_min_us.display(),
                    snap.sysctl.sched_deadline_period_max_us.display()
                ),
            ),
            (
                "printk / cfs / uffd",
                format!(
                    "ratelimit {}/{} cfs {}us oops_limit {} hard {} soft {} oom_dump {} user_reserve {} uffd {} ngroups {} bpf_stats {} core_sort_vma {} io_delay {} printk_delay {} lock_depth {}",
                    snap.sysctl.printk_ratelimit.display(),
                    snap.sysctl.printk_ratelimit_burst.display(),
                    snap.sysctl.sched_cfs_bandwidth_slice_us.display(),
                    snap.sysctl.oops_limit.display(),
                    snap.sysctl.hardlockup_panic.display(),
                    snap.sysctl.softlockup_panic.display(),
                    snap.sysctl.oom_dump_tasks.display(),
                    snap.sysctl.user_reserve_kbytes.display(),
                    snap.sysctl.unprivileged_userfaultfd.display(),
                    snap.sysctl.ngroups_max.display(),
                    snap.sysctl.bpf_stats_enabled.display(),
                    snap.sysctl.core_sort_vma.display(),
                    snap.sysctl.io_delay_type.display(),
                    match snap.sysctl.printk_delay.value {
                        Some(0) => "0 无延迟".into(),
                        _ => snap.sysctl.printk_delay.display(),
                    },
                    snap.sysctl.max_lock_depth.display()
                ),
            ),
            (
                "keys / dumpable",
                format!(
                    "maxkeys {} maxbytes {} gc {} key-users {} cap_last {} dumpable {} autogroup {} cad {}",
                    snap.sysctl.keys_maxkeys.display(),
                    snap.sysctl.keys_maxbytes.display(),
                    snap.sysctl.keys_gc_delay.display(),
                    snap.sysctl.key_users.display(),
                    snap.sysctl.cap_last_cap.display(),
                    snap.sysctl.suid_dumpable.display(),
                    snap.sysctl.sched_autogroup.display(),
                    snap.sysctl.ctrl_alt_del.display()
                ),
            ),
            (
                "ipc",
                format!(
                    "shmmax {} shmall {} shmmni {} msgmax {} msgmnb {} msgmni {} auto_msgmni {} mqueue {} sysvipc {}/{}/{} shm_rmid_forced {}",
                    snap.sysctl.shmmax.display(),
                    snap.sysctl.shmall.display(),
                    snap.sysctl.shmmni.display(),
                    snap.sysctl.msgmax.display(),
                    snap.sysctl.msgmnb.display(),
                    snap.sysctl.msgmni.display(),
                    match snap.sysctl.auto_msgmni.value.as_deref() {
                        Some("0") => "0 关".into(),
                        _ => snap.sysctl.auto_msgmni.display(),
                    },
                    snap.sysctl.mqueue_queues_max.display(),
                    snap.sysctl.sysvipc_shm,
                    snap.sysctl.sysvipc_sem,
                    snap.sysctl.sysvipc_msg,
                    snap.sysctl.shm_rmid_forced.display()
                ),
            ),
            (
                "aio/inotify",
                format!(
                    "{} / {} watches {} dentry {}/{} inode {}/{} pty {}/{} overflowuid {} overflowgid {} vsyscall32 {} ldisc {} io_uring {}/{}",
                    snap.sysctl.aio_nr.display(),
                    snap.sysctl.aio_max_nr.display(),
                    snap.sysctl.inotify_max_user_watches.display(),
                    snap.sysctl.dentry_nr.display(),
                    snap.sysctl.dentry_unused.display(),
                    snap.sysctl.inode_inuse.display(),
                    snap.sysctl.inode_free.display(),
                    snap.sysctl.pty_max.display(),
                    snap.sysctl.pty_nr.display(),
                    snap.sysctl.overflowuid.display(),
                    snap.sysctl.overflowgid.display(),
                    snap.sysctl.vsyscall32.display(),
                    snap.sysctl.ldisc_autoload.display(),
                    snap.sysctl.io_uring_disabled.display(),
                    snap.sysctl.io_uring_group.display()
                ),
            ),
            (
                "bpf/perf",
                format!(
                    "unpriv_bpf {} jit {}/{} binfmt {} perf {} sample_rate {} mlock_kb {} max_stack {} ctx_stack {} cpu% {}",
                    snap.security.unprivileged_bpf_disabled.display(),
                    snap.security.bpf_jit_enable.display(),
                    snap.security.bpf_jit_harden.display(),
                    snap.security.binfmt_misc_status.display(),
                    snap.security.perf_event_paranoid.display(),
                    snap.sysctl.perf_event_max_sample_rate.display(),
                    snap.sysctl.perf_event_mlock_kb.display(),
                    snap.sysctl.perf_event_max_stack.display(),
                    snap.sysctl.perf_event_max_contexts_per_stack.display(),
                    snap.sysctl.perf_cpu_time_max_percent.display()
                ),
            ),
            (
                "file-nr",
                format!(
                    "{} / {}",
                    snap.sysctl.file_nr_alloc.display(),
                    snap.sysctl.file_nr_max.display()
                ),
            ),
            ("pid_max", snap.sysctl.pid_max.display()),
            (
                "vm",
                format!(
                    "swappiness {} overcommit {} overcommit_kbytes {} dirty_bytes {}/{} watermark {} boost {} pipe_pages {}/{} compact_unevict {} zone_reclaim {} dirty_expire {} dirtytime {} memfd_noexec {} compact_proact {} page_lock {} min_slab {} min_unmapped {} extfrag {} stat_interval {} hugetlb_vmemmap {} percpu_high {} numa_stat {} legacy_va {} hugetlb_shm {} core_note {} zonelist {} lowmem_reserve {} nr_overcommit_hp {} hugepages_mempolicy {} nr_hugepages {} acpi_video {}",
                    snap.sysctl.swappiness.display(),
                    snap.sysctl.overcommit_memory.display(),
                    snap.sysctl.overcommit_kbytes.display(),
                    snap.sysctl.dirty_bytes.display(),
                    snap.sysctl.dirty_background_bytes.display(),
                    snap.sysctl.watermark_scale_factor.display(),
                    snap.sysctl.watermark_boost_factor.display(),
                    snap.sysctl.pipe_user_pages_soft.display(),
                    snap.sysctl.pipe_user_pages_hard.display(),
                    snap.sysctl.compact_unevictable_allowed.display(),
                    snap.sysctl.zone_reclaim_mode.display(),
                    snap.sysctl.dirty_expire_centisecs.display(),
                    snap.sysctl.dirtytime_expire_seconds.display(),
                    snap.sysctl.memfd_noexec.display(),
                    snap.sysctl.compaction_proactiveness.display(),
                    snap.sysctl.page_lock_unfairness.display(),
                    snap.sysctl.min_slab_ratio.display(),
                    snap.sysctl.min_unmapped_ratio.display(),
                    snap.sysctl.extfrag_threshold.display(),
                    snap.sysctl.stat_interval.display(),
                    match snap.sysctl.hugetlb_optimize_vmemmap.value.as_deref() {
                        Some("0") => "0 关".into(),
                        _ => snap.sysctl.hugetlb_optimize_vmemmap.display(),
                    },
                    match snap.sysctl.percpu_pagelist_high_fraction.value {
                        Some(0) => "0 默认".into(),
                        _ => snap.sysctl.percpu_pagelist_high_fraction.display(),
                    },
                    snap.sysctl.numa_stat.display(),
                    match snap.sysctl.legacy_va_layout.value.as_deref() {
                        Some("0") => "0 新布局".into(),
                        _ => snap.sysctl.legacy_va_layout.display(),
                    },
                    match snap.sysctl.hugetlb_shm_group.value {
                        Some(0) => "0 无".into(),
                        _ => snap.sysctl.hugetlb_shm_group.display(),
                    },
                    snap.sysctl.core_file_note_size_limit.display(),
                    snap.sysctl.numa_zonelist_order.display(),
                    snap.sysctl.lowmem_reserve_ratio.display(),
                    snap.sysctl.nr_overcommit_hugepages.display(),
                    match snap.sysctl.nr_hugepages_mempolicy.value {
                        Some(0) => "0".into(),
                        _ => snap.sysctl.nr_hugepages_mempolicy.display(),
                    },
                    match snap.sysctl.nr_hugepages.value {
                        Some(0) => "0".into(),
                        _ => snap.sysctl.nr_hugepages.display(),
                    },
                    match snap.sysctl.acpi_video_flags.value {
                        Some(0) => "0".into(),
                        _ => snap.sysctl.acpi_video_flags.display(),
                    }
                ),
            ),
            (
                "bootloader",
                format!(
                    "arch {} type {} version {} firmware_sysfs {}/{} real_root {} schedstats {} traceoff_warn {}",
                    snap.sysctl.kernel_arch.display(),
                    snap.sysctl.bootloader_type.display(),
                    snap.sysctl.bootloader_version.display(),
                    match snap.sysctl.firmware_force_sysfs_fallback.value.as_deref() {
                        Some("0") => "0".into(),
                        _ => snap.sysctl.firmware_force_sysfs_fallback.display(),
                    },
                    match snap.sysctl.firmware_ignore_sysfs_fallback.value.as_deref() {
                        Some("0") => "0".into(),
                        _ => snap.sysctl.firmware_ignore_sysfs_fallback.display(),
                    },
                    match snap.sysctl.real_root_dev.value {
                        Some(0) => "0".into(),
                        _ => snap.sysctl.real_root_dev.display(),
                    },
                    match snap.sysctl.sched_schedstats.value.as_deref() {
                        Some("0") => "0 关".into(),
                        _ => snap.sysctl.sched_schedstats.display(),
                    },
                    match snap.sysctl.traceoff_on_warning.value.as_deref() {
                        Some("0") => "0 关".into(),
                        _ => snap.sysctl.traceoff_on_warning.display(),
                    }
                ),
            ),
            (
                "cgroup",
                format!(
                    "{} v1 {}",
                    snap.cgroup.controllers.display(),
                    if snap.cgroup.v1_enabled.is_empty() {
                        "—".into()
                    } else {
                        snap.cgroup.v1_enabled.join(" ")
                    }
                ),
            ),
            (
                "consoles",
                if snap.sysctl.consoles.is_empty() {
                    "—".into()
                } else {
                    snap.sysctl
                        .consoles
                        .iter()
                        .map(|c| c.name.clone())
                        .collect::<Vec<_>>()
                        .join(" ")
                },
            ),
        ],
    );
    for n in &snap.software.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }
    for n in &snap.clock.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }
    if let Some(cpu) = &snap.psi.cpu {
        html.push_str(&format!(
            "<p>PSI cpu some avg10={:.2} memory={} io={}</p>",
            cpu.some.avg10,
            snap.psi
                .memory
                .as_ref()
                .map(|m| format!("{:.2}", m.some.avg10))
                .unwrap_or_else(|| "n/a".into()),
            snap.psi
                .io
                .as_ref()
                .map(|m| format!("{:.2}", m.some.avg10))
                .unwrap_or_else(|| "n/a".into()),
        ));
    }
    if snap.irq.sysfs_irqs > 0 {
        html.push_str(&format!(
            "<p class=\"muted\">sysfs irq {}</p>",
            snap.irq.sysfs_irqs
        ));
    }
    for n in &snap.irq.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }
    if !snap.irq.lines.is_empty() {
        html.push_str("<table><tr><th>IRQ</th><th>合计</th><th>affinity</th><th>说明</th></tr>");
        for l in snap.irq.lines.iter().take(12) {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&l.irq),
                l.total,
                esc(l.affinity.value.as_deref().unwrap_or("—")),
                esc(&l.extra)
            ));
        }
        html.push_str("</table>");
    }
    if !snap.irq.softirqs.is_empty() {
        html.push_str("<table><tr><th>softirq</th><th>合计</th></tr>");
        for l in snap.irq.softirqs.iter().take(8) {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                esc(&l.irq),
                l.total
            ));
        }
        html.push_str("</table>");
    }
    if !snap.iomem.summaries.is_empty() {
        html.push_str("<table><tr><th>iomem</th><th>段数</th><th>大小</th></tr>");
        for s in snap.iomem.summaries.iter().take(16) {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&s.name),
                s.count,
                esc(&format_bytes(s.size))
            ));
        }
        html.push_str("</table>");
    }
    for n in &snap.iomem.notes {
        html.push_str(&format!("<p class=\"muted\">{}</p>", esc(n)));
    }
    if !snap.iomem.ioports.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ioports {}</p>",
            esc(&snap
                .iomem
                .ioports
                .iter()
                .take(16)
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(" "))
        ));
    }

    html.push_str("</body></html>");
    html
}

fn section(html: &mut String, title: &str) {
    html.push_str(&format!("<h2>{}</h2>", esc(title)));
}

fn kv(html: &mut String, rows: &[(&str, String)]) {
    html.push_str("<table>");
    for (k, v) in rows {
        html.push_str(&format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v)));
    }
    html.push_str("</table>");
}

fn access_cell(k: AccessKind) -> &'static str {
    match k {
        AccessKind::Ok => "ok",
        AccessKind::PermissionDenied => "权限不足",
        AccessKind::NotFound => "不存在",
        AccessKind::Unsupported => "不支持",
        AccessKind::Error => "错误",
    }
}

fn html_name_list(
    html: &mut String,
    label: &str,
    names: &[String],
    notes: &[String],
    path_frag: &str,
) {
    if !names.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">{} {}</p>",
            esc(label),
            esc(&names.join(" "))
        ));
    } else if let Some(n) = notes.iter().find(|s| s.contains(path_frag)) {
        html.push_str(&format!("<p class=\"warn\">{} {}</p>", esc(label), esc(n)));
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn alert_level_label(level: AlertLevel) -> &'static str {
    match level {
        AlertLevel::Ok => "ok",
        AlertLevel::Low => "low",
        AlertLevel::High => "high",
        AlertLevel::Crit => "crit",
    }
}

fn opt_f(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3}")).unwrap_or_else(|| "—".into())
}

fn html_dmi_memory(html: &mut String, snap: &HardwareSnapshot) {
    if !snap.dmi.memory_arrays.is_empty() {
        html.push_str("<p class=\"muted\">SMBIOS Type 16 物理内存阵列</p>");
        html.push_str("<table><tr><th>位置</th><th>ECC</th><th>最大容量</th><th>槽位</th></tr>");
        for a in &snap.dmi.memory_arrays {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(a.location.as_deref().unwrap_or("—")),
                esc(a.ecc.as_deref().unwrap_or("—")),
                a.max_capacity_mb
                    .map(|n| format!("{n} MB"))
                    .unwrap_or_else(|| "—".into()),
                a.devices
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into())
            ));
        }
        html.push_str("</table>");
    }
    if !snap.dmi.memory_devices.is_empty() {
        html.push_str("<p class=\"muted\">SMBIOS Type 17 / SPD-like</p>");
        html.push_str("<table><tr><th>槽位</th><th>Bank</th><th>容量</th><th>类型</th><th>外形</th><th>速度</th><th>配置</th><th>数据/总宽</th><th>Rank</th><th>厂商</th><th>序列号</th><th>料号</th></tr>");
        for m in &snap.dmi.memory_devices {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(m.locator.as_deref().unwrap_or("—")),
                esc(m.bank.as_deref().unwrap_or("—")),
                m.size_label(),
                esc(m.r#type.as_deref().unwrap_or("—")),
                esc(m.form_factor.as_deref().unwrap_or("—")),
                m.speed_mts
                    .map(|n| format!("{n} MT/s"))
                    .unwrap_or_else(|| "—".into()),
                m.configured_mts
                    .map(|n| format!("{n} MT/s"))
                    .unwrap_or_else(|| "—".into()),
                match (m.data_width, m.total_width) {
                    (Some(d), Some(t)) => format!("{d}/{t}"),
                    (Some(d), None) => d.to_string(),
                    (None, Some(t)) => format!("/{t}"),
                    _ => "—".into(),
                },
                m.rank
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into()),
                esc(m.manufacturer.as_deref().unwrap_or("—")),
                esc(m.serial.as_deref().unwrap_or("—")),
                esc(m.part.as_deref().unwrap_or("—"))
            ));
        }
        html.push_str("</table>");
    }
}

fn html_dmi_board(html: &mut String, snap: &HardwareSnapshot) {
    if !snap.dmi.processors.is_empty() {
        html.push_str("<p class=\"muted\">SMBIOS Type 4 处理器</p>");
        html.push_str("<table><tr><th>插座</th><th>厂商</th><th>型号</th><th>最大</th><th>当前</th><th>核心</th><th>线程</th><th>状态</th></tr>");
        for p in &snap.dmi.processors {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(p.socket.as_deref().unwrap_or("—")),
                esc(p.manufacturer.as_deref().unwrap_or("—")),
                esc(p.version.as_deref().unwrap_or("—")),
                p.max_mhz.map(|n| format!("{n} MHz")).unwrap_or_else(|| "—".into()),
                p.current_mhz.map(|n| format!("{n} MHz")).unwrap_or_else(|| "—".into()),
                p.cores.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                p.threads.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                if !p.populated {
                    "empty"
                } else if p.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }
        html.push_str("</table>");
    }
    if !snap.dmi.caches.is_empty() {
        html.push_str("<p class=\"muted\">SMBIOS Type 7 缓存</p>");
        html.push_str("<table><tr><th>名称</th><th>级</th><th>类型</th><th>大小</th><th>相联</th></tr>");
        for c in &snap.dmi.caches {
            html.push_str(&format!(
                "<tr><td>{}</td><td>L{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(c.socket.as_deref().unwrap_or("—")),
                c.level.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                esc(c.kind.as_deref().unwrap_or("—")),
                c.size_kb
                    .map(|n| format!("{n} KiB"))
                    .unwrap_or_else(|| "—".into()),
                esc(c.associativity.as_deref().unwrap_or("—"))
            ));
        }
        html.push_str("</table>");
    }
    if !snap.dmi.slots.is_empty() {
        html.push_str("<p class=\"muted\">SMBIOS Type 9 系统插槽</p>");
        html.push_str("<table><tr><th>名称</th><th>类型</th><th>状态</th><th>总线</th></tr>");
        for s in &snap.dmi.slots {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(s.designation.as_deref().unwrap_or("—")),
                esc(s.kind.as_deref().unwrap_or("—")),
                esc(s.usage.as_deref().unwrap_or("—")),
                esc(s.bus.as_deref().unwrap_or("—"))
            ));
        }
        html.push_str("</table>");
    }
}

fn kb_html(s: &crate::Sample<u64>) -> String {
    s.value
        .map(|v| format_bytes(v * 1024))
        .unwrap_or_else(|| s.access_label())
}

/// 字节/秒，用于网卡差分速率。
pub fn format_bps(bps: f64) -> String {
    const UNITS: [&str; 5] = ["B/s", "KiB/s", "MiB/s", "GiB/s", "TiB/s"];
    let mut v = bps.abs();
    let mut i = 0;
    while v >= 1024.0 && i + 1 < UNITS.len() {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.2} {}", UNITS[i])
}

pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i + 1 < UNITS.len() {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.2} {}", UNITS[i])
}

struct ReportSection {
    title: String,
    rows: Vec<(String, String)>,
}

fn pair(k: impl Into<String>, v: impl Into<String>) -> (String, String) {
    (k.into(), v.into())
}

fn report_sections(snap: &HardwareSnapshot) -> Vec<ReportSection> {
    let mut sections = Vec::new();

    let mut cpu = vec![
        pair("型号", snap.cpu.model_name.compact()),
        pair("厂商", snap.cpu.vendor.compact()),
        pair("逻辑 CPU", snap.cpu.logical_cpus.to_string()),
        pair("封装数", snap.cpu.physical_packages.to_string()),
        pair(
            "利用率",
            snap.cpu
                .utilization_pct
                .map(|v| format!("{v:.1}%"))
                .unwrap_or_else(|| "n/a".into()),
        ),
        pair(
            "SMT",
            format!(
                "active {} control {}",
                snap.cpu.smt_active.compact(),
                snap.cpu.smt_control.compact()
            ),
        ),
        pair("microcode", snap.cpu.microcode.compact()),
    ];
    for p in snap.dmi.processors.iter().take(8) {
        cpu.push(pair(
            p.socket.as_deref().unwrap_or("CPU"),
            format!(
                "{}  {}  max {} MHz  cores {}  threads {}  {}",
                p.manufacturer.as_deref().unwrap_or("—"),
                p.version.as_deref().unwrap_or("—"),
                p.max_mhz.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                p.cores.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                p.threads.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                if !p.populated {
                    "empty"
                } else if p.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
        ));
    }
    for c in snap.dmi.caches.iter().take(8) {
        cpu.push(pair(
            c.socket.as_deref().unwrap_or("cache"),
            format!(
                "L{} {}  {} KiB  {}",
                c.level.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                c.kind.as_deref().unwrap_or("—"),
                c.size_kb.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                c.associativity.as_deref().unwrap_or("—")
            ),
        ));
    }
    sections.push(ReportSection {
        title: "CPU".into(),
        rows: cpu,
    });

    let mut dmi = vec![
        pair(
            "BIOS",
            format!(
                "{} {}  rom {}  rel {}",
                snap.dmi.bios_vendor.compact(),
                snap.dmi.bios_version.compact(),
                snap.dmi
                    .bios_rom_kb
                    .map(|n| format!("{n} KiB"))
                    .unwrap_or_else(|| "—".into()),
                snap.dmi.bios_release.as_deref().unwrap_or("—")
            ),
        ),
        pair("厂商", snap.dmi.sys_vendor.compact()),
        pair("产品", snap.dmi.product_name.compact()),
        pair(
            "主板",
            format!(
                "{} {}",
                snap.dmi.board_vendor.compact(),
                snap.dmi.board_name.compact()
            ),
        ),
        pair("序列号", snap.dmi.product_serial.compact()),
        pair(
            "固件",
            format!(
                "{}  Secure Boot {}",
                snap.firmware.interface.compact(),
                snap.firmware.secure_boot.compact()
            ),
        ),
    ];
    for m in snap.dmi.memory_devices.iter().take(16) {
        dmi.push(pair(
            m.locator.as_deref().unwrap_or("DIMM"),
            format!(
                "{}  {}  {} MT/s  rank {}",
                m.size_label(),
                m.r#type.as_deref().unwrap_or("—"),
                m.speed_mts
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into()),
                m.rank.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
            ),
        ));
    }
    for s in snap.dmi.slots.iter().take(16) {
        dmi.push(pair(
            s.designation.as_deref().unwrap_or("slot"),
            format!(
                "{}  {}  {}",
                s.kind.as_deref().unwrap_or("—"),
                s.usage.as_deref().unwrap_or("—"),
                s.bus.as_deref().unwrap_or("—")
            ),
        ));
    }
    sections.push(ReportSection {
        title: "DMI / 主板".into(),
        rows: dmi,
    });

    sections.push(ReportSection {
        title: "内存".into(),
        rows: vec![
            pair("物理", kb_html(&snap.memory.total_kb)),
            pair("可用", kb_html(&snap.memory.available_kb)),
            pair("空闲", kb_html(&snap.memory.free_kb)),
            pair(
                "Swap",
                format!(
                    "{} / {}",
                    kb_html(&snap.memory.swap_total_kb),
                    kb_html(&snap.memory.swap_free_kb)
                ),
            ),
            pair("THP", snap.memory.thp_enabled.compact()),
        ],
    });

    let mut gpu = Vec::new();
    if snap.gpu.devices.is_empty() {
        gpu.push(pair("GPU", "未发现 DRM 设备"));
    }
    for g in snap.gpu.devices.iter().take(8) {
        gpu.push(pair(
            &g.id,
            format!(
                "{}  PCI {}  {}%  VBIOS {}",
                g.driver,
                g.pci_slot.compact(),
                g.busy_percent
                    .value
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| g.busy_percent.compact()),
                g.vbios.compact()
            ),
        ));
    }
    sections.push(ReportSection {
        title: "GPU".into(),
        rows: gpu,
    });

    let mut sensors = Vec::new();
    let mut temps = 0usize;
    for chip in &snap.sensors.chips {
        let name = chip.name.value.as_deref().unwrap_or("hwmon");
        for ch in &chip.channels {
            if ch.kind != SensorKind::Temp {
                continue;
            }
            if temps >= 24 {
                break;
            }
            sensors.push(pair(
                format!("{name}/{}", ch.label),
                ch.value
                    .map(|v| format!("{v:.1} {}", ch.unit))
                    .unwrap_or_else(|| ch.raw.compact()),
            ));
            temps += 1;
        }
    }
    for z in snap.sensors.thermal_zones.iter().take(8) {
        sensors.push(pair(
            format!("thermal {}", z.r#type.compact()),
            z.temp_c
                .value
                .map(|v| format!("{v:.1} °C"))
                .unwrap_or_else(|| z.temp_c.compact()),
        ));
    }
    if sensors.is_empty() {
        sensors.push(pair("传感器", "无温度读数"));
    }
    sections.push(ReportSection {
        title: "传感器".into(),
        rows: sensors,
    });

    let mut power = Vec::new();
    for p in snap.power.supplies.iter().take(8) {
        power.push(pair(
            &p.name,
            format!(
                "{}  {}  {}%",
                p.kind.compact(),
                p.status.compact(),
                p.capacity_pct
                    .value
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into())
            ),
        ));
    }
    if power.is_empty() {
        power.push(pair("电源", "无 power_supply"));
    }
    for a in snap.alerts.iter().take(8) {
        power.push(pair(
            format!("{:?} {}", a.level, a.label),
            a.message.clone(),
        ));
    }
    sections.push(ReportSection {
        title: "电源 / 告警".into(),
        rows: power,
    });

    let mut storage = Vec::new();
    for d in snap.block.devices.iter().take(16) {
        storage.push(pair(
            &d.name,
            format!(
                "{}  {}  {}",
                d.model.compact(),
                d.size_bytes
                    .value
                    .map(format_bytes)
                    .unwrap_or_else(|| d.size_bytes.compact()),
                d.queue_scheduler.compact()
            ),
        ));
    }
    for c in snap.nvme.controllers.iter().take(8) {
        storage.push(pair(
            &c.name,
            format!(
                "{}  fw {}  {}",
                c.model.compact(),
                c.firmware.compact(),
                c.serial.compact()
            ),
        ));
    }
    if storage.is_empty() {
        storage.push(pair("存储", "无块设备"));
    }
    sections.push(ReportSection {
        title: "存储".into(),
        rows: storage,
    });

    let mut net = Vec::new();
    for i in snap.net.interfaces.iter().take(16) {
        net.push(pair(
            &i.name,
            format!(
                "{}  {}  mtu {}  {}",
                i.operstate.compact(),
                i.driver.compact(),
                i.mtu.compact(),
                if i.addresses.is_empty() {
                    "—".into()
                } else {
                    i.addresses.join(" ")
                }
            ),
        ));
    }
    net.push(pair("tcp_congestion", snap.net.tcp_congestion.compact()));
    sections.push(ReportSection {
        title: "网络".into(),
        rows: net,
    });

    let mut pci = vec![pair("设备数", snap.pci.devices.len().to_string())];
    for d in snap.pci.devices.iter().take(24) {
        pci.push(pair(
            &d.slot,
            format!(
                "{} {}  {}",
                d.vendor_name.as_deref().unwrap_or(&d.vendor_id),
                d.device_name.as_deref().unwrap_or(&d.device_id),
                d.driver.compact()
            ),
        ));
    }
    sections.push(ReportSection {
        title: "PCI".into(),
        rows: pci,
    });

    let mut usb = Vec::new();
    for d in snap.usb.devices.iter().take(24) {
        usb.push(pair(
            &d.sys_name,
            format!(
                "{} {}  {}",
                d.vendor_name
                    .as_deref()
                    .or(d.manufacturer.value.as_deref())
                    .unwrap_or("—"),
                d.product_name
                    .as_deref()
                    .or(d.product.value.as_deref())
                    .unwrap_or("—"),
                d.speed.compact()
            ),
        ));
    }
    if usb.is_empty() {
        usb.push(pair("USB", "无设备"));
    }
    sections.push(ReportSection {
        title: "USB".into(),
        rows: usb,
    });

    let mut os = vec![
        pair("OS", snap.software.os_name.compact()),
        pair("ID", snap.software.os_id.compact()),
        pair("内核", snap.software.kernel_release.compact()),
        pair("hostname", snap.software.hostname.compact()),
        pair(
            "loadavg",
            format!(
                "{} {} {}",
                snap.software.load_1.compact(),
                snap.software.load_5.compact(),
                snap.software.load_15.compact()
            ),
        ),
        pair("arch", snap.sysctl.kernel_arch.compact()),
        pair("lockdown", snap.security.lockdown.compact()),
        pair("KVM", snap.kvm.device.compact()),
    ];
    if let Some(n) = snap.buses.notes.iter().find(|s| s.starts_with("无 ")) {
        os.push(pair("leftover", n.clone()));
    }
    sections.push(ReportSection {
        title: "OS".into(),
        rows: os,
    });

    sections
}

fn csv_formula_leading(s: &str) -> bool {
    matches!(
        s.as_bytes().first(),
        Some(b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r')
    )
}

fn csv_escape(s: &str) -> String {
    let formula = csv_formula_leading(s);
    let needs_quote = formula
        || s.bytes()
            .any(|b| matches!(b, b',' | b'"' | b'\n' | b'\r'));
    if !needs_quote {
        return s.to_string();
    }
    let mut out = String::from("\"");
    if formula {
        out.push('\'');
    }
    for c in s.chars() {
        if c == '"' {
            out.push_str("\"\"");
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escape_quotes_specials() {
        assert_eq!(csv_escape("ok"), "ok");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
        assert_eq!(csv_escape("=1+1"), "\"'=1+1\"");
        assert_eq!(csv_escape("+cmd"), "\"'+cmd\"");
        assert_eq!(csv_escape("-1"), "\"'-1\"");
        assert_eq!(csv_escape("@SUM(A1)"), "\"'@SUM(A1)\"");
        assert_eq!(csv_escape("—"), "—");
    }

    #[test]
    fn report_format_parse() {
        assert_eq!(ReportFormat::parse("JSON"), Some(ReportFormat::Json));
        assert_eq!(ReportFormat::parse("txt"), Some(ReportFormat::Text));
        assert_eq!(ReportFormat::parse("markdown"), Some(ReportFormat::Markdown));
        assert_eq!(ReportFormat::parse("htm"), Some(ReportFormat::Html));
        assert_eq!(ReportFormat::parse("xml"), None);
    }

    #[test]
    fn text_summary_omits_sample_hints() {
        let snap = crate::snapshot::HardwareSnapshot::collect(&crate::access::ProbeCtx::default());
        let text = to_text(&snap);
        assert!(text.contains("AIDA Linux 硬件报告"));
        assert!(!text.contains("容器或精简虚拟机"));
        let csv = to_csv(&snap);
        assert!(csv.starts_with("section,key,value"));
        assert!(!csv.contains("容器或精简虚拟机"));
        let md = to_markdown(&snap);
        assert!(md.contains("# AIDA Linux 硬件报告"));
        assert!(!md.contains("容器或精简虚拟机"));
    }
}
