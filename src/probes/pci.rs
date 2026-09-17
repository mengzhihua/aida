//! PCI 设备：扫描 `/sys/bus/pci/devices`，不调用 `lspci`。
//! 名称解析顺序：系统 pci.ids → 内置常见厂商表 → 仅显示 ID。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct PciReport {
    pub devices: Vec<PciDevice>,
    pub ids_source: Sample<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PciDevice {
    pub slot: String,
    pub vendor_id: String,
    pub device_id: String,
    pub class_code: String,
    pub revision: Sample<String>,
    pub subsystem_vendor: Sample<String>,
    pub subsystem_device: Sample<String>,
    pub irq: Sample<String>,
    pub numa_node: Sample<String>,
    pub enable: Sample<String>,
    pub current_link_speed: Sample<String>,
    pub current_link_width: Sample<String>,
    pub max_link_speed: Sample<String>,
    pub max_link_width: Sample<String>,
    pub msi_irqs: Sample<u64>,
    pub local_cpulist: Sample<String>,
    pub driver: Sample<String>,
    pub vendor_name: Option<String>,
    pub device_name: Option<String>,
    pub class_name: String,
}

impl PciDevice {
    /// 表格用的链路摘要。缺 `current_link_*` / `msi_irqs` 时用破折号，避免把整条 sysfs 路径铺进列里。
    pub fn link_label(&self) -> String {
        let msi = compact_field(&self.msi_irqs);
        match self.current_link_speed.value.as_deref() {
            Some(s) => format!(
                "{} x{} msi {}",
                s,
                compact_field(&self.current_link_width),
                msi
            ),
            None => format!("— msi {msi}"),
        }
    }
}

fn compact_field<T: Serialize + std::fmt::Display>(s: &Sample<T>) -> String {
    match (&s.value, s.access) {
        (Some(v), AccessKind::Ok) => v.to_string(),
        (_, AccessKind::NotFound) => "—".into(),
        _ => s.display(),
    }
}

pub fn collect(ctx: &ProbeCtx) -> PciReport {
    let mut notes = Vec::new();
    let ids = load_pci_ids(ctx);
    let ids_source = match &ids {
        Some((path, _)) => Sample::ok(path.clone(), path.clone()),
        None => Sample::missing("/usr/share/misc/pci.ids|hwdata/pci.ids"),
    };
    if ids.is_none() {
        notes.push(
            "未找到 pci.ids，将显示十六进制 ID。Debian/Ubuntu 包 pci.ids / hwdata，RHEL 系包 hwdata。"
                .into(),
        );
    }

    let root = ctx.sys_path("bus/pci/devices");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            return PciReport {
                devices: Vec::new(),
                ids_source,
                notes,
            };
        }
    };

    let db = ids.as_ref().map(|(_, db)| db);
    let mut devices = Vec::new();
    for slot in names {
        let dir = root.join(&slot);
        let vendor_id = hex_id(access::read_trimmed(dir.join("vendor")));
        let device_id = hex_id(access::read_trimmed(dir.join("device")));
        let class_code = hex_id(access::read_trimmed(dir.join("class")));
        let (vendor_name, device_name) = lookup(db, &vendor_id, &device_id);
        let driver = read_driver(&dir);
        devices.push(PciDevice {
            slot,
            class_name: class_name(&class_code),
            vendor_id,
            device_id,
            class_code,
            revision: access::read_trimmed(dir.join("revision")),
            subsystem_vendor: access::read_trimmed(dir.join("subsystem_vendor")),
            subsystem_device: access::read_trimmed(dir.join("subsystem_device")),
            irq: access::read_trimmed(dir.join("irq")),
            numa_node: access::read_trimmed(dir.join("numa_node")),
            enable: access::read_trimmed(dir.join("enable")),
            current_link_speed: access::read_trimmed(dir.join("current_link_speed")),
            current_link_width: access::read_trimmed(dir.join("current_link_width")),
            max_link_speed: access::read_trimmed(dir.join("max_link_speed")),
            max_link_width: access::read_trimmed(dir.join("max_link_width")),
            msi_irqs: match access::list_dir_names(dir.join("msi_irqs")) {
                Sample {
                    access: AccessKind::Ok,
                    value: Some(n),
                    source,
                    ..
                } => Sample::ok(n.len() as u64, source),
                s => Sample {
                    value: None,
                    access: s.access,
                    source: s.source,
                    hint: s.hint,
                },
            },
            local_cpulist: access::read_trimmed(dir.join("local_cpulist")),
            driver,
            vendor_name,
            device_name,
        });
    }
    devices.sort_by(|a, b| a.slot.cmp(&b.slot));
    PciReport {
        devices,
        ids_source,
        notes,
    }
}

