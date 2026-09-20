//! GPU：DRM sysfs（amdgpu / i915 / xe / nouveau）+ NVIDIA procfs。
//!
//! 不调用 `nvidia-smi` / `glxinfo` / `vulkaninfo`。无独显的虚拟机应得到
//! `not_found` 说明，而不是空列表假装探测失败。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct GpuReport {
    pub devices: Vec<GpuDevice>,
    pub nvidia_kernel: Sample<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GpuDevice {
    pub id: String,
    pub driver: String,
    pub pci_slot: Sample<String>,
    pub vendor_id: Sample<String>,
    pub device_id: Sample<String>,
    pub busy_percent: Sample<u64>,
    pub vram_total_bytes: Sample<u64>,
    pub vram_used_bytes: Sample<u64>,
    pub vbios: Sample<String>,
    pub clocks: Vec<GpuClock>,
    pub connectors: Vec<GpuConnector>,
    pub extra: BTreeMap<String, Sample<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GpuClock {
    pub name: String,
    pub current_mhz: Sample<u64>,
    pub min_mhz: Sample<u64>,
    pub max_mhz: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GpuConnector {
    pub name: String,
    pub status: Sample<String>,
    pub enabled: Sample<String>,
    pub edid: Option<EdidInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EdidInfo {
    pub manufacturer: String,
    pub product_code: u16,
    pub serial: Option<u32>,
    pub year: Option<u16>,
    pub week: Option<u8>,
    pub name: Option<String>,
    pub width_cm: Option<u8>,
    pub height_cm: Option<u8>,
    pub h_active: Option<u16>,
    pub v_active: Option<u16>,
}

pub fn collect(ctx: &ProbeCtx) -> GpuReport {
    let mut devices = Vec::new();
    let mut notes = Vec::new();
    let drm_root = ctx.sys_path("class/drm");
    match access::list_dir_names(&drm_root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(names),
            ..
        } => {
            let mut cards: Vec<String> = names
                .iter()
                .filter(|n| is_card_name(n))
                .cloned()
                .collect();
            cards.sort();
            for card in &cards {
                devices.push(read_drm_card(ctx, &drm_root, card, &names));
            }
        }
        s => notes.push(s.access_label()),
    }

    let nvidia_kernel = access::read_trimmed(ctx.proc_path("driver/nvidia/version"));
    if let Some(gpus) = access::list_dir_names(ctx.proc_path("driver/nvidia/gpus")).value {
        for slot in gpus {
            if devices.iter().any(|d| d.pci_slot.value.as_deref() == Some(&slot)) {
                continue;
            }
            devices.push(read_nvidia_proc(ctx, &slot));
        }
    }

    append_pci_display_fallback(ctx, &mut devices);

    if devices.is_empty() {
        notes.push(
            "未发现 DRM 卡或 PCI 显示控制器。虚拟机无 GPU、或内核未加载 drm 时属于正常情况。".into(),
        );
    }
    GpuReport {
        devices,
        nvidia_kernel,
        notes,
    }
}

/// GUI 快路径：更新忙闲/显存/时钟/连接器状态，不重读 EDID、不扫 PCI 回退。
pub fn refresh_runtime(report: &mut GpuReport, ctx: &ProbeCtx) {
    let drm_root = ctx.sys_path("class/drm");
    for dev in &mut report.devices {
        if !is_card_name(&dev.id) {
            continue;
        }
        let card_dir = drm_root.join(&dev.id);
        let device = card_dir.join("device");
        let mut busy = access::read_u64(device.join("gpu_busy_percent"));
        if dev.driver == "i915" {
            busy = first_ok(busy, access::read_u64(device.join("gt_busy_percent")));
        }
        dev.busy_percent = busy;
        dev.vram_used_bytes = access::read_u64(device.join("mem_info_vram_used"));
        match dev.driver.as_str() {
            "amdgpu" => {
                if let Some(mhz) =
                    parse_pp_dpm_current(&access::read_trimmed(device.join("pp_dpm_sclk")))
                {
                    if let Some(c) = dev.clocks.iter_mut().find(|c| c.name == "sclk") {
                        c.current_mhz = mhz;
                    }
                }
                if let Some(mhz) =
                    parse_pp_dpm_current(&access::read_trimmed(device.join("pp_dpm_mclk")))
                {
                    if let Some(c) = dev.clocks.iter_mut().find(|c| c.name == "mclk") {
                        c.current_mhz = mhz;
                    }
                }
            }
            "i915" => {
                if let Some(c) = dev.clocks.iter_mut().find(|c| c.name == "gt") {
                    *c = i915_clock(&device, "gt");
                }
            }
            "xe" => {
                if let Some(c) = dev.clocks.iter_mut().find(|c| c.name == "gt") {
                    c.current_mhz = first_u64(&[
                        device.join("tile0/gt0/freq0/cur_freq"),
                        device.join("gt/gt0/freq0/cur_freq"),
                    ]);
                }
            }
            _ => {}
        }
        for conn in &mut dev.connectors {
            let p = drm_root.join(format!("{}-{}", dev.id, conn.name));
            conn.status = access::read_trimmed(p.join("status"));
            conn.enabled = access::read_trimmed(p.join("enabled"));
        }
    }
}

fn is_card_name(name: &str) -> bool {
    let rest = match name.strip_prefix("card") {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

fn read_drm_card(ctx: &ProbeCtx, drm_root: &Path, card: &str, all_names: &[String]) -> GpuDevice {
    let card_dir = drm_root.join(card);
    let dev = card_dir.join("device");
    let uevent = access::read_trimmed(dev.join("uevent"));
    let (driver, slot_from_uevent) = parse_uevent(uevent.value.as_deref().unwrap_or(""));
    let pci_slot = slot_from_uevent
        .map(|s| Sample::ok(s, dev.join("uevent").display().to_string()))
        .unwrap_or_else(|| pci_slot_from_symlink(&dev));

    let driver = driver.unwrap_or_else(|| {
        fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "unknown".into())
    });

    let mut clocks = Vec::new();
    let mut extra = BTreeMap::new();
    let mut busy = access::read_u64(dev.join("gpu_busy_percent"));
    let vram_total = access::read_u64(dev.join("mem_info_vram_total"));
    let vram_used = access::read_u64(dev.join("mem_info_vram_used"));
    let mut vbios = access::read_trimmed(dev.join("vbios_version"));

    match driver.as_str() {
        "amdgpu" => {
            if let Some(mhz) = parse_pp_dpm_current(&access::read_trimmed(dev.join("pp_dpm_sclk"))) {
                clocks.push(GpuClock {
                    name: "sclk".into(),
                    current_mhz: mhz,
                    min_mhz: Sample::missing(dev.join("pp_dpm_sclk").display().to_string()),
                    max_mhz: Sample::missing(dev.join("pp_dpm_sclk").display().to_string()),
                });
            }
            if let Some(mhz) = parse_pp_dpm_current(&access::read_trimmed(dev.join("pp_dpm_mclk"))) {
                clocks.push(GpuClock {
                    name: "mclk".into(),
                    current_mhz: mhz,
                    min_mhz: Sample::missing(dev.join("pp_dpm_mclk").display().to_string()),
                    max_mhz: Sample::missing(dev.join("pp_dpm_mclk").display().to_string()),
                });
            }
            push_extra(&mut extra, "link_speed", access::read_trimmed(dev.join("current_link_speed")));
            push_extra(&mut extra, "link_width", access::read_trimmed(dev.join("current_link_width")));
            push_extra(
                &mut extra,
                "power_dpm",
                access::read_trimmed(dev.join("power_dpm_force_performance_level")),
            );
        }
        "i915" => {
            clocks.push(i915_clock(&dev, "gt"));
            busy = first_ok(busy, access::read_u64(dev.join("gt_busy_percent")));
        }
        "xe" => {
            clocks.push(GpuClock {
                name: "gt".into(),
                current_mhz: first_u64(&[
                    dev.join("tile0/gt0/freq0/cur_freq"),
                    dev.join("gt/gt0/freq0/cur_freq"),
                ]),
                min_mhz: first_u64(&[dev.join("tile0/gt0/freq0/min_freq")]),
                max_mhz: first_u64(&[dev.join("tile0/gt0/freq0/max_freq")]),
            });
        }
        "nouveau" => {
            push_extra(&mut extra, "pstate", access::read_trimmed(dev.join("pstate")));
        }
        "nvidia" => {
            if let Some(slot) = pci_slot.value.clone() {
                merge_nvidia_proc(ctx, &slot, &mut vbios, &mut extra);
            }
        }
        _ => {}
    }

    let prefix = format!("{card}-");
    let mut connectors = Vec::new();
    for name in all_names {
        if let Some(rest) = name.strip_prefix(&prefix) {
            if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let p = drm_root.join(name);
            connectors.push(GpuConnector {
                name: rest.to_string(),
                status: access::read_trimmed(p.join("status")),
                enabled: access::read_trimmed(p.join("enabled")),
                edid: parse_edid_file(&access::read_bytes(p.join("edid"))),
            });
        }
    }

    GpuDevice {
        id: card.to_string(),
        driver,
        pci_slot,
        vendor_id: hex_sample(access::read_trimmed(dev.join("vendor"))),
        device_id: hex_sample(access::read_trimmed(dev.join("device"))),
        busy_percent: busy,
        vram_total_bytes: vram_total,
        vram_used_bytes: vram_used,
        vbios,
        clocks,
        connectors,
        extra,
    }
}

fn i915_clock(dev: &Path, name: &str) -> GpuClock {
    GpuClock {
        name: name.into(),
        current_mhz: first_u64(&[
            dev.join("gt_cur_freq_mhz"),
            dev.join("gt_act_freq_mhz"),
        ]),
        min_mhz: access::read_u64(dev.join("gt_min_freq_mhz")),
        max_mhz: access::read_u64(dev.join("gt_max_freq_mhz")),
    }
}

fn first_u64(paths: &[std::path::PathBuf]) -> Sample<u64> {
    let mut last = None;
    for p in paths {
        let s = access::read_u64(p);
        if s.access == AccessKind::Ok && s.value.is_some() {
            return s;
        }
        last = Some(s);
    }
    last.unwrap_or_else(|| Sample::missing("clock"))
}

fn first_ok(a: Sample<u64>, b: Sample<u64>) -> Sample<u64> {
    if a.access == AccessKind::Ok && a.value.is_some() {
        a
    } else {
        b
    }
}

fn push_extra(map: &mut BTreeMap<String, Sample<String>>, key: &str, sample: Sample<String>) {
    if sample.access != AccessKind::NotFound {
        map.insert(key.into(), sample);
    }
}

fn hex_sample(s: Sample<String>) -> Sample<String> {
    match (s.access, s.value) {
        (AccessKind::Ok, Some(v)) => {
            let t = v.trim().trim_start_matches("0x").to_ascii_lowercase();
            Sample::ok(t, s.source)
        }
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

fn parse_edid_file(sample: &Sample<Vec<u8>>) -> Option<EdidInfo> {
    parse_edid(sample.value.as_deref()?)
}

/// 解析 EDID 1.3/1.4 前 128 字节。不调用 `edid-decode`。
pub fn parse_edid(buf: &[u8]) -> Option<EdidInfo> {
    if buf.len() < 128 || buf[0] != 0x00 || buf[1] != 0xff || buf[7] != 0x00 {
        return None;
    }
    let id = u16::from_be_bytes([buf[8], buf[9]]);
    let letter = |shift: u16| {
        let n = ((id >> shift) & 0x1f) as u8;
        if n == 0 {
            '?'
        } else {
            (b'@' + n) as char
        }
    };
    let manufacturer = format!("{}{}{}", letter(10), letter(5), letter(0));
    let product_code = u16::from_le_bytes([buf[10], buf[11]]);
    let serial_raw = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let serial = if serial_raw == 0 {
        None
    } else {
        Some(serial_raw)
    };
    let week = if buf[16] == 0 || buf[16] == 0xff {
        None
    } else {
        Some(buf[16])
    };
    let year = if buf[17] == 0xff {
        None
    } else {
        Some(1990 + buf[17] as u16)
    };
    let width_cm = if buf[21] == 0 { None } else { Some(buf[21]) };
    let height_cm = if buf[22] == 0 { None } else { Some(buf[22]) };
    let mut name = None;
    let mut serial_str: Option<String> = None;
    let mut h_active = None;
    let mut v_active = None;
    for off in [54usize, 72, 90, 108] {
        if off + 18 > buf.len() {
            break;
        }
        let d = &buf[off..off + 18];
        let pixclk = u16::from_le_bytes([d[0], d[1]]);
        if pixclk != 0 {
            if h_active.is_none() {
                h_active = Some(d[2] as u16 | ((d[4] as u16 & 0xf0) << 4));
                v_active = Some(d[5] as u16 | ((d[7] as u16 & 0xf0) << 4));
            }
            continue;
        }
        match d[3] {
            0xfc => {
                if name.is_none() {
                    name = Some(edid_text(&d[5..]));
                }
            }
            0xff => {
                if serial_str.is_none() {
                    serial_str = Some(edid_text(&d[5..]));
                }
            }
            _ => {}
        }
    }
    let _ = serial_str;
    Some(EdidInfo {
        manufacturer,
        product_code,
        serial,
        year,
        week,
        name,
        width_cm,
        height_cm,
        h_active,
        v_active,
    })
}

fn edid_text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| b == 0x0a || b == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn parse_uevent(text: &str) -> (Option<String>, Option<String>) {
    let mut driver = None;
    let mut slot = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("DRIVER=") {
            driver = Some(v.trim().to_string());
        }
        if let Some(v) = line.strip_prefix("PCI_SLOT_NAME=") {
            slot = Some(v.trim().to_string());
        }
    }
    (driver, slot)
}

fn pci_slot_from_symlink(dev: &Path) -> Sample<String> {
    match fs::read_link(dev) {
        Ok(p) => {
            let s = p.to_string_lossy();
            if let Some(slot) = s.split('/').rev().find(|c| c.contains(':') && c.contains('.')) {
                Sample::ok(slot.to_string(), dev.display().to_string())
            } else {
                Sample::missing(dev.display().to_string())
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Sample::missing(dev.display().to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Sample::denied(dev.display().to_string())
        }
        Err(e) => Sample::error(dev.display().to_string(), e.to_string()),
    }
}

/// amdgpu `pp_dpm_*`：带 `*` 的那一行是当前档，单位 MHz。
pub fn parse_pp_dpm_current(sample: &Sample<String>) -> Option<Sample<u64>> {
    let text = sample.value.as_deref()?;
    for line in text.lines() {
        if !line.contains('*') {
            continue;
        }
        let mhz = line
            .split_whitespace()
            .find_map(|tok| tok.trim_end_matches("Mhz").trim_end_matches("MHz").parse::<u64>().ok())?;
        return Some(Sample::ok(mhz, sample.source.clone()));
    }
    None
}

fn read_nvidia_proc(ctx: &ProbeCtx, slot: &str) -> GpuDevice {
    let info = access::read_trimmed(ctx.proc_path(format!("driver/nvidia/gpus/{slot}/information")));
    let parsed = info
        .value
        .as_deref()
        .map(parse_nvidia_information)
        .unwrap_or_default();
    let mut extra = BTreeMap::new();
    if let Some(model) = parsed.get("Model") {
        extra.insert("model".into(), Sample::ok(model.clone(), info.source.clone()));
    }
    GpuDevice {
        id: format!("nvidia-{slot}"),
        driver: "nvidia".into(),
        pci_slot: Sample::ok(slot.to_string(), info.source.clone()),
        vendor_id: Sample::ok("10de".to_string(), "nvidia-proc"),
        device_id: Sample::missing("nvidia-proc"),
        busy_percent: Sample::unsupported(
            info.source.clone(),
            "专有驱动不在 sysfs 暴露 gpu_busy_percent；本工具不调用 nvidia-smi",
        ),
        vram_total_bytes: Sample::unsupported(info.source.clone(), "未走 NVML"),
        vram_used_bytes: Sample::unsupported(info.source.clone(), "未走 NVML"),
        vbios: parsed
            .get("Video BIOS")
            .map(|v| Sample::ok(v.clone(), info.source.clone()))
            .unwrap_or_else(|| Sample {
                value: None,
                access: info.access,
                source: info.source.clone(),
                hint: info.hint.clone(),
            }),
        clocks: Vec::new(),
        connectors: Vec::new(),
        extra,
    }
}

fn merge_nvidia_proc(
    ctx: &ProbeCtx,
    slot: &str,
    vbios: &mut Sample<String>,
    extra: &mut BTreeMap<String, Sample<String>>,
) {
    let info = access::read_trimmed(ctx.proc_path(format!("driver/nvidia/gpus/{slot}/information")));
    if let Some(text) = info.value.as_deref() {
        let parsed = parse_nvidia_information(text);
        if vbios.value.is_none() {
            if let Some(v) = parsed.get("Video BIOS") {
                *vbios = Sample::ok(v.clone(), info.source.clone());
            }
        }
        if let Some(m) = parsed.get("Model") {
            extra.insert("model".into(), Sample::ok(m.clone(), info.source));
        }
    }
}

pub fn parse_nvidia_information(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

fn append_pci_display_fallback(ctx: &ProbeCtx, devices: &mut Vec<GpuDevice>) {
    let root = ctx.sys_path("bus/pci/devices");
    let Some(names) = access::list_dir_names(&root).value else {
        return;
    };
    for slot in names {
        let dir = root.join(&slot);
        let class = access::read_trimmed(dir.join("class"));
        let Some(c) = class.value.as_deref() else {
            continue;
        };
        let code = c.trim().trim_start_matches("0x");
        let Ok(v) = u32::from_str_radix(code, 16) else {
            continue;
        };
        if (v >> 16) & 0xFF != 0x03 {
            continue;
        }
        if devices.iter().any(|d| d.pci_slot.value.as_deref() == Some(&slot)) {
            continue;
        }
        let driver = fs::read_link(dir.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "none".into());
        devices.push(GpuDevice {
            id: format!("pci-{slot}"),
            driver,
            pci_slot: Sample::ok(slot, dir.display().to_string()),
            vendor_id: hex_sample(access::read_trimmed(dir.join("vendor"))),
            device_id: hex_sample(access::read_trimmed(dir.join("device"))),
            busy_percent: Sample::missing("drm"),
            vram_total_bytes: Sample::missing("drm"),
            vram_used_bytes: Sample::missing("drm"),
            vbios: Sample::missing("drm"),
            clocks: Vec::new(),
            connectors: Vec::new(),
            extra: BTreeMap::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn card_name_filter() {
        assert!(is_card_name("card0"));
        assert!(is_card_name("card12"));
        assert!(!is_card_name("card0-HDMI-A-1"));
        assert!(!is_card_name("renderD128"));
    }

    #[test]
    fn pp_dpm_star_line() {
        let s = Sample::ok(
            "0: 500Mhz\n1: 1500Mhz *\n2: 2000Mhz\n".into(),
            "pp_dpm_sclk",
        );
        assert_eq!(parse_pp_dpm_current(&s).unwrap().value, Some(1500));
    }

    #[test]
    fn nvidia_info_pairs() {
        let m = parse_nvidia_information("Model:   GeForce RTX\nVideo BIOS: 90.02\n");
        assert_eq!(m.get("Model").unwrap(), "GeForce RTX");
        assert_eq!(m.get("Video BIOS").unwrap(), "90.02");
    }

    #[test]
    fn drm_fixture_amdgpu() {
        let root = std::env::temp_dir().join(format!("aida-gpu-{}", std::process::id()));
        let card = root.join("sys/class/drm/card0/device");
        fs::create_dir_all(&card).unwrap();
        fs::create_dir_all(root.join("sys/class/drm/card0-HDMI-A-1")).unwrap();
        fs::write(card.join("uevent"), "DRIVER=amdgpu\nPCI_SLOT_NAME=0000:0a:00.0\n").unwrap();
        fs::write(card.join("vendor"), "0x1002\n").unwrap();
        fs::write(card.join("device"), "0x73df\n").unwrap();
        fs::write(card.join("gpu_busy_percent"), "17\n").unwrap();
        fs::write(card.join("mem_info_vram_total"), "8589934592\n").unwrap();
        fs::write(card.join("mem_info_vram_used"), "1073741824\n").unwrap();
        fs::write(card.join("pp_dpm_sclk"), "0: 500Mhz\n1: 2000Mhz *\n").unwrap();
        fs::write(root.join("sys/class/drm/card0-HDMI-A-1/status"), "connected\n").unwrap();
        fs::write(root.join("sys/class/drm/card0-HDMI-A-1/enabled"), "enabled\n").unwrap();
        fs::write(
            root.join("sys/class/drm/card0-HDMI-A-1/edid"),
            sample_edid(b"DEL", "Test LCD", 1920, 1080),
        )
        .unwrap();
        fs::create_dir_all(root.join("sys/bus/pci/devices")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let report = collect(&ctx);
        assert_eq!(report.devices.len(), 1);
        let d = &report.devices[0];
        assert_eq!(d.driver, "amdgpu");
        assert_eq!(d.busy_percent.value, Some(17));
        assert_eq!(d.vram_total_bytes.value, Some(8589934592));
        assert_eq!(d.clocks[0].current_mhz.value, Some(2000));
        assert_eq!(d.connectors[0].status.value.as_deref(), Some("connected"));
        let edid = d.connectors[0].edid.as_ref().unwrap();
        assert_eq!(edid.manufacturer, "DEL");
        assert_eq!(edid.name.as_deref(), Some("Test LCD"));
        assert_eq!(edid.h_active, Some(1920));
        assert_eq!(edid.v_active, Some(1080));
        let _ = fs::remove_dir_all(&root);
    }

    fn sample_edid(mfg: &[u8; 3], name: &str, h: u16, v: u16) -> Vec<u8> {
        let mut buf = vec![0u8; 128];
        buf[0] = 0x00;
        buf[1] = 0xff;
        buf[2] = 0xff;
        buf[3] = 0xff;
        buf[4] = 0xff;
        buf[5] = 0xff;
        buf[6] = 0xff;
        buf[7] = 0x00;
        let c1 = (mfg[0] - b'@') as u16;
        let c2 = (mfg[1] - b'@') as u16;
        let c3 = (mfg[2] - b'@') as u16;
        let id = (c1 << 10) | (c2 << 5) | c3;
        buf[8] = (id >> 8) as u8;
        buf[9] = id as u8;
        buf[17] = (2024 - 1990) as u8;
        buf[21] = 60;
        buf[22] = 34;
        // detailed timing 0: 148.5 MHz-ish dummy clock, 1920x1080
        let pix = 14850u16;
        buf[54] = pix as u8;
        buf[55] = (pix >> 8) as u8;
        buf[56] = (h & 0xff) as u8;
        buf[58] = ((h >> 8) << 4) as u8;
        buf[59] = (v & 0xff) as u8;
        buf[61] = ((v >> 8) << 4) as u8;
        // monitor name descriptor
        buf[72] = 0;
        buf[73] = 0;
        buf[74] = 0;
        buf[75] = 0xfc;
        buf[76] = 0;
        for (i, b) in name.bytes().take(13).enumerate() {
            buf[77 + i] = b;
        }
        if name.len() < 13 {
            buf[77 + name.len()] = 0x0a;
        }
        let sum: u8 = buf[..127].iter().fold(0u8, |a, b| a.wrapping_add(*b));
        buf[127] = 0u8.wrapping_sub(sum);
        buf
    }
}
