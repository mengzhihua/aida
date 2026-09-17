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
    pub hugepages: Vec<HugePagePool>,
    pub notes: Vec<String>,
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
    let hugepages = read_hugepages(ctx);
    if hugepages.is_empty() {
        notes.push("未发现 hugepages-* 池（内核未启用大页时正常）。".into());
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
        hugepages,
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
            "MemTotal:        16384 kB\nMemAvailable:     8000 kB\nMemFree:          1000 kB\nBuffers:           100 kB\nCached:           2000 kB\nSwapTotal:           0 kB\nSwapFree:            0 kB\nDirty:               1 kB\nAnonPages:         300 kB\nShmem:              10 kB\nMapped:            20 kB\nSReclaimable:      30 kB\nCommitLimit:     8000 kB\nCommitted_AS:     4000 kB\n",
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
        let _ = fs::remove_dir_all(&root);
    }
}
