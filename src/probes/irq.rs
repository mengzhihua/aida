//! 中断：`/proc/interrupts`。不调用 `cat`/`irqbalance` 工具。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct IrqReport {
    pub cpu_count: usize,
    pub lines: Vec<IrqLine>,
    pub softirqs: Vec<IrqLine>,
    /// `/sys/kernel/irq` 目录项数，与 `/proc/interrupts` 行数不必相等（后者含 NMI/ERR）。
    pub sysfs_irqs: usize,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IrqLine {
    pub irq: String,
    pub counts: Vec<u64>,
    pub total: u64,
    pub extra: String,
    pub affinity: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> IrqReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("interrupts"));
    let (cpu_count, mut lines) = match (sample.access, sample.value.as_deref()) {
        (crate::access::AccessKind::Ok, Some(text)) => parse_interrupts(text),
        _ => {
            notes.push(sample.access_label());
            (0, Vec::new())
        }
    };
    lines.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.irq.cmp(&b.irq)));
    for line in &mut lines {
        if line.irq.chars().all(|c| c.is_ascii_digit()) {
            line.affinity = access::read_trimmed(ctx.proc_path(format!("irq/{}/smp_affinity_list", line.irq)));
        }
    }
    let soft_sample = access::read_trimmed(ctx.proc_path("softirqs"));
    let softirqs = match (soft_sample.access, soft_sample.value.as_deref()) {
        (crate::access::AccessKind::Ok, Some(text)) => {
            let (_, mut v) = parse_interrupts(text);
            v.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.irq.cmp(&b.irq)));
            v
        }
        _ => {
            notes.push(soft_sample.access_label());
            Vec::new()
        }
    };
    let sysfs_irqs = match access::list_dir_names(ctx.sys_path("kernel/irq")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n.len(),
        _ => 0,
    };
    IrqReport {
        cpu_count,
        lines,
        softirqs,
        sysfs_irqs,
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
            affinity: Sample::missing("smp_affinity_list"),
        });
    }
    (n, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::{AccessKind, ProbeCtx};

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
        assert_eq!(err.affinity.access, AccessKind::NotFound);
    }

    #[test]
    fn collect_softirqs_fixture() {
        let root = std::env::temp_dir().join(format!("aida-irq-{}", std::process::id()));
        std::fs::create_dir_all(root.join("proc")).unwrap();
        std::fs::write(
            root.join("proc/interrupts"),
            "           CPU0       CPU1\n 24:          2          0   IO-APIC\n",
        )
        .unwrap();
        std::fs::write(
            root.join("proc/softirqs"),
            "                    CPU0       CPU1\n          HI:          0          1\n       TIMER:         10          5\n",
        )
        .unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.cpu_count, 2);
        assert_eq!(r.lines.len(), 1);
        assert_eq!(r.softirqs.len(), 2);
        assert_eq!(r.softirqs[0].irq, "TIMER");
        assert_eq!(r.softirqs[0].total, 15);
        std::fs::create_dir_all(root.join("proc/irq/24")).unwrap();
        std::fs::write(root.join("proc/irq/24/smp_affinity_list"), "0-1\n").unwrap();
        std::fs::create_dir_all(root.join("sys/kernel/irq/24")).unwrap();
        std::fs::create_dir_all(root.join("sys/kernel/irq/28")).unwrap();
        let r2 = collect(&ctx);
        assert_eq!(r2.lines[0].affinity.value.as_deref(), Some("0-1"));
        assert_eq!(r2.sysfs_irqs, 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn softirqs_without_interrupts() {
        let root = std::env::temp_dir().join(format!("aida-irq-soft-{}", std::process::id()));
        std::fs::create_dir_all(root.join("proc")).unwrap();
        std::fs::write(
            root.join("proc/softirqs"),
            "                    CPU0       CPU1\n       TIMER:         10          5\n",
        )
        .unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert!(r.lines.is_empty());
        assert_eq!(r.softirqs.len(), 1);
        assert_eq!(r.softirqs[0].irq, "TIMER");
        assert_eq!(r.softirqs[0].total, 15);
        assert!(!r.notes.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
