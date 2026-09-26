//! 电源：`/sys/class/power_supply`（电池 / AC）。不调用 `upower`/`acpi`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct PowerReport {
    pub supplies: Vec<PowerSupply>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PowerSupply {
    pub name: String,
    pub kind: Sample<String>,
    pub status: Sample<String>,
    pub present: Sample<String>,
    pub online: Sample<String>,
    pub capacity_pct: Sample<u64>,
    pub voltage_v: Sample<f64>,
    pub current_a: Sample<f64>,
    pub power_w: Sample<f64>,
    pub energy_now_wh: Sample<f64>,
    pub energy_full_wh: Sample<f64>,
    pub cycle_count: Sample<u64>,
    pub technology: Sample<String>,
    pub model: Sample<String>,
    pub manufacturer: Sample<String>,
    pub serial: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> PowerReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("class/power_supply");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            notes.push("无电源类设备：台式机/服务器无电池，或容器未导出 power_supply。".into());
            return PowerReport {
                supplies: Vec::new(),
                notes,
            };
        }
    };
    let mut supplies = Vec::new();
    for name in names {
        let dir = root.join(&name);
        supplies.push(PowerSupply {
            name,
            kind: access::read_trimmed(dir.join("type")),
            status: access::read_trimmed(dir.join("status")),
            present: access::read_trimmed(dir.join("present")),
            online: access::read_trimmed(dir.join("online")),
            capacity_pct: read_u64(dir.join("capacity")),
            voltage_v: scaled(dir.join("voltage_now"), 1_000_000.0),
            current_a: scaled(dir.join("current_now"), 1_000_000.0),
            power_w: scaled(dir.join("power_now"), 1_000_000.0),
            energy_now_wh: scaled(dir.join("energy_now"), 1_000_000.0),
            energy_full_wh: scaled(dir.join("energy_full"), 1_000_000.0),
            cycle_count: read_u64(dir.join("cycle_count")),
            technology: access::read_trimmed(dir.join("technology")),
            model: access::read_trimmed(dir.join("model_name")),
            manufacturer: access::read_trimmed(dir.join("manufacturer")),
            serial: access::read_trimmed(dir.join("serial_number")),
        });
    }
    if supplies.is_empty() {
        notes.push("power_supply 目录为空。".into());
    }
    PowerReport { supplies, notes }
}

/// GUI 快路径：只更新状态、容量、电压、电流、功率。型号和序列号留在启动采集。
/// 没有电源设备时不再打开 `power_supply`。
pub fn refresh_runtime(report: &mut PowerReport, ctx: &ProbeCtx) {
    if report.supplies.is_empty() {
        return;
    }
    let root = ctx.sys_path("class/power_supply");
    for s in &mut report.supplies {
        let dir = root.join(&s.name);
        s.status = access::read_trimmed(dir.join("status"));
        s.present = access::read_trimmed(dir.join("present"));
        s.online = access::read_trimmed(dir.join("online"));
        s.capacity_pct = read_u64(dir.join("capacity"));
        s.voltage_v = scaled(dir.join("voltage_now"), 1_000_000.0);
        s.current_a = scaled(dir.join("current_now"), 1_000_000.0);
        s.power_w = scaled(dir.join("power_now"), 1_000_000.0);
        s.energy_now_wh = scaled(dir.join("energy_now"), 1_000_000.0);
    }
}

fn read_u64(path: std::path::PathBuf) -> Sample<u64> {
    let s = access::read_trimmed(&path);
    match (s.access, s.value.as_deref()) {
        (AccessKind::Ok, Some(t)) => match t.parse::<u64>() {
            Ok(v) => Sample::ok(v, s.source),
            Err(_) => Sample::error(s.source, "无法解析"),
        },
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

fn scaled(path: std::path::PathBuf, div: f64) -> Sample<f64> {
    let s = access::read_trimmed(&path);
    match (s.access, s.value.as_deref()) {
        (AccessKind::Ok, Some(t)) => match t.parse::<i64>() {
            Ok(v) => Sample::ok(v as f64 / div, s.source),
            Err(_) => Sample::error(s.source, "无法解析"),
        },
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn battery_fixture() {
        let root = std::env::temp_dir().join(format!("aida-psu-{}", std::process::id()));
        let bat = root.join("sys/class/power_supply/BAT0");
        fs::create_dir_all(&bat).unwrap();
        fs::write(bat.join("type"), "Battery\n").unwrap();
        fs::write(bat.join("status"), "Discharging\n").unwrap();
        fs::write(bat.join("capacity"), "77\n").unwrap();
        fs::write(bat.join("voltage_now"), "12000000\n").unwrap();
        fs::write(bat.join("energy_now"), "40000000\n").unwrap();
        fs::write(bat.join("energy_full"), "50000000\n").unwrap();
        fs::write(bat.join("cycle_count"), "120\n").unwrap();
        let ac = root.join("sys/class/power_supply/AC");
        fs::create_dir_all(&ac).unwrap();
        fs::write(ac.join("type"), "Mains\n").unwrap();
        fs::write(ac.join("online"), "1\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.supplies.len(), 2);
        let bat = r.supplies.iter().find(|s| s.name == "BAT0").unwrap();
        assert_eq!(bat.capacity_pct.value, Some(77));
        assert_eq!(bat.voltage_v.value, Some(12.0));
        assert_eq!(bat.energy_now_wh.value, Some(40.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refresh_runtime_keeps_model() {
        let root = std::env::temp_dir().join(format!("aida-psu-fast-{}", std::process::id()));
        let bat = root.join("sys/class/power_supply/BAT0");
        fs::create_dir_all(&bat).unwrap();
        fs::write(bat.join("type"), "Battery\n").unwrap();
        fs::write(bat.join("status"), "Discharging\n").unwrap();
        fs::write(bat.join("capacity"), "77\n").unwrap();
        fs::write(bat.join("model_name"), "TestBat\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let mut r = collect(&ctx);
        fs::write(bat.join("capacity"), "10\n").unwrap();
        fs::write(bat.join("model_name"), "Other\n").unwrap();
        refresh_runtime(&mut r, &ctx);
        let bat = r.supplies.iter().find(|s| s.name == "BAT0").unwrap();
        assert_eq!(bat.capacity_pct.value, Some(10));
        assert_eq!(bat.model.value.as_deref(), Some("TestBat"));
        let mut empty = PowerReport {
            supplies: Vec::new(),
            notes: Vec::new(),
        };
        refresh_runtime(&mut empty, &ctx);
        assert!(empty.supplies.is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
