//! 内核加密算法：`/proc/crypto`。不调用 `cryptsetup`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx};

#[derive(Clone, Debug, Serialize)]
pub struct CryptoReport {
    pub total: usize,
    pub internal: usize,
    pub failed_selftest: usize,
    pub types: Vec<CryptoTypeCount>,
    /// 非 internal 算法，上限 32，避免把 70+ 条全塞进 JSON。
    pub algs: Vec<CryptoAlg>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CryptoTypeCount {
    pub type_name: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CryptoAlg {
    pub name: String,
    pub driver: String,
    pub module: String,
    pub type_name: String,
    pub selftest: String,
    pub priority: Option<i64>,
}

pub fn collect(ctx: &ProbeCtx) -> CryptoReport {
    let mut notes = Vec::new();
    let sample = access::read_trimmed(ctx.proc_path("crypto"));
    match (sample.access, sample.value.as_deref()) {
        (AccessKind::Ok, Some(text)) => {
            let (mut report, skipped) = from_parsed(parse_crypto(text));
            if skipped > 0 {
                notes.push(format!("另有 {skipped} 个非 internal 算法未列入表（已截断）。"));
            }
            report.notes = notes;
            report
        }
        _ => {
            notes.push(sample.access_label());
            CryptoReport {
                total: 0,
                internal: 0,
                failed_selftest: 0,
                types: Vec::new(),
                algs: Vec::new(),
                notes,
            }
        }
    }
}

fn from_parsed(all: Vec<CryptoAlgRaw>) -> (CryptoReport, usize) {
    let total = all.len();
    let internal = all.iter().filter(|a| a.internal).count();
    let failed_selftest = all
        .iter()
        .filter(|a| a.selftest != "passed" && !a.selftest.is_empty())
        .count();
    let mut type_map = std::collections::BTreeMap::<String, usize>::new();
    for a in &all {
        *type_map.entry(a.type_name.clone()).or_insert(0) += 1;
    }
    let types = type_map
        .into_iter()
        .map(|(type_name, count)| CryptoTypeCount { type_name, count })
        .collect();
    let visible: Vec<CryptoAlg> = all
        .into_iter()
        .filter(|a| !a.internal)
        .map(|a| CryptoAlg {
            name: a.name,
            driver: a.driver,
            module: a.module,
            type_name: a.type_name,
            selftest: a.selftest,
            priority: a.priority,
        })
        .collect();
    let skipped = visible.len().saturating_sub(32);
    (
        CryptoReport {
            total,
            internal,
            failed_selftest,
            types,
            algs: visible.into_iter().take(32).collect(),
            notes: Vec::new(),
        },
        skipped,
    )
}

struct CryptoAlgRaw {
    name: String,
    driver: String,
    module: String,
    type_name: String,
    selftest: String,
    internal: bool,
    priority: Option<i64>,
}

/// `/proc/crypto` 用空行分隔算法块，每行 `key : value`。
fn parse_crypto(text: &str) -> Vec<CryptoAlgRaw> {
    let mut out = Vec::new();
    let mut cur = CryptoAlgRaw {
        name: String::new(),
        driver: String::new(),
        module: String::new(),
        type_name: String::new(),
        selftest: String::new(),
        internal: false,
        priority: None,
    };
    let mut has = false;
    let flush = |cur: &mut CryptoAlgRaw, has: &mut bool, out: &mut Vec<CryptoAlgRaw>| {
        if *has && !cur.name.is_empty() {
            out.push(std::mem::replace(
                cur,
                CryptoAlgRaw {
                    name: String::new(),
                    driver: String::new(),
                    module: String::new(),
                    type_name: String::new(),
                    selftest: String::new(),
                    internal: false,
                    priority: None,
                },
            ));
        }
        *has = false;
    };
    for line in text.lines() {
        if line.trim().is_empty() {
            flush(&mut cur, &mut has, &mut out);
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        has = true;
        let key = k.trim();
        let val = v.trim();
        match key {
            "name" => cur.name = val.to_string(),
            "driver" => cur.driver = val.to_string(),
            "module" => cur.module = val.to_string(),
            "type" => cur.type_name = val.to_string(),
            "selftest" => cur.selftest = val.to_string(),
            "internal" => cur.internal = val == "yes",
            "priority" => cur.priority = val.parse().ok(),
            _ => {}
        }
    }
    flush(&mut cur, &mut has, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_algs_and_skips_internal() {
        let text = "\
name         : cbc(aes)
driver       : cbc(ecb(aes-fixed-time))
module       : kernel
priority     : 101
selftest     : passed
internal     : no
type         : lskcipher

name         : __ecb-aes-aesni
driver       : ecb-aes-aesni
module       : aesni_intel
selftest     : passed
internal     : yes
type         : skcipher
";
        let parsed = parse_crypto(text);
        assert_eq!(parsed.len(), 2);
        assert!(!parsed[0].internal);
        assert!(parsed[1].internal);
        let (r, skipped) = from_parsed(parsed);
        assert_eq!(r.total, 2);
        assert_eq!(r.internal, 1);
        assert_eq!(r.algs.len(), 1);
        assert_eq!(r.algs[0].name, "cbc(aes)");
        assert_eq!(r.algs[0].type_name, "lskcipher");
        assert_eq!(skipped, 0);
        assert_eq!(r.types.iter().find(|t| t.type_name == "lskcipher").unwrap().count, 1);
    }
}
