//! hwmon + thermal_zone 传感器。
//!
//! 路径约定：`/sys/class/hwmon/hwmonN/{temp,fan,in,power,curr}*_input`。
//! 值的单位按内核 ABI：温度 millidegree C，电压 mV，功率 uW，电流 mA，风扇 RPM。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct SensorReport {
    pub chips: Vec<HwmonChip>,
    pub thermal_zones: Vec<ThermalZone>,
    pub cooling: Vec<CoolingDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HwmonChip {
    pub path: String,
    pub name: Sample<String>,
    pub channels: Vec<SensorChannel>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SensorChannel {
    pub key: String,
    pub label: String,
    pub kind: SensorKind,
    pub raw: Sample<i64>,
    /// 换算后的 SI 友好值：°C / V / W / A / RPM。
    pub value: Option<f64>,
    pub unit: String,
    pub max: Option<f64>,
    pub crit: Option<f64>,
    pub min: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SensorKind {
    Temp,
    Fan,
    Voltage,
    Power,
    Current,
    Other,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThermalZone {
    pub path: String,
    pub r#type: Sample<String>,
    pub temp_c: Sample<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CoolingDevice {
    pub name: String,
    pub r#type: Sample<String>,
    pub cur_state: Sample<String>,
    pub max_state: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> SensorReport {
    let mut chips = Vec::new();
    let mut notes = Vec::new();
    let hwmon_root = ctx.sys_path("class/hwmon");
    match access::list_dir_names(&hwmon_root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(names),
            ..
        } => {
            for name in names.into_iter().filter(|n| n.starts_with("hwmon")) {
                chips.push(read_chip(&hwmon_root.join(&name)));
            }
            if chips.is_empty() {
                notes.push("hwmon 目录存在但没有 hwmonN 节点。".into());
            }
        }
        s => {
            notes.push(s.access_label());
        }
    }

    let mut thermal_zones = Vec::new();
    let tz_root = ctx.sys_path("class/thermal");
    if let Some(names) = access::list_dir_names(&tz_root).value {
        for name in names.into_iter().filter(|n| n.starts_with("thermal_zone")) {
            let p = tz_root.join(&name);
            let raw = access::read_trimmed(p.join("temp"));
            let temp_c = match (raw.access, raw.value.as_deref()) {
                (AccessKind::Ok, Some(t)) => match t.parse::<i64>() {
                    Ok(v) => Sample::ok(v as f64 / 1000.0, raw.source),
                    Err(_) => Sample::error(raw.source, "无法解析 thermal_zone temp"),
                },
                _ => Sample {
                    value: None,
                    access: raw.access,
                    source: raw.source,
                    hint: raw.hint,
                },
            };
            thermal_zones.push(ThermalZone {
                path: p.display().to_string(),
                r#type: access::read_trimmed(p.join("type")),
                temp_c,
            });
        }
    }

    let mut cooling = Vec::new();
    if let Some(names) = access::list_dir_names(&tz_root).value {
        for name in names.into_iter().filter(|n| n.starts_with("cooling_device")) {
            let p = tz_root.join(&name);
            cooling.push(CoolingDevice {
                name,
                r#type: access::read_trimmed(p.join("type")),
                cur_state: access::read_trimmed(p.join("cur_state")),
                max_state: access::read_trimmed(p.join("max_state")),
            });
        }
    }

    if chips.is_empty() && thermal_zones.is_empty() {
        notes.push(
            "没有可读传感器。桌面机请确认已加载 coretemp/k10temp/it87 等模块；笔记本还可能走 thermal_zone。"
                .into(),
        );
    }

    SensorReport {
        chips,
        thermal_zones,
        cooling,
        notes,
    }
}

fn read_chip(dir: &std::path::Path) -> HwmonChip {
    let name = access::read_trimmed(dir.join("name"));
    let mut channels = Vec::new();
    if let Some(entries) = access::list_dir_names(dir).value {
        let mut inputs: Vec<String> = entries
            .into_iter()
            .filter(|n| n.ends_with("_input"))
            .collect();
        inputs.sort();
        for input in inputs {
            let prefix = input.trim_end_matches("_input");
            let (kind, scale, unit) = classify(prefix);
            let raw_s = access::read_trimmed(dir.join(&input));
            let raw = match (raw_s.access, raw_s.value.as_deref()) {
                (AccessKind::Ok, Some(t)) => match t.parse::<i64>() {
                    Ok(v) => Sample::ok(v, raw_s.source),
                    Err(_) => Sample::error(raw_s.source, "非整数"),
                },
                _ => Sample {
                    value: None,
                    access: raw_s.access,
                    source: raw_s.source,
                    hint: raw_s.hint,
                },
            };
            let value = raw.value.map(|v| v as f64 / scale);
            let label_s = access::read_trimmed(dir.join(format!("{prefix}_label")));
            let label = label_s.value.clone().unwrap_or_else(|| {
                format!("{} ({prefix})", name.value.clone().unwrap_or_default())
            });
            channels.push(SensorChannel {
                key: format!(
                    "{}:{prefix}",
                    name.value
                        .clone()
                        .unwrap_or_else(|| dir.display().to_string())
                ),
                label,
                kind,
                raw,
                value,
                unit: unit.to_string(),
                max: scaled(dir, prefix, "_max", scale),
                crit: scaled(dir, prefix, "_crit", scale),
                min: scaled(dir, prefix, "_min", scale),
            });
        }
    }
    HwmonChip {
        path: dir.display().to_string(),
        name,
        channels,
    }
}

fn scaled(dir: &std::path::Path, prefix: &str, suffix: &str, scale: f64) -> Option<f64> {
    let s = access::read_trimmed(dir.join(format!("{prefix}{suffix}")));
    s.value
        .and_then(|t| t.parse::<i64>().ok())
        .map(|v| v as f64 / scale)
}

fn classify(prefix: &str) -> (SensorKind, f64, &'static str) {
    if prefix.starts_with("temp") {
        (SensorKind::Temp, 1000.0, "°C")
    } else if prefix.starts_with("fan") {
        (SensorKind::Fan, 1.0, "RPM")
    } else if prefix.starts_with("in") {
        (SensorKind::Voltage, 1000.0, "V")
    } else if prefix.starts_with("power") {
        (SensorKind::Power, 1_000_000.0, "W")
    } else if prefix.starts_with("curr") {
        (SensorKind::Current, 1000.0, "A")
    } else {
        (SensorKind::Other, 1.0, "")
    }
}

/// 供 UI 折线图使用的扁平温度序列。
pub fn temperature_series(report: &SensorReport) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for chip in &report.chips {
        for ch in &chip.channels {
            if ch.kind == SensorKind::Temp {
                if let Some(v) = ch.value {
                    out.push((ch.key.clone(), v));
                }
            }
        }
    }
    for tz in &report.thermal_zones {
        if let Some(v) = tz.temp_c.value {
            let name = tz.r#type.value.clone().unwrap_or_else(|| tz.path.clone());
            out.push((name, v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reads_hwmon_fixture() {
        let root = std::env::temp_dir().join(format!("aida-hwmon-{}", std::process::id()));
        let chip = root.join("sys/class/hwmon/hwmon0");
        fs::create_dir_all(&chip).unwrap();
        fs::write(chip.join("name"), "coretemp\n").unwrap();
        fs::write(chip.join("temp1_input"), "45000\n").unwrap();
        fs::write(chip.join("temp1_label"), "Package id 0\n").unwrap();
        fs::write(chip.join("temp1_max"), "80000\n").unwrap();
        fs::write(chip.join("temp1_crit"), "100000\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let report = collect(&ctx);
        assert_eq!(report.chips.len(), 1);
        assert_eq!(report.chips[0].channels[0].value, Some(45.0));
        assert_eq!(report.chips[0].channels[0].max, Some(80.0));
        assert_eq!(report.chips[0].channels[0].crit, Some(100.0));
        let _ = fs::remove_dir_all(&root);
    }
}
