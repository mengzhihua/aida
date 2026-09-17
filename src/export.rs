//! JSON / HTML 报告导出。HTML 为单文件，无外部资源。

use crate::access::AccessKind;
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
    html.push_str("h1,h2{color:#7ec8ff;} table{border-collapse:collapse;width:100%;margin:12px 0;}");
    html.push_str(
        "td,th{border:1px solid #333;padding:6px 8px;text-align:left;vertical-align:top;}",
    );
    html.push_str("th{background:#1c2230;} .muted{color:#aaa;font-size:12px;} .warn{color:#ffb347;}");
    html.push_str("</style></head><body>");
    html.push_str(&format!(
        "<h1>AIDA Linux 硬件报告</h1><p class=\"muted\">v{} · unix_ms {}</p>",
        snap.version, snap.collected_at_unix_ms
    ));
    html.push_str(&format!(
        "<p>{}</p>",
        esc(&snap.privilege.summary)
    ));

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
        ],
    );

    section(&mut html, "DMI / 主板");
    kv(
        &mut html,
        &[
            ("系统厂商", snap.dmi.sys_vendor.display()),
            ("产品", snap.dmi.product_name.display()),
            ("序列号", snap.dmi.product_serial.display()),
            ("UUID", snap.dmi.product_uuid.display()),
            ("主板", format!("{} {}", snap.dmi.board_vendor.display(), snap.dmi.board_name.display())),
            ("BIOS", format!("{} {}", snap.dmi.bios_vendor.display(), snap.dmi.bios_version.display())),
        ],
    );
    for n in &snap.dmi.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "传感器");
    html.push_str("<table><tr><th>芯片</th><th>通道</th><th>值</th><th>状态</th></tr>");
    for chip in &snap.sensors.chips {
        for ch in &chip.channels {
            let val = ch
                .value
                .map(|v| format!("{v:.3} {}", ch.unit))
                .unwrap_or_else(|| ch.raw.access_label());
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(chip.name.value.as_deref().unwrap_or("?")),
                esc(&ch.label),
                esc(&val),
                access_cell(ch.raw.access)
            ));
        }
    }
    if snap.sensors.chips.is_empty() {
        html.push_str("<tr><td colspan=\"4\" class=\"warn\">无 hwmon 数据</td></tr>");
    }
    html.push_str("</table>");

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
    html.push_str("<table><tr><th>槽位</th><th>ID</th><th>名称</th><th>类别</th><th>驱动</th></tr>");
    for d in &snap.pci.devices {
        let name = match (&d.vendor_name, &d.device_name) {
            (Some(v), Some(n)) => format!("{v} {n}"),
            (Some(v), None) => v.clone(),
            _ => String::from("—"),
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}:{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&d.slot),
            esc(&d.vendor_id),
            esc(&d.device_id),
            esc(&name),
            esc(&d.class_name),
            esc(&d.driver.display())
        ));
    }
    html.push_str("</table>");

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
                ("ID", format!("{}:{}", g.vendor_id.display(), g.device_id.display())),
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
    }
    for n in &snap.gpu.notes {
        html.push_str(&format!("<p class=\"warn\">{}</p>", esc(n)));
    }

    section(&mut html, "存储");
    html.push_str("<table><tr><th>设备</th><th>类型</th><th>容量</th><th>型号</th></tr>");
    for b in &snap.block.devices {
        let size = b
            .size_bytes
            .value
            .map(format_bytes)
            .unwrap_or_else(|| b.size_bytes.access_label());
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&b.name),
            esc(&b.r#type),
            esc(&size),
            esc(&b.model.display())
        ));
    }
    html.push_str("</table>");

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
        ],
    );

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
