//! 内存：`/proc/meminfo` + hugepages + THP。不调用 `free`/`numactl`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct MemoryReport {
    pub total_kb: Sample<u64>,
    pub available_kb: Sample<u64>,
    pub free_kb: Sample<u64>,
    pub buffers_kb: Sample<u64>,
    pub cached_kb: Sample<u64>,
    pub swap_total_kb: Sample<u64>,
    pub swap_free_kb: Sample<u64>,
    pub dirty_kb: Sample<u64>,
    pub anon_kb: Sample<u64>,
    pub shmem_kb: Sample<u64>,
    pub mapped_kb: Sample<u64>,
    pub sreclaimable_kb: Sample<u64>,
    pub commit_limit_kb: Sample<u64>,
    pub committed_as_kb: Sample<u64>,
    pub thp_enabled: Sample<String>,
    pub thp_defrag: Sample<String>,
    pub thp_shmem: Sample<String>,
    pub anon_huge_kb: Sample<u64>,
    pub vmalloc_used_kb: Sample<u64>,
    pub directmap_4k_kb: Sample<u64>,
    pub directmap_2m_kb: Sample<u64>,
    pub directmap_1g_kb: Sample<u64>,
    pub hugepages: Vec<HugePagePool>,
    pub buddy: Vec<BuddyZone>,
    pub vmstat: Vmstat,
    pub ksm: KsmInfo,
    pub mem_blocks: MemoryBlocks,
    pub memory_tiers: Vec<String>,
    pub zones: Vec<MemZone>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuddyZone {
    pub node: u32,
    pub zone: String,
    /// order 0, 1, 2… 的空闲块个数（见 `/proc/buddyinfo`）。
    pub free_counts: Vec<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Vmstat {
    pub nr_free_pages: Sample<u64>,
    pub pgfault: Sample<u64>,
    pub pgmajfault: Sample<u64>,
    pub pgpgin: Sample<u64>,
    pub pgpgout: Sample<u64>,
    pub pswpin: Sample<u64>,
    pub pswpout: Sample<u64>,
    pub oom_kill: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct KsmInfo {
    pub run: Sample<String>,
    pub pages_shared: Sample<u64>,
    pub pages_sharing: Sample<u64>,
    pub pages_unshared: Sample<u64>,
    pub full_scans: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryBlocks {
    /// 内核 ABI 为十六进制；已换算成字节。
    pub block_size_bytes: Sample<u64>,
    pub total: usize,
    pub online: usize,
    pub offline: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemZone {
    pub node: u32,
    pub zone: String,
    pub free: Option<u64>,
    pub present: Option<u64>,
    pub managed: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HugePagePool {
    pub size_kb: u64,
    pub nr: Sample<u64>,
    pub free: Sample<u64>,
    pub surplus: Sample<u64>,
}

pub fn collect(ctx: &ProbeCtx) -> MemoryReport {
    let mut notes = Vec::new();
    let mem = parse_meminfo(&access::read_trimmed(ctx.proc_path("meminfo")));
    let thp_enabled = access::read_trimmed(ctx.sys_path("kernel/mm/transparent_hugepage/enabled"));
    let thp_defrag = access::read_trimmed(ctx.sys_path("kernel/mm/transparent_hugepage/defrag"));
    let thp_shmem = access::read_trimmed(ctx.sys_path("kernel/mm/transparent_hugepage/shmem_enabled"));
    let hugepages = read_hugepages(ctx);
    if hugepages.is_empty() {
        notes.push("未发现 hugepages-* 池（内核未启用大页时正常）。".into());
    }
    let buddy = parse_buddyinfo(&access::read_trimmed(ctx.proc_path("buddyinfo")));
    let vmstat = parse_vmstat(&access::read_trimmed(ctx.proc_path("vmstat")));
    let ksm = read_ksm(ctx);
    let mem_blocks = read_memory_blocks(ctx);
    let memory_tiers = match access::list_dir_names(ctx.sys_path("bus/memory_tiering/devices")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(mut n),
            ..
        } => {
            n.retain(|x| x.starts_with("memory_tier"));
            n.sort();
            n.truncate(8);
            n
        }
        s if s.access == AccessKind::PermissionDenied || s.access == AccessKind::Error => {
            notes.push(s.access_label());
            Vec::new()
        }
        _ => Vec::new(),
    };
    let zones = parse_zoneinfo(&access::read_trimmed(ctx.proc_path("zoneinfo")));
    if zones.is_empty() {
        notes.push("无 zoneinfo（容器或权限不足时常见）。".into());
    }
    if let (Some(total), Some(avail)) = (mem.total_kb.value, mem.available_kb.value) {
        if total > 0 {
            let used_pct = 100.0 * (total.saturating_sub(avail) as f64) / total as f64;
            if used_pct > 90.0 {
                notes.push(format!("可用内存偏低（约 {used_pct:.0}% 已用，按 MemAvailable 计）。"));
            }
        }
    }
    MemoryReport {
        total_kb: mem.total_kb,
        available_kb: mem.available_kb,
        free_kb: mem.free_kb,
        buffers_kb: mem.buffers_kb,
        cached_kb: mem.cached_kb,
        swap_total_kb: mem.swap_total_kb,
        swap_free_kb: mem.swap_free_kb,
        dirty_kb: mem.dirty_kb,
        anon_kb: mem.anon_kb,
        shmem_kb: mem.shmem_kb,
        mapped_kb: mem.mapped_kb,
        sreclaimable_kb: mem.sreclaimable_kb,
        commit_limit_kb: mem.commit_limit_kb,
        committed_as_kb: mem.committed_as_kb,
        thp_enabled,
        thp_defrag,
        thp_shmem,
        anon_huge_kb: mem.anon_huge_kb,
        vmalloc_used_kb: mem.vmalloc_used_kb,
        directmap_4k_kb: mem.directmap_4k_kb,
        directmap_2m_kb: mem.directmap_2m_kb,
        directmap_1g_kb: mem.directmap_1g_kb,
        hugepages,
        buddy,
        vmstat,
        ksm,
        mem_blocks,
        memory_tiers,
        zones,
        notes,
    }
}

struct ParsedMem {
    total_kb: Sample<u64>,
    available_kb: Sample<u64>,
    free_kb: Sample<u64>,
    buffers_kb: Sample<u64>,
    cached_kb: Sample<u64>,
    swap_total_kb: Sample<u64>,
    swap_free_kb: Sample<u64>,
    dirty_kb: Sample<u64>,
    anon_kb: Sample<u64>,
    shmem_kb: Sample<u64>,
    mapped_kb: Sample<u64>,
    sreclaimable_kb: Sample<u64>,
    commit_limit_kb: Sample<u64>,
    committed_as_kb: Sample<u64>,
    anon_huge_kb: Sample<u64>,
    vmalloc_used_kb: Sample<u64>,
    directmap_4k_kb: Sample<u64>,
    directmap_2m_kb: Sample<u64>,
    directmap_1g_kb: Sample<u64>,
}

fn parse_meminfo(sample: &Sample<String>) -> ParsedMem {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return ParsedMem {
            total_kb: miss(),
            available_kb: miss(),
            free_kb: miss(),
            buffers_kb: miss(),
            cached_kb: miss(),
            swap_total_kb: miss(),
            swap_free_kb: miss(),
            dirty_kb: miss(),
            anon_kb: miss(),
            shmem_kb: miss(),
            mapped_kb: miss(),
            sreclaimable_kb: miss(),
            commit_limit_kb: miss(),
            committed_as_kb: miss(),
            anon_huge_kb: miss(),
            vmalloc_used_kb: miss(),
            directmap_4k_kb: miss(),
            directmap_2m_kb: miss(),
            directmap_1g_kb: miss(),
        };
    };
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        if let Some((k, rest)) = line.split_once(':') {
            if let Some(n) = rest.split_whitespace().next().and_then(|s| s.parse::<u64>().ok()) {
                map.insert(k.trim().to_string(), n);
            }
        }
    }
    let pick = |key: &str| {
        map.get(key)
            .copied()
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss)
    };
    ParsedMem {
        total_kb: pick("MemTotal"),
        available_kb: pick("MemAvailable"),
        free_kb: pick("MemFree"),
        buffers_kb: pick("Buffers"),
        cached_kb: pick("Cached"),
        swap_total_kb: pick("SwapTotal"),
        swap_free_kb: pick("SwapFree"),
        dirty_kb: pick("Dirty"),
        anon_kb: pick("AnonPages"),
        shmem_kb: pick("Shmem"),
        mapped_kb: pick("Mapped"),
        sreclaimable_kb: pick("SReclaimable"),
        commit_limit_kb: pick("CommitLimit"),
        committed_as_kb: pick("Committed_AS"),
        anon_huge_kb: pick("AnonHugePages"),
        vmalloc_used_kb: pick("VmallocUsed"),
        directmap_4k_kb: pick("DirectMap4k"),
        directmap_2m_kb: pick("DirectMap2M"),
        directmap_1g_kb: pick("DirectMap1G"),
    }
}

fn read_hugepages(ctx: &ProbeCtx) -> Vec<HugePagePool> {
    let root = ctx.sys_path("kernel/mm/hugepages");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("hugepages-")) {
        let size_kb = name
            .trim_start_matches("hugepages-")
            .trim_end_matches("kB")
            .parse()
            .unwrap_or(0);
        let dir = root.join(&name);
        out.push(HugePagePool {
            size_kb,
            nr: read_u64(dir.join("nr_hugepages")),
            free: read_u64(dir.join("free_hugepages")),
            surplus: read_u64(dir.join("surplus_hugepages")),
        });
    }
    out.sort_by_key(|p| p.size_kb);
    out
}

