//! KVM / 宿主机虚拟化：`/dev/kvm` + `/sys/module/kvm*`。不调用 `virsh`/`kvm-ok`。

use std::fs;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct KvmReport {
    pub device: Sample<String>,
    pub module: Sample<String>,
    pub vendor: Sample<String>,
    pub nested: Sample<String>,
    pub ept: Sample<String>,
    pub npt: Sample<String>,
    pub vpid: Sample<String>,
    pub nx_huge_pages: Sample<String>,
    pub tdp_mmu: Sample<String>,
    pub notes: Vec<String>,
}

pub fn collect(ctx: &ProbeCtx) -> KvmReport {
    let mut notes = Vec::new();
    let device = node_status(ctx.dev_path("kvm"));
    if device.access == AccessKind::NotFound {
        notes.push("无 /dev/kvm。未加载 kvm 模块、或当前内核不是 KVM 宿主机时常见。".into());
    } else if device.access == AccessKind::PermissionDenied {
        notes.push(device.access_label());
    }

    let module = access::read_trimmed(ctx.sys_path("module/kvm/refcnt"));
    let intel = ctx.sys_path("module/kvm_intel/parameters");
    let amd = ctx.sys_path("module/kvm_amd/parameters");
    let has_intel = intel.join("nested").exists();
    let has_amd = amd.join("nested").exists();
    let vendor = if has_intel {
        Sample::ok("kvm_intel".into(), intel.display().to_string())
    } else if has_amd {
        Sample::ok("kvm_amd".into(), amd.display().to_string())
    } else if module.access == AccessKind::Ok {
        Sample::ok("kvm".into(), ctx.sys_path("module/kvm").display().to_string())
    } else {
        Sample::missing("module/kvm_intel|kvm_amd")
    };

    let nested = if has_intel {
        access::read_trimmed(intel.join("nested"))
    } else {
        access::read_trimmed(amd.join("nested"))
    };

    KvmReport {
        device,
        module: match module.access {
            AccessKind::Ok => Sample::ok("loaded".into(), module.source),
            _ => module,
        },
        vendor,
        nested,
        ept: access::read_trimmed(intel.join("ept")),
        npt: access::read_trimmed(amd.join("npt")),
        vpid: access::read_trimmed(intel.join("vpid")),
        nx_huge_pages: access::read_trimmed(ctx.sys_path("module/kvm/parameters/nx_huge_pages")),
        tdp_mmu: access::read_trimmed(ctx.sys_path("module/kvm/parameters/tdp_mmu")),
        notes,
    }
}

fn node_status(path: impl AsRef<std::path::Path>) -> Sample<String> {
    let path = path.as_ref();
    let source = path.display().to_string();
    match fs::metadata(path) {
        Ok(_) => Sample::ok("exists".into(), source),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Sample::missing(source),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Sample::denied(source),
        Err(e) => Sample::error(source, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intel_nested_fixture() {
        let root = std::env::temp_dir().join(format!("aida-kvm-{}", std::process::id()));
        let intel = root.join("sys/module/kvm_intel/parameters");
        fs::create_dir_all(&intel).unwrap();
        fs::write(intel.join("nested"), "Y\n").unwrap();
        fs::write(intel.join("ept"), "Y\n").unwrap();
        fs::write(intel.join("vpid"), "Y\n").unwrap();
        let kvm = root.join("sys/module/kvm/parameters");
        fs::create_dir_all(&kvm).unwrap();
        fs::write(kvm.join("nx_huge_pages"), "Y\n").unwrap();
        fs::write(kvm.join("tdp_mmu"), "Y\n").unwrap();
        fs::write(root.join("sys/module/kvm/refcnt"), "1\n").unwrap();
        fs::create_dir_all(root.join("dev")).unwrap();
        fs::write(root.join("dev/kvm"), "").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.device.value.as_deref(), Some("exists"));
        assert_eq!(r.vendor.value.as_deref(), Some("kvm_intel"));
        assert_eq!(r.nested.value.as_deref(), Some("Y"));
        assert_eq!(r.ept.value.as_deref(), Some("Y"));
        let _ = fs::remove_dir_all(&root);
    }
}
