//! SCSI host：`/sys/class/scsi_host`。不调用 `lsscsi`/`sg_inq`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct ScsiReport {
    pub hosts: Vec<ScsiHost>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScsiHost {
    pub name: String,
    pub proc_name: Sample<String>,
    pub unique_id: Sample<String>,
    pub can_queue: Sample<u64>,
    pub cmd_per_lun: Sample<u64>,
    pub sg_tablesize: Sample<u64>,
    pub state: Sample<String>,
    pub host_busy: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> ScsiReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("class/scsi_host");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            } else {
                notes.push("无 scsi_host（virtio-blk / NVMe 常见）。".into());
            }
            return ScsiReport {
                hosts: Vec::new(),
                notes,
            };
        }
    };
    let mut hosts = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("host")) {
        let dir = root.join(&name);
        hosts.push(ScsiHost {
            proc_name: access::read_trimmed(dir.join("proc_name")),
            unique_id: access::read_trimmed(dir.join("unique_id")),
            can_queue: access::read_u64(dir.join("can_queue")),
            cmd_per_lun: access::read_u64(dir.join("cmd_per_lun")),
            sg_tablesize: access::read_u64(dir.join("sg_tablesize")),
            state: access::read_trimmed(dir.join("state")),
            host_busy: access::read_trimmed(dir.join("host_busy")),
            name,
        });
    }
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    if hosts.is_empty() {
        notes.push("scsi_host 目录为空。".into());
    }
    ScsiReport { hosts, notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn host0_fixture() {
        let root = std::env::temp_dir().join(format!("aida-scsi-{}", std::process::id()));
        let dir = root.join("sys/class/scsi_host/host0");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("proc_name"), "ahci\n").unwrap();
        fs::write(dir.join("unique_id"), "1\n").unwrap();
        fs::write(dir.join("can_queue"), "32\n").unwrap();
        fs::write(dir.join("cmd_per_lun"), "1\n").unwrap();
        fs::write(dir.join("state"), "running\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.hosts.len(), 1);
        assert_eq!(r.hosts[0].proc_name.value.as_deref(), Some("ahci"));
        assert_eq!(r.hosts[0].can_queue.value, Some(32));
        let _ = fs::remove_dir_all(&root);
    }
}
