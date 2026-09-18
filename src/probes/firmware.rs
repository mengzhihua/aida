//! 固件接口：EFI sysfs / Secure Boot。不调用 `efibootmgr`/`mokutil`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct FirmwareReport {
    pub interface: Sample<String>,
    pub secure_boot: Sample<String>,
    pub fw_platform_size: Sample<String>,
    pub acpi_tables: Vec<String>,
    pub tpms: Vec<TpmDevice>,
    pub rng_current: Sample<String>,
    pub rng_available: Sample<String>,
    pub acpi_pm_profile: Sample<String>,
    pub pstore_files: usize,
    pub firmware_timeout: Sample<u64>,
    pub memmap_entries: usize,
    /// `/sys/firmware/devicetree/base/model`；x86 上通常不存在。
    pub dt_model: Sample<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TpmDevice {
    pub name: String,
    pub version_major: Sample<String>,
    pub pcr_banks: Vec<String>,
}

pub fn collect(ctx: &ProbeCtx) -> FirmwareReport {
    let mut notes = Vec::new();
    let efi_dir = ctx.sys_path("firmware/efi");
    let interface = match access::list_dir_names(&efi_dir) {
        Sample {
            access: AccessKind::Ok,
            value: Some(_),
            source,
            ..
        } => Sample::ok("EFI".into(), source),
        s => {
            notes.push(
                "未挂载 EFI sysfs。可能是 legacy BIOS、虚拟机未开 UEFI，或容器未绑定 /sys/firmware。"
                    .into(),
            );
            Sample {
                value: None,
                access: s.access,
                source: s.source,
                hint: s.hint,
            }
        }
    };
    let fw_platform_size = access::read_trimmed(efi_dir.join("fw_platform_size"));
    let secure_boot = read_secure_boot(ctx);
    let acpi_tables = list_acpi(ctx);
    let tpms = collect_tpm(ctx);
    if tpms.is_empty() {
        notes.push("无 TPM sysfs（虚拟机未转发 TPM 时常见）。".into());
    }
    let rng_dir = ctx.sys_path("class/misc/hw_random");
    let pstore_files = match access::list_dir_names(ctx.sys_path("fs/pstore")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n.len(),
        s if s.access == AccessKind::PermissionDenied || s.access == AccessKind::Error => {
            notes.push(s.access_label());
            0
        }
        _ => 0,
    };
    let memmap_entries = match access::list_dir_names(ctx.sys_path("firmware/memmap")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n.iter().filter(|x| x.chars().all(|c| c.is_ascii_digit())).count(),
        _ => 0,
    };
    FirmwareReport {
        interface,
        secure_boot,
        fw_platform_size,
        acpi_tables,
        tpms,
        rng_current: access::read_trimmed(rng_dir.join("rng_current")),
        rng_available: access::read_trimmed(rng_dir.join("rng_available")),
        acpi_pm_profile: access::read_trimmed(ctx.sys_path("firmware/acpi/pm_profile")),
        pstore_files,
        firmware_timeout: access::read_u64(ctx.sys_path("class/firmware/timeout")),
        memmap_entries,
        dt_model: read_dt_model(ctx),
        notes,
    }
}

/// 先 `sysfs` 再 `/proc/device-tree`。x86 上两者都不存在是正常的。
fn read_dt_model(ctx: &ProbeCtx) -> Sample<String> {
    let sys = access::read_trimmed(ctx.sys_path("firmware/devicetree/base/model"));
    if sys.access != AccessKind::NotFound {
        return sys;
    }
    access::read_trimmed(ctx.proc_path("device-tree/model"))
}

