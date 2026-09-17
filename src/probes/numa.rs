//! NUMA：`/sys/devices/system/node/nodeN`，不调用 `numactl`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct NumaReport {
    pub nodes: Vec<NumaNode>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumaNode {
    pub id: u32,
    pub cpulist: Sample<String>,
    pub mem_total_kb: Sample<u64>,
    pub mem_free_kb: Sample<u64>,
    pub mem_used_kb: Sample<u64>,
    pub distance: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> NumaReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("devices/system/node");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            return NumaReport {
                nodes: Vec::new(),
                notes,
            };
        }
    };
    let mut nodes = Vec::new();
    for name in names.into_iter().filter(|n| {
        n.starts_with("node") && n.len() > 4 && n[4..].chars().all(|c| c.is_ascii_digit())
    }) {
        let id: u32 = name[4..].parse().unwrap_or(0);
        let dir = root.join(&name);
        let mem = parse_node_meminfo(&access::read_trimmed(dir.join("meminfo")));
        nodes.push(NumaNode {
            id,
            cpulist: access::read_trimmed(dir.join("cpulist")),
            mem_total_kb: mem.total,
            mem_free_kb: mem.free,
            mem_used_kb: mem.used,
            distance: access::read_trimmed(dir.join("distance")),
        });
    }
    nodes.sort_by_key(|n| n.id);
    if nodes.len() <= 1 {
        notes.push(
            "单 NUMA 节点（或未启用 NUMA）。内存带宽基准仍是整机 STREAM，不是按节点绑核。".into(),
        );
    }
    NumaReport { nodes, notes }
}

struct Mem {
    total: Sample<u64>,
    free: Sample<u64>,
    used: Sample<u64>,
}

fn parse_node_meminfo(sample: &Sample<String>) -> Mem {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return Mem {
            total: miss(),
            free: miss(),
            used: miss(),
        };
    };
    let mut total = None;
    let mut free = None;
    let mut used = None;
    for line in text.lines() {
        // "Node 0 MemTotal:  123 kB"
        let t = line.trim();
        let n = t
            .split_whitespace()
            .rev()
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok());
        if t.contains("MemTotal:") {
            total = n;
        } else if t.contains("MemFree:") {
            free = n;
        } else if t.contains("MemUsed:") {
            used = n;
        }
    }
    Mem {
        total: total
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
        free: free
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
        used: used
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_meminfo_and_fixture() {
        let root = std::env::temp_dir().join(format!("aida-numa-{}", std::process::id()));
        let node = root.join("sys/devices/system/node/node0");
        fs::create_dir_all(&node).unwrap();
        fs::write(node.join("cpulist"), "0-3\n").unwrap();
        fs::write(
            node.join("meminfo"),
            "Node 0 MemTotal:       16398384 kB\nNode 0 MemFree:         1000 kB\nNode 0 MemUsed:        2000 kB\n",
        )
        .unwrap();
        fs::write(node.join("distance"), "10\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.nodes.len(), 1);
        assert_eq!(r.nodes[0].mem_total_kb.value, Some(16398384));
        assert_eq!(r.nodes[0].cpulist.value.as_deref(), Some("0-3"));
        let _ = fs::remove_dir_all(&root);
    }
}
