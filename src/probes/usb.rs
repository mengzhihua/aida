//! USB 树：`/sys/bus/usb/devices`，不调用 `lsusb`。

use std::fs;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct UsbReport {
    pub devices: Vec<UsbDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UsbDevice {
    pub sys_name: String,
    pub busnum: Sample<String>,
    pub devnum: Sample<String>,
    pub vendor_id: Sample<String>,
    pub product_id: Sample<String>,
    pub manufacturer: Sample<String>,
    pub product: Sample<String>,
    pub serial: Sample<String>,
    pub speed: Sample<String>,
    pub class_code: Sample<String>,
    pub parent: Option<String>,
    pub vendor_name: Option<String>,
    pub product_name: Option<String>,
}

pub fn collect(ctx: &ProbeCtx) -> UsbReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("bus/usb/devices");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            return UsbReport {
                devices: Vec::new(),
                notes,
            };
        }
    };
    let db = load_usb_ids(ctx);
    let mut devices = Vec::new();
    for name in names {
        if name.contains(':') {
            continue; // 接口节点 1-1:1.0
        }
        let dir = root.join(&name);
        let vendor_id = hex4(access::read_trimmed(dir.join("idVendor")));
        let product_id = hex4(access::read_trimmed(dir.join("idProduct")));
        let (vendor_name, product_name) = lookup(
            db.as_ref(),
            vendor_id.value.as_deref(),
            product_id.value.as_deref(),
        );
        devices.push(UsbDevice {
            parent: parent_of(&name),
            sys_name: name,
            busnum: access::read_trimmed(dir.join("busnum")),
            devnum: access::read_trimmed(dir.join("devnum")),
            manufacturer: access::read_trimmed(dir.join("manufacturer")),
            product: access::read_trimmed(dir.join("product")),
            serial: access::read_trimmed(dir.join("serial")),
            speed: access::read_trimmed(dir.join("speed")),
            class_code: access::read_trimmed(dir.join("bDeviceClass")),
            vendor_id,
            product_id,
            vendor_name,
            product_name,
        });
    }
    devices.sort_by(|a, b| a.sys_name.cmp(&b.sys_name));
    if devices.is_empty() {
        notes.push("没有 USB 设备节点（无头虚拟机常见）。".into());
    }
    UsbReport { devices, notes }
}

fn hex4(s: Sample<String>) -> Sample<String> {
    match (s.access, s.value) {
        (AccessKind::Ok, Some(v)) => Sample::ok(v.trim().to_ascii_lowercase(), s.source),
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

fn parent_of(name: &str) -> Option<String> {
    // 1-2.3 → 1-2 ；1-2 → usb1 ；usb1 → None
    if name.starts_with("usb") {
        return None;
    }
    if let Some((bus, rest)) = name.split_once('-') {
        if let Some((hub, _)) = rest.rsplit_once('.') {
            return Some(format!("{bus}-{hub}"));
        }
        return Some(format!("usb{bus}"));
    }
    None
}

struct UsbDb {
    vendors: std::collections::HashMap<u16, String>,
    products: std::collections::HashMap<(u16, u16), String>,
}

fn load_usb_ids(ctx: &ProbeCtx) -> Option<UsbDb> {
    let candidates = [
        ctx.usr_share.join("misc/usb.ids"),
        ctx.usr_share.join("hwdata/usb.ids"),
    ];
    for p in candidates {
        if let Ok(text) = fs::read_to_string(&p) {
            return Some(parse_usb_ids(&text));
        }
    }
    None
}

fn parse_usb_ids(text: &str) -> UsbDb {
    let mut vendors = std::collections::HashMap::new();
    let mut products = std::collections::HashMap::new();
    let mut cur: Option<u16> = None;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('\t') {
            if rest.starts_with('\t') {
                continue;
            }
            if let Some(v) = cur {
                if rest.len() >= 5 {
                    if let Ok(pid) = u16::from_str_radix(rest[..4].trim(), 16) {
                        products.insert((v, pid), rest[4..].trim().to_string());
                    }
                }
            }
        } else if line.len() >= 5 {
            if let Ok(vid) = u16::from_str_radix(line[..4].trim(), 16) {
                vendors.insert(vid, line[4..].trim().to_string());
                cur = Some(vid);
            }
        }
    }
    UsbDb { vendors, products }
}

fn lookup(
    db: Option<&UsbDb>,
    vendor: Option<&str>,
    product: Option<&str>,
) -> (Option<String>, Option<String>) {
    let vid = vendor.and_then(|s| u16::from_str_radix(s, 16).ok());
    let pid = product.and_then(|s| u16::from_str_radix(s, 16).ok());
    let vname = vid.and_then(|id| db.and_then(|d| d.vendors.get(&id).cloned()));
    let pname = match (db, vid, pid) {
        (Some(d), Some(v), Some(p)) => d.products.get(&(v, p)).cloned(),
        _ => None,
    };
    (vname, pname)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parent_chain() {
        assert_eq!(parent_of("usb1"), None);
        assert_eq!(parent_of("1-2").as_deref(), Some("usb1"));
        assert_eq!(parent_of("1-2.3").as_deref(), Some("1-2"));
    }

    #[test]
    fn usb_fixture() {
        let root = std::env::temp_dir().join(format!("aida-usb-{}", std::process::id()));
        let dev = root.join("sys/bus/usb/devices/1-1");
        fs::create_dir_all(&dev).unwrap();
        fs::create_dir_all(root.join("sys/bus/usb/devices/1-1:1.0")).unwrap();
        fs::write(dev.join("idVendor"), "1d6b\n").unwrap();
        fs::write(dev.join("idProduct"), "0002\n").unwrap();
        fs::write(dev.join("manufacturer"), "Linux\n").unwrap();
        fs::write(dev.join("product"), "Hub\n").unwrap();
        fs::write(dev.join("busnum"), "1\n").unwrap();
        fs::write(dev.join("devnum"), "2\n").unwrap();
        fs::write(dev.join("speed"), "480\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].product.value.as_deref(), Some("Hub"));
        assert_eq!(r.devices[0].parent.as_deref(), Some("usb1"));
        let _ = fs::remove_dir_all(&root);
    }
}
