//! NVMe 识别信息（sysfs）与 SMART/Health log（ioctl，不调用 nvme-cli）。
//!
//! sysfs：`/sys/class/nvme/nvmeN/{model,serial,firmware_rev,...}`
//! SMART：对 `/dev/nvmeN` 发 Admin Get Log Page (LID=0x02)。

use std::fs::File;
use std::os::unix::io::AsRawFd;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

const NVME_ADMIN_GET_LOG_PAGE: u8 = 0x02;
const NVME_LOG_SMART: u8 = 0x02;

#[repr(C)]
#[derive(Clone, Copy)]
struct NvmeAdminCmd {
    opcode: u8,
    flags: u8,
    rsvd1: u16,
    nsid: u32,
    cdw2: u32,
    cdw3: u32,
    metadata: u64,
    addr: u64,
    metadata_len: u32,
    data_len: u32,
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
    timeout_ms: u32,
    result: u32,
}

const _: () = assert!(std::mem::size_of::<NvmeAdminCmd>() == 72);

// Linux _IOWR('N', 0x41, struct nvme_passthru_cmd) == 0xC0484E41 on 64-bit
const NVME_IOCTL_ADMIN_CMD: libc::c_ulong = 0xC048_4E41;

#[derive(Clone, Debug, Serialize)]
pub struct NvmeReport {
    pub controllers: Vec<NvmeController>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NvmeController {
    pub name: String,
    pub sysfs: String,
    pub model: Sample<String>,
    pub serial: Sample<String>,
    pub firmware: Sample<String>,
    pub transport: Sample<String>,
    pub address: Sample<String>,
    pub subsysnqn: Sample<String>,
    pub cntlid: Sample<String>,
    pub namespaces: Vec<NvmeNamespace>,
    pub smart: Sample<NvmeSmart>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NvmeNamespace {
    pub name: String,
    pub size_bytes: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NvmeSmart {
    pub critical_warning: u8,
    pub temperature_c: Option<f64>,
    pub available_spare_pct: u8,
    pub available_spare_threshold_pct: u8,
    pub percentage_used: u8,
    pub data_units_read: u64,
    pub data_units_written: u64,
    pub host_read_commands: u64,
    pub host_write_commands: u64,
    pub power_cycles: u64,
    pub power_on_hours: u64,
    pub unsafe_shutdowns: u64,
    pub media_errors: u64,
}

pub fn collect(ctx: &ProbeCtx) -> NvmeReport {
    let root = ctx.sys_path("class/nvme");
    let mut notes = Vec::new();
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            return NvmeReport {
                controllers: Vec::new(),
                notes,
            };
        }
    };

    let mut controllers = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("nvme")) {
        if name.contains('n') && name.trim_start_matches("nvme").contains('n') {
            // nvme0n1 是命名空间，控制器枚举只要 nvme0。
            continue;
        }
        // 上面过滤不够：nvme0 不含第二段 n。nvme0n1 形如 nvme + digits + n + digits。
        if is_namespace_name(&name) {
            continue;
        }
        controllers.push(read_controller(ctx, &root.join(&name), &name));
    }
    if controllers.is_empty() {
        notes.push("未发现 NVMe 控制器（本机可能只有 virtio/SATA 盘）。".into());
    }
    NvmeReport {
        controllers,
        notes,
    }
}

fn is_namespace_name(name: &str) -> bool {
    // nvme0 -> false, nvme0n1 -> true
    let rest = match name.strip_prefix("nvme") {
        Some(r) => r,
        None => return false,
    };
    rest.contains('n') && rest.chars().any(|c| c.is_ascii_digit())
}

fn read_controller(ctx: &ProbeCtx, dir: &std::path::Path, name: &str) -> NvmeController {
    let mut namespaces = Vec::new();
    if let Some(entries) = access::list_dir_names(dir).value {
        for ns in entries.into_iter().filter(|n| is_namespace_name(n)) {
            let size_path = ctx.sys_path(format!("class/block/{ns}/size"));
            let size = match access::read_trimmed(&size_path) {
                Sample {
                    access: AccessKind::Ok,
                    value: Some(s),
                    source,
                    ..
                } => match s.parse::<u64>() {
                    Ok(sectors) => Sample::ok(sectors.saturating_mul(512), source),
                    Err(_) => Sample::error(source, "无法解析 size"),
                },
                s => Sample {
                    value: None,
                    access: s.access,
                    source: s.source,
                    hint: s.hint,
                },
            };
            namespaces.push(NvmeNamespace { name: ns, size_bytes: size });
        }
    }

    let dev_node = ctx.dev_path(name);
    let smart = read_smart(&dev_node);

    NvmeController {
        name: name.to_string(),
        sysfs: dir.display().to_string(),
        model: access::read_trimmed(dir.join("model")),
        serial: access::read_trimmed(dir.join("serial")),
        firmware: access::read_trimmed(dir.join("firmware_rev")),
        transport: access::read_trimmed(dir.join("transport")),
        address: access::read_trimmed(dir.join("address")),
        subsysnqn: access::read_trimmed(dir.join("subsysnqn")),
        cntlid: access::read_trimmed(dir.join("cntlid")),
        namespaces,
        smart,
    }
}

