//! 物理地址图：`/proc/iomem`。不调用 `lshw`。地址在非 root 下常被清零。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct IomemReport {
    pub regions: Vec<IomemRegion>,
    pub summaries: Vec<IomemSummary>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IomemRegion {
    pub start: u64,
    pub end: u64,
    pub size: u64,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct IomemSummary {
    pub name: String,
    pub count: usize,
    pub size: u64,
}

pub fn collect(ctx: &ProbeCtx) -> IomemReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("iomem"));
    let regions = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_iomem(text),
        _ => {
            notes.push(sample.access_label());
            Vec::new()
        }
    };
    if regions.iter().any(|r| r.start == 0 && r.end == 0) {
        notes.push(
            "iomem 起止地址为 0：非 root 时内核会隐藏真实物理地址，只保留区域名称。".into(),
        );
    }
    let summaries = summarize(&regions);
    IomemReport {
        regions,
        summaries,
        notes,
    }
}

pub fn parse_iomem(text: &str) -> Vec<IomemRegion> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let Some((range, name)) = t.split_once(" : ") else {
            continue;
        };
        let Some((a, b)) = range.split_once('-') else {
            continue;
        };
        let start = u64::from_str_radix(a.trim(), 16).unwrap_or(0);
        let end = u64::from_str_radix(b.trim(), 16).unwrap_or(0);
        let size = end.saturating_sub(start).saturating_add(1);
        out.push(IomemRegion {
            start,
            end,
            size,
            name: name.trim().to_string(),
        });
    }
    out
}

fn summarize(regions: &[IomemRegion]) -> Vec<IomemSummary> {
    let mut map: std::collections::BTreeMap<String, (usize, u64)> = std::collections::BTreeMap::new();
    for r in regions {
        let e = map.entry(r.name.clone()).or_insert((0, 0));
        e.0 += 1;
        e.1 = e.1.saturating_add(r.size);
    }
    let mut v: Vec<_> = map
        .into_iter()
        .map(|(name, (count, size))| IomemSummary { name, count, size })
        .collect();
    v.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ram_and_reserved() {
        let text = "\
00000000-0009ffff : System RAM
000a0000-000fffff : Reserved
10000000-1fffffff : System RAM
";
        let r = parse_iomem(text);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].name, "System RAM");
        assert_eq!(r[0].size, 0xa0000);
        let s = summarize(&r);
        let ram = s.iter().find(|x| x.name == "System RAM").unwrap();
        assert_eq!(ram.count, 2);
    }
}
