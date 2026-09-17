//! SATA/ATA：`/sys/class/ata_{port,link,device}`。不调用 `hdparm`/`smartctl`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct AtaReport {
    pub ports: Vec<AtaPort>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AtaPort {
    pub name: String,
    pub port_no: Sample<String>,
    pub nr_pmp_links: Sample<String>,
    pub links: Vec<AtaLink>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AtaLink {
    pub name: String,
    pub sata_spd: Sample<String>,
    pub sata_spd_limit: Sample<String>,
    pub devices: Vec<AtaDevice>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AtaDevice {
    pub name: String,
    pub class: Sample<String>,
    pub trim: Sample<String>,
    pub model: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> AtaReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("class/ata_port");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            } else {
                notes.push(
                    "无 ATA/SATA 端口。NVMe 或 virtio 盘不走 ata_port；桌面 SATA 才会导出。"
                        .into(),
                );
            }
            return AtaReport {
                ports: Vec::new(),
                notes,
            };
        }
    };
    let mut ports = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("ata")) {
        let dir = root.join(&name);
        ports.push(AtaPort {
            port_no: access::read_trimmed(dir.join("port_no")),
            nr_pmp_links: access::read_trimmed(dir.join("nr_pmp_links")),
            links: collect_links(ctx, &name),
            name,
        });
    }
    AtaReport { ports, notes }
}

fn collect_links(ctx: &ProbeCtx, port: &str) -> Vec<AtaLink> {
    let root = ctx.sys_path("class/ata_link");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    // linkN 对应 ataN（PMP 时还有 linkN.M）。
    let prefix = port.strip_prefix("ata").unwrap_or(port);
    let mut out = Vec::new();
    for name in names {
        let rest = name.strip_prefix("link").unwrap_or(&name);
        if rest != prefix && !rest.starts_with(&format!("{prefix}.")) {
            continue;
        }
        let dir = root.join(&name);
        out.push(AtaLink {
            sata_spd: access::read_trimmed(dir.join("sata_spd")),
            sata_spd_limit: access::read_trimmed(dir.join("sata_spd_limit")),
            devices: collect_devices(ctx, &name),
            name,
        });
    }
    out
}

fn collect_devices(ctx: &ProbeCtx, link: &str) -> Vec<AtaDevice> {
    let root = ctx.sys_path("class/ata_device");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let suffix = link.strip_prefix("link").unwrap_or(link);
    let mut out = Vec::new();
    for name in names {
        let rest = name.strip_prefix("dev").unwrap_or(&name);
        if rest != suffix && !rest.starts_with(&format!("{suffix}.")) {
            continue;
        }
        let dir = root.join(&name);
        out.push(AtaDevice {
            class: access::read_trimmed(dir.join("class")),
            trim: access::read_trimmed(dir.join("trim")),
            model: identify_model(&access::read_bytes(dir.join("id"))),
            name,
        });
    }
    out
}

fn identify_model(id: &Sample<Vec<u8>>) -> Sample<String> {
    match (id.access, id.value.as_deref()) {
        (AccessKind::Ok, Some(buf)) => {
            if let Some(m) = model_from_identify(buf) {
                Sample::ok(m, id.source.clone())
            } else {
                Sample::error(id.source.clone(), "无法从 IDENTIFY 解析型号")
            }
        }
        _ => Sample {
            value: None,
            access: id.access,
            source: id.source.clone(),
            hint: id.hint.clone(),
        },
    }
}

/// ATA IDENTIFY DEVICE 字 27–46 为型号；512 字节原始缓冲按字对调。短文本当 sysfs 直出。
pub fn model_from_identify(buf: &[u8]) -> Option<String> {
    if buf.len() >= 512 {
        let mut s = String::new();
        for w in 27..47 {
            let o = w * 2;
            s.push(buf[o + 1] as char);
            s.push(buf[o] as char);
        }
        let t = s.replace('\0', " ").trim().to_string();
        if t.chars().any(|c| c.is_ascii_graphic()) {
            return Some(t);
        }
    }
    let t = std::str::from_utf8(buf).ok()?.trim();
    if t.is_empty() || t.len() > 80 {
        return None;
    }
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn identify_swapped_model() {
        let mut buf = vec![0u8; 512];
        // words 27-46 at bytes 54-93: "TESTDISK MODEL           " with ATA swap
        let model = b"TESTDISK MODEL           ";
        for (i, chunk) in model.chunks(2).enumerate() {
            let o = 54 + i * 2;
            buf[o + 1] = chunk[0];
            buf[o] = chunk.get(1).copied().unwrap_or(b' ');
        }
        assert_eq!(model_from_identify(&buf).unwrap().trim(), "TESTDISK MODEL");
    }

    #[test]
    fn port_link_device_fixture() {
        let root = std::env::temp_dir().join(format!("aida-ata-{}", std::process::id()));
        let port = root.join("sys/class/ata_port/ata1");
        let link = root.join("sys/class/ata_link/link1");
        let dev = root.join("sys/class/ata_device/dev1.0");
        fs::create_dir_all(&port).unwrap();
        fs::create_dir_all(&link).unwrap();
        fs::create_dir_all(&dev).unwrap();
        fs::write(port.join("port_no"), "1\n").unwrap();
        fs::write(port.join("nr_pmp_links"), "0\n").unwrap();
        fs::write(link.join("sata_spd"), "6.0 Gbps\n").unwrap();
        fs::write(link.join("sata_spd_limit"), "6.0 Gbps\n").unwrap();
        fs::write(dev.join("class"), "ata\n").unwrap();
        fs::write(dev.join("trim"), "queued\n").unwrap();
        fs::write(dev.join("id"), "SSD 1TB\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ports.len(), 1);
        assert_eq!(r.ports[0].links[0].sata_spd.value.as_deref(), Some("6.0 Gbps"));
        assert_eq!(r.ports[0].links[0].devices[0].model.value.as_deref(), Some("SSD 1TB"));
        let _ = fs::remove_dir_all(&root);
    }
}
