//! 额外总线外设：rfkill / Bluetooth / Thunderbolt / V4L / MMC / MEI / IEEE802.11 / Type-C / SPI。
//! 不调用 `rfkill`/`bluetoothctl`/`boltctl`/`v4l2-ctl`/`mmc`/`iw`/`lsusb`/`spi-tools`。

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
    pub misc: Vec<String>,
    pub hidraw: Vec<HidrawDev>,
    pub virtio_ports: Vec<VirtioPort>,
    pub gpio: Vec<GpioChip>,
    pub mtd: Vec<MtdDev>,
    pub infiniband: Vec<IbDev>,
    /// `phyN` 名；空表示无 mac80211（云 VM 常见）。
    pub ieee80211: Vec<String>,
    pub typec: Vec<String>,
    /// USB gadget UDC。
    pub udc: Vec<String>,
    pub dax: Vec<String>,
    pub wmi: Vec<String>,
    pub spi: Vec<String>,
    pub serio: Vec<String>,
    pub ubi: Vec<String>,
    pub scsi_generic: Vec<String>,
    pub wwan: Vec<String>,
    pub ppp: Vec<String>,
    pub phy: Vec<String>,
    pub remoteproc: Vec<String>,
    pub extcon: Vec<String>,
    pub tee: Vec<String>,
    pub mdio_bus: Vec<String>,
    pub spi_master: Vec<String>,
    pub i2c_dev: Vec<String>,
    pub nvme_subsystem: Vec<String>,
    pub w1: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HidrawDev {
    pub name: String,
    pub hid_name: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VirtioPort {
    pub name: String,
    pub port_name: Sample<String>,
    pub guest_connected: Sample<String>,
    pub host_connected: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GpioChip {
    pub name: String,
    pub label: Sample<String>,
    pub ngpio: Sample<u64>,
    pub base: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MtdDev {
    pub name: String,
    pub mtd_name: Sample<String>,
    pub size: Sample<u64>,
    pub erasesize: Sample<u64>,
    pub typ: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IbDev {
    pub name: String,
    pub node_guid: Sample<String>,
    pub node_type: Sample<String>,
    pub ports: usize,
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
    let misc = read_misc(ctx, &mut notes);
    let hidraw = read_hidraw(ctx);
    let virtio_ports = read_virtio_ports(ctx);
    let gpio = read_gpio(ctx, &mut notes);
    let mtd = read_mtd(ctx, &mut notes);
    let infiniband = read_infiniband(ctx, &mut notes);
    let mut missing = Vec::new();
    let ieee80211 = list_optional_names(
        ctx.sys_path("class/ieee80211"),
        8,
        "ieee80211",
        &mut notes,
        &mut missing,
    );
    let typec = list_optional_names(ctx.sys_path("class/typec"), 8, "typec", &mut notes, &mut missing);
    let udc = list_optional_names(ctx.sys_path("class/udc"), 8, "udc", &mut notes, &mut missing);
    let dax = list_optional_names(ctx.sys_path("class/dax"), 8, "dax", &mut notes, &mut missing);
    let wmi = list_optional_names(
        ctx.sys_path("bus/wmi/devices"),
        8,
        "wmi",
        &mut notes,
        &mut missing,
    );
    let spi = list_optional_names(
        ctx.sys_path("bus/spi/devices"),
        8,
        "spi",
        &mut notes,
        &mut missing,
    );
    let serio = list_optional_names(
        ctx.sys_path("bus/serio/devices"),
        8,
        "serio",
        &mut notes,
        &mut missing,
    );
    let ubi = list_optional_names(ctx.sys_path("class/ubi"), 8, "ubi", &mut notes, &mut missing);
    let scsi_generic = list_optional_names(
        ctx.sys_path("class/scsi_generic"),
        8,
        "scsi_generic",
        &mut notes,
        &mut missing,
    );
    let wwan = list_optional_names(ctx.sys_path("class/wwan"), 8, "wwan", &mut notes, &mut missing);
    let ppp = list_optional_names(ctx.sys_path("class/ppp"), 8, "ppp", &mut notes, &mut missing);
    let phy = list_optional_names(ctx.sys_path("class/phy"), 8, "phy", &mut notes, &mut missing);
    let remoteproc = list_optional_names(
        ctx.sys_path("class/remoteproc"),
        8,
        "remoteproc",
        &mut notes,
        &mut missing,
    );
    let extcon = list_optional_names(ctx.sys_path("class/extcon"), 8, "extcon", &mut notes, &mut missing);
    let tee = list_optional_names(ctx.sys_path("class/tee"), 8, "tee", &mut notes, &mut missing);
    let mdio_bus = list_optional_names(
        ctx.sys_path("class/mdio_bus"),
        8,
        "mdio_bus",
        &mut notes,
        &mut missing,
    );
    let spi_master = list_optional_names(
        ctx.sys_path("class/spi_master"),
        8,
        "spi_master",
        &mut notes,
        &mut missing,
    );
    let i2c_dev = list_optional_names(
        ctx.sys_path("class/i2c-dev"),
        8,
        "i2c-dev",
        &mut notes,
        &mut missing,
    );
    let nvme_subsystem = list_optional_names(
        ctx.sys_path("class/nvme-subsystem"),
        8,
        "nvme-subsystem",
        &mut notes,
        &mut missing,
    );
    let w1 = list_optional_names(
        ctx.sys_path("bus/w1/devices"),
        8,
        "w1",
        &mut notes,
        &mut missing,
    );
    if !missing.is_empty() {
        notes.push(format!(
            "无 {}（云主机/无对应硬件时常见）。",
            missing.join("/")
        ));
    }
    BusesReport {
        rfkill,
        bluetooth,
        thunderbolt,
        video,
        mmc,
        mei,
        serial,
        tty_drivers,
        misc,
        hidraw,
        virtio_ports,
        gpio,
        mtd,
        infiniband,
        ieee80211,
        typec,
        udc,
        dax,
        wmi,
        spi,
        serio,
        ubi,
        scsi_generic,
        wwan,
        ppp,
        phy,
        remoteproc,
        extcon,
        tee,
        mdio_bus,
        spi_master,
        i2c_dev,
        nvme_subsystem,
        w1,
        notes,
    }
}

fn list_optional_names(
    path: impl AsRef<std::path::Path>,
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    match dir_list(path) {
        DirList::Names(mut n) => {
            n.sort();
            n.truncate(cap);
            n
        }
        DirList::Missing => {
            missing.push(label);
            Vec::new()
        }
        DirList::Failed(l) => {
            notes.push(l);
            Vec::new()
        }
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
        let typ = access::read_trimmed(dir.join("type"));
        // 8250 预留槽 type=0 表示没探到 UART；USB/ACM 通常没有 type 节点。
        if name.starts_with("ttyS") && typ.value.as_deref() == Some("0") {
            continue;
        }
        out.push(SerialPort {
            uartclk: access::read_u64(dir.join("uartclk")),
            irq: access::read_trimmed(dir.join("irq")),
            typ,
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

fn read_hidraw(ctx: &ProbeCtx) -> Vec<HidrawDev> {
    let root = ctx.sys_path("class/hidraw");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("hidraw")) {
        let dir = root.join(&name);
        let hid_name = match access::read_trimmed(dir.join("device/uevent")) {
            Sample {
                access: AccessKind::Ok,
                value: Some(text),
                source,
                ..
            } => text
                .lines()
                .find_map(|l| l.strip_prefix("HID_NAME="))
                .map(|v| Sample::ok(v.to_string(), source.clone()))
                .unwrap_or_else(|| Sample::ok(text, source)),
            s => s,
        };
        out.push(HidrawDev { hid_name, name });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_virtio_ports(ctx: &ProbeCtx) -> Vec<VirtioPort> {
    let root = ctx.sys_path("class/virtio-ports");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        let dir = root.join(&name);
        out.push(VirtioPort {
            port_name: access::read_trimmed(dir.join("name")),
            guest_connected: access::read_trimmed(dir.join("guest_connected")),
            host_connected: access::read_trimmed(dir.join("host_connected")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_gpio(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<GpioChip> {
    let root = ctx.sys_path("class/gpio");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => {
            notes.push("无 GPIO class（服务器/虚拟机常见）。".into());
            return Vec::new();
        }
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names.into_iter().filter(|n| n.starts_with("gpiochip")) {
        let dir = root.join(&name);
        out.push(GpioChip {
            label: access::read_trimmed(dir.join("label")),
            ngpio: access::read_u64(dir.join("ngpio")),
            base: access::read_trimmed(dir.join("base")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_mtd(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<MtdDev> {
    let root = ctx.sys_path("class/mtd");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => {
            notes.push("无 MTD class（无 NOR/NAND 时常见）。".into());
            return Vec::new();
        }
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names
        .into_iter()
        .filter(|n| n.starts_with("mtd") && !n.ends_with("ro"))
    {
        let dir = root.join(&name);
        out.push(MtdDev {
            mtd_name: access::read_trimmed(dir.join("name")),
            size: access::read_u64(dir.join("size")),
            erasesize: access::read_u64(dir.join("erasesize")),
            typ: access::read_trimmed(dir.join("type")),
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_infiniband(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<IbDev> {
    let root = ctx.sys_path("class/infiniband");
    let names = match dir_list(&root) {
        DirList::Names(n) => n,
        DirList::Missing => {
            notes.push("无 InfiniBand class。".into());
            return Vec::new();
        }
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names {
        let dir = root.join(&name);
        let ports = match access::list_dir_names(dir.join("ports")) {
            Sample {
                access: AccessKind::Ok,
                value: Some(n),
                ..
            } => n.len(),
            _ => 0,
        };
        out.push(IbDev {
            node_guid: access::read_trimmed(dir.join("node_guid")),
            node_type: access::read_trimmed(dir.join("node_type")),
            ports,
            name,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_misc(ctx: &ProbeCtx, notes: &mut Vec<String>) -> Vec<String> {
    let root = ctx.sys_path("class/misc");
    match dir_list(&root) {
        DirList::Names(mut n) => {
            n.sort();
            n.truncate(32);
            n
        }
        DirList::Missing => Vec::new(),
        DirList::Failed(l) => {
            notes.push(l);
            Vec::new()
        }
    }
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
        let hid = root.join("sys/class/hidraw/hidraw0/device");
        fs::create_dir_all(&hid).unwrap();
        fs::write(hid.join("uevent"), "HID_NAME=Test Keyboard\nHID_ID=0003:0000:0000\n").unwrap();
        let gpio = root.join("sys/class/gpio/gpiochip0");
        fs::create_dir_all(&gpio).unwrap();
        fs::write(gpio.join("label"), "INTC0001\n").unwrap();
        fs::write(gpio.join("ngpio"), "8\n").unwrap();
        fs::write(gpio.join("base"), "0\n").unwrap();
        let mtd = root.join("sys/class/mtd/mtd0");
        fs::create_dir_all(&mtd).unwrap();
        fs::write(mtd.join("name"), "spi-flash\n").unwrap();
        fs::write(mtd.join("size"), "16777216\n").unwrap();
        fs::write(mtd.join("erasesize"), "4096\n").unwrap();
        fs::write(mtd.join("type"), "nor\n").unwrap();
        fs::create_dir_all(root.join("sys/class/mtd/mtd0ro")).unwrap();
        let ib = root.join("sys/class/infiniband/mlx5_0/ports/1");
        fs::create_dir_all(&ib).unwrap();
        fs::create_dir_all(root.join("sys/class/ieee80211/phy0")).unwrap();
        fs::create_dir_all(root.join("sys/class/scsi_generic/sg0")).unwrap();
        fs::create_dir_all(root.join("sys/class/phy/eth0-phy")).unwrap();
        fs::create_dir_all(root.join("sys/class/remoteproc/remoteproc0")).unwrap();
        fs::create_dir_all(root.join("sys/class/extcon/extcon0")).unwrap();
        fs::create_dir_all(root.join("sys/class/spi_master/spi0")).unwrap();
        fs::create_dir_all(root.join("sys/class/i2c-dev/i2c-0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/spi/devices/spi0.0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/serio/devices/serio0")).unwrap();
        fs::write(
            root.join("sys/class/infiniband/mlx5_0/node_guid"),
            "0000:0000:0000:0001\n",
        )
        .unwrap();
        fs::write(root.join("sys/class/infiniband/mlx5_0/node_type"), "CA\n").unwrap();
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
        assert_eq!(r.hidraw[0].hid_name.value.as_deref(), Some("Test Keyboard"));
        assert_eq!(r.gpio[0].ngpio.value, Some(8));
        assert_eq!(r.mtd[0].mtd_name.value.as_deref(), Some("spi-flash"));
        assert_eq!(r.mtd.len(), 1);
        assert_eq!(r.infiniband[0].ports, 1);
        assert_eq!(r.ieee80211, vec!["phy0".to_string()]);
        assert_eq!(r.scsi_generic, vec!["sg0".to_string()]);
        assert_eq!(r.phy, vec!["eth0-phy".to_string()]);
        assert_eq!(r.remoteproc, vec!["remoteproc0".to_string()]);
        assert_eq!(r.extcon, vec!["extcon0".to_string()]);
        assert_eq!(r.spi_master, vec!["spi0".to_string()]);
        assert_eq!(r.i2c_dev, vec!["i2c-0".to_string()]);
        assert_eq!(r.spi, vec!["spi0.0".to_string()]);
        assert_eq!(r.serio, vec!["serio0".to_string()]);
        assert!(
            r.notes.iter().any(|n| n.contains("typec")),
            "missing typec/udc/dax/wmi/ubi should share one note: {:?}",
            r.notes
        );
        let tty = root.join("sys/class/tty/ttyS0");
        fs::create_dir_all(&tty).unwrap();
        fs::write(tty.join("uartclk"), "1843200\n").unwrap();
        fs::write(tty.join("irq"), "4\n").unwrap();
        fs::write(tty.join("type"), "4\n").unwrap();
        let idle = root.join("sys/class/tty/ttyS1");
        fs::create_dir_all(&idle).unwrap();
        fs::write(idle.join("type"), "0\n").unwrap();
        fs::create_dir_all(root.join("sys/class/tty/hvc0")).unwrap();
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
