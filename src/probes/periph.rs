//! 剩余 sysfs class：DMA / PWM / IIO / nvmem / regulator / devlink / pci_bus。
//! 不调用 `i2cdetect`/`pwmconfig`/`devlink`/`lshw`，不写 `export`，不转储 nvmem 二进制。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct PeriphReport {
    /// `/proc/dma`：ISA DMA 通道（x86 上 4: cascade 很常见）。
    pub dma_isa: Vec<IsaDma>,
    /// `/sys/class/dma`：dmaengine 通道。空目录不是读失败。
    pub dmaengine: Vec<DmaEngineChan>,
    pub pwm_chips: Vec<PwmChip>,
    pub iio: Vec<IioDev>,
    pub nvmem: Vec<NvmemDev>,
    pub regulators: Vec<Regulator>,
    pub devlinks: Vec<Devlink>,
    pub pci_buses: Vec<PciBus>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IsaDma {
    pub channel: u32,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DmaEngineChan {
    pub name: String,
    pub in_use: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PwmChip {
    pub name: String,
    pub npwm: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IioDev {
    pub name: String,
    pub iio_name: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NvmemDev {
    pub name: String,
    pub typ: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Regulator {
    pub name: String,
    pub regulator_name: Sample<String>,
    pub state: Sample<String>,
    pub microvolts: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Devlink {
    pub name: String,
    pub status: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PciBus {
    pub name: String,
    pub cpulist: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> PeriphReport {
    let mut notes = Vec::new();
    let dma_sample = access::read_trimmed(ctx.proc_path("dma"));
    let dma_isa = parse_proc_dma(&dma_sample);
    if dma_sample.access == AccessKind::PermissionDenied || dma_sample.access == AccessKind::Error {
        notes.push(dma_sample.access_label());
    } else if dma_sample.access == AccessKind::NotFound {
        notes.push("无 /proc/dma（非 ISA DMA 平台常见）。".into());
    }
    let dmaengine = read_dmaengine(ctx, &mut notes);
    let pwm_chips = read_pwm(ctx, &mut notes);
    let iio = read_iio(ctx, &mut notes);
    let nvmem = read_nvmem(ctx, &mut notes);
    let regulators = read_regulators(ctx, &mut notes);
    let devlinks = read_devlinks(ctx, &mut notes);
    let pci_buses = read_pci_buses(ctx, &mut notes);
    PeriphReport {
        dma_isa,
        dmaengine,
        pwm_chips,
        iio,
        nvmem,
        regulators,
        devlinks,
        pci_buses,
        notes,
    }
}

/// `/proc/dma`：` 4: cascade`，没有表头。
pub fn parse_proc_dma(sample: &Sample<String>) -> Vec<IsaDma> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let Some((n, name)) = t.split_once(':') else {
            continue;
        };
        let Ok(channel) = n.trim().parse::<u32>() else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        out.push(IsaDma {
            channel,
            name: name.to_string(),
        });
        if out.len() >= 16 {
            break;
        }
    }
    out
}

enum DirList {
    Names(Vec<String>),
    Missing,
    Failed(String),
}

fn dir_list(path: impl AsRef<std::path::Path>) -> DirList {
    match access::list_dir_names(path) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => DirList::Names(n),
        s if s.access == AccessKind::NotFound => DirList::Missing,
        s => DirList::Failed(s.access_label()),
    }
}

fn read_dmaengine(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<DmaEngineChan> {
    let root = ctx.sys_path("class/dma");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names {
        if name == "uevent" || name == "power" {
            continue;
        }
        let dir = root.join(&name);
        out.push(DmaEngineChan {
            in_use: access::read_trimmed(dir.join("in_use")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_pwm(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<PwmChip> {
    let root = ctx.sys_path("class/pwm");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("pwmchip")) {
        out.push(PwmChip {
            npwm: access::read_u64(root.join(&name).join("npwm")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_iio(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<IioDev> {
    let root = ctx.sys_path("bus/iio/devices");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("iio:device")) {
        out.push(IioDev {
            iio_name: access::read_trimmed(root.join(&name).join("name")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_nvmem(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<NvmemDev> {
    let root = ctx.sys_path("bus/nvmem/devices");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names {
        if name == "uevent" || name == "power" {
            continue;
        }
        // 只读 type，不要读 `nvmem` 二进制。
        out.push(NvmemDev {
            typ: access::read_trimmed(root.join(&name).join("type")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_regulators(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<Regulator> {
    let root = ctx.sys_path("class/regulator");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("regulator")) {
        let dir = root.join(&name);
        out.push(Regulator {
            regulator_name: access::read_trimmed(dir.join("name")),
            state: access::read_trimmed(dir.join("state")),
            microvolts: access::read_u64(dir.join("microvolts")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_devlinks(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<Devlink> {
    let root = ctx.sys_path("class/devlink");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names {
        if name == "uevent" || name == "power" {
            continue;
        }
        out.push(Devlink {
            status: access::read_trimmed(root.join(&name).join("status")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_pci_buses(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<PciBus> {
    let root = ctx.sys_path("class/pci_bus");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names {
        if !name.contains(':') {
            continue;
        }
        out.push(PciBus {
            cpulist: access::read_trimmed(root.join(&name).join("cpulistaffinity")),
            name,
        });
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_cascade_dma() {
        let v = parse_proc_dma(&Sample::ok(" 4: cascade\n 2: floppy\n".into(), "dma"));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].channel, 4);
        assert_eq!(v[0].name, "cascade");
        assert_eq!(v[1].channel, 2);
        assert_eq!(v[1].name, "floppy");
    }

    #[test]
    fn periph_sysfs_fixture() {
        let root = std::env::temp_dir().join(format!("aida-periph-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("proc")).unwrap();
        fs::write(root.join("proc/dma"), " 4: cascade\n").unwrap();
        let dma = root.join("sys/class/dma/dma0chan0");
        fs::create_dir_all(&dma).unwrap();
        fs::write(dma.join("in_use"), "0\n").unwrap();
        let pwm = root.join("sys/class/pwm/pwmchip0");
        fs::create_dir_all(&pwm).unwrap();
        fs::write(pwm.join("npwm"), "4\n").unwrap();
        let iio = root.join("sys/bus/iio/devices/iio:device0");
        fs::create_dir_all(&iio).unwrap();
        fs::write(iio.join("name"), "adc\n").unwrap();
        let nv = root.join("sys/bus/nvmem/devices/imk-0");
        fs::create_dir_all(&nv).unwrap();
        fs::write(nv.join("type"), "eeprom\n").unwrap();
        fs::write(nv.join("nvmem"), b"\xff\xffsecret").unwrap();
        let reg = root.join("sys/class/regulator/regulator.0");
        fs::create_dir_all(&reg).unwrap();
        fs::write(reg.join("name"), "vdd\n").unwrap();
        fs::write(reg.join("state"), "enabled\n").unwrap();
        fs::write(reg.join("microvolts"), "3300000\n").unwrap();
        let dl = root.join("sys/class/devlink/pci:0000:01:00.0--pci:0000:00:1c.0");
        fs::create_dir_all(&dl).unwrap();
        fs::write(dl.join("status"), "available\n").unwrap();
        let bus = root.join("sys/class/pci_bus/0000:00");
        fs::create_dir_all(&bus).unwrap();
        fs::write(bus.join("cpulistaffinity"), "0-3\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.dma_isa[0].name, "cascade");
        assert_eq!(r.dmaengine[0].name, "dma0chan0");
        assert_eq!(r.dmaengine[0].in_use.value.as_deref(), Some("0"));
        assert_eq!(r.pwm_chips[0].npwm.value, Some(4));
        assert_eq!(r.iio[0].iio_name.value.as_deref(), Some("adc"));
        assert_eq!(r.nvmem[0].typ.value.as_deref(), Some("eeprom"));
        assert_eq!(r.regulators[0].microvolts.value, Some(3_300_000));
        assert_eq!(r.regulators[0].state.value.as_deref(), Some("enabled"));
        assert_eq!(r.devlinks[0].status.value.as_deref(), Some("available"));
        assert_eq!(r.pci_buses[0].cpulist.value.as_deref(), Some("0-3"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_dma_class_is_not_silent_empty() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aida-periph-deny-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("proc")).unwrap();
        fs::write(root.join("proc/dma"), " 4: cascade\n").unwrap();
        let dma = root.join("sys/class/dma");
        fs::create_dir_all(&dma).unwrap();
        fs::set_permissions(&dma, fs::Permissions::from_mode(0o000)).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let _ = fs::set_permissions(&dma, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&root);
        assert!(r.dmaengine.is_empty());
        assert!(
            r.notes
                .iter()
                .any(|n| n.contains("权限") || n.contains("失败")),
            "denied class/dma must not look empty: {:?}",
            r.notes
        );
    }
}
