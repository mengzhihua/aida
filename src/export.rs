//! JSON / HTML 报告导出。HTML 为单文件，无外部资源。

use crate::access::AccessKind;
use crate::alerts::AlertLevel;
use crate::snapshot::HardwareSnapshot;

pub fn to_json_pretty(snap: &HardwareSnapshot) -> Result<String, String> {
    serde_json::to_string_pretty(snap).map_err(|e| e.to_string())
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
            ("KVM", snap.kvm.device.display()),
            ("nested", snap.kvm.nested.display()),
            ("microcode", snap.cpu.microcode.display()),
        ],
    );
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
        html.push_str("<table><tr><th>cpufreq</th><th>driver</th><th>governor</th><th>kHz</th></tr>");
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
                    "{} {}",
                    snap.dmi.bios_vendor.display(),
                    snap.dmi.bios_version.display()
                ),
            ),
        ],
    );
    for n in &snap.dmi.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

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
                z.present.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                z.managed.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
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
        "<p class=\"muted\">ACPI {} PnP {} MSR {} platform {} workqueue {} events {}</p>",
        snap.platform.acpi_devices,
        snap.platform.pnp_devices,
        snap.platform.msr_devices,
        esc(&if snap.platform.platform_devices.is_empty() {
            "—".into()
        } else {
            snap.platform.platform_devices.join(" ")
        }),
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
        "<p class=\"muted\">conntrack {} / {} cong {} TcpExt TW {} timeout {} octets {}/{}</p>",
        snap.net.conntrack_count.display(),
        snap.net.conntrack_max.display(),
        snap.net.tcp_congestion.display(),
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
        "<p class=\"muted\">IPv6 in {} out {} octets {}/{} TCP6 {} unix {} inet6 {} ipv6_route {} fastopen {} somaxconn {} ka {} sack {} syn/synack {}/{} retries2 {} qdisc {} budget {} rp_filter {} redirects {} tcp/udp {}/{} tcp6/udp6 {}/{} raw {} udplite {} raw6 {} udplite6 {} igmp6 {} busy_poll {} weight {} notsent {}</p>",
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
    if !snap.net.iptables.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">iptables {}</p>",
            esc(&snap.net.iptables.join(" "))
        ));
    }
    if !snap.net.ip6tables.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\">ip6tables {}</p>",
            esc(&snap.net.ip6tables.join(" "))
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
                    .map(|d| {
                        format!(
                            "{} {} {}",
                            d.name,
                            d.class.display(),
                            d.model.display()
                        )
                    })
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
        html.push_str("<table><tr><th>LUN</th><th>vendor</th><th>model</th><th>type</th><th>状态</th></tr>");
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
    html.push_str("<table><tr><th>挂载点</th><th>fstype</th><th>源</th><th>kind</th><th>用量</th></tr>");
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
            ("内核", snap.software.kernel_release.display()),
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
                    "hardlinks {} symlinks {} fifos {} regular {}",
                    snap.security.protected_hardlinks.display(),
                    snap.security.protected_symlinks.display(),
                    snap.security.protected_fifos.display(),
                    snap.security.protected_regular.display()
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
                    "nmi {} wd {} thresh {} panic {} sysrq {} min_free {} hung {}",
                    snap.sysctl.nmi_watchdog.display(),
                    snap.sysctl.watchdog.display(),
                    snap.sysctl.watchdog_thresh.display(),
                    snap.sysctl.panic.display(),
                    snap.sysctl.sysrq.display(),
                    snap.sysctl.min_free_kbytes.display(),
                    snap.sysctl.hung_task_timeout_secs.display()
                ),
            ),
            (
                "sched / oom",
                format!(
                    "rt {}/{}us rr {}ms numa {} tmig {} panic_oom {} oom_alloc {} laptop {} kexec_off {} hung_panic {}",
                    snap.sysctl.sched_rt_runtime_us.display(),
                    snap.sysctl.sched_rt_period_us.display(),
                    snap.sysctl.sched_rr_timeslice_ms.display(),
                    snap.sysctl.numa_balancing.display(),
                    snap.sysctl.timer_migration.display(),
                    snap.sysctl.panic_on_oom.display(),
                    snap.sysctl.oom_kill_allocating_task.display(),
                    snap.sysctl.laptop_mode.display(),
                    snap.sysctl.kexec_load_disabled.display(),
                    snap.sysctl.hung_task_panic.display()
                ),
            ),
            (
                "printk / cfs / uffd",
                format!(
                    "ratelimit {}/{} cfs {}us oops_limit {} hard {} soft {} oom_dump {} user_reserve {} uffd {} ngroups {}",
                    snap.sysctl.printk_ratelimit.display(),
                    snap.sysctl.printk_ratelimit_burst.display(),
                    snap.sysctl.sched_cfs_bandwidth_slice_us.display(),
                    snap.sysctl.oops_limit.display(),
                    snap.sysctl.hardlockup_panic.display(),
                    snap.sysctl.softlockup_panic.display(),
                    snap.sysctl.oom_dump_tasks.display(),
                    snap.sysctl.user_reserve_kbytes.display(),
                    snap.sysctl.unprivileged_userfaultfd.display(),
                    snap.sysctl.ngroups_max.display()
                ),
            ),
            (
                "keys / dumpable",
                format!(
                    "maxkeys {} maxbytes {} cap_last {} dumpable {} autogroup {} cad {}",
                    snap.sysctl.keys_maxkeys.display(),
                    snap.sysctl.keys_maxbytes.display(),
                    snap.sysctl.cap_last_cap.display(),
                    snap.sysctl.suid_dumpable.display(),
                    snap.sysctl.sched_autogroup.display(),
                    snap.sysctl.ctrl_alt_del.display()
                ),
            ),
            (
                "ipc",
                format!(
                    "shmmax {} shmmni {} mqueue {} sysvipc {}/{}/{}",
                    snap.sysctl.shmmax.display(),
                    snap.sysctl.shmmni.display(),
                    snap.sysctl.mqueue_queues_max.display(),
                    snap.sysctl.sysvipc_shm,
                    snap.sysctl.sysvipc_sem,
                    snap.sysctl.sysvipc_msg
                ),
            ),
            (
                "aio/inotify",
                format!(
                    "{} / {} watches {}",
                    snap.sysctl.aio_nr.display(),
                    snap.sysctl.aio_max_nr.display(),
                    snap.sysctl.inotify_max_user_watches.display()
                ),
            ),
            (
                "bpf/perf",
                format!(
                    "unpriv_bpf {} jit {}/{} binfmt {} perf {}",
                    snap.security.unprivileged_bpf_disabled.display(),
                    snap.security.bpf_jit_enable.display(),
                    snap.security.bpf_jit_harden.display(),
                    snap.security.binfmt_misc_status.display(),
                    snap.security.perf_event_paranoid.display()
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
                    "swappiness {} overcommit {} watermark {} dirty_expire {}",
                    snap.sysctl.swappiness.display(),
                    snap.sysctl.overcommit_memory.display(),
                    snap.sysctl.watermark_scale_factor.display(),
                    snap.sysctl.dirty_expire_centisecs.display()
                ),
            ),
            ("cgroup", snap.cgroup.controllers.display()),
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