fn read_secure_boot(ctx: &ProbeCtx) -> Sample<String> {
    let dir = ctx.sys_path("firmware/efi/efivars");
    let names = match access::list_dir_names(&dir) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            return Sample {
                value: None,
                access: s.access,
                source: s.source,
                hint: s.hint,
            };
        }
    };
    for name in names {
        if name.starts_with("SecureBoot-") {
            let path = dir.join(&name);
            let bytes = access::read_bytes(&path);
            return match (bytes.access, bytes.value.as_deref()) {
                (AccessKind::Ok, Some(buf)) if buf.len() >= 5 => {
                    let enabled = buf[4] != 0;
                    Sample::ok(
                        if enabled {
                            "enabled".into()
                        } else {
                            "disabled".into()
                        },
                        path.display().to_string(),
                    )
                }
                (AccessKind::Ok, Some(_)) => Sample::error(path.display().to_string(), "efivar 过短"),
                _ => Sample {
                    value: None,
                    access: bytes.access,
                    source: bytes.source,
                    hint: bytes.hint,
                },
            };
        }
    }
    Sample::missing(dir.display().to_string())
}

fn list_acpi(ctx: &ProbeCtx) -> Vec<String> {
    let root = ctx.sys_path("firmware/acpi/tables");
    match access::list_dir_names(&root).value {
        Some(names) => names
            .into_iter()
            .filter(|n| n != "data" && n != "dynamic")
            .collect(),
        None => Vec::new(),
    }
}

fn collect_tpm(ctx: &ProbeCtx) -> Vec<TpmDevice> {
    let root = ctx.sys_path("class/tpm");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("tpm")) {
        let dir = root.join(&name);
        let pcr_banks = match access::list_dir_names(&dir).value {
            Some(n) => n.into_iter().filter(|x| x.starts_with("pcr-")).collect(),
            None => Vec::new(),
        };
        out.push(TpmDevice {
            version_major: access::read_trimmed(dir.join("tpm_version_major")),
            pcr_banks,
            name,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn secure_boot_efivar() {
        let root = std::env::temp_dir().join(format!("aida-efi-{}", std::process::id()));
        let vars = root.join("sys/firmware/efi/efivars");
        fs::create_dir_all(&vars).unwrap();
        fs::write(root.join("sys/firmware/efi/fw_platform_size"), "64\n").unwrap();
        let mut buf = vec![0u8, 0, 0, 0, 1];
        buf[0] = 7;
        fs::write(
            vars.join("SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c"),
            buf,
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
        assert_eq!(r.interface.value.as_deref(), Some("EFI"));
        assert_eq!(r.secure_boot.value.as_deref(), Some("enabled"));
        fs::create_dir_all(root.join("sys/firmware/acpi/tables")).unwrap();
        fs::write(root.join("sys/firmware/acpi/tables/FACP"), b"").unwrap();
        fs::write(root.join("sys/firmware/acpi/tables/DSDT"), b"").unwrap();
        fs::create_dir_all(root.join("sys/class/tpm/tpm0/pcr-sha256")).unwrap();
        fs::write(root.join("sys/class/tpm/tpm0/tpm_version_major"), "2\n").unwrap();
        fs::create_dir_all(root.join("sys/class/misc/hw_random")).unwrap();
        fs::write(root.join("sys/class/misc/hw_random/rng_current"), "virtio_rng.0\n").unwrap();
        let r2 = collect(&ctx);
        assert!(r2.acpi_tables.contains(&"FACP".into()));
        assert_eq!(r2.tpms[0].version_major.value.as_deref(), Some("2"));
        assert_eq!(r2.rng_current.value.as_deref(), Some("virtio_rng.0"));
        fs::create_dir_all(root.join("sys/class/firmware")).unwrap();
        fs::write(root.join("sys/class/firmware/timeout"), "60\n").unwrap();
        fs::create_dir_all(root.join("sys/firmware/memmap/0")).unwrap();
        fs::create_dir_all(root.join("sys/firmware/memmap/1")).unwrap();
        let r3 = collect(&ctx);
        assert_eq!(r3.firmware_timeout.value, Some(60));
        assert_eq!(r3.memmap_entries, 2);
        fs::create_dir_all(root.join("sys/firmware/devicetree/base")).unwrap();
        fs::write(root.join("sys/firmware/devicetree/base/model"), "Test Board\n").unwrap();
        let r4 = collect(&ctx);
        assert_eq!(r4.dt_model.value.as_deref(), Some("Test Board"));
        let _ = fs::remove_dir_all(&root);
    }
}