fn read_smart(dev: &std::path::Path) -> Sample<NvmeSmart> {
    let source = dev.display().to_string();
    if !dev.exists() {
        return Sample::missing(source);
    }
    let file = match File::options().read(true).write(true).open(dev) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // 再试只读，部分内核允许 Get Log Page。
            match File::open(dev) {
                Ok(f) => f,
                Err(_) => return Sample::denied(source),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Sample::missing(source),
        Err(e) => return Sample::error(source, e.to_string()),
    };

    let mut buf = vec![0u8; 512];
    let mut cmd = NvmeAdminCmd {
        opcode: NVME_ADMIN_GET_LOG_PAGE,
        flags: 0,
        rsvd1: 0,
        nsid: 0xFFFF_FFFF,
        cdw2: 0,
        cdw3: 0,
        metadata: 0,
        addr: buf.as_mut_ptr() as u64,
        metadata_len: 0,
        data_len: 512,
        // NUMDL = 127 dwords (512 bytes), LID = 0x02
        cdw10: 127 | (u32::from(NVME_LOG_SMART) << 16),
        cdw11: 0,
        cdw12: 0,
        cdw13: 0,
        cdw14: 0,
        cdw15: 0,
        timeout_ms: 0,
        result: 0,
    };
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), NVME_IOCTL_ADMIN_CMD, &mut cmd) };
    if rc < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return Sample::denied(source);
        }
        return Sample::error(
            source,
            format!("NVME_IOCTL_ADMIN_CMD 失败: {err}（需要 disk 组或 root，且设备支持 Get Log Page）"),
        );
    }
    Sample::ok(parse_smart(&buf), source)
}

pub fn parse_smart(buf: &[u8]) -> NvmeSmart {
    let kelvin = if buf.len() >= 3 {
        u16::from_le_bytes([buf[1], buf[2]])
    } else {
        0
    };
    let temperature_c = if kelvin == 0 {
        None
    } else {
        Some(kelvin as f64 - 273.15)
    };
    NvmeSmart {
        critical_warning: buf.first().copied().unwrap_or(0),
        temperature_c,
        available_spare_pct: buf.get(3).copied().unwrap_or(0),
        available_spare_threshold_pct: buf.get(4).copied().unwrap_or(0),
        percentage_used: buf.get(5).copied().unwrap_or(0),
        data_units_read: u128_lo64(buf, 32),
        data_units_written: u128_lo64(buf, 48),
        host_read_commands: u128_lo64(buf, 64),
        host_write_commands: u128_lo64(buf, 80),
        power_cycles: u128_lo64(buf, 112),
        power_on_hours: u128_lo64(buf, 128),
        unsafe_shutdowns: u128_lo64(buf, 144),
        media_errors: u128_lo64(buf, 160),
    }
}

fn u128_lo64(buf: &[u8], off: usize) -> u64 {
    if buf.len() < off + 8 {
        return 0;
    }
    u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_temperature_kelvin() {
        let mut buf = vec![0u8; 512];
        buf[0] = 0;
        // 310 K ≈ 36.85 C
        buf[1] = 310u16.to_le_bytes()[0];
        buf[2] = 310u16.to_le_bytes()[1];
        buf[3] = 100;
        let s = parse_smart(&buf);
        assert!((s.temperature_c.unwrap() - 36.85).abs() < 0.01);
        assert_eq!(s.available_spare_pct, 100);
    }

    #[test]
    fn namespace_name_filter() {
        assert!(!is_namespace_name("nvme0"));
        assert!(is_namespace_name("nvme0n1"));
        assert!(is_namespace_name("nvme12n2"));
    }
}
