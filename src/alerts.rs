//! 传感器阈值告警：用 hwmon 自带的 `*_max` / `*_crit` / `*_min`，可写 JSONL 日志。
//!
//! 不引入外部告警总线。只在值越过阈值时记一条，恢复时再记一条，避免刷屏。

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::probes::hwmon::{SensorKind, SensorReport};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Ok,
    Low,
    High,
    Crit,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Alert {
    pub key: String,
    pub label: String,
    pub level: AlertLevel,
    pub value: f64,
    pub threshold: f64,
    pub unit: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AlertEvent {
    pub unix_ms: u64,
    pub alert: Alert,
}

#[derive(Clone, Debug, Default)]
pub struct AlertLogger {
    last: HashMap<String, AlertLevel>,
}

impl AlertLogger {
    pub fn ingest(&mut self, now_ms: u64, current: &[Alert]) -> Vec<AlertEvent> {
        let mut events = Vec::new();
        let mut seen = HashMap::new();
        for a in current {
            seen.insert(a.key.clone(), a.level);
            let prev = self.last.get(&a.key).copied().unwrap_or(AlertLevel::Ok);
            if prev != a.level {
                events.push(AlertEvent {
                    unix_ms: now_ms,
                    alert: a.clone(),
                });
            }
        }
        for (key, prev) in self.last.clone() {
            if !seen.contains_key(&key) && prev != AlertLevel::Ok {
                events.push(AlertEvent {
                    unix_ms: now_ms,
                    alert: Alert {
                        key: key.clone(),
                        label: key,
                        level: AlertLevel::Ok,
                        value: 0.0,
                        threshold: 0.0,
                        unit: String::new(),
                        message: "恢复正常".into(),
                    },
                });
            }
        }
        self.last = seen;
        events
    }
}

pub fn evaluate(sensors: &SensorReport) -> Vec<Alert> {
    let mut out = Vec::new();
    for chip in &sensors.chips {
        for ch in &chip.channels {
            let Some(v) = ch.value else { continue };
            if let Some(crit) = ch.crit {
                if v >= crit {
                    out.push(Alert {
                        key: ch.key.clone(),
                        label: ch.label.clone(),
                        level: AlertLevel::Crit,
                        value: v,
                        threshold: crit,
                        unit: ch.unit.clone(),
                        message: format!("{} = {v:.2} {} ≥ crit {crit:.2}", ch.label, ch.unit),
                    });
                    continue;
                }
            }
            if let Some(max) = ch.max {
                if v >= max {
                    out.push(Alert {
                        key: ch.key.clone(),
                        label: ch.label.clone(),
                        level: AlertLevel::High,
                        value: v,
                        threshold: max,
                        unit: ch.unit.clone(),
                        message: format!("{} = {v:.2} {} ≥ max {max:.2}", ch.label, ch.unit),
                    });
                    continue;
                }
            }
            if let Some(min) = ch.min {
                if ch.kind != SensorKind::Other && v <= min {
                    out.push(Alert {
                        key: ch.key.clone(),
                        label: ch.label.clone(),
                        level: AlertLevel::Low,
                        value: v,
                        threshold: min,
                        unit: ch.unit.clone(),
                        message: format!("{} = {v:.2} {} ≤ min {min:.2}", ch.label, ch.unit),
                    });
                }
            }
        }
    }
    out
}

pub fn default_log_path() -> PathBuf {
    if let Ok(p) = std::env::var("AIDA_ALERT_LOG") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| std::env::temp_dir());
    base.join("aida/alerts.jsonl")
}

pub fn append_jsonl(path: &Path, events: &[AlertEvent]) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    for e in events {
        let line = serde_json::to_string(e).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::{AccessKind, Sample};
    use crate::probes::hwmon::{HwmonChip, SensorChannel};

    fn ch(key: &str, value: f64, max: Option<f64>, crit: Option<f64>) -> SensorChannel {
        SensorChannel {
            key: key.into(),
            label: key.into(),
            kind: SensorKind::Temp,
            raw: Sample {
                value: Some((value * 1000.0) as i64),
                access: AccessKind::Ok,
                source: "t".into(),
                hint: None,
            },
            value: Some(value),
            unit: "°C".into(),
            max,
            crit,
            min: None,
        }
    }

    #[test]
    fn crit_beats_max() {
        let sensors = SensorReport {
            chips: vec![HwmonChip {
                path: "x".into(),
                name: Sample::ok("coretemp".into(), "n"),
                channels: vec![ch("t", 100.0, Some(80.0), Some(95.0))],
            }],
            thermal_zones: vec![],
            cooling: vec![],
            notes: vec![],
        };
        let a = evaluate(&sensors);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].level, AlertLevel::Crit);
    }

    #[test]
    fn logger_emits_on_change_only() {
        let mut log = AlertLogger::default();
        let hot = Alert {
            key: "t".into(),
            label: "t".into(),
            level: AlertLevel::High,
            value: 90.0,
            threshold: 80.0,
            unit: "°C".into(),
            message: "hot".into(),
        };
        assert_eq!(log.ingest(1, &[hot.clone()]).len(), 1);
        assert_eq!(log.ingest(2, &[hot.clone()]).len(), 0);
        assert_eq!(log.ingest(3, &[]).len(), 1);
    }

    #[test]
    fn jsonl_appends_transitions() {
        let dir = std::env::temp_dir().join(format!("aida-alert-log-{}", std::process::id()));
        let path = dir.join("alerts.jsonl");
        let mut log = AlertLogger::default();
        let hot = Alert {
            key: "t".into(),
            label: "t".into(),
            level: AlertLevel::High,
            value: 90.0,
            threshold: 80.0,
            unit: "°C".into(),
            message: "hot".into(),
        };
        let events = log.ingest(1, &[hot]);
        append_jsonl(&path, &events).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"level\":\"high\""));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