pub fn parse_buddyinfo(sample: &Sample<String>) -> Vec<BuddyZone> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("Node ") {
            continue;
        }
        let Some((left, rest)) = line.split_once(", zone") else {
            continue;
        };
        let node = left
            .trim_start_matches("Node ")
            .trim()
            .parse()
            .unwrap_or(0);
        let mut toks = rest.split_whitespace();
        let zone = toks.next().unwrap_or("?").to_string();
        let free_counts: Vec<u64> = toks.filter_map(|t| t.parse().ok()).collect();
        if free_counts.is_empty() {
            continue;
        }
        out.push(BuddyZone {
            node,
            zone,
            free_counts,
        });
    }
    out
}

/// `/proc/zoneinfo`：每个 Node/zone 的 free/present/managed 页数。
pub fn parse_zoneinfo(sample: &Sample<String>) -> Vec<MemZone> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur: Option<MemZone> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Node ") {
            if let Some(z) = cur.take() {
                out.push(z);
            }
            let Some((node_s, zone_s)) = rest.split_once(", zone") else {
                continue;
            };
            let node = node_s.trim().parse().unwrap_or(0);
            let zone = zone_s.trim().to_string();
            if zone.is_empty() {
                continue;
            }
            cur = Some(MemZone {
                node,
                zone,
                free: None,
                present: None,
                managed: None,
            });
            continue;
        }
        let Some(z) = cur.as_mut() else { continue };
        let mut it = t.split_whitespace();
        match (it.next(), it.next(), it.next()) {
            (Some("pages"), Some("free"), Some(v)) => z.free = v.parse().ok(),
            (Some("present"), Some(v), _) => z.present = v.parse().ok(),
            (Some("managed"), Some(v), _) => z.managed = v.parse().ok(),
            _ => {}
        }
    }
    if let Some(z) = cur {
        out.push(z);
    }
    out
}

