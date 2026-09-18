//! 块设备：`/sys/block`，覆盖 virtio / SATA / NVMe 命名空间，不调用 lsblk。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct BlockReport {
    pub devices: Vec<BlockDevice>,
    pub loops: Vec<LoopDevice>,
    pub mapper: Vec<DmDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DmDevice {
    pub name: String,
    pub mapper_name: Sample<String>,
    pub uuid: Sample<String>,
    pub suspended: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopDevice {
    pub name: String,
    pub size_bytes: Sample<u64>,
    pub backing_file: Sample<String>,
    pub autoclear: Sample<String>,
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
    pub physical_block_size: Sample<u64>,
    pub logical_block_size: Sample<u64>,
    pub nr_requests: Sample<u64>,
    pub discard_max_bytes: Sample<u64>,
    pub dax: Sample<String>,
    pub write_cache: Sample<String>,
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
                loops: Vec::new(),
                mapper: Vec::new(),
                notes,
            };
        }
    };

    let stats = parse_diskstats(ctx);
    let mut devices = Vec::new();
    let mut loops = Vec::new();
    let mut mapper = Vec::new();
    let mut unused_loops = 0usize;
    for name in names {
        if name.starts_with("loop") {
            match read_loop(&root.join(&name), &name) {
                LoopRead::Used(lp) => loops.push(lp),
                LoopRead::Idle => unused_loops += 1,
                LoopRead::Failed(lp) => {
                    notes.push(lp.backing_file.access_label());
                    loops.push(lp);
                }
            }
            continue;
        }
        if name.starts_with("ram") || name.starts_with("zram") {
            continue;
        }
        if name.starts_with("dm-") {
            let dir = root.join(&name);
            mapper.push(DmDevice {
                mapper_name: access::read_trimmed(dir.join("dm/name")),
                uuid: access::read_trimmed(dir.join("dm/uuid")),
                suspended: access::read_trimmed(dir.join("dm/suspended")),
                name: name.clone(),
            });
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
            physical_block_size: access::read_u64(dir.join("queue/physical_block_size")),
            logical_block_size: access::read_u64(dir.join("queue/logical_block_size")),
            nr_requests: access::read_u64(dir.join("queue/nr_requests")),
            discard_max_bytes: access::read_u64(dir.join("queue/discard_max_bytes")),
            dax: access::read_trimmed(dir.join("queue/dax")),
            write_cache: access::read_trimmed(dir.join("queue/write_cache")),
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
    loops.sort_by(|a, b| a.name.cmp(&b.name));
    mapper.sort_by(|a, b| a.name.cmp(&b.name));
    if unused_loops > 0 {
        notes.push(format!(
            "{unused_loops} 个 loop 空闲（无 backing_file）。不要调用 losetup。"
        ));
    }
    if mapper.is_empty() {
        notes.push("无 device-mapper 设备（无 LVM/crypt 时常见）。".into());
    }
    BlockReport {
        devices,
        loops,
        mapper,
        notes,
    }
}

enum LoopRead {
    Used(LoopDevice),
    Idle,
    Failed(LoopDevice),
}

fn backing_is_idle(backing: &Sample<String>) -> bool {
    match backing.access {
        AccessKind::NotFound => true,
        AccessKind::Ok => backing
            .value
            .as_deref()
            .map(|s| s.is_empty())
            .unwrap_or(true),
        _ => false,
    }
}

fn read_loop(dir: &std::path::Path, name: &str) -> LoopRead {
    let backing = access::read_trimmed(dir.join("loop/backing_file"));
    if backing_is_idle(&backing) {
        return LoopRead::Idle;
    }
    let failed = backing.access != AccessKind::Ok;
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
    let lp = LoopDevice {
        size_bytes: size,
        backing_file: backing,
        autoclear: access::read_trimmed(dir.join("loop/autoclear")),
        name: name.to_string(),
    };
    if failed {
        LoopRead::Failed(lp)
    } else {
        LoopRead::Used(lp)
    }
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
        std::fs::write(dir.join("queue/physical_block_size"), "512\n").unwrap();
        std::fs::write(dir.join("queue/logical_block_size"), "512\n").unwrap();
        std::fs::write(dir.join("queue/nr_requests"), "256\n").unwrap();
        std::fs::write(dir.join("queue/discard_max_bytes"), "0\n").unwrap();
        std::fs::write(dir.join("queue/dax"), "0\n").unwrap();
        std::fs::write(dir.join("queue/write_cache"), "write through\n").unwrap();
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
        assert_eq!(r.devices[0].physical_block_size.value, Some(512));
        assert_eq!(r.devices[0].nr_requests.value, Some(256));
        assert_eq!(r.devices[0].write_cache.value.as_deref(), Some("write through"));
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

    #[test]
    fn loop_with_backing_skips_empty() {
        let root = std::env::temp_dir().join(format!("aida-loop-{}", std::process::id()));
        let l0 = root.join("sys/block/loop0");
        std::fs::create_dir_all(l0.join("loop")).unwrap();
        std::fs::write(l0.join("size"), "0\n").unwrap();
        let l1 = root.join("sys/block/loop1");
        std::fs::create_dir_all(l1.join("loop")).unwrap();
        std::fs::write(l1.join("size"), "2048\n").unwrap();
        std::fs::write(l1.join("loop/backing_file"), "/tmp/disk.img\n").unwrap();
        std::fs::write(l1.join("loop/autoclear"), "0\n").unwrap();
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
        assert!(r.devices.is_empty());
        assert_eq!(r.loops.len(), 1);
        assert_eq!(r.loops[0].name, "loop1");
        assert_eq!(r.loops[0].backing_file.value.as_deref(), Some("/tmp/disk.img"));
        assert_eq!(r.loops[0].size_bytes.value, Some(2048 * 512));
        assert!(r.notes.iter().any(|n| n.contains("loop")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_backing_is_not_idle() {
        let denied = Sample {
            value: None,
            access: AccessKind::PermissionDenied,
            source: "loop0/backing_file".into(),
            hint: Some("权限不足".into()),
        };
        assert!(!backing_is_idle(&denied));
        assert!(backing_is_idle(&Sample::missing("loop1/backing_file")));
        assert!(backing_is_idle(&Sample::ok(String::new(), "empty")));
    }

    #[test]
    fn device_mapper_sysfs() {
        let root = std::env::temp_dir().join(format!("aida-dm-{}", std::process::id()));
        let dm = root.join("sys/block/dm-0");
        std::fs::create_dir_all(dm.join("dm")).unwrap();
        std::fs::create_dir_all(dm.join("queue")).unwrap();
        std::fs::write(dm.join("size"), "2048\n").unwrap();
        std::fs::write(dm.join("queue/rotational"), "0\n").unwrap();
        std::fs::write(dm.join("dm/name"), "vg-root\n").unwrap();
        std::fs::write(dm.join("dm/uuid"), "LVM-abc\n").unwrap();
        std::fs::write(dm.join("dm/suspended"), "0\n").unwrap();
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
        assert_eq!(r.mapper.len(), 1);
        assert_eq!(r.mapper[0].mapper_name.value.as_deref(), Some("vg-root"));
        assert_eq!(r.devices[0].r#type, "Device Mapper");
        let _ = std::fs::remove_dir_all(&root);
    }
}
