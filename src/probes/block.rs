//! 块设备：`/sys/block`，覆盖 virtio / SATA / NVMe 命名空间，不调用 lsblk。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct BlockReport {
    pub devices: Vec<BlockDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockDevice {
    pub name: String,
    pub size_bytes: Sample<u64>,
    pub rotational: Sample<bool>,
    pub model: Sample<String>,
    pub vendor: Sample<String>,
    pub serial: Sample<String>,
    pub queue_scheduler: Sample<String>,
    pub removable: Sample<String>,
    pub r#type: String,
}

pub fn collect(ctx: &ProbeCtx) -> BlockReport {
    let root = ctx.sys_path("block");
    let mut notes = Vec::new();
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            return BlockReport {
                devices: Vec::new(),
                notes,
            };
        }
    };

    let mut devices = Vec::new();
    for name in names {
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
            continue;
        }
        // 跳过分区：sda1 / nvme0n1p1 / vda1。保留 nvme0n1 / vda / sda。
        if is_partition(&name) {
            continue;
        }
        let dir = root.join(&name);
        let size = match access::read_trimmed(dir.join("size")) {
            Sample {
                access: AccessKind::Ok,
                value: Some(s),
                source,
                ..
            } => match s.parse::<u64>() {
                Ok(sectors) => Sample::ok(sectors.saturating_mul(512), source),
                Err(_) => Sample::error(source, "无法解析 size"),
            },
            s => Sample {
                value: None,
                access: s.access,
                source: s.source,
                hint: s.hint,
            },
        };
        let rotational = match access::read_trimmed(dir.join("queue/rotational")) {
            Sample {
                access: AccessKind::Ok,
                value: Some(s),
                source,
                ..
            } => Sample::ok(s.trim() == "1", source),
            s => Sample {
                value: None,
                access: s.access,
                source: s.source,
                hint: s.hint,
            },
        };
        devices.push(BlockDevice {
            r#type: classify(&name, rotational.value),
            name,
            size_bytes: size,
            rotational,
            model: first_existing(&[
                dir.join("device/model"),
                dir.join("device/name"),
            ]),
            vendor: access::read_trimmed(dir.join("device/vendor")),
            serial: access::read_trimmed(dir.join("serial")),
            queue_scheduler: access::read_trimmed(dir.join("queue/scheduler")),
            removable: access::read_trimmed(dir.join("removable")),
        });
    }
    BlockReport { devices, notes }
}

fn first_existing(paths: &[std::path::PathBuf]) -> Sample<String> {
    let mut last = None;
    for p in paths {
        let s = access::read_trimmed(p);
        if s.access == AccessKind::Ok && s.value.is_some() {
            return s;
        }
        last = Some(s);
    }
    last.unwrap_or_else(|| Sample::missing("model"))
}

fn is_partition(name: &str) -> bool {
    if name.starts_with("nvme") || name.starts_with("mmcblk") || name.starts_with("loop") {
        return name.contains('p') && name.rsplit('p').next().map(|s| s.chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
    }
    // sda1, vda2, xvda3
    let mut chars = name.chars().rev();
    let mut saw_digit = false;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            saw_digit = true;
            continue;
        }
        return saw_digit && c.is_ascii_alphabetic();
    }
    false
}

fn classify(name: &str, rotational: Option<bool>) -> String {
    if name.starts_with("nvme") {
        "NVMe".into()
    } else if name.starts_with("vd") || name.starts_with("xvd") {
        "Virtio / Xen 虚拟盘".into()
    } else if name.starts_with("md") {
        "MD RAID".into()
    } else if name.starts_with("dm-") {
        "Device Mapper".into()
    } else if rotational == Some(true) {
        "HDD (rotational)".into()
    } else if rotational == Some(false) {
        "SSD / 非旋转盘".into()
    } else {
        "Block".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_filter() {
        assert!(is_partition("sda1"));
        assert!(is_partition("vda2"));
        assert!(!is_partition("vda"));
        assert!(!is_partition("sda"));
        assert!(is_partition("nvme0n1p1"));
        assert!(!is_partition("nvme0n1"));
    }
}