fn parse_vmstat(sample: &Sample<String>) -> Vmstat {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let pick = |map: &std::collections::BTreeMap<&str, u64>, key: &str| {
        map.get(key)
            .copied()
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss)
    };
    let Some(text) = sample.value.as_deref() else {
        return Vmstat {
            nr_free_pages: miss(),
            pgfault: miss(),
            pgmajfault: miss(),
            pgpgin: miss(),
            pgpgout: miss(),
            pswpin: miss(),
            pswpout: miss(),
            oom_kill: miss(),
        };
    };
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            if let Ok(n) = v.parse::<u64>() {
                map.insert(k, n);
            }
        }
    }
    Vmstat {
        nr_free_pages: pick(&map, "nr_free_pages"),
        pgfault: pick(&map, "pgfault"),
        pgmajfault: pick(&map, "pgmajfault"),
        pgpgin: pick(&map, "pgpgin"),
        pgpgout: pick(&map, "pgpgout"),
        pswpin: pick(&map, "pswpin"),
        pswpout: pick(&map, "pswpout"),
        oom_kill: pick(&map, "oom_kill"),
    }
}

fn read_ksm(ctx: &ProbeCtx) -> KsmInfo {
    let dir = ctx.sys_path("kernel/mm/ksm");
    KsmInfo {
        run: access::read_trimmed(dir.join("run")),
        pages_shared: access::read_u64(dir.join("pages_shared")),
        pages_sharing: access::read_u64(dir.join("pages_sharing")),
        pages_unshared: access::read_u64(dir.join("pages_unshared")),
        full_scans: access::read_u64(dir.join("full_scans")),
    }
}

