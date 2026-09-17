//! virtio 设备：`/sys/bus/virtio/devices`。不调用 `lspci`/`virsh`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct VirtioReport {
    pub devices: Vec<VirtioDevice>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VirtioDevice {
    pub name: String,
    pub kind: String,
    pub driver: Sample<String>,
    pub vendor: Sample<String>,
    pub status: Sample<String>,
    pub features: Sample<String>,
    pub modalias: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> VirtioReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("bus/virtio/devices");
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
                notes.push("无 virtio 总线。裸机或未启用 virtio 时常见。".into());
            }
            return VirtioReport {
                devices: Vec::new(),
                notes,
            };
        }
    };
    let mut devices = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("virtio")) {
        let dir = root.join(&name);
        let modalias = access::read_trimmed(dir.join("modalias"));
        let kind = virtio_kind(modalias.value.as_deref());
        devices.push(VirtioDevice {
            kind,
            driver: read_driver_link(&dir),
            vendor: access::read_trimmed(dir.join("vendor")),
            status: access::read_trimmed(dir.join("status")),
            features: access::read_trimmed(dir.join("features")),
            modalias,
            name,
        });
    }
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    VirtioReport { devices, notes }
}

fn read_driver_link(dir: &std::path::Path) -> Sample<String> {
    let link = dir.join("driver");
    match std::fs::read_link(&link) {
        Ok(p) => Sample::ok(
            p.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            link.display().to_string(),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Sample::missing(link.display().to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Sample::denied(link.display().to_string())
        }
        Err(e) => Sample::error(link.display().to_string(), e.to_string()),
    }
}

/// `virtio:d00000005v00001AF4` → 设备 ID 5 = balloon。
pub fn virtio_kind(modalias: Option<&str>) -> String {
    let Some(m) = modalias else {
        return "unknown".into();
    };
    let id = m.strip_prefix("virtio:d").and_then(|rest| {
        rest.split('v').next().and_then(|hex| u32::from_str_radix(hex, 16).ok())
    });
    // include/uapi/linux/virtio_ids.h
    match id {
        Some(1) => "net",
        Some(2) => "block",
        Some(3) => "console",
        Some(4) => "rng",
        Some(5) => "balloon",
        Some(8) => "scsi",
        Some(9) => "9p",
        Some(16) => "gpu",
        Some(17) => "clock",
        Some(18) => "input",
        Some(19) => "vsock",
        Some(20) => "crypto",
        Some(23) => "iommu",
        Some(24) => "mem",
        Some(25) => "sound",
        Some(26) => "fs",
        Some(n) => return format!("id {n}"),
        None => "unknown",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn kind_from_modalias() {
        assert_eq!(
            virtio_kind(Some("virtio:d00000005v00001AF4")),
            "balloon"
        );
        assert_eq!(virtio_kind(Some("virtio:d00000002v00001AF4")), "block");
        assert_eq!(virtio_kind(Some("virtio:d00000001v00001AF4")), "net");
        assert_eq!(virtio_kind(Some("virtio:d00000010v00001AF4")), "gpu");
        assert_eq!(virtio_kind(Some("virtio:d00000012v00001AF4")), "input");
        assert_eq!(virtio_kind(Some("virtio:d00000014v00001AF4")), "crypto");
        assert_eq!(virtio_kind(Some("virtio:d00000017v00001AF4")), "iommu");
        assert_eq!(virtio_kind(Some("virtio:d0000001av00001AF4")), "fs");
    }

    #[test]
    fn fixture_devices() {
        let root = std::env::temp_dir().join(format!("aida-virtio-{}", std::process::id()));
        let dev = root.join("sys/bus/virtio/devices/virtio0");
        fs::create_dir_all(&dev).unwrap();
        fs::write(dev.join("modalias"), "virtio:d00000005v00001AF4\n").unwrap();
        fs::write(dev.join("vendor"), "0x1af4\n").unwrap();
        fs::write(dev.join("status"), "0x0000000f\n").unwrap();
        let drivers = root.join("sys/bus/virtio/drivers/virtio_balloon");
        fs::create_dir_all(&drivers).unwrap();
        let _ = symlink(&drivers, dev.join("driver"));
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].kind, "balloon");
        assert_eq!(
            r.devices[0].driver.value.as_deref(),
            Some("virtio_balloon")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
