//! 中断：`/proc/interrupts`。不调用 `cat`/`irqbalance` 工具。

use serde::Serialize;

use crate::access::{self, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct IrqReport {
    pub cpu_count: usize,
    pub lines: Vec<IrqLine>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IrqLine {
    pub irq: String,
    pub counts: Vec<u64>,
    pub total: u64,
    pub extra: String,
}

pub fn collect(ctx: &ProbeCtx) -> IrqReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("interrupts"));
    let (cpu_count, mut lines) = match (sample.access, sample.value.as_deref()) {
        (crate::access::AccessKind::Ok, Some(text)) => parse_interrupts(text),
        _ => {
            notes.push(sample.access_label());
            return IrqReport {
                cpu_count: 0,
                lines: Vec::new(),
                notes,
            };
        }
    };
    lines.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.irq.cmp(&b.irq)));
    IrqReport {
        cpu_count,
        lines,
        notes,
    }
}

pub fn parse_interrupts(text: &str) -> (usize, Vec<IrqLine>) {
    let mut lines_iter = text.lines();
    let header = lines_iter.next().unwrap_or("");
    let cpu_count = header.split_whitespace().filter(|t| t.starts_with("CPU")).count();
    let n = if cpu_count == 0 { 1 } else { cpu_count };
    let mut out = Vec::new();
    for line in lines_iter {
        let Some((left, right)) = line.split_once(':') else {
            continue;
        };
        let irq = left.trim().to_string();
        if irq.is_empty() {
            continue;
        }
        let mut counts = Vec::new();
        let mut rest = Vec::new();
        for tok in right.split_whitespace() {
            if rest.is_empty() {
                if let Ok(v) = tok.parse::<u64>() {
                    if counts.len() < n {
                        counts.push(v);
                        continue;
                    }
                }
            }
            rest.push(tok);
        }
        if counts.is_empty() {
            continue;
        }
        let total: u64 = counts.iter().sum();
        out.push(IrqLine {
            irq,
            counts,
            total,
            extra: rest.join(" "),
        });
    }
    (n, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_per_cpu_and_err() {
        let text = "\
           CPU0       CPU1       CPU2       CPU3       
 24:          2          0          0          0   IO-APIC   5-edge      ACPI:Ged
 28:          0          0          1          0  PCI-MSIX-0000:00:01.0   0-edge      virtio0-config
NMI:          0          0          0          0   Non-maskable interrupts
ERR:          0
";
        let (n, lines) = parse_interrupts(text);
        assert_eq!(n, 4);
        let irq24 = lines.iter().find(|l| l.irq == "24").unwrap();
        assert_eq!(irq24.total, 2);
        assert!(irq24.extra.contains("ACPI:Ged"));
        let virtio = lines.iter().find(|l| l.irq == "28").unwrap();
        assert_eq!(virtio.total, 1);
        let err = lines.iter().find(|l| l.irq == "ERR").unwrap();
        assert_eq!(err.total, 0);
    }
}
