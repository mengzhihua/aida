//! RAPL / powercap：`/sys/class/powercap`。不调用 `turbostat`/`rapl-read`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct RaplReport {
    pub zones: Vec<RaplZone>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RaplZone {
    pub name: String,
    pub label: Sample<String>,
    pub energy_uj: Sample<u64>,
    pub max_energy_range_uj: Sample<u64>,
    pub power_limit_uw: Sample<u64>,
    pub max_power_uw: Sample<u64>,
    /// 两次采样之间的平均功率；单次 collect 为 None。
    pub power_w: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct RaplSnap {
    pub name: String,
    pub energy_uj: u64,
}

pub fn collect(ctx: &ProbeCtx) -> RaplReport {
    collect_with_prev(ctx, None, 0.0)
}

pub fn collect_with_prev(ctx: &ProbeCtx, prev: Option<&[RaplSnap]>, dt_sec: f64) -> RaplReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("class/powercap");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            } else {
                notes.push(
                    "无 powercap/RAPL。虚拟机、ARM、或内核未开 Intel RAPL/AMD energy 时常见。"
                        .into(),
                );
            }
            return RaplReport {
                zones: Vec::new(),
                notes,
            };
        }
    };
    let mut zones = Vec::new();
    for name in names {
        let dir = root.join(&name);
        if !dir.join("energy_uj").exists() {
            continue;
        }
        let energy = access::read_u64(dir.join("energy_uj"));
        let max_range = access::read_u64(dir.join("max_energy_range_uj"));
        let power_w = match (prev, energy.value) {
            (Some(p), Some(now)) if dt_sec > 0.0 => p
                .iter()
                .find(|x| x.name == name)
                .and_then(|old| energy_delta(now, old.energy_uj, max_range.value))
                .map(|duj| duj / dt_sec / 1_000_000.0),
            _ => None,
        };
        zones.push(RaplZone {
            label: access::read_trimmed(dir.join("name")),
            energy_uj: energy,
            max_energy_range_uj: max_range,
            power_limit_uw: access::read_u64(dir.join("constraint_0_power_limit_uw")),
            max_power_uw: access::read_u64(dir.join("constraint_0_max_power_uw")),
            power_w,
            name,
        });
    }
    if zones.is_empty() && notes.is_empty() {
        notes.push("powercap 下没有 energy_uj 节点。".into());
    }
    RaplReport { zones, notes }
}

/// RAPL `energy_uj` 会在 `max_energy_range_uj` 处回绕。
fn energy_delta(now: u64, old: u64, max_range: Option<u64>) -> Option<f64> {
    if now >= old {
        Some((now - old) as f64)
    } else {
        max_range.map(|r| r.saturating_sub(old).saturating_add(now) as f64)
    }
}

pub fn counters(report: &RaplReport) -> Vec<RaplSnap> {
    report
        .zones
        .iter()
        .filter_map(|z| {
            Some(RaplSnap {
                name: z.name.clone(),
                energy_uj: z.energy_uj.value?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn package_energy_rate() {
        let root = std::env::temp_dir().join(format!("aida-rapl-{}", std::process::id()));
        let z = root.join("sys/class/powercap/intel-rapl:0");
        fs::create_dir_all(&z).unwrap();
        fs::write(z.join("name"), "package-0\n").unwrap();
        fs::write(z.join("energy_uj"), "2000000\n").unwrap();
        fs::write(z.join("max_energy_range_uj"), "10000000\n").unwrap();
        fs::write(z.join("constraint_0_power_limit_uw"), "45000000\n").unwrap();
        fs::write(z.join("constraint_0_max_power_uw"), "45000000\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let prev = [RaplSnap {
            name: "intel-rapl:0".into(),
            energy_uj: 1_000_000,
        }];
        let r = collect_with_prev(&ctx, Some(&prev), 1.0);
        assert_eq!(r.zones.len(), 1);
        assert_eq!(r.zones[0].label.value.as_deref(), Some("package-0"));
        assert_eq!(r.zones[0].power_w, Some(1.0));
        fs::write(z.join("energy_uj"), "500000\n").unwrap();
        let wrap_prev = [RaplSnap {
            name: "intel-rapl:0".into(),
            energy_uj: 9_500_000,
        }];
        let wrap = collect_with_prev(&ctx, Some(&wrap_prev), 1.0);
        assert_eq!(wrap.zones[0].power_w, Some(1.0));
        let _ = fs::remove_dir_all(&root);
    }
}
