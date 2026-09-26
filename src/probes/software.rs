//! 系统软件信息：os-release、内核、内存、启动参数。不调用 hostnamectl / uname。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct SoftwareInfo {
    pub os_name: Sample<String>,
    pub os_id: Sample<String>,
    /// `/etc/os-release` 的 `ID_LIKE`（如 ubuntu→debian，centos→rhel fedora）。
    pub os_like: Sample<String>,
    pub os_version: Sample<String>,
    pub kernel_release: Sample<String>,
    pub ostype: Sample<String>,
    pub kernel_version_banner: Sample<String>,
    pub hostname: Sample<String>,
    pub uptime_sec: Sample<f64>,
    pub cmdline: Sample<String>,
    pub mem_total_kb: Sample<u64>,
    pub mem_available_kb: Sample<u64>,
    pub swap_total_kb: Sample<u64>,
    pub desktop: Sample<String>,
    pub load_1: Sample<f64>,
    pub load_5: Sample<f64>,
    pub load_15: Sample<f64>,
    pub procs: Sample<String>,
    pub boot_time_unix: Sample<u64>,
    pub tainted: Sample<u64>,
    pub taint_flags: Vec<String>,
    pub lsm: Sample<String>,
    pub selinux_enforce: Sample<String>,
    pub entropy_avail: Sample<u64>,
    pub machine_id: Sample<String>,
    pub domainname: Sample<String>,
    pub config_gz: Sample<String>,
    pub bpf_fs_entries: usize,
    /// `/proc/locks` 行数。空文件表示当前无锁，不是读取失败。
    pub file_locks: usize,
    pub oops_count: Sample<u64>,
    pub warn_count: Sample<u64>,
    pub kexec_loaded: Sample<String>,
    pub fscaps: Sample<String>,
    pub uevent_seqnum: Sample<u64>,
    pub cpu_byteorder: Sample<String>,
    pub address_bits: Sample<String>,
    pub profiling: Sample<String>,
    pub filesystems: Vec<String>,
    /// 已装软件（dpkg/apk 状态文件）。对标 AIDA64 Software，不调用 `dpkg -l`。
    pub package_manager: Option<String>,
    pub package_count: usize,
    pub packages: Vec<InstalledPackage>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
}

pub fn collect(ctx: &ProbeCtx) -> SoftwareInfo {
    let os = parse_os_release(&access::read_trimmed(ctx.etc.join("os-release")));
    let mem = parse_meminfo(&access::read_trimmed(ctx.proc_path("meminfo")));
    let uptime = match access::read_trimmed(ctx.proc_path("uptime")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(s),
            source,
            ..
        } => match s
            .split_whitespace()
            .next()
            .and_then(|x| x.parse::<f64>().ok())
        {
            Some(v) => Sample::ok(v, source),
            None => Sample::error(source, "无法解析 uptime"),
        },
        s => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    };

    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .map(|v| Sample::ok(v, "env:XDG_CURRENT_DESKTOP|DESKTOP_SESSION"))
        .unwrap_or_else(|_| Sample::missing("env:XDG_CURRENT_DESKTOP（无图形会话时正常为空）"));

    let load = parse_loadavg(&access::read_trimmed(ctx.proc_path("loadavg")));
    let boot_time_unix = parse_btime(&access::read_trimmed(ctx.proc_path("stat")));
    let (tainted, taint_flags) =
        parse_taint(&access::read_trimmed(ctx.proc_path("sys/kernel/tainted")));

    let mut notes = Vec::new();
    let locks = access::read_trimmed(ctx.proc_path("locks"));
        let file_locks =
            if locks.access == AccessKind::PermissionDenied || locks.access == AccessKind::Error {
            notes.push(locks.access_label());
            0
        } else {
            count_lock_lines(&locks)
        };

    let (package_manager, package_count, packages) = collect_packages(ctx);

    SoftwareInfo {
        os_name: os.pretty,
        os_id: os.id,
        os_like: os.id_like,
        os_version: os.version,
        kernel_release: access::read_trimmed(ctx.proc_path("sys/kernel/osrelease")),
        ostype: access::read_trimmed(ctx.proc_path("sys/kernel/ostype")),
        kernel_version_banner: access::read_trimmed(ctx.proc_path("version")),
        hostname: access::read_trimmed(ctx.proc_path("sys/kernel/hostname")),
        uptime_sec: uptime,
        cmdline: access::read_trimmed(ctx.proc_path("cmdline")),
        mem_total_kb: mem.total,
        mem_available_kb: mem.available,
        swap_total_kb: mem.swap,
        desktop,
        load_1: load.l1,
        load_5: load.l5,
        load_15: load.l15,
        procs: load.procs,
        boot_time_unix,
        tainted,
        taint_flags,
        lsm: access::read_trimmed(ctx.sys_path("kernel/security/lsm")),
        selinux_enforce: access::read_trimmed(ctx.sys_path("fs/selinux/enforce")),
        entropy_avail: access::read_u64(ctx.proc_path("sys/kernel/random/entropy_avail")),
        machine_id: access::read_trimmed(ctx.etc.join("machine-id")),
        domainname: access::read_trimmed(ctx.proc_path("sys/kernel/domainname")),
        config_gz: config_gz_sample(ctx),
        bpf_fs_entries: match access::list_dir_names(ctx.sys_path("fs/bpf")) {
            Sample {
                access: AccessKind::Ok,
                value: Some(n),
                ..
            } => n.len(),
            _ => 0,
        },
        file_locks,
        oops_count: access::read_u64(ctx.sys_path("kernel/oops_count")),
        warn_count: access::read_u64(ctx.sys_path("kernel/warn_count")),
        kexec_loaded: access::read_trimmed(ctx.sys_path("kernel/kexec_loaded")),
        fscaps: access::read_trimmed(ctx.sys_path("kernel/fscaps")),
        uevent_seqnum: access::read_u64(ctx.sys_path("kernel/uevent_seqnum")),
        cpu_byteorder: access::read_trimmed(ctx.sys_path("kernel/cpu_byteorder")),
        address_bits: access::read_trimmed(ctx.sys_path("kernel/address_bits")),
        profiling: access::read_trimmed(ctx.sys_path("kernel/profiling")),
        filesystems: parse_filesystems(&access::read_trimmed(ctx.proc_path("filesystems"))),
        package_manager,
        package_count,
        packages,
        notes,
    }
}

