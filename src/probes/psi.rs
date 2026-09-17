//! PSI：`/proc/pressure/{cpu,memory,io}`。不调用 `psi`/`stress` 工具。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct PsiReport {
    pub cpu: Option<PsiResource>,
    pub memory: Option<PsiResource>,
    pub io: Option<PsiResource>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PsiResource {
    pub some: PsiLine,
    pub full: Option<PsiLine>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PsiLine {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total: u64,
}

pub fn collect(ctx: &ProbeCtx) -> PsiReport {
    let mut notes = Vec::new();
    let cpu = read_res(ctx, "cpu", &mut notes);
    let memory = read_res(ctx, "memory", &mut notes);
    let io = read_res(ctx, "io", &mut notes);
    if cpu.is_none() && memory.is_none() && io.is_none() && notes.is_empty() {
        notes.push("未导出 /proc/pressure（内核未开 CONFIG_PSI 时常见）。".into());
    }
    PsiReport {
        cpu,
        memory,
        io,
        notes,
    }
}

fn read_res(ctx: &ProbeCtx, name: &str, notes: &mut Vec<String>) -> Option<PsiResource> {
    let sample = access::read_trimmed(ctx.proc_path(format!("pressure/{name}")));
    match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_psi(text),
        (AccessKind::NotFound, _) => None,
        _ => {
            notes.push(format!("{name}: {}", sample.access_label()));
            None
        }
    }
}

pub fn parse_psi(text: &str) -> Option<PsiResource> {
    let mut some = None;
    let mut full = None;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(kind) = it.next() else { continue };
        let parsed = parse_psi_fields(it)?;
        match kind {
            "some" => some = Some(parsed),
            "full" => full = Some(parsed),
            _ => {}
        }
    }
    Some(PsiResource {
        some: some?,
        full,
    })
}

fn parse_psi_fields(it: std::str::SplitWhitespace<'_>) -> Option<PsiLine> {
    let mut avg10 = None;
    let mut avg60 = None;
    let mut avg300 = None;
    let mut total = None;
    for tok in it {
        if let Some((k, v)) = tok.split_once('=') {
            match k {
                "avg10" => avg10 = v.parse().ok(),
                "avg60" => avg60 = v.parse().ok(),
                "avg300" => avg300 = v.parse().ok(),
                "total" => total = v.parse().ok(),
                _ => {}
            }
        }
    }
    Some(PsiLine {
        avg10: avg10?,
        avg60: avg60?,
        avg300: avg300?,
        total: total?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_some_and_full() {
        let t = "\
some avg10=0.12 avg60=0.04 avg300=0.01 total=1280
full avg10=0.00 avg60=0.00 avg300=0.00 total=3
";
        let r = parse_psi(t).unwrap();
        assert_eq!(r.some.avg10, 0.12);
        assert_eq!(r.some.total, 1280);
        assert_eq!(r.full.unwrap().total, 3);
    }
}