fn hex_id(s: Sample<String>) -> String {
    s.value
        .map(|v| {
            let t = v.trim().trim_start_matches("0x").to_ascii_lowercase();
            t
        })
        .unwrap_or_else(|| "????".into())
}

fn read_driver(dir: &Path) -> Sample<String> {
    let link = dir.join("driver");
    match fs::read_link(&link) {
        Ok(p) => Sample::ok(
            p.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            link.display().to_string(),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Sample::missing(link.display().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Sample::denied(link.display().to_string())
        }
        Err(e) => Sample::error(link.display().to_string(), e.to_string()),
    }
}

struct PciDb {
    vendors: HashMap<u16, String>,
    devices: HashMap<(u16, u16), String>,
}

fn load_pci_ids(ctx: &ProbeCtx) -> Option<(String, PciDb)> {
    let candidates = [
        ctx.usr_share.join("misc/pci.ids"),
        ctx.usr_share.join("hwdata/pci.ids"),
        Path::new("/usr/share/misc/pci.ids").to_path_buf(),
        Path::new("/usr/share/hwdata/pci.ids").to_path_buf(),
    ];
    for p in candidates {
        if let Ok(text) = fs::read_to_string(&p) {
            return Some((p.display().to_string(), parse_pci_ids(&text)));
        }
    }
    None
}

fn parse_pci_ids(text: &str) -> PciDb {
    let mut vendors = HashMap::new();
    let mut devices = HashMap::new();
    let mut current_vendor: Option<u16> = None;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('\t') {
            if rest.starts_with('\t') {
                continue; // subsystem line
            }
            if let Some(v) = current_vendor {
                if rest.len() >= 5 {
                    if let Ok(did) = u16::from_str_radix(rest[..4].trim(), 16) {
                        let name = rest[4..].trim().to_string();
                        devices.insert((v, did), name);
                    }
                }
            }
        } else if line.len() >= 5 {
            if let Ok(vid) = u16::from_str_radix(line[..4].trim(), 16) {
                let name = line[4..].trim().to_string();
                vendors.insert(vid, name);
                current_vendor = Some(vid);
            }
        }
    }
    // 内置兜底厂商名，即使没有 pci.ids 也能认出常见设备。
    for (id, name) in builtin_vendors() {
        vendors.entry(id).or_insert_with(|| name.to_string());
    }
    PciDb { vendors, devices }
}

