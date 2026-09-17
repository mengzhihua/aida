//! 挂载点与 swap：`/proc/self/mountinfo`、`/proc/swaps`。不调用 `findmnt`/`swapon`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct FsReport {
    pub mounts: Vec<Mount>,
    pub swaps: Vec<Swap>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Mount {
    pub mount_id: u32,
    pub parent_id: u32,
    pub dev: String,
    pub root: String,
    pub target: String,
    pub options: String,
    pub fstype: String,
    pub source: String,
    pub kind: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct Swap {
    pub filename: String,
    pub kind: String,
    pub size_kb: u64,
    pub used_kb: u64,
    pub priority: i64,
}

pub fn collect(ctx: &ProbeCtx) -> FsReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("self/mountinfo"));
    let mounts = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_mountinfo(text),
        _ => {
            notes.push(sample.access_label());
            Vec::new()
        }
    };
    let swaps = parse_swaps(&access::read_trimmed(ctx.proc_path("swaps")));
    if mounts.is_empty() && notes.is_empty() {
        notes.push("mountinfo 为空。".into());
    }
    FsReport {
        mounts,
        swaps,
        notes,
    }
}

pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines().filter_map(parse_mountinfo_line).collect()
}

pub fn parse_mountinfo_line(line: &str) -> Option<Mount> {
    let (left, right) = line.split_once(" - ")?;
    let mut l = left.split_whitespace();
    let mount_id = l.next()?.parse().ok()?;
    let parent_id = l.next()?.parse().ok()?;
    let dev = l.next()?.to_string();
    let root = l.next()?.to_string();
    let target = l.next()?.to_string();
    let options = l.next()?.to_string();
    let mut r = right.split_whitespace();
    let fstype = r.next()?.to_string();
    let source = r.next().unwrap_or("-").to_string();
    let kind = classify(&fstype);
    Some(Mount {
        mount_id,
        parent_id,
        dev,
        root,
        target,
        options,
        fstype,
        source,
        kind,
    })
}

fn classify(fstype: &str) -> &'static str {
    match fstype {
        "proc" | "sysfs" | "devpts" | "cgroup" | "cgroup2" | "pstore" | "bpf" | "debugfs"
        | "tracefs" | "securityfs" | "fusectl" | "configfs" | "mqueue" | "hugetlbfs"
        | "autofs" | "nsfs" | "binfmt_misc" | "devtmpfs" | "efivarfs" | "resctrl" => "virtual",
        "tmpfs" | "ramfs" | "shm" => "tmpfs",
        _ => "storage",
    }
}

fn parse_swaps(sample: &Sample<String>) -> Vec<Swap> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let mut it = line.split_whitespace();
        let Some(filename) = it.next() else { continue };
        let Some(kind) = it.next() else { continue };
        let Some(size_kb) = it.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        let Some(used_kb) = it.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        let priority = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        out.push(Swap {
            filename: filename.to_string(),
            kind: kind.to_string(),
            size_kb,
            used_kb,
            priority,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_root_and_proc() {
        let overlay = "255 182 0:39 / / rw,relatime - overlay overlay rw,lowerdir=/a,upperdir=/b";
        let m = parse_mountinfo_line(overlay).unwrap();
        assert_eq!(m.target, "/");
        assert_eq!(m.fstype, "overlay");
        assert_eq!(m.kind, "storage");
        let proc = "256 255 0:47 / /proc rw,relatime - proc proc rw";
        let p = parse_mountinfo_line(proc).unwrap();
        assert_eq!(p.kind, "virtual");
        assert_eq!(p.fstype, "proc");
    }

    #[test]
    fn swap_file_line() {
        let s = Sample::ok(
            "Filename Type Size Used Priority\n/swapfile file 1048572 0 -2\n".into(),
            "swaps",
        );
        let v = parse_swaps(&s);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].filename, "/swapfile");
        assert_eq!(v[0].size_kb, 1048572);
    }
}
