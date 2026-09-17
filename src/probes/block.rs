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
    pub rd_ios: Sample<u64>,
    pub wr_ios: Sample<u64>,
    pub rd_bytes: Sample<u64>,
    pub wr_bytes: Sample<u64>,
    /// 两次采样之间的吞吐；单次 collect 未差分时为 None。
    pub rd_bps: Option<f64>,
    pub wr_bps: Option<f64>,
    pub partitions: Vec<BlockPart>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockPart {
    pub name: String,
    pub size_bytes: Sample<u64>,
}

#[derive(Clone, Debug)]
pub struct DiskSnap {
    pub name: String,
    pub rd_bytes: u64,
    pub wr_bytes: u64,
}

pub fn collect(ctx: &ProbeCtx) -> BlockReport {
    collect_with_prev(ctx, None, 0.0)
}

pub fn collect_with_prev(ctx: &ProbeCtx, prev: Option<&[DiskSnap]>, dt_sec: f64) -> BlockReport {
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

    let stats = parse_diskstats(ctx);
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
        let io = stats.get(&name);
        let rd_bytes = io.map(|s| Sample::ok(s.rd_bytes, "/proc/diskstats"));
        let wr_bytes = io.map(|s| Sample::ok(s.wr_bytes, "/proc/diskstats"));
        let (rd_bps, wr_bps) = match (prev, io) {
            (Some(p), Some(now)) if dt_sec > 0.0 => {
                if let Some(old) = p.iter().find(|x| x.name == name) {
                    (
                        Some((now.rd_bytes.saturating_sub(old.rd_bytes) as f64) / dt_sec),
                        Some((now.wr_bytes.saturating_sub(old.wr_bytes) as f64) / dt_sec),
                    )
                } else {
                    (None, None)
                }
            }
            _ => (None, None),
        };
        let partitions = list_partitions(&dir, &name);
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
            rd_ios: io
                .map(|s| Sample::ok(s.rd_ios, "/proc/diskstats"))
                .unwrap_or_else(|| Sample::missing("/proc/diskstats")),
            wr_ios: io
                .map(|s| Sample::ok(s.wr_ios, "/proc/diskstats"))
                .unwrap_or_else(|| Sample::missing("/proc/diskstats")),
            rd_bytes: rd_bytes.unwrap_or_else(|| Sample::missing("/proc/diskstats")),
            wr_bytes: wr_bytes.unwrap_or_else(|| Sample::missing("/proc/diskstats")),
            rd_bps,
            wr_bps,
            partitions,
        });
    }
    BlockReport { devices, notes }
}

pub fn counters(report: &BlockReport) -> Vec<DiskSnap> {
    report
        .devices
        .iter()
        .filter_map(|d| {
            Some(DiskSnap {
                name: d.name.clone(),
                rd_bytes: d.rd_bytes.value?,
                wr_bytes: d.wr_bytes.value?,
            })
        })
        .collect()
}

#[derive(Clone)]
struct DiskStat {
    rd_ios: u64,
    wr_ios: u64,
    rd_bytes: u64,
    wr_bytes: u64,
}

fn parse_diskstats(ctx: &ProbeCtx) -> std::collections::HashMap<String, DiskStat> {
    let mut map = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(ctx.proc_path("diskstats")) else {
        return map;
    };
    for line in text.lines() {
        if let Some((name, st)) = parse_diskstats_line(line) {
            map.insert(name, st);
        }
    }
    map
}

fn parse_diskstats_line(line: &str) -> Option<(String, DiskStat)> {
    let mut it = line.split_whitespace();
    let _maj = it.next()?;
    let _min = it.next()?;
    let name = it.next()?.to_string();
    let rd_ios = it.next()?.parse().ok()?;
    let _rd_merge = it.next()?;
    let rd_sect = it.next()?.parse::<u64>().ok()?;
    let _rd_tick = it.next()?;
    let wr_ios = it.next()?.parse().ok()?;
    let _wr_merge = it.next()?;
    let wr_sect = it.next()?.parse::<u64>().ok()?;
    Some((
        name,
        DiskStat {
            rd_ios,
            wr_ios,
            rd_bytes: rd_sect.saturating_mul(512),
            wr_bytes: wr_sect.saturating_mul(512),
        },
    ))
}

fn list_partitions(dir: &std::path::Path, parent: &str) -> Vec<BlockPart> {
    let names = match access::list_dir_names(dir).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        if !name.starts_with(parent) || name == parent || !is_partition(&name) {
            continue;
        }
        let size = match access::read_trimmed(dir.join(&name).join("size")) {
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
        out.push(BlockPart {
            name,
            size_bytes: size,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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

    #[test]
    fn diskstats_rate() {
        let root = std::env::temp_dir().join(format!("aida-blk-{}", std::process::id()));
        let dir = root.join("sys/block/vda");
        std::fs::create_dir_all(dir.join("queue")).unwrap();
        std::fs::write(dir.join("size"), "2048\n").unwrap();
        std::fs::write(dir.join("queue/rotational"), "0\n").unwrap();
        std::fs::create_dir_all(root.join("proc")).unwrap();
        std::fs::write(
            root.join("proc/diskstats"),
            " 254 0 vda 10 0 200 0 20 0 400 0 0 0 0\n",
        )
        .unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let prev = [DiskSnap {
            name: "vda".into(),
            rd_bytes: 100 * 512,
            wr_bytes: 100 * 512,
        }];
        let r = collect_with_prev(&ctx, Some(&prev), 1.0);
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].rd_bytes.value, Some(200 * 512));
        assert_eq!(r.devices[0].rd_bps, Some(100.0 * 512.0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_child_partitions() {
        let root = std::env::temp_dir().join(format!("aida-part-{}", std::process::id()));
        let dir = root.join("sys/block/vda");
        std::fs::create_dir_all(dir.join("queue")).unwrap();
        std::fs::write(dir.join("size"), "2048\n").unwrap();
        std::fs::write(dir.join("queue/rotational"), "0\n").unwrap();
        let p1 = dir.join("vda1");
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::write(p1.join("size"), "1024\n").unwrap();
        std::fs::create_dir_all(root.join("proc")).unwrap();
        std::fs::write(root.join("proc/diskstats"), "").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].partitions.len(), 1);
        assert_eq!(r.devices[0].partitions[0].name, "vda1");
        assert_eq!(r.devices[0].partitions[0].size_bytes.value, Some(1024 * 512));
        let _ = std::fs::remove_dir_all(&root);
    }
}