fn builtin_vendors() -> [(u16, &'static str); 12] {
    [
        (0x8086, "Intel Corporation"),
        (0x1022, "Advanced Micro Devices, Inc. [AMD/ATI]"),
        (0x10de, "NVIDIA Corporation"),
        (0x1af4, "Red Hat, Inc. (Virtio)"),
        (0x15ad, "VMware"),
        (0x1b36, "QEMU"),
        (0x14e4, "Broadcom"),
        (0x1969, "Qualcomm Atheros"),
        (0x10ec, "Realtek Semiconductor"),
        (0x1002, "AMD/ATI"),
        (0x144d, "Samsung Electronics (NVMe)"),
        (0x1d6a, "Fungible / Amazon (Nitro)"),
    ]
}

fn lookup(db: Option<&PciDb>, vendor: &str, device: &str) -> (Option<String>, Option<String>) {
    let vid = u16::from_str_radix(vendor, 16).ok();
    let did = u16::from_str_radix(device, 16).ok();
    let mut vendor_name = vid.and_then(|id| {
        db.and_then(|d| d.vendors.get(&id).cloned())
            .or_else(|| builtin_vendors().iter().find(|(i, _)| *i == id).map(|(_, n)| n.to_string()))
    });
    let device_name = match (db, vid, did) {
        (Some(d), Some(v), Some(dev)) => d.devices.get(&(v, dev)).cloned(),
        _ => None,
    };
    if vendor_name.is_none() {
        if let Some(v) = vid {
            vendor_name = builtin_vendors()
                .iter()
                .find(|(i, _)| *i == v)
                .map(|(_, n)| n.to_string());
        }
    }
    (vendor_name, device_name)
}

fn class_name(code: &str) -> String {
    let v = u32::from_str_radix(code.trim_start_matches("0x"), 16).unwrap_or(0);
    let base = ((v >> 16) & 0xFF) as u8;
    let sub = ((v >> 8) & 0xFF) as u8;
    let base_s = match base {
        0x00 => "Unclassified",
        0x01 => "Mass storage",
        0x02 => "Network",
        0x03 => "Display",
        0x04 => "Multimedia",
        0x05 => "Memory",
        0x06 => "Bridge",
        0x07 => "Communication",
        0x08 => "System peripheral",
        0x09 => "Input",
        0x0a => "Docking",
        0x0b => "Processor",
        0x0c => "Serial bus",
        0x0d => "Wireless",
        0x12 => "Processing accelerators",
        _ => "Other",
    };
    let extra = match (base, sub) {
        (0x01, 0x00) => "SCSI",
        (0x01, 0x01) => "IDE",
        (0x01, 0x06) => "SATA",
        (0x01, 0x07) => "SAS",
        (0x01, 0x08) => "NVMe",
        (0x01, 0x80) => "Other (virtio-blk 等)",
        (0x02, 0x00) => "Ethernet",
        (0x03, 0x00) => "VGA",
        (0x06, 0x00) => "Host",
        (0x06, 0x01) => "ISA",
        (0x06, 0x04) => "PCI-to-PCI",
        (0x0c, 0x03) => "USB",
        (0x0c, 0x05) => "SMBus",
        _ => "",
    };
    if extra.is_empty() {
        base_s.to_string()
    } else {
        format!("{base_s} / {extra}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ids_and_lookup() {
        let text = "\
8086  Intel Corporation
\t0d57  Star Lake Host Bridge
1af4  Red Hat, Inc.
\t1041  Virtio network device
";
        let db = parse_pci_ids(text);
        let (v, d) = lookup(Some(&db), "8086", "0d57");
        assert_eq!(v.as_deref(), Some("Intel Corporation"));
        assert_eq!(d.as_deref(), Some("Star Lake Host Bridge"));
        let (v2, _) = lookup(None, "1af4", "1042");
        assert!(v2.unwrap().contains("Virtio"));
    }

    #[test]
    fn missing_link_and_msi_stay_compact() {
        let d = PciDevice {
            slot: "0000:00:03.0".into(),
            vendor_id: "1af4".into(),
            device_id: "1041".into(),
            class_code: "020000".into(),
            revision: Sample::missing("revision"),
            subsystem_vendor: Sample::missing("subsystem_vendor"),
            subsystem_device: Sample::missing("subsystem_device"),
            irq: Sample::missing("irq"),
            numa_node: Sample::missing("numa_node"),
            enable: Sample::ok("1".into(), "enable"),
            current_link_speed: Sample::missing("current_link_speed"),
            current_link_width: Sample::missing("current_link_width"),
            max_link_speed: Sample::missing("max_link_speed"),
            max_link_width: Sample::missing("max_link_width"),
            msi_irqs: Sample::missing("msi_irqs"),
            local_cpulist: Sample::missing("local_cpulist"),
            driver: Sample::ok("virtio-pci".into(), "driver"),
            vendor_name: Some("Red Hat, Inc. (Virtio)".into()),
            device_name: None,
            class_name: "Network".into(),
        };
        let label = d.link_label();
        assert_eq!(label, "— msi —");
        assert!(!label.contains("current_link"));
        assert!(!label.contains("msi_irqs"));
    }
}
