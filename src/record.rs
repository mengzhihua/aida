//! 运行期指标记录：CPU / 内存 / 网络 / 磁盘 / 温度，按采样追加 JSONL。
//!
//! 对标 iStat Menus 的历史记录。不引入数据库；路径默认
//! `$AIDA_RECORD_LOG` 或 `$XDG_STATE_HOME/aida/history.jsonl`。

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
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
        // RX/TX、RD/WR 各自记是否见到样本。共享标志会把缺失侧写成 Some(0)。
        let mut net_rx = None;
        let mut net_tx = None;
        for i in &snap.net.interfaces {
            if i.name == "lo" {
                continue;
            }
            add_rate(&mut net_rx, i.rx_bps);
            add_rate(&mut net_tx, i.tx_bps);
        }
        let (disk_rd, disk_wr) = sum_disk_rates(
            snap.block
                .devices
                .iter()
                .map(|d| (d.r#type.as_str(), d.rd_bps, d.wr_bps)),
        );
        let hottest = hwmon::temperature_series(&snap.sensors)
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1));
        Self {
            cpu_pct: snap.cpu.utilization_pct,
            mem_used_pct,
            mem_used_kb,
            mem_total_kb,
            net_rx_bps: net_rx,
            net_tx_bps: net_tx,
            disk_rd_bps: disk_rd,
            disk_wr_bps: disk_wr,
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

fn add_rate(acc: &mut Option<f64>, v: Option<f64>) {
    if let Some(v) = v {
        *acc.get_or_insert(0.0) += v;
    }
}

/// 有非 mapper 块设备时跳过 Device Mapper，避免和底层盘双计。
/// 只有 `dm-*`（backing 是被排除的 loop/zram，或环境只暴露 mapper）时保留 mapper 速率。
fn sum_disk_rates<'a, I>(devices: I) -> (Option<f64>, Option<f64>)
where
    I: IntoIterator<Item = (&'a str, Option<f64>, Option<f64>)>,
{
    let items: Vec<_> = devices.into_iter().collect();
    let has_non_mapper = items.iter().any(|(ty, _, _)| *ty != "Device Mapper");
    let mut rd = None;
    let mut wr = None;
    for (ty, r, w) in items {
        if has_non_mapper && ty == "Device Mapper" {
            continue;
        }
        add_rate(&mut rd, r);
        add_rate(&mut wr, w);
    }
    (rd, wr)
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

/// GUI 历史页最多回放这么多样本（约 30 分钟 @ 1Hz）。磁盘 JSONL 也按这个裁。
pub const HISTORY_LOAD_CAP: usize = 1800;
/// 从文件尾估算每条样本的字节，用来只读最后一段。
pub const HISTORY_BYTES_PER_SAMPLE: u64 = 1024;
/// 录满 cap 后每隔这么多样本把磁盘裁回 cap。
pub const HISTORY_ROTATE_EVERY: usize = 256;

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

/// 读 JSONL 末尾最多 `cap` 条。大文件只看尾部窗口，不扫整份。
/// 缺文件当空历史，不是失败。坏行跳过。
pub fn load_recent(path: &Path, cap: usize) -> Result<Vec<HistorySample>, String> {
    if cap == 0 || !path.exists() {
        return Ok(Vec::new());
    }
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let window = (cap as u64)
        .saturating_add(2)
        .saturating_mul(HISTORY_BYTES_PER_SAMPLE);
    let mut reader = BufReader::new(&mut f);
    if len > window {
        reader
            .seek(SeekFrom::Start(len - window))
            .map_err(|e| e.to_string())?;
        let mut skip = Vec::new();
        reader.read_until(b'\n', &mut skip).map_err(|e| e.to_string())?;
    }
    let mut q = std::collections::VecDeque::with_capacity(cap.min(256));
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(s) = serde_json::from_str::<HistorySample>(line) else {
            continue;
        };
        if q.len() == cap {
            q.pop_front();
        }
        q.push_back(s);
    }
    Ok(q.into_iter().collect())
}

pub fn rewrite_jsonl(path: &Path, samples: &[HistorySample]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("jsonl.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        for s in samples {
            let line = serde_json::to_string(s).map_err(|e| e.to_string())?;
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
        }
        f.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// 启动时把超过 cap 的旧日志裁成尾部。按实际编码长度判断，不靠宽松字节预算。
pub fn retain_recent(path: &Path, cap: usize) -> Result<Vec<HistorySample>, String> {
    let samples = load_recent(path, cap)?;
    if !path.exists() {
        return Ok(samples);
    }
    let len = fs::metadata(path).map_err(|e| e.to_string())?.len();
    let mut encoded = 0u64;
    for s in &samples {
        let line = serde_json::to_string(s).map_err(|e| e.to_string())?;
        encoded = encoded.saturating_add(line.len() as u64 + 1);
    }
    if len > encoded.saturating_add(64) {
        rewrite_jsonl(path, &samples)?;
    }
    Ok(samples)
}

pub fn clear_jsonl(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, "").map_err(|e| e.to_string())
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
    fn add_rate_keeps_missing_side_none() {
        let mut rx = None;
        let mut tx = None;
        add_rate(&mut rx, Some(1000.0));
        add_rate(&mut tx, None);
        add_rate(&mut rx, Some(500.0));
        assert_eq!(rx, Some(1500.0));
        assert_eq!(tx, None);
        add_rate(&mut tx, Some(0.0));
        assert_eq!(tx, Some(0.0));
    }

    #[test]
    fn disk_rates_skip_mapper_when_backing_present() {
        let (rd, wr) = sum_disk_rates([
            ("virtio", Some(100.0), Some(20.0)),
            ("Device Mapper", Some(100.0), Some(20.0)),
        ]);
        assert_eq!(rd, Some(100.0));
        assert_eq!(wr, Some(20.0));
    }

    #[test]
    fn disk_rates_keep_mapper_when_only_dm() {
        let (rd, wr) = sum_disk_rates([("Device Mapper", Some(50.0), None)]);
        assert_eq!(rd, Some(50.0));
        assert_eq!(wr, None);
    }

    #[test]
    fn compact_line_omits_missing_net_and_disk_sides() {
        let m = StatusMeters {
            cpu_pct: Some(1.0),
            net_rx_bps: Some(1000.0),
            net_tx_bps: None,
            disk_rd_bps: None,
            disk_wr_bps: Some(12_300.0),
            ..StatusMeters::default()
        };
        let line = m.compact_line();
        assert!(line.contains("↓1.0K"), "{line}");
        assert!(!line.contains('↑'), "{line}");
        assert!(line.contains("W12.3K"), "{line}");
        assert!(!line.contains("R12") && !line.contains("R0"), "{line}");
        let sample = m.to_sample(9);
        assert_eq!(sample.net_rx_bps, Some(1000.0));
        assert_eq!(sample.net_tx_bps, None);
        assert_eq!(sample.disk_rd_bps, None);
        assert_eq!(sample.disk_wr_bps, Some(12_300.0));
        assert_eq!(sample.unix_ms, 9);
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
        let loaded = load_recent(&path, 8).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].unix_ms, 1);
        assert_eq!(loaded[1].cpu_pct, Some(10.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_recent_keeps_tail_and_skips_garbage() {
        let dir = std::env::temp_dir().join(format!("aida-record-tail-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            "not-json\n{\"unix_ms\":1,\"cpu_pct\":1.0}\n\n{\"unix_ms\":2,\"cpu_pct\":2.0}\n{\"unix_ms\":3,\"cpu_pct\":3.0}\n",
        )
        .unwrap();
        let loaded = load_recent(&path, 2).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].unix_ms, 2);
        assert_eq!(loaded[1].unix_ms, 3);
        clear_jsonl(&path).unwrap();
        assert_eq!(load_recent(&path, 8).unwrap().len(), 0);
        let missing = dir.join("no-such.jsonl");
        assert!(load_recent(&missing, 8).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_recent_seeks_tail_of_padded_file() {
        let dir = std::env::temp_dir().join(format!("aida-record-seek-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        fs::create_dir_all(&dir).unwrap();
        let mut body = "x".repeat(16 * 1024);
        body.push('\n');
        for i in 0..12u64 {
            body.push_str(&format!("{{\"unix_ms\":{i},\"cpu_pct\":{i}.0}}\n"));
        }
        fs::write(&path, &body).unwrap();
        assert!(fs::metadata(&path).unwrap().len() > 8 * HISTORY_BYTES_PER_SAMPLE);
        let loaded = load_recent(&path, 3).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].unix_ms, 9);
        assert_eq!(loaded[2].unix_ms, 11);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retain_recent_rewrites_oversize_log() {
        let dir = std::env::temp_dir().join(format!("aida-record-retain-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        fs::create_dir_all(&dir).unwrap();
        let mut body = "pad".repeat(8 * 1024);
        body.push('\n');
        for i in 0..20u64 {
            body.push_str(&format!("{{\"unix_ms\":{i},\"cpu_pct\":1.0}}\n"));
        }
        fs::write(&path, &body).unwrap();
        let before = fs::metadata(&path).unwrap().len();
        let loaded = retain_recent(&path, 4).unwrap();
        assert_eq!(loaded.len(), 4);
        assert_eq!(loaded[0].unix_ms, 16);
        let after = fs::metadata(&path).unwrap().len();
        assert!(after < before, "before={before} after={after}");
        assert_eq!(load_recent(&path, 8).unwrap().len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retain_recent_trims_many_small_lines_under_byte_budget() {
        let dir = std::env::temp_dir().join(format!("aida-record-lines-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        fs::create_dir_all(&dir).unwrap();
        let mut body = String::new();
        for i in 0..40u64 {
            body.push_str(&format!("{{\"unix_ms\":{i},\"cpu_pct\":1.0}}\n"));
        }
        fs::write(&path, &body).unwrap();
        let before = fs::metadata(&path).unwrap().len();
        assert!(before < (4 + 8) * HISTORY_BYTES_PER_SAMPLE);
        let loaded = retain_recent(&path, 4).unwrap();
        assert_eq!(loaded.len(), 4);
        assert_eq!(loaded[0].unix_ms, 36);
        let after = fs::metadata(&path).unwrap().len();
        assert!(after < before, "before={before} after={after}");
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
