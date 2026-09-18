//! 运行期指标记录：CPU / 内存 / 网络 / 磁盘 / 温度，按采样追加 JSONL。
//!
//! 对标 iStat Menus 的历史记录。不引入数据库；路径默认
//! `$AIDA_RECORD_LOG` 或 `$XDG_STATE_HOME/aida/history.jsonl`。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::probes::hwmon;
use crate::snapshot::HardwareSnapshot;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct StatusMeters {
    pub cpu_pct: Option<f32>,
    pub mem_used_pct: Option<f32>,
    pub mem_used_kb: Option<u64>,
    pub mem_total_kb: Option<u64>,
    pub net_rx_bps: Option<f64>,
    pub net_tx_bps: Option<f64>,
    pub disk_rd_bps: Option<f64>,
    pub disk_wr_bps: Option<f64>,
    pub temp_c: Option<f64>,
    pub temp_key: Option<String>,
    pub load_1: Option<f64>,
    pub load_5: Option<f64>,
    pub load_15: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct HistorySample {
    pub unix_ms: u64,
    pub cpu_pct: Option<f32>,
    pub mem_used_pct: Option<f32>,
    pub mem_used_kb: Option<u64>,
    pub mem_total_kb: Option<u64>,
    pub net_rx_bps: Option<f64>,
    pub net_tx_bps: Option<f64>,
    pub disk_rd_bps: Option<f64>,
    pub disk_wr_bps: Option<f64>,
    pub temp_c: Option<f64>,
    pub temp_key: Option<String>,
    pub load_1: Option<f64>,
    pub load_5: Option<f64>,
    pub load_15: Option<f64>,
}

impl StatusMeters {
    pub fn from_snapshot(snap: &HardwareSnapshot) -> Self {
        let (mem_used_kb, mem_total_kb, mem_used_pct) =
            match (snap.memory.total_kb.value, snap.memory.available_kb.value) {
                (Some(total), Some(avail)) if total > 0 => {
                    let used = total.saturating_sub(avail);
                    (
                        Some(used),
                        Some(total),
                        Some((used as f32) * 100.0 / total as f32),
                    )
                }
                (Some(total), _) => (None, Some(total), None),
                _ => (None, None, None),
            };
        let mut net_rx = 0.0;
        let mut net_tx = 0.0;
        let mut net_any = false;
        for i in &snap.net.interfaces {
            if i.name == "lo" {
                continue;
            }
            if let Some(v) = i.rx_bps {
                net_rx += v;
                net_any = true;
            }
            if let Some(v) = i.tx_bps {
                net_tx += v;
                net_any = true;
            }
        }
        let mut disk_rd = 0.0;
        let mut disk_wr = 0.0;
        let mut disk_any = false;
        for d in &snap.block.devices {
            if d.r#type == "Device Mapper" {
                continue;
            }
            if let Some(v) = d.rd_bps {
                disk_rd += v;
                disk_any = true;
            }
            if let Some(v) = d.wr_bps {
                disk_wr += v;
                disk_any = true;
            }
        }
        let hottest = hwmon::temperature_series(&snap.sensors)
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1));
        Self {
            cpu_pct: snap.cpu.utilization_pct,
            mem_used_pct,
            mem_used_kb,
            mem_total_kb,
            net_rx_bps: net_any.then_some(net_rx),
            net_tx_bps: net_any.then_some(net_tx),
            disk_rd_bps: disk_any.then_some(disk_rd),
            disk_wr_bps: disk_any.then_some(disk_wr),
            temp_c: hottest.as_ref().map(|h| h.1),
            temp_key: hottest.map(|h| h.0),
            load_1: snap.software.load_1.value,
            load_5: snap.software.load_5.value,
            load_15: snap.software.load_15.value,
        }
    }

    pub fn to_sample(&self, unix_ms: u64) -> HistorySample {
        HistorySample {
            unix_ms,
            cpu_pct: self.cpu_pct,
            mem_used_pct: self.mem_used_pct,
            mem_used_kb: self.mem_used_kb,
            mem_total_kb: self.mem_total_kb,
            net_rx_bps: self.net_rx_bps,
            net_tx_bps: self.net_tx_bps,
            disk_rd_bps: self.disk_rd_bps,
            disk_wr_bps: self.disk_wr_bps,
            temp_c: self.temp_c,
            temp_key: self.temp_key.clone(),
            load_1: self.load_1,
            load_5: self.load_5,
            load_15: self.load_15,
        }
    }

    /// iStat Menus 风格的一行摘要，状态栏与置顶条共用。
    pub fn compact_line(&self) -> String {
        let cpu = self
            .cpu_pct
            .map(|v| format!("CPU {v:.1}%"))
            .unwrap_or_else(|| "CPU —".into());
        let mem = match (self.mem_used_pct, self.mem_used_kb, self.mem_total_kb) {
            (Some(pct), Some(used), Some(total)) => {
                format!("MEM {pct:.0}% {}/{}", format_mem(used), format_mem(total))
            }
            (Some(pct), _, _) => format!("MEM {pct:.0}%"),
            _ => "MEM —".into(),
        };
        let net = match (self.net_rx_bps, self.net_tx_bps) {
            (Some(rx), Some(tx)) => format!("↓{} ↑{}", format_rate(rx), format_rate(tx)),
            (Some(rx), None) => format!("↓{}", format_rate(rx)),
            (None, Some(tx)) => format!("↑{}", format_rate(tx)),
            _ => "NET —".into(),
        };
        let disk = match (self.disk_rd_bps, self.disk_wr_bps) {
            (Some(rd), Some(wr)) => format!("R{} W{}", format_rate(rd), format_rate(wr)),
            (Some(rd), None) => format!("R{}", format_rate(rd)),
            (None, Some(wr)) => format!("W{}", format_rate(wr)),
            _ => "DISK —".into(),
        };
        let temp = self
            .temp_c
            .map(|v| format!("{v:.1}°C"))
            .unwrap_or_else(|| "TEMP —".into());
        let load = match (self.load_1, self.load_5, self.load_15) {
            (Some(a), Some(b), Some(c)) => format!("  LD {a:.2} {b:.2} {c:.2}"),
            _ => String::new(),
        };
        format!("{cpu}  {mem}  {net}  {disk}  {temp}{load}")
    }
}

