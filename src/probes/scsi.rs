//! SCSI host：`/sys/class/scsi_host`。不调用 `lsscsi`/`sg_inq`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct ScsiReport {
    pub hosts: Vec<ScsiHost>,
    pub devices: Vec<ScsiDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScsiDevice {
    pub name: String,
    pub vendor: Sample<String>,
    pub model: Sample<String>,
    pub rev: Sample<String>,
    pub type_code: Sample<String>,
    pub state: Sample<String>,
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
                devices: read_devices(ctx, &mut notes),
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
    let devices = read_devices(ctx, &mut notes);
    ScsiReport {
        hosts,
        devices,
        notes,
    }
}

fn read_devices(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<ScsiDevice> {
    let root = ctx.sys_path("class/scsi_device");
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
    for name in names {
        let dir = root.join(&name).join("device");
        out.push(ScsiDevice {
            vendor: access::read_trimmed(dir.join("vendor")),
            model: access::read_trimmed(dir.join("model")),
            rev: access::read_trimmed(dir.join("rev")),
            type_code: access::read_trimmed(dir.join("type")),
            state: access::read_trimmed(dir.join("state")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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
        let lun = root.join("sys/class/scsi_device/0:0:0:0/device");
        fs::create_dir_all(&lun).unwrap();
        fs::write(lun.join("vendor"), "ATA\n").unwrap();
        fs::write(lun.join("model"), "VBOX HARDDISK\n").unwrap();
        fs::write(lun.join("rev"), "1.0\n").unwrap();
        fs::write(lun.join("type"), "0\n").unwrap();
        fs::write(lun.join("state"), "running\n").unwrap();
        let r = collect(&ctx);
        assert_eq!(r.hosts.len(), 1);
        assert_eq!(r.hosts[0].proc_name.value.as_deref(), Some("ahci"));
        assert_eq!(r.hosts[0].can_queue.value, Some(32));
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].name, "0:0:0:0");
        assert_eq!(r.devices[0].model.value.as_deref(), Some("VBOX HARDDISK"));
        let _ = fs::remove_dir_all(&root);
    }
}
