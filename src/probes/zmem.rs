//! 压缩内存：zram 块设备 + zswap 参数。不调用 `zramctl`/`swapon`。
//!
//! zram 仍从 `/sys/block` 跳过常规块设备列表，避免和 virtio/NVMe 混在一张表里。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct ZmemReport {
    pub zram: Vec<ZramDevice>,
    pub zswap: ZswapInfo,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ZramDevice {
    pub name: String,
    pub disksize: Sample<u64>,
    pub algorithm: Sample<String>,
    pub orig_bytes: Sample<u64>,
    pub compr_bytes: Sample<u64>,
    pub mem_used: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ZswapInfo {
    pub enabled: Sample<String>,
    pub compressor: Sample<String>,
    pub zpool: Sample<String>,
    pub max_pool_percent: Sample<String>,
    pub shrinker_enabled: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> ZmemReport {
    let mut notes = Vec::new();
    let zram = read_zram(ctx, &mut notes);
    let zswap = read_zswap(ctx);
    if zswap.enabled.access == AccessKind::NotFound {
        notes.push("无 zswap 模块参数（内核未编译 CONFIG_ZSWAP 时常见）。".into());
    }
    ZmemReport {
        zram,
        zswap,
        notes,
    }
}

fn read_zram(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<ZramDevice> {
    let root = ctx.sys_path("block");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s if s.access == AccessKind::NotFound => return Vec::new(),
        s => {
            notes.push(s.access_label());
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("zram")) {
        if name.chars().skip(4).any(|c| !c.is_ascii_digit()) {
            continue;
        }
        let dir = root.join(&name);
        let mm = access::read_trimmed(dir.join("mm_stat"));
        let (orig, compr, used) = match mm.value.as_deref() {
            Some(text) => parse_mm_stat(text),
            None => (None, None, None),
        };
        out.push(ZramDevice {
            disksize: access::read_u64(dir.join("disksize")),
            algorithm: current_algorithm(access::read_trimmed(dir.join("comp_algorithm"))),
            orig_bytes: sample_or_missing(orig, &mm, "mm_stat.orig"),
            compr_bytes: sample_or_missing(compr, &mm, "mm_stat.compr"),
            mem_used: sample_or_missing(used, &mm, "mm_stat.mem_used"),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    if out.is_empty() {
        notes.push("无 zram 设备（未加载 zram 或未 mkswap 时正常）。".into());
    }
    out
}

fn sample_or_missing(v: Option<u64>, mm: &Sample<String>, field: &str) -> Sample<u64> {
    match v {
        Some(n) => Sample::ok(n, mm.source.clone()),
        None => Sample {
            value: None,
            access: mm.access,
            source: format!("{}:{field}", mm.source),
            hint: mm.hint.clone(),
        },
    }
}

fn current_algorithm(s: Sample<String>) -> Sample<String> {
    match s.value {
        Some(text) => Sample::ok(parse_algorithm(&text), s.source),
        None => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

/// `comp_algorithm` 形如 `[lzo] lz4 zstd`，方括号里是当前算法。
pub fn parse_algorithm(text: &str) -> String {
    if let Some(start) = text.find('[') {
        if let Some(rel_end) = text[start + 1..].find(']') {
            return text[start + 1..start + 1 + rel_end].trim().to_string();
        }
    }
    text.split_whitespace().next().unwrap_or("").to_string()
}

/// `/sys/block/zramN/mm_stat`：orig compr mem_used …（字节）。
pub fn parse_mm_stat(text: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let mut it = text.split_whitespace();
    let orig = it.next().and_then(|s| s.parse().ok());
    let compr = it.next().and_then(|s| s.parse().ok());
    let used = it.next().and_then(|s| s.parse().ok());
    (orig, compr, used)
}

fn read_zswap(ctx: &ProbeCtx) -> ZswapInfo {
    let dir = ctx.sys_path("module/zswap/parameters");
    ZswapInfo {
        enabled: access::read_trimmed(dir.join("enabled")),
        compressor: access::read_trimmed(dir.join("compressor")),
        zpool: access::read_trimmed(dir.join("zpool")),
        max_pool_percent: access::read_trimmed(dir.join("max_pool_percent")),
        shrinker_enabled: access::read_trimmed(dir.join("shrinker_enabled")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn algorithm_and_mm_stat() {
        assert_eq!(parse_algorithm("[lzo] lz4 zstd"), "lzo");
        assert_eq!(parse_algorithm("zstd"), "zstd");
        let (o, c, u) = parse_mm_stat("4096 1024 2048 0 3 0 0 0");
        assert_eq!(o, Some(4096));
        assert_eq!(c, Some(1024));
        assert_eq!(u, Some(2048));
    }

    #[test]
    fn zram_and_zswap_fixture() {
        let root = std::env::temp_dir().join(format!("aida-zmem-{}", std::process::id()));
        let zram = root.join("sys/block/zram0");
        fs::create_dir_all(&zram).unwrap();
        fs::write(zram.join("disksize"), "1073741824\n").unwrap();
        fs::write(zram.join("comp_algorithm"), "[lz4] lzo zstd\n").unwrap();
        fs::write(zram.join("mm_stat"), "8192 2048 4096 0 1 0 0 0\n").unwrap();
        let zs = root.join("sys/module/zswap/parameters");
        fs::create_dir_all(&zs).unwrap();
        fs::write(zs.join("enabled"), "Y\n").unwrap();
        fs::write(zs.join("compressor"), "lzo\n").unwrap();
        fs::write(zs.join("zpool"), "zsmalloc\n").unwrap();
        fs::write(zs.join("max_pool_percent"), "20\n").unwrap();
        fs::write(zs.join("shrinker_enabled"), "N\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.zram.len(), 1);
        assert_eq!(r.zram[0].disksize.value, Some(1073741824));
        assert_eq!(r.zram[0].algorithm.value.as_deref(), Some("lz4"));
        assert_eq!(r.zram[0].orig_bytes.value, Some(8192));
        assert_eq!(r.zswap.enabled.value.as_deref(), Some("Y"));
        assert_eq!(r.zswap.zpool.value.as_deref(), Some("zsmalloc"));
        let _ = fs::remove_dir_all(&root);
    }
}
