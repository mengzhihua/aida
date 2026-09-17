//! IOMMU 分组：`/sys/kernel/iommu_groups`。不调用 `find`/`lspci`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct IommuReport {
    pub groups: Vec<IommuGroup>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IommuGroup {
    pub id: String,
    pub devices: Vec<String>,
}

pub fn collect(ctx: &ProbeCtx) -> IommuReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("kernel/iommu_groups");
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
                notes.push("无 IOMMU 分组。虚拟机未开 vIOMMU、或内核未启用 IOMMU 时常见。".into());
            }
            return IommuReport {
                groups: Vec::new(),
                notes,
            };
        }
    };
    let mut groups = Vec::new();
    for id in names {
        if !id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let dev_dir = root.join(&id).join("devices");
        let devices = match access::list_dir_names(&dev_dir) {
            Sample {
                access: AccessKind::Ok,
                value: Some(n),
                ..
            } => n,
            s if s.access == AccessKind::NotFound => Vec::new(),
            s => {
                notes.push(s.access_label());
                Vec::new()
            }
        };
        groups.push(IommuGroup { id, devices });
    }
    groups.sort_by(|a, b| a.id.cmp(&b.id));
    if groups.is_empty() {
        notes.push("iommu_groups 为空（未启用 IOMMU）。".into());
    }
    IommuReport { groups, notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn two_groups_fixture() {
        let root = std::env::temp_dir().join(format!("aida-iommu-{}", std::process::id()));
        let g0 = root.join("sys/kernel/iommu_groups/0/devices");
        let g1 = root.join("sys/kernel/iommu_groups/1/devices");
        fs::create_dir_all(&g0).unwrap();
        fs::create_dir_all(&g1).unwrap();
        fs::create_dir_all(g0.join("0000:00:02.0")).unwrap();
        fs::create_dir_all(g1.join("0000:01:00.0")).unwrap();
        fs::create_dir_all(g1.join("0000:01:00.1")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.groups.len(), 2);
        assert_eq!(r.groups[0].devices, vec!["0000:00:02.0"]);
        assert_eq!(r.groups[1].devices.len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_group_devices_leave_a_note() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aida-iommu-deny-{}", std::process::id()));
        let g0 = root.join("sys/kernel/iommu_groups/0/devices");
        fs::create_dir_all(&g0).unwrap();
        fs::set_permissions(&g0, fs::Permissions::from_mode(0o000)).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let _ = fs::set_permissions(&g0, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&root);
        assert_eq!(r.groups.len(), 1);
        assert!(
            r.notes.iter().any(|n| n.contains("权限") || n.contains("失败")),
            "device list PermissionDenied must be noted: {:?}",
            r.notes
        );
    }
}