pub fn format_mem(kb: u64) -> String {
    let b = kb as f64 * 1024.0;
    if b >= 1_073_741_824.0 {
        format!("{:.1}G", b / 1_073_741_824.0)
    } else if b >= 1_048_576.0 {
        format!("{:.1}M", b / 1_048_576.0)
    } else {
        format!("{kb}K")
    }
}

pub fn format_rate(bps: f64) -> String {
    let v = bps.abs();
    if v < 1000.0 {
        format!("{:.0}B", bps)
    } else if v < 1_000_000.0 {
        format!("{:.1}K", bps / 1000.0)
    } else if v < 1_000_000_000.0 {
        format!("{:.1}M", bps / 1_000_000.0)
    } else {
        format!("{:.2}G", bps / 1_000_000_000.0)
    }
}

pub fn default_log_path() -> PathBuf {
    if let Ok(p) = std::env::var("AIDA_RECORD_LOG") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| std::env::temp_dir());
    base.join("aida/history.jsonl")
}

pub fn append_jsonl(path: &Path, samples: &[HistorySample]) -> Result<(), String> {
    if samples.is_empty() {
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
    for s in samples {
        let line = serde_json::to_string(s).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_line_formats_istat_style() {
        let m = StatusMeters {
            cpu_pct: Some(12.34),
            mem_used_pct: Some(48.6),
            mem_used_kb: Some(4000),
            mem_total_kb: Some(8192),
            net_rx_bps: Some(1_234_000.0),
            net_tx_bps: Some(800.0),
            disk_rd_bps: Some(0.0),
            disk_wr_bps: Some(12_300.0),
            temp_c: Some(45.21),
            temp_key: Some("coretemp".into()),
            load_1: Some(0.05),
            load_5: Some(0.06),
            load_15: Some(0.03),
        };
        let line = m.compact_line();
        assert!(line.contains("CPU 12.3%"), "{line}");
        assert!(
            line.contains("MEM 49%") || line.contains("MEM 48%"),
            "{line}"
        );
        assert!(line.contains("3.9M/8.0M"), "{line}");
        assert!(line.contains("↓1.2M"), "{line}");
        assert!(line.contains("↑800B"), "{line}");
        assert!(line.contains("R0B"), "{line}");
        assert!(line.contains("W12.3K"), "{line}");
        assert!(line.contains("45.2°C"), "{line}");
        assert!(line.contains("LD 0.05 0.06 0.03"), "{line}");
    }

    #[test]
    fn format_rate_uses_si_units() {
        assert_eq!(format_rate(800.0), "800B");
        assert_eq!(format_rate(12_300.0), "12.3K");
        assert_eq!(format_rate(1_234_000.0), "1.2M");
    }

    #[test]
    fn jsonl_appends_history_samples() {
        let dir = std::env::temp_dir().join(format!("aida-record-log-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        let meters = StatusMeters {
            cpu_pct: Some(10.0),
            mem_used_pct: Some(20.0),
            ..StatusMeters::default()
        };
        append_jsonl(&path, &[meters.to_sample(1)]).unwrap();
        append_jsonl(&path, &[meters.to_sample(2)]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("\"cpu_pct\":10.0"));
        assert!(text.contains("\"unix_ms\":2"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
