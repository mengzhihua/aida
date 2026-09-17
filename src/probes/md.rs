//! MD RAID：`/proc/mdstat` + `/sys/block/md*`。不调用 `mdadm`/`cat`。

use serde::Serialize;

use crate::access::{self, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct MdReport {
    pub personalities: String,
    pub arrays: Vec<MdArray>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MdArray {
    pub name: String,
    pub level: String,
    pub state: String,
    pub members: String,
    pub detail: String,
    pub chunk_size: Sample<String>,
    pub degraded: Sample<String>,
    pub sync_action: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> MdReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("mdstat"));
    let (personalities, mut arrays) = match sample.value.as_deref() {
        Some(text) => parse_mdstat(text),
        None => {
            notes.push(sample.access_label());
            (String::new(), Vec::new())
        }
    };
    for a in &mut arrays {
        let dir = ctx.sys_path(format!("block/{}", a.name));
        a.chunk_size = access::read_trimmed(dir.join("md/chunk_size"));
        a.degraded = access::read_trimmed(dir.join("md/degraded"));
        a.sync_action = access::read_trimmed(dir.join("md/sync_action"));
    }
    if arrays.is_empty() {
        notes.push("无 MD RAID 阵列（virtio/NVMe 单盘时正常）。".into());
    }
    MdReport {
        personalities,
        arrays,
        notes,
    }
}

pub fn parse_mdstat(text: &str) -> (String, Vec<MdArray>) {
    let mut personalities = String::new();
    let mut arrays = Vec::new();
    let mut cur: Option<MdArray> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Personalities :") {
            personalities = rest.trim().to_string();
            continue;
        }
        if line.starts_with("unused devices") {
            if let Some(a) = cur.take() {
                arrays.push(a);
            }
            continue;
        }
        if !line.starts_with(' ') && line.contains(" : ") {
            if let Some(a) = cur.take() {
                arrays.push(a);
            }
            let Some((name, rest)) = line.split_once(" : ") else {
                continue;
            };
            let (state, level, members) = parse_array_header(rest, &personalities);
            cur = Some(MdArray {
                name: name.trim().to_string(),
                level,
                state,
                members,
                detail: String::new(),
                chunk_size: Sample::missing("md/chunk_size"),
                degraded: Sample::missing("md/degraded"),
                sync_action: Sample::missing("md/sync_action"),
            });
        } else if line.starts_with(' ') {
            if let Some(a) = cur.as_mut() {
                let extra = line.trim();
                if a.detail.is_empty() {
                    a.detail = extra.to_string();
                } else {
                    a.detail.push(' ');
                    a.detail.push_str(extra);
                }
            }
        }
    }
    if let Some(a) = cur {
        arrays.push(a);
    }
    (personalities, arrays)
}

fn parse_array_header(rest: &str, personalities: &str) -> (String, String, String) {
    let known = personality_names(personalities);
    let mut toks = rest.split_whitespace().peekable();
    let state = toks.next().unwrap_or("").to_string();
    while toks.peek().is_some_and(|t| t.starts_with('(')) {
        toks.next();
    }
    let level = match toks.peek().copied() {
        Some(t) if is_raid_level(t, &known) => toks.next().unwrap_or("").to_string(),
        _ => String::new(),
    };
    let members = toks.collect::<Vec<_>>().join(" ");
    (state, level, members)
}

fn personality_names(personalities: &str) -> Vec<String> {
    personalities
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == '[' || c == ']').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn is_raid_level(tok: &str, known: &[String]) -> bool {
    let t = tok.trim_matches(|c| c == '[' || c == ']');
    t.starts_with("raid")
        || matches!(t, "linear" | "multipath" | "faulty")
        || known.iter().any(|k| k == t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_raid1() {
        let text = "\
Personalities : [raid1] [raid6]
md0 : active raid1 sda1[0] sdb1[1]
      1048576 blocks super 1.2 [2/2] [UU]
unused devices: <none>
";
        let (p, a) = parse_mdstat(text);
        assert!(p.contains("raid1"));
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, "md0");
        assert_eq!(a[0].level, "raid1");
        assert_eq!(a[0].state, "active");
        assert!(a[0].members.contains("sda1"));
        assert!(a[0].detail.contains("[UU]"));
    }

    #[test]
    fn inactive_and_auto_read_only() {
        let text = "\
Personalities : [raid1]
md127 : inactive sdb11
md0 : active (auto-read-only) raid1 sda1[0] sdb1[1]
      1024 blocks
unused devices: <none>
";
        let (_, a) = parse_mdstat(text);
        let inactive = a.iter().find(|x| x.name == "md127").unwrap();
        assert_eq!(inactive.state, "inactive");
        assert_eq!(inactive.level, "");
        assert_eq!(inactive.members, "sdb11");
        let ro = a.iter().find(|x| x.name == "md0").unwrap();
        assert_eq!(ro.state, "active");
        assert_eq!(ro.level, "raid1");
        assert!(ro.members.contains("sda1"));
        assert!(!ro.level.contains("auto"));
    }

    #[test]
    fn sysfs_degraded() {
        let root = std::env::temp_dir().join(format!("aida-md-{}", std::process::id()));
        fs::create_dir_all(root.join("proc")).unwrap();
        fs::write(
            root.join("proc/mdstat"),
            "Personalities : [raid1]\nmd0 : active raid1 sda1[0]\n      1024 blocks\nunused devices: <none>\n",
        )
        .unwrap();
        let md = root.join("sys/block/md0/md");
        fs::create_dir_all(&md).unwrap();
        fs::write(md.join("degraded"), "1\n").unwrap();
        fs::write(md.join("sync_action"), "idle\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.arrays[0].degraded.value.as_deref(), Some("1"));
        let _ = fs::remove_dir_all(&root);
    }
}
