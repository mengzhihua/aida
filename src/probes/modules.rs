//! 内核模块：`/proc/modules`。不调用 `lsmod`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct ModulesReport {
    pub modules: Vec<KernelModule>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct KernelModule {
    pub name: String,
    pub size_bytes: u64,
    pub refcount: u64,
    pub used_by: Vec<String>,
    pub state: String,
}

pub fn collect(ctx: &ProbeCtx) -> ModulesReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("modules"));
    let modules = match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => parse_modules(text),
        (AccessKind::Ok, None) => {
            notes.push("未加载可卸载模块（内置内核或容器常见，/proc/modules 为空）。".into());
            Vec::new()
        }
        _ => {
            notes.push(sample.access_label());
            Vec::new()
        }
    };
    ModulesReport { modules, notes }
}

pub fn parse_modules(text: &str) -> Vec<KernelModule> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let Some(size_bytes) = it.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        let Some(refcount) = it.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        let used = it.next().unwrap_or("-");
        let used_by = if used == "-" {
            Vec::new()
        } else {
            used.split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        };
        let state = it.next().unwrap_or("").to_string();
        out.push(KernelModule {
            name: name.to_string(),
            size_bytes,
            refcount,
            used_by,
            state,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lsmod_style() {
        let text = "nf_conntrack 163840 2 nf_nat,xt_conntrack Live 0x00000000\next4 983040 1 - Live 0x00000000\n";
        let m = parse_modules(text);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].name, "ext4");
        assert_eq!(m[1].used_by, vec!["nf_nat", "xt_conntrack"]);
        assert_eq!(m[1].state, "Live");
    }
}