fn read_memory_blocks(ctx: &ProbeCtx) -> MemoryBlocks {
    let root = ctx.sys_path("devices/system/memory");
    let block_size_bytes = match access::read_trimmed(root.join("block_size_bytes")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(s),
            source,
            ..
        } => match parse_hex_u64(&s) {
            Some(v) => Sample::ok(v, source),
            None => Sample::error(source, "无法解析十六进制 block_size_bytes"),
        },
        s => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    };
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => {
            return MemoryBlocks {
                block_size_bytes,
                total: 0,
                online: 0,
                offline: 0,
            };
        }
    };
    let mut total = 0usize;
    let mut online = 0usize;
    let mut offline = 0usize;
    for name in names.into_iter().filter(|n| n.starts_with("memory")) {
        if !name
            .trim_start_matches("memory")
            .chars()
            .all(|c| c.is_ascii_digit())
        {
            continue;
        }
        total += 1;
        match access::read_trimmed(root.join(&name).join("state")).value.as_deref() {
            Some("online") => online += 1,
            Some("offline") => offline += 1,
            Some(s) if s.starts_with("online") => online += 1,
            _ => {}
        }
    }
    MemoryBlocks {
        block_size_bytes,
        total,
        online,
        offline,
    }
}

/// `/sys/devices/system/memory/block_size_bytes` 按 ABI 是十六进制。
fn parse_hex_u64(s: &str) -> Option<u64> {
    let t = s
        .trim()
        .strip_prefix("0x")
        .or_else(|| s.trim().strip_prefix("0X"))
        .unwrap_or_else(|| s.trim());
    u64::from_str_radix(t, 16).ok()
}

