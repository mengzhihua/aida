//! 挂载点与 swap：`/proc/self/mountinfo`、`/proc/swaps`。不调用 `findmnt`/`swapon`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct FsReport {
    pub mounts: Vec<Mount>,
    pub swaps: Vec<Swap>,
    pub ext4: Vec<Ext4Fs>,
    pub xfs_stats: Sample<String>,
    pub nfsd_threads: Sample<u64>,
    pub nfs_volumes: usize,
    pub fuse_conns: usize,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Ext4Fs {
    pub name: String,
    pub lifetime_write_kbytes: Sample<u64>,
    pub session_write_kbytes: Sample<u64>,
    pub errors_count: Sample<u64>,
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
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub avail_bytes: Option<u64>,
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
    let mut mounts = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_mountinfo(text),
        _ => {
            notes.push(sample.access_label());
            Vec::new()
        }
    };
    for m in &mut mounts {
        if m.kind == "virtual" {
            continue;
        }
        if let Some((total, used, avail)) = usage_of(&m.target) {
            m.total_bytes = Some(total);
            m.used_bytes = Some(used);
            m.avail_bytes = Some(avail);
        }
    }
    let swaps = parse_swaps(&access::read_trimmed(ctx.proc_path("swaps")));
    let ext4 = read_ext4(ctx);
    let xfs_stats = xfs_rw_summary(ctx);
    let nfsd_threads = access::read_u64(ctx.proc_path("fs/nfsd/threads"));
    let nfs_volumes = count_nfs_volumes(&access::read_trimmed(ctx.proc_path("net/nfsfs/volumes")));
    let fuse_conns = match access::list_dir_names(ctx.sys_path("fs/fuse/connections")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n.len(),
        _ => 0,
    };
    if mounts.is_empty() && notes.is_empty() {
        notes.push("mountinfo 为空。".into());
    }
    FsReport {
        mounts,
        swaps,
        ext4,
        xfs_stats,
        nfsd_threads,
        nfs_volumes,
        fuse_conns,
        notes,
    }
}

fn count_nfs_volumes(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .count()
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
        total_bytes: None,
        used_bytes: None,
        avail_bytes: None,
    })
}

fn classify(fstype: &str) -> &'static str {
    match fstype {
        "proc" | "sysfs" | "devpts" | "cgroup" | "cgroup2" | "pstore" | "bpf" | "debugfs"
        | "tracefs" | "securityfs" | "fusectl" | "configfs" | "mqueue" | "hugetlbfs"
        | "autofs" | "nsfs" | "binfmt_misc" | "devtmpfs" | "efivarfs" | "resctrl"
        | "selinuxfs" | "rpc_pipefs" => "virtual",
        "tmpfs" | "ramfs" | "shm" => "tmpfs",
        _ => "storage",
    }
}

/// `statvfs(2)`，等价于 `df` 的块用量，不 spawn `df`/`findmnt`。
pub fn usage_of(path: &str) -> Option<(u64, u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut s = unsafe { std::mem::zeroed::<libc::statvfs>() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let fr = if s.f_frsize != 0 {
        s.f_frsize as u64
    } else {
        s.f_bsize as u64
    };
    let total = (s.f_blocks as u64).saturating_mul(fr);
    if total == 0 {
        return None;
    }
    let free = (s.f_bfree as u64).saturating_mul(fr);
    let avail = (s.f_bavail as u64).saturating_mul(fr);
    let used = total.saturating_sub(free);
    Some((total, used, avail))
}

fn read_ext4(ctx: &ProbeCtx) -> Vec<Ext4Fs> {
    let root = ctx.sys_path("fs/ext4");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        if name == "features" {
            continue;
        }
        let dir = root.join(&name);
        out.push(Ext4Fs {
            lifetime_write_kbytes: access::read_u64(dir.join("lifetime_write_kbytes")),
            session_write_kbytes: access::read_u64(dir.join("session_write_kbytes")),
            errors_count: access::read_u64(dir.join("errors_count")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn xfs_rw_summary(ctx: &ProbeCtx) -> Sample<String> {
    let sample = access::read_trimmed(ctx.sys_path("fs/xfs/stats/stats"));
    match sample.value.as_deref() {
        Some(text) => {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("rw ") {
                    return Sample::ok(rest.trim().to_string(), sample.source);
                }
            }
            Sample::ok(String::new(), sample.source)
        }
        None => Sample {
            value: None,
            access: sample.access,
            source: sample.source,
            hint: sample.hint,
        },
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
        let se = "90 18 0:23 / /sys/fs/selinux rw - selinuxfs selinuxfs rw";
        assert_eq!(parse_mountinfo_line(se).unwrap().kind, "virtual");
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

    #[test]
    fn nfs_volume_rows() {
        let n = count_nfs_volumes(&Sample::ok(
            "NV SERVER PORT DEV      FSC\nv4 10.0.0.1 2049 0:42     no\n".into(),
            "volumes",
        ));
        assert_eq!(n, 1);
    }

    #[test]
    fn statvfs_tmp_has_size() {
        let (total, used, avail) = usage_of("/tmp").expect("statvfs /tmp");
        assert!(total > 0);
        assert!(used <= total);
        assert!(avail <= total);
    }

    #[test]
    fn ext4_sysfs_fixture() {
        let root = std::env::temp_dir().join(format!("aida-fs-{}", std::process::id()));
        let e = root.join("sys/fs/ext4/vda");
        std::fs::create_dir_all(&e).unwrap();
        std::fs::create_dir_all(root.join("sys/fs/ext4/features")).unwrap();
        std::fs::write(e.join("lifetime_write_kbytes"), "100\n").unwrap();
        std::fs::write(e.join("session_write_kbytes"), "10\n").unwrap();
        std::fs::write(e.join("errors_count"), "0\n").unwrap();
        std::fs::create_dir_all(root.join("sys/fs/xfs/stats")).unwrap();
        std::fs::write(root.join("sys/fs/xfs/stats/stats"), "rw 3 4\nattr 0 0\n").unwrap();
        std::fs::create_dir_all(root.join("proc/self")).unwrap();
        std::fs::write(root.join("proc/self/mountinfo"), "").unwrap();
        std::fs::write(root.join("proc/swaps"), "Filename Type Size Used Priority\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ext4.len(), 1);
        assert_eq!(r.ext4[0].name, "vda");
        assert_eq!(r.ext4[0].lifetime_write_kbytes.value, Some(100));
        assert_eq!(r.xfs_stats.value.as_deref(), Some("3 4"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
