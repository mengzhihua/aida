//! 额外总线外设：rfkill / Bluetooth / Thunderbolt / V4L / MMC / MEI。
//! 不调用 `rfkill`/`bluetoothctl`/`boltctl`/`v4l2-ctl`/`mmc`/`mei-amt-version`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct BusesReport {
    pub rfkill: Vec<RfkillDev>,
    pub bluetooth: Vec<BluetoothDev>,
    pub thunderbolt: Vec<ThunderboltDev>,
    pub video: Vec<VideoDev>,
    pub mmc: Vec<MmcHost>,
    pub mei: Vec<MeiDev>,
    pub serial: Vec<SerialPort>,
    pub tty_drivers: Vec<TtyDriver>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SerialPort {
    pub name: String,
    pub uartclk: Sample<u64>,
    pub irq: Sample<String>,
    pub typ: Sample<String>,
    pub port: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TtyDriver {
    pub name: String,
    pub device: String,
    pub major: String,
    pub minors: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RfkillDev {
    pub name: String,
    pub kind: Sample<String>,
    pub state: Sample<String>,
    pub hard: Sample<String>,
    pub soft: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BluetoothDev {
    pub name: String,
    pub address: Sample<String>,
    pub dev_name: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThunderboltDev {
    pub name: String,
    pub vendor: Sample<String>,
    pub device: Sample<String>,
    pub unique_id: Sample<String>,
    pub authorized: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VideoDev {
    pub name: String,
    pub dev_name: Sample<String>,
    pub index: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MmcHost {
    pub name: String,
    pub card: Option<String>,
    pub cid: Sample<String>,
    pub name_tag: Sample<String>,
    pub serial: Sample<String>,
    pub date: Sample<String>,
    pub r#type: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MeiDev {
    pub name: String,
    pub fw_status: Sample<String>,
    pub trx: Sample<String>,
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

pub fn collect(ctx: &ProbeCtx) -> BusesReport {
    let mut notes = Vec::new();
    let rfkill = read_rfkill(ctx, &mut notes);
    let bluetooth = read_bluetooth(ctx, &mut notes);
    let thunderbolt = read_thunderbolt(ctx, &mut notes);
    let video = read_video(ctx, &mut notes);
    let mmc = read_mmc(ctx, &mut notes);
    let mei = read_mei(ctx, &mut notes);
    let serial = read_serial(ctx, &mut notes);
    let tty_drivers = parse_tty_drivers(&access::read_trimmed(ctx.proc_path("tty/drivers")));
    BusesReport {
        rfkill,
        bluetooth,
        thunderbolt,
        video,
        mmc,
        mei,
        serial,
        tty_drivers,
        notes,
    }
}

fn read_rfkill(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<RfkillDev> {
    let root = ctx.sys_path("class/rfkill");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => {
            notes.push("无 rfkill 类（服务器/无无线时常见）。".into());
            return Vec::new();
        }
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("rfkill")) {
        let dir = root.join(&name);
        out.push(RfkillDev {
            kind: access::read_trimmed(dir.join("type")),
            state: access::read_trimmed(dir.join("state")),
            hard: access::read_trimmed(dir.join("hard")),
            soft: access::read_trimmed(dir.join("soft")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_bluetooth(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<BluetoothDev> {
    let root = ctx.sys_path("class/bluetooth");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("hci")) {
        let dir = root.join(&name);
        out.push(BluetoothDev {
            address: access::read_trimmed(dir.join("address")),
            dev_name: access::read_trimmed(dir.join("name")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_thunderbolt(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<ThunderboltDev> {
    let root = ctx.sys_path("bus/thunderbolt/devices");
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
        if name == "domain0" || name.starts_with("domain") {
            continue;
        }
        let dir = root.join(&name);
        out.push(ThunderboltDev {
            vendor: access::read_trimmed(dir.join("vendor_name")),
            device: access::read_trimmed(dir.join("device_name")),
            unique_id: access::read_trimmed(dir.join("unique_id")),
            authorized: access::read_trimmed(dir.join("authorized")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_video(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<VideoDev> {
    let root = ctx.sys_path("class/video4linux");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("video") || n.starts_with("media")) {
        let dir = root.join(&name);
        out.push(VideoDev {
            dev_name: access::read_trimmed(dir.join("name")),
            index: access::read_trimmed(dir.join("index")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_mmc(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<MmcHost> {
    let root = ctx.sys_path("class/mmc_host");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("mmc")) {
        let dir = root.join(&name);
        let children = access::list_dir_names(&dir).value.unwrap_or_default();
        let card = children
            .into_iter()
            .find(|c| c.starts_with(&format!("{name}:")) || (c.starts_with("mmc") && c.contains(':')));
        let (cid, name_tag, serial, date, ty) = if let Some(ref card_name) = card {
            let cdir = dir.join(card_name);
            (
                access::read_trimmed(cdir.join("cid")),
                access::read_trimmed(cdir.join("name")),
                access::read_trimmed(cdir.join("serial")),
                access::read_trimmed(cdir.join("date")),
                access::read_trimmed(cdir.join("type")),
            )
        } else {
            (
                Sample::missing("mmc card"),
                Sample::missing("mmc card"),
                Sample::missing("mmc card"),
                Sample::missing("mmc card"),
                Sample::missing("mmc card"),
            )
        };
        out.push(MmcHost {
            name,
            card,
            cid,
            name_tag,
            serial,
            date,
            r#type: ty,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_mei(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<MeiDev> {
    let root = ctx.sys_path("class/mei");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("mei")) {
        let dir = root.join(&name);
        out.push(MeiDev {
            fw_status: access::read_trimmed(dir.join("fw_status")),
            trx: access::read_trimmed(dir.join("trx")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn is_serial_tty(name: &str) -> bool {
    name.starts_with("ttyS")
        || name.starts_with("ttyUSB")
        || name.starts_with("ttyACM")
        || name.starts_with("ttyAMA")
        || name.starts_with("ttyO")
        || name.starts_with("hvc")
}

fn read_serial(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<SerialPort> {
    let root = ctx.sys_path("class/tty");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => return Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| is_serial_tty(n)) {
        let dir = root.join(&name);
        out.push(SerialPort {
            uartclk: access::read_u64(dir.join("uartclk")),
            irq: access::read_trimmed(dir.join("irq")),
            typ: access::read_trimmed(dir.join("type")),
            port: access::read_trimmed(dir.join("port")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// `/proc/tty/drivers`：`serial /dev/ttyS 4 64 serial`
pub fn parse_tty_drivers(sample: &Sample<String>) -> Vec<TtyDriver> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 5 {
            continue;
        }
        out.push(TtyDriver {
            name: cols[0].to_string(),
            device: cols[1].to_string(),
            major: cols[2].to_string(),
            minors: cols[3].to_string(),
            kind: cols[4].to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rfkill_v4l_mmc_fixture() {
        let root = std::env::temp_dir().join(format!("aida-buses-{}", std::process::id()));
        let rf = root.join("sys/class/rfkill/rfkill0");
        fs::create_dir_all(&rf).unwrap();
        fs::write(rf.join("type"), "wlan\n").unwrap();
        fs::write(rf.join("state"), "1\n").unwrap();
        fs::write(rf.join("hard"), "0\n").unwrap();
        fs::write(rf.join("soft"), "0\n").unwrap();
        let v4l = root.join("sys/class/video4linux/video0");
        fs::create_dir_all(&v4l).unwrap();
        fs::write(v4l.join("name"), "USB Camera\n").unwrap();
        fs::write(v4l.join("index"), "0\n").unwrap();
        let mmc = root.join("sys/class/mmc_host/mmc0/mmc0:0001");
        fs::create_dir_all(&mmc).unwrap();
        fs::write(mmc.join("cid"), "aabb\n").unwrap();
        fs::write(mmc.join("name"), "SD32G\n").unwrap();
        fs::write(mmc.join("type"), "SD\n").unwrap();
        let mei = root.join("sys/class/mei/mei0");
        fs::create_dir_all(&mei).unwrap();
        fs::write(mei.join("fw_status"), "00\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.rfkill[0].kind.value.as_deref(), Some("wlan"));
        assert_eq!(r.video[0].dev_name.value.as_deref(), Some("USB Camera"));
        assert_eq!(r.mmc[0].name_tag.value.as_deref(), Some("SD32G"));
        assert_eq!(r.mei[0].name, "mei0");
        let tty = root.join("sys/class/tty/ttyS0");
        fs::create_dir_all(&tty).unwrap();
        fs::write(tty.join("uartclk"), "1843200\n").unwrap();
        fs::write(tty.join("irq"), "4\n").unwrap();
        fs::write(tty.join("type"), "4\n").unwrap();
        fs::create_dir_all(root.join("sys/class/tty/tty0")).unwrap();
        fs::create_dir_all(root.join("proc/tty")).unwrap();
        fs::write(
            root.join("proc/tty/drivers"),
            "serial               /dev/ttyS       4      64 serial\n",
        )
        .unwrap();
        let r2 = collect(&ctx);
        assert_eq!(r2.serial.len(), 1);
        assert_eq!(r2.serial[0].name, "ttyS0");
        assert_eq!(r2.serial[0].uartclk.value, Some(1843200));
        assert_eq!(r2.tty_drivers[0].kind, "serial");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_rfkill_is_not_empty_class() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aida-buses-deny-{}", std::process::id()));
        let rf = root.join("sys/class/rfkill");
        fs::create_dir_all(&rf).unwrap();
        fs::set_permissions(&rf, fs::Permissions::from_mode(0o000)).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let _ = fs::set_permissions(&rf, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&root);
        assert!(
            r.notes.iter().any(|n| n.contains("权限") || n.contains("失败")),
            "{:?}",
            r.notes
        );
        assert!(!r.notes.iter().any(|n| n.contains("无 rfkill")));
    }
}
