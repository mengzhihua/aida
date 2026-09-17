//! cgroup v2：`/sys/fs/cgroup`。不调用 `systemd-cgls`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct CgroupReport {
    pub controllers: Sample<String>,
    pub subtree_control: Sample<String>,
    pub memory_current: Sample<u64>,
    pub cpu_usage_usec: Sample<u64>,
    pub nr_descendants: Sample<u64>,
    pub groups: Vec<CgroupNode>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CgroupNode {
    pub name: String,
    pub memory_current: Sample<u64>,
    pub procs: Sample<u64>,
}

pub fn collect(ctx: &ProbeCtx) -> CgroupReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("fs/cgroup");
    let controllers = access::read_trimmed(root.join("cgroup.controllers"));
    if controllers.access == AccessKind::NotFound {
        notes.push("无 cgroup v2 根（旧内核或未挂 cgroup2 时常见）。".into());
        return CgroupReport {
            controllers,
            subtree_control: Sample::missing("cgroup.subtree_control"),
            memory_current: Sample::missing("memory.current"),
            cpu_usage_usec: Sample::missing("cpu.stat"),
            nr_descendants: Sample::missing("cgroup.stat"),
            groups: Vec::new(),
            notes,
        };
    }
    if controllers.access != AccessKind::Ok && controllers.access != AccessKind::NotFound {
        notes.push(controllers.access_label());
    }
    let cpu_usage_usec = parse_cpu_usage(&access::read_trimmed(root.join("cpu.stat")));
    let nr_descendants = parse_cgroup_stat_field(
        &access::read_trimmed(root.join("cgroup.stat")),
        "nr_descendants",
    );
    let groups = list_groups(&root);
    CgroupReport {
        subtree_control: access::read_trimmed(root.join("cgroup.subtree_control")),
        memory_current: access::read_u64(root.join("memory.current")),
        cpu_usage_usec,
        nr_descendants,
        groups,
        controllers,
        notes,
    }
}

fn list_groups(root: &std::path::Path) -> Vec<CgroupNode> {
    let names = match access::list_dir_names(root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        if name.contains('.') && !(name.ends_with(".slice") || name.ends_with(".scope")) {
            continue;
        }
        let dir = root.join(&name);
        if !dir.is_dir() {
            continue;
        }
        if !dir.join("cgroup.procs").exists() {
            continue;
        }
        out.push(CgroupNode {
            memory_current: access::read_u64(dir.join("memory.current")),
            procs: count_procs(&access::read_trimmed(dir.join("cgroup.procs"))),
            name,
        });
        if out.len() >= 24 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn count_procs(sample: &Sample<String>) -> Sample<u64> {
    match sample.value.as_deref() {
        Some(t) => Sample::ok(
            t.lines().filter(|l| !l.trim().is_empty()).count() as u64,
            sample.source.clone(),
        ),
        None => Sample {
            value: None,
            access: sample.access,
            source: sample.source.clone(),
            hint: sample.hint.clone(),
        },
    }
}

pub fn parse_cpu_usage(sample: &Sample<String>) -> Sample<u64> {
    let Some(text) = sample.value.as_deref() else {
        return Sample {
            value: None,
            access: sample.access,
            source: sample.source.clone(),
            hint: sample.hint.clone(),
        };
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("usage_usec ") {
            if let Ok(v) = rest.trim().parse::<u64>() {
                return Sample::ok(v, sample.source.clone());
            }
        }
    }
    Sample::error(sample.source.clone(), "cpu.stat 无 usage_usec")
}

fn parse_cgroup_stat_field(sample: &Sample<String>, key: &str) -> Sample<u64> {
    let Some(text) = sample.value.as_deref() else {
        return Sample {
            value: None,
            access: sample.access,
            source: sample.source.clone(),
            hint: sample.hint.clone(),
        };
    };
    let prefix = format!("{key} ");
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            if let Ok(v) = rest.trim().parse::<u64>() {
                return Sample::ok(v, sample.source.clone());
            }
        }
    }
    Sample::error(sample.source.clone(), format!("cgroup.stat 无 {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn root_and_slice_fixture() {
        let root = std::env::temp_dir().join(format!("aida-cg-{}", std::process::id()));
        let cg = root.join("sys/fs/cgroup");
        fs::create_dir_all(cg.join("system.slice")).unwrap();
        fs::write(cg.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        fs::write(cg.join("cgroup.subtree_control"), "cpu memory\n").unwrap();
        fs::write(cg.join("memory.current"), "4096\n").unwrap();
        fs::write(cg.join("cpu.stat"), "usage_usec 12345\nuser_usec 1000\n").unwrap();
        fs::write(cg.join("cgroup.stat"), "nr_descendants 3\nnr_dying_descendants 0\n").unwrap();
        fs::write(cg.join("cgroup.procs"), "1\n").unwrap();
        fs::write(cg.join("system.slice/cgroup.procs"), "10\n20\n").unwrap();
        fs::write(cg.join("system.slice/memory.current"), "2048\n").unwrap();
        fs::create_dir_all(cg.join("dev-hugepages.mount")).unwrap();
        fs::write(cg.join("dev-hugepages.mount/cgroup.procs"), "").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.memory_current.value, Some(4096));
        assert_eq!(r.cpu_usage_usec.value, Some(12345));
        assert_eq!(r.nr_descendants.value, Some(3));
        assert!(r.controllers.value.as_deref().unwrap().contains("memory"));
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].name, "system.slice");
        assert_eq!(r.groups[0].procs.value, Some(2));
        let _ = fs::remove_dir_all(&root);
    }
}
