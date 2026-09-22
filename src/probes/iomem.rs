//! 物理地址图：`/proc/iomem`。不调用 `lshw`。地址在非 root 下常被清零。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct IomemReport {
    pub regions: Vec<IomemRegion>,
    pub summaries: Vec<IomemSummary>,
    pub ioports: Vec<IomemRegion>,
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
    let ports_sample = access::read_trimmed(ctx.proc_path("ioports"));
    let ioports = match (ports_sample.access, ports_sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_iomem(text),
        _ => Vec::new(),
    };
    if !ioports.is_empty() && ioports.iter().all(|r| r.start == 0 && r.end == 0) {
        notes.push("ioports 地址为 0：非 root 时内核会隐藏真实端口范围。".into());
    }
    IomemReport {
        regions,
        summaries,
        ioports,
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
        // 非 root 时内核把起止写成 0-0，此时 +1 会把「条目数」显示成 N 字节。
        let size = if start == 0 && end == 0 {
            0
        } else {
            end.saturating_sub(start).saturating_add(1)
        };
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
        let ports = parse_iomem("0000-0000 : serial\n0000-0000 : PCI conf1\n");
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].name, "serial");
        assert_eq!(ports[0].size, 0);
        let hidden = parse_iomem(
            "00000000-00000000 : System RAM\n00000000-00000000 : System RAM\n00000000-00000000 : System RAM\n00000000-00000000 : virtio-pci-modern\n00000000-00000000 : virtio-pci-modern\n00000000-00000000 : virtio-pci-modern\n00000000-00000000 : virtio-pci-modern\n00000000-00000000 : virtio-pci-modern\n",
        );
        let sum = summarize(&hidden);
        let ram = sum.iter().find(|x| x.name == "System RAM").unwrap();
        assert_eq!(ram.count, 3);
        assert_eq!(ram.size, 0, "hidden addresses must not become N bytes");
        let virt = sum.iter().find(|x| x.name == "virtio-pci-modern").unwrap();
        assert_eq!(virt.count, 5);
        assert_eq!(virt.size, 0);
    }
}
