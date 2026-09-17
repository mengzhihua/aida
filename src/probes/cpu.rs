//! CPU：`/proc/cpuinfo` + `/sys/devices/system/cpu` + `/proc/stat`。
//!
//! 不调用 `lscpu`。利用率需要两次 `/proc/stat` 采样；CLI 会短暂 sleep，
//! GUI 则在帧之间缓存上一次计数。

use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct CpuInfo {
    pub model_name: Sample<String>,
    pub vendor: Sample<String>,
    pub logical_cpus: usize,
    pub physical_packages: usize,
    pub cores_per_package: Sample<u32>,
    pub flags: Vec<String>,
    pub bugs: Vec<String>,
    pub address_sizes: Sample<String>,
    pub hypervisor: bool,
    pub microcode: Sample<String>,
    pub smt_active: Sample<String>,
    pub smt_control: Sample<String>,
    pub isolated: Sample<String>,
    pub online: Sample<String>,
    pub logical: Vec<LogicalCpu>,
    pub caches: Vec<CpuCache>,
    /// 两次 /proc/stat 之间的整机利用率（0-100）。首次采样为 None。
    pub utilization_pct: Option<f32>,
    /// `/sys/devices/system/cpu/vulnerabilities/*`
    pub vulnerabilities: Vec<CpuVuln>,
    pub idle_states: Vec<CpuIdleState>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpuVuln {
    pub name: String,
    pub status: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LogicalCpu {
    pub processor: u32,
    pub physical_id: Option<u32>,
    pub core_id: Option<u32>,
    pub apicid: Option<u32>,
    pub mhz_from_cpuinfo: Option<f64>,
    pub scaling_cur_khz: Sample<u64>,
    pub scaling_min_khz: Sample<u64>,
    pub scaling_max_khz: Sample<u64>,
    pub governor: Sample<String>,
    pub online: Sample<String>,
    pub utilization_pct: Option<f32>,
    pub thread_siblings: Sample<String>,
    pub core_siblings: Sample<String>,
    pub package_cpus: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpuIdleState {
    pub name: Sample<String>,
    pub desc: Sample<String>,
    pub latency_us: Sample<String>,
    pub residency_us: Sample<String>,
    pub disable: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpuCache {
    pub cpu: u32,
    pub index: String,
    pub level: Sample<String>,
    pub kind: Sample<String>,
    pub size: Sample<String>,
    pub line_size: Sample<String>,
    pub associativity: Sample<String>,
    pub shared_cpu_list: Sample<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CpuStatSnap {
    pub total: u64,
    pub idle: u64,
    /// (logical id, total, idle)
    pub per_cpu: Vec<(u32, u64, u64)>,
}

pub fn collect(ctx: &ProbeCtx) -> CpuInfo {
    collect_with_util(ctx, Some(Duration::from_millis(120)))
}

pub fn collect_with_util(ctx: &ProbeCtx, sample_for: Option<Duration>) -> CpuInfo {
    let cpuinfo_path = ctx.proc_path("cpuinfo");
    let cpuinfo = access::read_trimmed(&cpuinfo_path);
    let blocks = match &cpuinfo.value {
        Some(text) => parse_cpuinfo(text),
        None => Vec::new(),
    };

    let first = blocks.first();
    let model_name = first
        .and_then(|m| m.get("model name").cloned())
        .map(|v| Sample::ok(v, cpuinfo_path.display().to_string()))
        .unwrap_or_else(|| match cpuinfo.access {
            AccessKind::Ok => Sample::missing(cpuinfo_path.display().to_string()),
            _ => Sample {
                value: None,
                access: cpuinfo.access,
                source: cpuinfo.source.clone(),
                hint: cpuinfo.hint.clone(),
            },
        });
    let vendor = first
        .and_then(|m| m.get("vendor_id").cloned())
        .map(|v| Sample::ok(v, cpuinfo_path.display().to_string()))
        .unwrap_or_else(|| Sample::missing(cpuinfo_path.display().to_string()));
    let flags: Vec<String> = first
        .and_then(|m| m.get("flags"))
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    let bugs: Vec<String> = first
        .and_then(|m| m.get("bugs"))
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    let address_sizes = first
        .and_then(|m| m.get("address sizes").cloned())
        .map(|v| Sample::ok(v, cpuinfo_path.display().to_string()))
        .unwrap_or_else(|| Sample::missing(cpuinfo_path.display().to_string()));
    let hypervisor = flags.iter().any(|f| f == "hypervisor");
    let microcode = first
        .and_then(|m| m.get("microcode").cloned())
        .map(|v| Sample::ok(v, cpuinfo_path.display().to_string()))
        .unwrap_or_else(|| Sample::missing(cpuinfo_path.display().to_string()));

    let mut logical = Vec::new();
    let mut packages = std::collections::BTreeSet::new();
    let mut cores_seen: BTreeMap<u32, std::collections::BTreeSet<u32>> = BTreeMap::new();

    for block in &blocks {
        let processor = block
            .get("processor")
            .and_then(|s| s.parse().ok())
            .unwrap_or(logical.len() as u32);
        let physical_id = block.get("physical id").and_then(|s| s.parse().ok());
        let core_id = block.get("core id").and_then(|s| s.parse().ok());
        if let Some(p) = physical_id {
            packages.insert(p);
            if let Some(c) = core_id {
                cores_seen.entry(p).or_default().insert(c);
            }
        }
        let mhz_from_cpuinfo = block.get("cpu MHz").and_then(|s| s.parse().ok());
        let cpu_dir = ctx.sys_path(format!("devices/system/cpu/cpu{processor}"));
        logical.push(logical_from_sysfs(
            &cpu_dir,
            processor,
            physical_id,
            core_id,
            block.get("apicid").and_then(|s| s.parse().ok()),
            mhz_from_cpuinfo,
        ));
    }

    if logical.is_empty() {
        // 无 cpuinfo 时仍尝试 sysfs 枚举。
        if let Some(names) = access::list_dir_names(ctx.sys_path("devices/system/cpu")).value {
            for name in names {
                if let Some(rest) = name.strip_prefix("cpu") {
                    if let Ok(processor) = rest.parse::<u32>() {
                        let cpu_dir = ctx.sys_path(format!("devices/system/cpu/cpu{processor}"));
                        logical.push(logical_from_sysfs(
                            &cpu_dir, processor, None, None, None, None,
                        ));
                    }
                }
            }
        }
    }

    let cpu0 = logical.first().map(|l| l.processor).unwrap_or(0);
    let caches = read_caches(ctx, cpu0);
    let idle_states = collect_idle_states(ctx, cpu0);

    let cores_per_package = cores_seen
        .values()
        .map(|s| s.len() as u32)
        .max()
        .or_else(|| first.and_then(|m| m.get("cpu cores")).and_then(|s| s.parse().ok()));
    let cores_per_package = match cores_per_package {
        Some(n) => Sample::ok(n, ctx.proc_path("cpuinfo").display().to_string()),
        None => Sample::missing(ctx.proc_path("cpuinfo").display().to_string()),
    };

    let utilization_pct = match sample_for {
        Some(d) if !d.is_zero() => {
            let a = read_proc_stat(ctx);
            std::thread::sleep(d);
            let b = read_proc_stat(ctx);
            apply_per_cpu(&mut logical, &a, &b);
            utilization(&a, &b)
        }
        _ => None,
    };

    let mut notes = Vec::new();
    if hypervisor {
        notes.push("cpuinfo flags 含 hypervisor，当前像是虚拟机/容器 CPU。".into());
    }
    if logical
        .iter()
        .all(|l| l.scaling_cur_khz.access == AccessKind::NotFound)
    {
        notes.push(
            "无 cpufreq sysfs：虚拟机或内核未启用 CPU 频率驱动，频率只能看 cpuinfo 的 cpu MHz。"
                .into(),
        );
    }

    let smt_active = access::read_trimmed(ctx.sys_path("devices/system/cpu/smt/active"));
    let smt_control = access::read_trimmed(ctx.sys_path("devices/system/cpu/smt/control"));
    let isolated = access::read_trimmed(ctx.sys_path("devices/system/cpu/isolated"));
    let online = access::read_trimmed(ctx.sys_path("devices/system/cpu/online"));
    if smt_control
        .value
        .as_deref()
        .is_some_and(|v| v == "notsupported" || v == "not implemented")
    {
        notes.push("SMT sysfs 为 notsupported（虚拟机或未开 CONFIG_HOTPLUG_SMT 时常见）。".into());
    }

    CpuInfo {
        model_name,
        vendor,
        logical_cpus: logical.len(),
        physical_packages: if packages.is_empty() {
            1
        } else {
            packages.len()
        },
        cores_per_package,
        flags,
        bugs,
        address_sizes,
        hypervisor,
        microcode,
        smt_active,
        smt_control,
        isolated,
        online,
        logical,
        caches,
        utilization_pct,
        vulnerabilities: collect_vulns(ctx),
        idle_states,
        notes,
    }
}

pub fn read_proc_stat(ctx: &ProbeCtx) -> Option<CpuStatSnap> {
    let text = fs::read_to_string(ctx.proc_path("stat")).ok()?;
    let mut all = None;
    let mut per_cpu = Vec::new();
    for line in text.lines() {
        if !line.starts_with("cpu") {
            continue;
        }
        let mut it = line.split_whitespace();
        let label = it.next()?;
        let nums: Vec<u64> = it.filter_map(|s| s.parse().ok()).collect();
        if nums.len() < 4 {
            continue;
        }
        let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
        let total: u64 = nums.iter().take(10).sum();
        if label == "cpu" {
            all = Some((total, idle));
        } else if let Some(rest) = label.strip_prefix("cpu") {
            if let Ok(id) = rest.parse::<u32>() {
                per_cpu.push((id, total, idle));
            }
        }
    }
    let (total, idle) = all?;
    Some(CpuStatSnap {
        total,
        idle,
        per_cpu,
    })
}

pub fn apply_per_cpu(
    logical: &mut [LogicalCpu],
    a: &Option<CpuStatSnap>,
    b: &Option<CpuStatSnap>,
) {
    let (Some(a), Some(b)) = (a, b) else {
        return;
    };
    for l in logical {
        let pa = a.per_cpu.iter().find(|x| x.0 == l.processor);
        let pb = b.per_cpu.iter().find(|x| x.0 == l.processor);
        if let (Some(pa), Some(pb)) = (pa, pb) {
            let dt = pb.1.saturating_sub(pa.1);
            if dt > 0 {
                let di = pb.2.saturating_sub(pa.2);
                l.utilization_pct = Some(((dt - di) as f32) * 100.0 / dt as f32);
            }
        }
    }
}

pub fn utilization(a: &Option<CpuStatSnap>, b: &Option<CpuStatSnap>) -> Option<f32> {
    let a = a.as_ref()?;
    let b = b.as_ref()?;
    let dt = b.total.saturating_sub(a.total);
    if dt == 0 {
        return None;
    }
    let di = b.idle.saturating_sub(a.idle);
    Some(((dt - di) as f32) * 100.0 / dt as f32)
}

fn logical_from_sysfs(
    cpu_dir: &std::path::Path,
    processor: u32,
    physical_id: Option<u32>,
    core_id: Option<u32>,
    apicid: Option<u32>,
    mhz_from_cpuinfo: Option<f64>,
) -> LogicalCpu {
    let topo = cpu_dir.join("topology");
    LogicalCpu {
        processor,
        physical_id,
        core_id,
        apicid,
        mhz_from_cpuinfo,
        scaling_cur_khz: read_u64(cpu_dir.join("cpufreq/scaling_cur_freq")),
        scaling_min_khz: read_u64(cpu_dir.join("cpufreq/scaling_min_freq")),
        scaling_max_khz: read_u64(cpu_dir.join("cpufreq/cpuinfo_max_freq")),
        governor: access::read_trimmed(cpu_dir.join("cpufreq/scaling_governor")),
        online: access::read_trimmed(cpu_dir.join("online")),
        utilization_pct: None,
        thread_siblings: access::read_trimmed(topo.join("thread_siblings_list")),
        core_siblings: access::read_trimmed(topo.join("core_siblings_list")),
        package_cpus: access::read_trimmed(topo.join("package_cpus_list")),
    }
}

fn collect_idle_states(ctx: &ProbeCtx, cpu: u32) -> Vec<CpuIdleState> {
    let root = ctx.sys_path(format!("devices/system/cpu/cpu{cpu}/cpuidle"));
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out: Vec<_> = names
        .into_iter()
        .filter(|n| n.starts_with("state"))
        .map(|n| {
            let p = root.join(&n);
            CpuIdleState {
                name: access::read_trimmed(p.join("name")),
                desc: access::read_trimmed(p.join("desc")),
                latency_us: access::read_trimmed(p.join("latency")),
                residency_us: access::read_trimmed(p.join("residency")),
                disable: access::read_trimmed(p.join("disable")),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.display().cmp(&b.name.display()));
    out
}

fn collect_vulns(ctx: &ProbeCtx) -> Vec<CpuVuln> {
    let root = ctx.sys_path("devices/system/cpu/vulnerabilities");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out: Vec<CpuVuln> = names
        .into_iter()
        .map(|name| CpuVuln {
            status: access::read_trimmed(root.join(&name)),
            name,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_u64(path: std::path::PathBuf) -> Sample<u64> {
    let s = access::read_trimmed(&path);
    match (s.access, s.value) {
        (AccessKind::Ok, Some(text)) => match access::parse_u64_str(&text) {
            Some(v) => Sample::ok(v, s.source),
            None => Sample::error(s.source, "无法解析为整数"),
        },
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

fn read_caches(ctx: &ProbeCtx, cpu: u32) -> Vec<CpuCache> {
    let dir = ctx.sys_path(format!("devices/system/cpu/cpu{cpu}/cache"));
    let names = match access::list_dir_names(&dir).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    names
        .into_iter()
        .filter(|n| n.starts_with("index"))
        .map(|index| {
            let p = dir.join(&index);
            CpuCache {
                cpu,
                index,
                level: access::read_trimmed(p.join("level")),
                kind: access::read_trimmed(p.join("type")),
                size: access::read_trimmed(p.join("size")),
                line_size: access::read_trimmed(p.join("coherency_line_size")),
                associativity: access::read_trimmed(p.join("ways_of_associativity")),
                shared_cpu_list: access::read_trimmed(p.join("shared_cpu_list")),
            }
        })
        .collect()
}

pub fn parse_cpuinfo(text: &str) -> Vec<BTreeMap<String, String>> {
    let mut out = Vec::new();
    let mut cur = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            cur.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_two_logical_cpus() {
        let text = "\
processor\t: 0
vendor_id\t: GenuineIntel
model name\t: Test CPU
cpu MHz\t\t: 2400.000
physical id\t: 0
core id\t\t: 0
cpu cores\t: 2
flags\t\t: fpu hypervisor sse

processor\t: 1
vendor_id\t: GenuineIntel
model name\t: Test CPU
cpu MHz\t\t: 2400.000
physical id\t: 0
core id\t\t: 1
cpu cores\t: 2
flags\t\t: fpu hypervisor sse
";
        let blocks = parse_cpuinfo(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].get("model name").unwrap(), "Test CPU");
        assert!(blocks[0].get("flags").unwrap().contains("hypervisor"));
    }
}
