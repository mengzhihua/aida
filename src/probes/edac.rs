//! 内存控制器 ECC：`/sys/devices/system/edac/mc`。不调用 `edac-util`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct EdacReport {
    pub controllers: Vec<EdacMc>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EdacMc {
    pub name: String,
    pub mc_name: Sample<String>,
    pub size_mb: Sample<u64>,
    pub ce_count: Sample<u64>,
    pub ue_count: Sample<u64>,
}

pub fn collect(ctx: &ProbeCtx) -> EdacReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("devices/system/edac/mc");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(
                "未导出 EDAC。消费级平台、虚拟机、或内核未开 CONFIG_EDAC 时常见。".into(),
            );
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            }
            return EdacReport {
                controllers: Vec::new(),
                notes,
            };
        }
    };
    let mut controllers = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("mc")) {
        let dir = root.join(&name);
        controllers.push(EdacMc {
            mc_name: access::read_trimmed(dir.join("mc_name")),
            size_mb: read_u64(dir.join("size_mb")),
            ce_count: read_u64(dir.join("ce_count")),
            ue_count: read_u64(dir.join("ue_count")),
            name,
        });
    }
    if controllers.is_empty() {
        notes.push("edac/mc 下没有 mcN（无 ECC 内存控制器）。".into());
    }
    EdacReport {
        controllers,
        notes,
    }
}

fn read_u64(path: std::path::PathBuf) -> Sample<u64> {
    let s = access::read_trimmed(&path);
    match (s.access, s.value.as_deref()) {
        (AccessKind::Ok, Some(t)) => match t.parse::<u64>() {
            Ok(v) => Sample::ok(v, s.source),
            Err(_) => Sample::error(s.source, "无法解析"),
        },
        _ => Sample {
            value: None,
            access: s.access,
            source: s.source,
            hint: s.hint,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn mc0_counts() {
        let root = std::env::temp_dir().join(format!("aida-edac-{}", std::process::id()));
        let mc = root.join("sys/devices/system/edac/mc/mc0");
        fs::create_dir_all(&mc).unwrap();
        fs::write(mc.join("mc_name"), "skx_edac\n").unwrap();
        fs::write(mc.join("size_mb"), "32768\n").unwrap();
        fs::write(mc.join("ce_count"), "2\n").unwrap();
        fs::write(mc.join("ue_count"), "0\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.controllers.len(), 1);
        assert_eq!(r.controllers[0].ce_count.value, Some(2));
        assert_eq!(r.controllers[0].ue_count.value, Some(0));
        let _ = fs::remove_dir_all(&root);
    }
}
