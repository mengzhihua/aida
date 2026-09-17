//! 系统软件信息：os-release、内核、内存、启动参数。不调用 hostnamectl / uname。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct SoftwareInfo {
    pub os_name: Sample<String>,
    pub os_id: Sample<String>,
    pub os_version: Sample<String>,
    pub kernel_release: Sample<String>,
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
    pub notes: Vec<String>,
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
        } => match s.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()) {
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
        .unwrap_or_else(|_| {
            Sample::missing("env:XDG_CURRENT_DESKTOP（无图形会话时正常为空）")
        });

    let load = parse_loadavg(&access::read_trimmed(ctx.proc_path("loadavg")));
    let boot_time_unix = parse_btime(&access::read_trimmed(ctx.proc_path("stat")));
    let (tainted, taint_flags) = parse_taint(&access::read_trimmed(
        ctx.proc_path("sys/kernel/tainted"),
    ));

    SoftwareInfo {
        os_name: os.pretty,
        os_id: os.id,
        os_version: os.version,
        kernel_release: access::read_trimmed(ctx.proc_path("sys/kernel/osrelease")),
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
        notes: Vec::new(),
    }
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
        return (Sample::error(sample.source.clone(), "无法解析 tainted"), Vec::new());
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

struct OsRelease {
    pretty: Sample<String>,
    id: Sample<String>,
    version: Sample<String>,
}

fn parse_os_release(sample: &Sample<String>) -> OsRelease {
    let source = sample.source.clone();
    let fail = |access: AccessKind, hint: Option<String>| OsRelease {
        pretty: Sample {
            value: None,
            access,
            source: source.clone(),
            hint: hint.clone(),
        },
        id: Sample {
            value: None,
            access,
            source: source.clone(),
            hint: hint.clone(),
        },
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
    let mut version = None;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').to_string();
            match k {
                "PRETTY_NAME" => pretty = Some(v),
                "ID" => id = Some(v),
                "VERSION_ID" => version = Some(v),
                _ => {}
            }
        }
    }
    OsRelease {
        pretty: pretty
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::missing(source.clone())),
        id: id
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::missing(source.clone())),
        version: version
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::missing(source)),
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
            let n = rest.split_whitespace().next().and_then(|s| s.parse::<u64>().ok());
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
            .unwrap_or_else(|| Sample::missing(source.clone())),
        available: available
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::missing(source.clone())),
        swap: swap
            .map(|v| Sample::ok(v, source.clone()))
            .unwrap_or_else(|| Sample::missing(source)),
    }
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
}
