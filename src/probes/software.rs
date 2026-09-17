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
        notes: Vec::new(),
    }
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
}