/// 慢路径：更新会变的内核计数。不重读已装包和 `config.gz`（这两项在启动时读一次）。
pub fn refresh_slow(info: &mut SoftwareInfo, ctx: &ProbeCtx) {
    refresh_runtime(info, ctx);
    let (tainted, taint_flags) =
        parse_taint(&access::read_trimmed(ctx.proc_path("sys/kernel/tainted")));
    info.tainted = tainted;
    info.taint_flags = taint_flags;
    info.oops_count = access::read_u64(ctx.sys_path("kernel/oops_count"));
    info.warn_count = access::read_u64(ctx.sys_path("kernel/warn_count"));
    info.kexec_loaded = access::read_trimmed(ctx.sys_path("kernel/kexec_loaded"));
    info.uevent_seqnum = access::read_u64(ctx.sys_path("kernel/uevent_seqnum"));
}

/// GUI 快路径：只更新 loadavg / uptime / entropy，不读 os-release、config.gz、locks。
pub fn refresh_runtime(info: &mut SoftwareInfo, ctx: &ProbeCtx) {
    let uptime = match access::read_trimmed(ctx.proc_path("uptime")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(s),
            source,
            ..
        } => match s
            .split_whitespace()
            .next()
            .and_then(|x| x.parse::<f64>().ok())
        {
            Some(v) => Sample::ok(v, source),
            None => Sample::error(source, "无法解析 uptime"),
        },
        s => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    };
    let load = parse_loadavg(&access::read_trimmed(ctx.proc_path("loadavg")));
    info.uptime_sec = uptime;
    info.load_1 = load.l1;
    info.load_5 = load.l5;
    info.load_15 = load.l15;
    info.procs = load.procs;
    info.entropy_avail = access::read_u64(ctx.proc_path("sys/kernel/random/entropy_avail"));
}

