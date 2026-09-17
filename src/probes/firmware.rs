//! 固件接口：EFI sysfs / Secure Boot。不调用 `efibootmgr`/`mokutil`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct FirmwareReport {
    pub interface: Sample<String>,
    pub secure_boot: Sample<String>,
    pub fw_platform_size: Sample<String>,
    pub notes: Vec<String>,
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
    FirmwareReport {
        interface,
        secure_boot,
        fw_platform_size,
        notes,
    }
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
        let _ = fs::remove_dir_all(&root);
    }
}