fn read_u64(path: std::path::PathBuf) -> Sample<u64> {
    let s = access::read_trimmed(&path);
    match (s.access, s.value.as_deref()) {
        (AccessKind::Ok, Some(t)) => match t.parse::<u64>() {
            Ok(v) => Sample::ok(v, s.source),
            Err(_) => Sample::error(s.source, "无法解析"),
        },
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_meminfo_and_hugepages() {
        let root = std::env::temp_dir().join(format!("aida-mem-{}", std::process::id()));
        fs::create_dir_all(root.join("proc")).unwrap();
        fs::write(
            root.join("proc/meminfo"),
            "MemTotal:        16384 kB\nMemAvailable:     8000 kB\nMemFree:          1000 kB\nBuffers:           100 kB\nCached:           2000 kB\nSwapTotal:           0 kB\nSwapFree:            0 kB\nDirty:               1 kB\nAnonPages:         300 kB\nShmem:              10 kB\nMapped:            20 kB\nSReclaimable:      30 kB\nCommitLimit:     8000 kB\nCommitted_AS:     4000 kB\nAnonHugePages:       0 kB\nVmallocUsed:        80 kB\nDirectMap4k:      1024 kB\nDirectMap2M:      4096 kB\nDirectMap1G:         0 kB\n",
        )
        .unwrap();
        let hp = root.join("sys/kernel/mm/hugepages/hugepages-2048kB");
        fs::create_dir_all(&hp).unwrap();
        fs::write(hp.join("nr_hugepages"), "2\n").unwrap();
        fs::write(hp.join("free_hugepages"), "1\n").unwrap();
        fs::write(hp.join("surplus_hugepages"), "0\n").unwrap();
        fs::create_dir_all(root.join("sys/kernel/mm/transparent_hugepage")).unwrap();
        fs::write(
            root.join("sys/kernel/mm/transparent_hugepage/enabled"),
            "always [madvise] never\n",
        )
        .unwrap();
        fs::write(
            root.join("sys/kernel/mm/transparent_hugepage/defrag"),
            "always defer defer+madvise [madvise] never\n",
        )
        .unwrap();
        fs::write(
            root.join("sys/kernel/mm/transparent_hugepage/shmem_enabled"),
            "always within_size advise [never] deny force\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/buddyinfo"),
            "Node 0, zone      DMA      1      0      2 \nNode 0, zone   Normal     10      4      1 \n",
        )
        .unwrap();
        fs::write(
            root.join("proc/vmstat"),
            "nr_free_pages 100\npgfault 50\npgmajfault 2\npgpgin 3\npgpgout 4\npswpin 0\npswpout 0\noom_kill 0\n",
        )
        .unwrap();
        let ksm = root.join("sys/kernel/mm/ksm");
        fs::create_dir_all(&ksm).unwrap();
        fs::write(ksm.join("run"), "0\n").unwrap();
        fs::write(ksm.join("pages_shared"), "0\n").unwrap();
        fs::write(ksm.join("pages_sharing"), "0\n").unwrap();
        fs::write(ksm.join("pages_unshared"), "0\n").unwrap();
        fs::write(ksm.join("full_scans"), "7\n").unwrap();
        let mem = root.join("sys/devices/system/memory");
        fs::create_dir_all(mem.join("memory0")).unwrap();
        fs::create_dir_all(mem.join("memory1")).unwrap();
        fs::write(mem.join("block_size_bytes"), "8000000\n").unwrap();
        fs::write(mem.join("memory0/state"), "online\n").unwrap();
        fs::write(mem.join("memory1/state"), "offline\n").unwrap();
        fs::create_dir_all(root.join("sys/bus/memory_tiering/devices/memory_tier4")).unwrap();
        fs::create_dir_all(root.join("sys/bus/memory_tiering/devices/other")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.total_kb.value, Some(16384));
        assert_eq!(r.available_kb.value, Some(8000));
        assert_eq!(r.hugepages.len(), 1);
        assert_eq!(r.hugepages[0].nr.value, Some(2));
        assert!(r.thp_enabled.value.as_deref().unwrap().contains("madvise"));
        assert!(r.thp_defrag.value.as_deref().unwrap().contains("madvise"));
        assert_eq!(r.directmap_4k_kb.value, Some(1024));
        assert_eq!(r.directmap_2m_kb.value, Some(4096));
        assert_eq!(r.buddy.len(), 2);
        assert_eq!(r.buddy[1].zone, "Normal");
        assert_eq!(r.buddy[1].free_counts, vec![10, 4, 1]);
        assert_eq!(r.vmstat.pgfault.value, Some(50));
        assert_eq!(r.ksm.run.value.as_deref(), Some("0"));
        assert_eq!(r.ksm.full_scans.value, Some(7));
        assert_eq!(r.mem_blocks.block_size_bytes.value, Some(0x8000000));
        assert_eq!(r.mem_blocks.total, 2);
        assert_eq!(r.mem_blocks.online, 1);
        assert_eq!(r.mem_blocks.offline, 1);
        assert_eq!(r.memory_tiers, vec!["memory_tier4".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn zoneinfo_dma_and_normal() {
        let text = "\
Node 0, zone      DMA
  pages free     3840
        present  3998
        managed  3840
Node 0, zone   Normal
  pages free     100
        present  200
        managed  180
";
        let z = parse_zoneinfo(&Sample::ok(text.into(), "zoneinfo"));
        assert_eq!(z.len(), 2);
        assert_eq!(z[0].zone, "DMA");
        assert_eq!(z[0].free, Some(3840));
        assert_eq!(z[0].present, Some(3998));
        assert_eq!(z[1].zone, "Normal");
        assert_eq!(z[1].managed, Some(180));
    }
}