/// procfs 上 `config.gz` 的 inode size 经常是 0，必须读字节才能知道压缩包长度。
/// 不解码 gzip，避免引入 flate2。
fn config_gz_sample(ctx: &ProbeCtx) -> Sample<String> {
    let path = ctx.proc_path("config.gz");
    match access::read_bytes(&path) {
        Sample {
            access: AccessKind::Ok,
            value: Some(buf),
            source,
            ..
        } => Sample::ok(format!("{} bytes", buf.len()), source),
        Sample {
            access: AccessKind::Ok,
            value: None,
            source,
            ..
        } => Sample::ok("present".into(), source),
        s => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

pub fn count_lock_lines(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// `/proc/filesystems`：只列块设备文件系统名（跳过 `nodev`）。
pub fn parse_filesystems(sample: &Sample<String>) -> Vec<String> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("nodev") {
            continue;
        }
        out.push(t.to_string());
        if out.len() >= 24 {
            break;
        }
    }
    out
}

fn parse_loadavg(sample: &Sample<String>) -> Load {
    let miss_f = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let miss_s = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return Load {
            l1: miss_f(),
            l5: miss_f(),
            l15: miss_f(),
            procs: miss_s(),
        };
    };
    let mut it = text.split_whitespace();
    let pick_f = |it: &mut std::str::SplitWhitespace, src: &str| {
        it.next()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| Sample::ok(v, src.to_string()))
            .unwrap_or_else(|| Sample::error(src.to_string(), "无法解析 loadavg"))
    };
    let src = sample.source.clone();
    Load {
        l1: pick_f(&mut it, &src),
        l5: pick_f(&mut it, &src),
        l15: pick_f(&mut it, &src),
        procs: it
            .next()
            .map(|s| Sample::ok(s.to_string(), src.clone()))
            .unwrap_or_else(miss_s),
    }
}

struct Load {
    l1: Sample<f64>,
    l5: Sample<f64>,
    l15: Sample<f64>,
    procs: Sample<String>,
}

fn parse_btime(sample: &Sample<String>) -> Sample<u64> {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return miss();
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("btime ") {
            if let Ok(v) = rest.trim().parse::<u64>() {
                return Sample::ok(v, sample.source.clone());
            }
        }
    }
    Sample::missing(sample.source.clone())
}

const TAINT_BITS: &[(u32, &str)] = &[
    (0, "P proprietary module"),
    (1, "F force load"),
    (2, "S SMP unsupported"),
    (3, "R force unload"),
    (4, "M MCE"),
    (5, "B bad page"),
    (6, "U userspace taint"),
    (7, "D died (oops/bug)"),
    (8, "A ACPI override"),
    (9, "W warning"),
    (10, "C staging driver"),
    (11, "I firmware workaround"),
    (12, "O out-of-tree module"),
    (13, "E unsigned module"),
    (14, "L soft lockup"),
    (15, "K livepatch"),
    (16, "X auxiliary taint"),
    (17, "T struct randomization"),
];

pub fn parse_taint(sample: &Sample<String>) -> (Sample<u64>, Vec<String>) {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return (miss(), Vec::new());
    };
    let Ok(v) = text.trim().parse::<u64>() else {
        return (
            Sample::error(sample.source.clone(), "无法解析 tainted"),
            Vec::new(),
        );
    };
    let flags = decode_taint(v);
    (Sample::ok(v, sample.source.clone()), flags)
}

pub fn decode_taint(v: u64) -> Vec<String> {
    TAINT_BITS
        .iter()
        .filter(|(bit, _)| v & (1u64 << bit) != 0)
        .map(|(bit, name)| format!("{bit}:{name}"))
        .collect()
}

pub(crate) struct OsRelease {
    pub pretty: Sample<String>,
    pub id: Sample<String>,
    pub id_like: Sample<String>,
    pub version: Sample<String>,
}

fn parse_os_release(sample: &Sample<String>) -> OsRelease {
    parse_os_release_fields(sample)
}

/// 解析 os-release。`ID_LIKE` 用来区分 Debian 系和 RHEL/CentOS 系。
pub(crate) fn parse_os_release_fields(sample: &Sample<String>) -> OsRelease {
    let source = sample.source.clone();
    let miss = |hint: Option<String>| Sample {
        value: None,
        access: sample.access,
        source: source.clone(),
        hint,
    };
    let fail = |access: AccessKind, hint: Option<String>| OsRelease {
        pretty: miss(hint.clone()),
        id: miss(hint.clone()),
        id_like: miss(hint.clone()),
        version: Sample {
            value: None,
            access,
            source: source.clone(),
            hint,
        },
    };
    let text = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(t)) => t,
        _ => return fail(sample.access, sample.hint.clone()),
    };
    let mut pretty = None;
    let mut id = None;
    let mut id_like = None;
    let mut version = None;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').to_string();
            match k {
                "PRETTY_NAME" => pretty = Some(v),
                "ID" => id = Some(v),
                "ID_LIKE" => id_like = Some(v),
                "VERSION_ID" => version = Some(v),
                _ => {}
            }
        }
    }
    OsRelease {
        pretty: pretty
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::absent(source.clone(), "os-release 无 PRETTY_NAME")),
        id: id
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::absent(source.clone(), "os-release 无 ID")),
        id_like: id_like
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| {
                Sample::absent(source.clone(), "os-release 无 ID_LIKE（Debian 等发行版常见）")
            }),
        version: version
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::absent(source, "os-release 无 VERSION_ID")),
    }
}

