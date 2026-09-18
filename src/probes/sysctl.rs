//! 内核可调参数与控制台：`/proc/sys/*`、`/proc/consoles`。不调用 `sysctl`。

use serde::Serialize;

use crate::access::{self, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct SysctlReport {
    pub file_nr_alloc: Sample<u64>,
    pub file_nr_max: Sample<u64>,
    pub inode_nr: Sample<u64>,
    pub pid_max: Sample<u64>,
    pub threads_max: Sample<u64>,
    pub swappiness: Sample<u64>,
    pub overcommit_memory: Sample<String>,
    pub overcommit_ratio: Sample<u64>,
    pub dirty_ratio: Sample<u64>,
    pub dirty_background_ratio: Sample<u64>,
    pub aslr: Sample<String>,
    pub core_pattern: Sample<String>,
    pub printk: Sample<String>,
    pub ip_forward: Sample<String>,
    pub aio_nr: Sample<u64>,
    pub aio_max_nr: Sample<u64>,
    pub inotify_max_user_watches: Sample<u64>,
    pub inotify_max_user_instances: Sample<u64>,
    pub nr_open: Sample<u64>,
    pub max_map_count: Sample<u64>,
    pub mmap_min_addr: Sample<u64>,
    pub boot_id: Sample<String>,
    pub consoles: Vec<Console>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Console {
    pub name: String,
    pub flags: String,
    pub extra: String,
}

pub fn collect(ctx: &ProbeCtx) -> SysctlReport {
    let mut notes = Vec::new();
    let file_nr = access::read_trimmed(ctx.proc_path("sys/fs/file-nr"));
    let (file_nr_alloc, file_nr_max) = parse_file_nr(&file_nr);
    let inode = access::read_trimmed(ctx.proc_path("sys/fs/inode-nr"));
    let inode_nr = match inode.value.as_deref() {
        Some(t) => t
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .map(|v| Sample::ok(v, inode.source.clone()))
            .unwrap_or_else(|| Sample::error(inode.source.clone(), "无法解析 inode-nr")),
        None => Sample {
            value: None,
            access: inode.access,
            source: inode.source,
            hint: inode.hint,
        },
    };
    let consoles = parse_consoles(&access::read_trimmed(ctx.proc_path("consoles")));
    if consoles.is_empty() {
        notes.push("无 /proc/consoles 行（容器里常见）。".into());
    }
    SysctlReport {
        file_nr_alloc,
        file_nr_max,
        inode_nr,
        pid_max: access::read_u64(ctx.proc_path("sys/kernel/pid_max")),
        threads_max: access::read_u64(ctx.proc_path("sys/kernel/threads-max")),
        swappiness: access::read_u64(ctx.proc_path("sys/vm/swappiness")),
        overcommit_memory: access::read_trimmed(ctx.proc_path("sys/vm/overcommit_memory")),
        overcommit_ratio: access::read_u64(ctx.proc_path("sys/vm/overcommit_ratio")),
        dirty_ratio: access::read_u64(ctx.proc_path("sys/vm/dirty_ratio")),
        dirty_background_ratio: access::read_u64(ctx.proc_path("sys/vm/dirty_background_ratio")),
        aslr: access::read_trimmed(ctx.proc_path("sys/kernel/randomize_va_space")),
        core_pattern: access::read_trimmed(ctx.proc_path("sys/kernel/core_pattern")),
        printk: access::read_trimmed(ctx.proc_path("sys/kernel/printk")),
        ip_forward: access::read_trimmed(ctx.proc_path("sys/net/ipv4/ip_forward")),
        aio_nr: access::read_u64(ctx.proc_path("sys/fs/aio-nr")),
        aio_max_nr: access::read_u64(ctx.proc_path("sys/fs/aio-max-nr")),
        inotify_max_user_watches: access::read_u64(ctx.proc_path("sys/fs/inotify/max_user_watches")),
        inotify_max_user_instances: access::read_u64(
            ctx.proc_path("sys/fs/inotify/max_user_instances"),
        ),
        nr_open: access::read_u64(ctx.proc_path("sys/fs/nr_open")),
        max_map_count: access::read_u64(ctx.proc_path("sys/vm/max_map_count")),
        mmap_min_addr: access::read_u64(ctx.proc_path("sys/vm/mmap_min_addr")),
        boot_id: access::read_trimmed(ctx.proc_path("sys/kernel/random/boot_id")),
        consoles,
        notes,
    }
}

/// `file-nr`：已分配、未用、上限。
pub fn parse_file_nr(sample: &Sample<String>) -> (Sample<u64>, Sample<u64>) {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return (miss(), miss());
    };
    let mut it = text.split_whitespace();
    let alloc = it.next().and_then(|s| s.parse().ok());
    let _unused = it.next();
    let max = it.next().and_then(|s| s.parse().ok());
    (
        alloc
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
        max.map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
    )
}

pub fn parse_consoles(sample: &Sample<String>) -> Vec<Console> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let flags = it.next().unwrap_or("").to_string();
        let extra = it.collect::<Vec<_>>().join(" ");
        out.push(Console {
            name: name.to_string(),
            flags,
            extra,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn file_nr_and_console_lines() {
        let nr = Sample::ok("864\t0\t9223372036854775807".into(), "file-nr");
        let (a, m) = parse_file_nr(&nr);
        assert_eq!(a.value, Some(864));
        assert_eq!(m.value, Some(9223372036854775807));
        let cons = parse_consoles(&Sample::ok(
            "ttyS0                -W- (EC    a)    4:64\n".into(),
            "consoles",
        ));
        assert_eq!(cons[0].name, "ttyS0");
        assert_eq!(cons[0].flags, "-W-");
        assert!(cons[0].extra.contains("4:64"));
    }

    #[test]
    fn collect_sysctl_fixture() {
        let root = std::env::temp_dir().join(format!("aida-sysctl-{}", std::process::id()));
        fs::create_dir_all(root.join("proc/sys/fs")).unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel")).unwrap();
        fs::create_dir_all(root.join("proc/sys/vm")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4")).unwrap();
        fs::write(root.join("proc/sys/fs/file-nr"), "10\t0\t100\n").unwrap();
        fs::write(root.join("proc/sys/fs/inode-nr"), "20\t5\n").unwrap();
        fs::write(root.join("proc/sys/kernel/pid_max"), "32768\n").unwrap();
        fs::write(root.join("proc/sys/kernel/threads-max"), "1000\n").unwrap();
        fs::write(root.join("proc/sys/vm/swappiness"), "60\n").unwrap();
        fs::write(root.join("proc/sys/vm/overcommit_memory"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/randomize_va_space"), "2\n").unwrap();
        fs::write(root.join("proc/sys/kernel/core_pattern"), "core\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/fs/inotify")).unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel/random")).unwrap();
        fs::write(root.join("proc/sys/fs/aio-nr"), "0\n").unwrap();
        fs::write(root.join("proc/sys/fs/aio-max-nr"), "65536\n").unwrap();
        fs::write(root.join("proc/sys/fs/inotify/max_user_watches"), "8192\n").unwrap();
        fs::write(root.join("proc/sys/kernel/random/boot_id"), "aaaa-bbbb\n").unwrap();
        fs::write(root.join("proc/consoles"), "tty0                 -WU (E    )    4:1\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.file_nr_alloc.value, Some(10));
        assert_eq!(r.pid_max.value, Some(32768));
        assert_eq!(r.aslr.value.as_deref(), Some("2"));
        assert_eq!(r.consoles[0].name, "tty0");
        assert_eq!(r.aio_max_nr.value, Some(65536));
        assert_eq!(r.boot_id.value.as_deref(), Some("aaaa-bbbb"));
        let _ = fs::remove_dir_all(&root);
    }
}
