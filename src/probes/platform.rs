//! 平台外设：watchdog / backlight / LED / I2C。不调用 `i2cdetect`/`lsmod`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct PlatformReport {
    pub watchdogs: Vec<Watchdog>,
    pub backlights: Vec<Backlight>,
    pub leds: Vec<Led>,
    pub i2c_adapters: Vec<I2cAdapter>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Watchdog {
    pub name: String,
    pub identity: Sample<String>,
    pub state: Sample<String>,
    pub timeout: Sample<String>,
    pub timeleft: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Backlight {
    pub name: String,
    pub kind: Sample<String>,
    pub actual: Sample<u64>,
    pub max: Sample<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Led {
    pub name: String,
    pub brightness: Sample<u64>,
    pub max_brightness: Sample<u64>,
    pub trigger: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct I2cAdapter {
    pub name: String,
    pub adapter_name: Sample<String>,
    pub clients: usize,
}

pub fn collect(ctx: &ProbeCtx) -> PlatformReport {
    let mut notes = Vec::new();
    let watchdogs = read_watchdogs(ctx);
    let backlights = read_backlights(ctx);
    let leds = read_leds(ctx);
    let i2c_adapters = read_i2c(ctx);
    if watchdogs.is_empty() {
        notes.push("无 watchdog 设备。".into());
    }
    if backlights.is_empty() {
        notes.push("无 backlight（服务器/无头虚拟机常见）。".into());
    }
    if i2c_adapters.is_empty() {
        notes.push("无 I2C 适配器（不调用 i2cdetect）。".into());
    }
    PlatformReport {
        watchdogs,
        backlights,
        leds,
        i2c_adapters,
        notes,
    }
}

fn read_watchdogs(ctx: &ProbeCtx) -> Vec<Watchdog> {
    let root = ctx.sys_path("class/watchdog");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("watchdog")) {
        let dir = root.join(&name);
        out.push(Watchdog {
            identity: access::read_trimmed(dir.join("identity")),
            state: access::read_trimmed(dir.join("state")),
            timeout: access::read_trimmed(dir.join("timeout")),
            timeleft: access::read_trimmed(dir.join("timeleft")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_backlights(ctx: &ProbeCtx) -> Vec<Backlight> {
    let root = ctx.sys_path("class/backlight");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        let dir = root.join(&name);
        out.push(Backlight {
            kind: access::read_trimmed(dir.join("type")),
            actual: access::read_u64(dir.join("actual_brightness")),
            max: access::read_u64(dir.join("max_brightness")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_leds(ctx: &ProbeCtx) -> Vec<Led> {
    let root = ctx.sys_path("class/leds");
    let names = match access::list_dir_names(&root).value {
        Some(n) => n,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        let dir = root.join(&name);
        out.push(Led {
            brightness: access::read_u64(dir.join("brightness")),
            max_brightness: access::read_u64(dir.join("max_brightness")),
            trigger: access::read_trimmed(dir.join("trigger")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_i2c(ctx: &ProbeCtx) -> Vec<I2cAdapter> {
    let root = ctx.sys_path("bus/i2c/devices");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        _ => return Vec::new(),
    };
    let mut adapters = Vec::new();
    let mut clients: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for name in &names {
        if let Some(bus) = name.split('-').next() {
            if name.contains('-') && !name.starts_with("i2c-") {
                *clients.entry(format!("i2c-{bus}")).or_insert(0) += 1;
            }
        }
    }
    for name in names.into_iter().filter(|n| n.starts_with("i2c-")) {
        let dir = root.join(&name);
        adapters.push(I2cAdapter {
            adapter_name: access::read_trimmed(dir.join("name")),
            clients: clients.get(&name).copied().unwrap_or(0),
            name,
        });
    }
    adapters.sort_by(|a, b| a.name.cmp(&b.name));
    adapters
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn watchdog_backlight_i2c_fixture() {
        let root = std::env::temp_dir().join(format!("aida-plat-{}", std::process::id()));
        let wd = root.join("sys/class/watchdog/watchdog0");
        fs::create_dir_all(&wd).unwrap();
        fs::write(wd.join("identity"), "Software Watchdog\n").unwrap();
        fs::write(wd.join("timeout"), "60\n").unwrap();
        let bl = root.join("sys/class/backlight/intel_backlight");
        fs::create_dir_all(&bl).unwrap();
        fs::write(bl.join("type"), "raw\n").unwrap();
        fs::write(bl.join("actual_brightness"), "200\n").unwrap();
        fs::write(bl.join("max_brightness"), "400\n").unwrap();
        let led = root.join("sys/class/leds/input0::capslock");
        fs::create_dir_all(&led).unwrap();
        fs::write(led.join("brightness"), "0\n").unwrap();
        fs::write(led.join("max_brightness"), "1\n").unwrap();
        let i2c = root.join("sys/bus/i2c/devices");
        fs::create_dir_all(i2c.join("i2c-0")).unwrap();
        fs::write(i2c.join("i2c-0/name"), "SMBus I801\n").unwrap();
        fs::create_dir_all(i2c.join("0-0050")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.watchdogs[0].identity.value.as_deref(), Some("Software Watchdog"));
        assert_eq!(r.backlights[0].actual.value, Some(200));
        assert_eq!(r.leds.len(), 1);
        assert_eq!(r.i2c_adapters[0].clients, 1);
        let _ = fs::remove_dir_all(&root);
    }
}