/// ubuntu/debian → debian；centos/rhel/rocky/fedora → rhel。
pub fn distro_family(id: &str, like: &str) -> &'static str {
    let blob = format!("{id} {like}").to_ascii_lowercase();
    let hit = |names: &[&str]| {
        blob.split(|c: char| c.is_ascii_whitespace() || c == ',')
            .any(|s| names.iter().any(|n| *n == s))
    };
    if hit(&[
        "debian",
        "ubuntu",
        "linuxmint",
        "pop",
        "raspbian",
        "elementary",
        "kali",
    ]) {
        "debian"
    } else if hit(&[
        "rhel",
        "centos",
        "fedora",
        "rocky",
        "alma",
        "almalinux",
        "ol",
        "amzn",
        "scientific",
        "redhat",
        "anolis",
        "opencloudos",
        "kylin",
        "uos",
    ]) {
        "rhel"
    } else if hit(&["suse", "opensuse", "sles", "opensuse-leap", "opensuse-tumbleweed"]) {
        "suse"
    } else if hit(&["arch", "manjaro", "endeavouros", "archlinux"]) {
        "arch"
    } else if hit(&["alpine"]) {
        "alpine"
    } else {
        "unknown"
    }
}

struct Mem {
    total: Sample<u64>,
    available: Sample<u64>,
    swap: Sample<u64>,
}

fn parse_meminfo(sample: &Sample<String>) -> Mem {
    let source = sample.source.clone();
    let empty = Sample {
        value: None,
        access: sample.access,
        source: source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return Mem {
            total: empty.clone(),
            available: empty.clone(),
            swap: empty,
        };
    };
    let mut total = None;
    let mut available = None;
    let mut swap = None;
    for line in text.lines() {
        if let Some((k, rest)) = line.split_once(':') {
            let n = rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok());
            match k {
                "MemTotal" => total = n,
                "MemAvailable" => available = n,
                "SwapTotal" => swap = n,
                _ => {}
            }
        }
    }
    Mem {
        total: total
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::absent(source.clone(), "meminfo 无 MemTotal")),
        available: available
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::absent(source.clone(), "meminfo 无 MemAvailable")),
        swap: swap
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::absent(source, "meminfo 无 SwapTotal")),
    }
}

const PACKAGE_CAP: usize = 256;

fn var_lib(ctx: &ProbeCtx, rel: &str) -> PathBuf {
    if ctx.etc == Path::new("/etc") {
        PathBuf::from("/var/lib").join(rel)
    } else {
        ctx.etc
            .parent()
            .unwrap_or(ctx.etc.as_path())
            .join("var/lib")
            .join(rel)
    }
}

fn collect_packages(ctx: &ProbeCtx) -> (Option<String>, usize, Vec<InstalledPackage>) {
    let dpkg = var_lib(ctx, "dpkg/status");
    if let Sample {
        access: AccessKind::Ok,
        value: Some(text),
        ..
    } = access::read_trimmed(&dpkg)
    {
        let all = parse_dpkg_status(&text);
        let count = all.len();
        let mut packages = all;
        packages.truncate(PACKAGE_CAP);
        return (Some("dpkg".into()), count, packages);
    }
    let apk = if ctx.etc == Path::new("/etc") {
        PathBuf::from("/lib/apk/db/installed")
    } else {
        ctx.etc
            .parent()
            .unwrap_or(ctx.etc.as_path())
            .join("lib/apk/db/installed")
    };
    if let Sample {
        access: AccessKind::Ok,
        value: Some(text),
        ..
    } = access::read_trimmed(&apk)
    {
        let all = parse_apk_installed(&text);
        let count = all.len();
        let mut packages = all;
        packages.truncate(PACKAGE_CAP);
        return (Some("apk".into()), count, packages);
    }
    (None, 0, Vec::new())
}

pub(crate) fn parse_dpkg_status(text: &str) -> Vec<InstalledPackage> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut installed = false;
    let mut push = |name: &mut Option<String>, version: &mut Option<String>, installed: &mut bool| {
        if *installed {
            if let (Some(n), Some(v)) = (name.take(), version.take()) {
                out.push(InstalledPackage {
                    name: n,
                    version: v,
                });
            }
        }
        *name = None;
        *version = None;
        *installed = false;
    };
    for line in text.lines() {
        if line.is_empty() {
            push(&mut name, &mut version, &mut installed);
            continue;
        }
        if let Some(v) = line.strip_prefix("Package: ") {
            name = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version: ") {
            version = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Status: ") {
            installed = v.contains("install ok installed");
        }
    }
    push(&mut name, &mut version, &mut installed);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub(crate) fn parse_apk_installed(text: &str) -> Vec<InstalledPackage> {
    let mut out = Vec::new();
    let mut name = None;
    let mut version = None;
    for line in text.lines() {
        if line.is_empty() {
            if let (Some(n), Some(v)) = (name.take(), version.take()) {
                out.push(InstalledPackage {
                    name: n,
                    version: v,
                });
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("P:") {
            name = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("V:") {
            version = Some(v.trim().to_string());
        }
    }
    if let (Some(n), Some(v)) = (name, version) {
        out.push(InstalledPackage {
            name: n,
            version: v,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_quotes() {
        let s = Sample::ok(
            "PRETTY_NAME=\"Ubuntu 24.04.4 LTS\"\nID=ubuntu\nVERSION_ID=\"24.04\"\n".into(),
            "/etc/os-release",
        );
        let os = parse_os_release(&s);
        assert_eq!(os.pretty.value.as_deref(), Some("Ubuntu 24.04.4 LTS"));
        assert_eq!(os.id.value.as_deref(), Some("ubuntu"));
        assert_eq!(os.id_like.value, None);
        assert_eq!(os.id_like.access, AccessKind::Absent);
        assert!(
            !os.id_like.display().contains("不存在"),
            "missing key must not look like a missing file: {}",
            os.id_like.display()
        );
    }

    #[test]
    fn os_release_centos_id_like() {
        let s = Sample::ok(
            "NAME=\"CentOS Linux\"\nID=\"centos\"\nID_LIKE=\"rhel fedora\"\nVERSION_ID=\"7\"\nPRETTY_NAME=\"CentOS Linux 7 (Core)\"\n".into(),
            "/etc/os-release",
        );
        let os = parse_os_release(&s);
        assert_eq!(os.id.value.as_deref(), Some("centos"));
        assert_eq!(os.id_like.value.as_deref(), Some("rhel fedora"));
        assert_eq!(distro_family("centos", "rhel fedora"), "rhel");
        assert_eq!(distro_family("ubuntu", "debian"), "debian");
        assert_eq!(distro_family("debian", ""), "debian");
        assert_eq!(distro_family("rocky", "rhel centos fedora"), "rhel");
        let pkgs = parse_dpkg_status(
            "Package: bash\nStatus: install ok installed\nVersion: 5.2\n\nPackage: foo\nStatus: deinstall ok config-files\nVersion: 1\n\nPackage: coreutils\nStatus: install ok installed\nVersion: 9.4\n",
        );
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "bash");
        assert_eq!(pkgs[1].name, "coreutils");
    }

    #[test]
    fn loadavg_and_btime() {
        let load = parse_loadavg(&Sample::ok("0.10 0.20 0.30 1/99 1234".into(), "loadavg"));
        assert_eq!(load.l1.value, Some(0.10));
        assert_eq!(load.l15.value, Some(0.30));
        assert_eq!(load.procs.value.as_deref(), Some("1/99"));
        let bt = parse_btime(&Sample::ok("cpu 1 2 3\nbtime 1700000000\n".into(), "stat"));
        assert_eq!(bt.value, Some(1700000000));
    }

    #[test]
    fn taint_bits() {
        assert!(decode_taint(0).is_empty());
        let f = decode_taint(1 << 12);
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("out-of-tree"));
        let (s, flags) = parse_taint(&Sample::ok("4096".into(), "tainted"));
        assert_eq!(s.value, Some(4096));
        assert_eq!(flags, f);
    }

    #[test]
    fn refresh_slow_keeps_packages_and_config_gz() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("aida-pkg-slow-{}", std::process::id()));
        fs::create_dir_all(root.join("proc/sys/kernel")).unwrap();
        fs::create_dir_all(root.join("sys/kernel")).unwrap();
        fs::create_dir_all(root.join("var/lib/dpkg")).unwrap();
        fs::create_dir_all(root.join("etc")).unwrap();
        fs::write(root.join("proc/loadavg"), "0.10 0.20 0.30 1/2 3\n").unwrap();
        fs::write(root.join("proc/sys/kernel/tainted"), "0\n").unwrap();
        fs::write(root.join("proc/config.gz"), [1, 2, 3, 4]).unwrap();
        fs::write(
            root.join("var/lib/dpkg/status"),
            "Package: bash\nStatus: install ok installed\nVersion: 1\n\n",
        )
        .unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let mut info = collect(&ctx);
        assert_eq!(info.package_count, 1);
        assert_eq!(info.config_gz.value.as_deref(), Some("4 bytes"));
        fs::write(
            root.join("var/lib/dpkg/status"),
            "Package: bash\nStatus: install ok installed\nVersion: 1\n\nPackage: coreutils\nStatus: install ok installed\nVersion: 2\n\n",
        )
        .unwrap();
        fs::write(root.join("proc/config.gz"), [1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        fs::write(root.join("proc/loadavg"), "1.50 1.00 0.50 2/9 8\n").unwrap();
        fs::write(root.join("proc/sys/kernel/tainted"), "1\n").unwrap();
        refresh_slow(&mut info, &ctx);
        assert_eq!(info.package_count, 1, "periodic refresh must not re-read dpkg");
        assert_eq!(info.packages.len(), 1);
        assert_eq!(info.config_gz.value.as_deref(), Some("4 bytes"));
        assert_eq!(info.load_1.value, Some(1.50));
        assert_eq!(info.tainted.value, Some(1));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn config_gz_uses_byte_length_not_inode_size() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("aida-cfggz-{}", std::process::id()));
        fs::create_dir_all(root.join("proc")).unwrap();
        fs::write(root.join("proc/config.gz"), [0x1f, 0x8b, 0x08, 0x00]).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let s = config_gz_sample(&ctx);
        assert_eq!(s.value.as_deref(), Some("4 bytes"));
        fs::write(root.join("proc/config.gz"), b"").unwrap();
        let empty = config_gz_sample(&ctx);
        assert_eq!(empty.value.as_deref(), Some("present"));
        assert_eq!(empty.access, AccessKind::Ok);
        fs::remove_file(root.join("proc/config.gz")).unwrap();
        let missing = config_gz_sample(&ctx);
        assert_eq!(missing.access, AccessKind::NotFound);
        assert_eq!(
            count_lock_lines(&Sample::ok(
                "1: POSIX ADVISORY WRITE 1\n2: FLOCK\n".into(),
                "locks"
            )),
            2
        );
        assert_eq!(count_lock_lines(&Sample::missing("locks")), 0);
        assert_eq!(
            parse_filesystems(&Sample::ok(
                "nodev\tsysfs\n\text4\n\txfs\nnodev\tfuse\n".into(),
                "fs"
            )),
            vec!["ext4".to_string(), "xfs".to_string()]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_locks_is_not_silent_zero() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aida-locks-deny-{}", std::process::id()));
        fs::create_dir_all(root.join("proc")).unwrap();
        let locks = root.join("proc/locks");
        fs::write(&locks, "1: POSIX ADVISORY WRITE 1\n").unwrap();
        fs::set_permissions(&locks, fs::Permissions::from_mode(0o000)).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let _ = fs::set_permissions(&locks, fs::Permissions::from_mode(0o644));
        let _ = fs::remove_dir_all(&root);
        assert_eq!(r.file_locks, 0);
        assert!(
            r.notes
                .iter()
                .any(|n| n.contains("权限") || n.contains("失败")),
            "denied /proc/locks must not look like zero locks: {:?}",
            r.notes
        );
    }

    #[test]
    fn cpu_byteorder_and_address_bits() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("aida-sw-arch-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/kernel")).unwrap();
        fs::create_dir_all(root.join("proc")).unwrap();
        fs::write(root.join("sys/kernel/cpu_byteorder"), "little\n").unwrap();
        fs::write(root.join("sys/kernel/address_bits"), "64\n").unwrap();
        fs::write(root.join("sys/kernel/profiling"), "0\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.cpu_byteorder.value.as_deref(), Some("little"));
        assert_eq!(r.address_bits.value.as_deref(), Some("64"));
        assert_eq!(r.profiling.value.as_deref(), Some("0"));
        fs::create_dir_all(root.join("proc/sys/kernel")).unwrap();
        fs::write(root.join("proc/sys/kernel/ostype"), "Linux\n").unwrap();
        let r2 = collect(&ctx);
        assert_eq!(r2.ostype.value.as_deref(), Some("Linux"));
        let _ = fs::remove_dir_all(&root);
    }
}
