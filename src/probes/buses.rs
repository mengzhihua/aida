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
    pub macvtap: Vec<String>,
    pub tun: Vec<String>,
    pub nvme_generic: Vec<String>,
    pub nvme_fabrics: Vec<String>,
    pub iscsi_endpoint: Vec<String>,
    pub iscsi_iface: Vec<String>,
    pub iscsi_connection: Vec<String>,
    pub container: Vec<String>,
    pub iscsi_flashnode: Vec<String>,
    pub nd: Vec<String>,
    pub dma_heap: Vec<String>,
    pub cxl: Vec<String>,
    pub devfreq: Vec<String>,
    pub fpga: Vec<String>,
    pub gnss: Vec<String>,
    pub rpmsg: Vec<String>,
    /// 设备崩溃转储是瞬时节点；GUI `refresh_live` 会单独更新这一项。
    pub devcoredump: Vec<String>,
    pub scsi_disk: Vec<String>,
    pub scsi_tape: Vec<String>,
    pub graphics: Vec<String>,
    pub cec: Vec<String>,
    pub media: Vec<String>,
    pub nbd: Vec<String>,
    pub vfio: Vec<String>,
    pub mdev: Vec<String>,
    pub vhost: Vec<String>,
    /// Fibre Channel：`fc_host` / `fc_remote_ports` / `fc_vports`，没有统一 `class/fc`。
    pub fc: Vec<String>,
    /// Compute accelerator class（`accelN`）。
    pub accel: Vec<String>,
    /// vDPA：先 `bus/vdpa/devices`，再 `class/vdpa`。
    pub vdpa: Vec<String>,
    /// Userspace I/O（`class/uio`）。
    pub uio: Vec<String>,
    /// auxiliary bus：先 `bus/auxiliary/devices`，再 `class/auxiliary`。
    pub auxiliary: Vec<String>,
    /// USB monitor（`class/usbmon`）。
    pub usbmon: Vec<String>,
    /// Generic Counter：先 `bus/counter/devices`，再 `class/counter`。
    pub counter: Vec<String>,
    /// DisplayPort AUX（`class/drm_dp_aux_dev`）。
    pub drm_dp_aux_dev: Vec<String>,
    /// MHI：先 `bus/mhi/devices`，再 `class/mhi`。
    pub mhi: Vec<String>,
    /// IPMI：`class/ipmi`，BMC 另见 `class/ipmi_bmc`。
    pub ipmi: Vec<String>,
    /// USB dual-role switch（`class/usb_role`）。
    pub usb_role: Vec<String>,
    /// I3C：先 `bus/i3c/devices`，再 `class/i3c`。
    pub i3c: Vec<String>,
    /// VDUSE 用户态 vDPA（`class/vduse`）。
    pub vduse: Vec<String>,
    /// Generic MUX（`class/mux`，`muxchipN`）。
    pub mux: Vec<String>,
    /// SoundWire：先 `bus/soundwire/devices`，再 `class/soundwire`。
    pub soundwire: Vec<String>,
    /// 红外遥控接收器（`class/rc`，`rcN`）。
    pub rc: Vec<String>,
    /// MIPI STM：`class/stm`，源设备另见 `class/stm_source`。
    pub stm: Vec<String>,
    /// PECI：先 `bus/peci/devices`，再 `class/peci`。
    pub peci: Vec<String>,
    /// PM wakeup 源（`class/wakeup`，`wakeupN`）。只列名，不读 event_count。
    pub wakeup: Vec<String>,
    /// x86 MSR 字符设备（`class/msr`，`msrN`）。只列名，不 dump `/dev/cpu/N/msr`。
    pub msr: Vec<String>,
    /// Data PLL（`class/dpll`）。空 = 无电信/同步硬件。
    pub dpll: Vec<String>,
    /// IOMMU 设备（`class/iommu`）。与 `iommu_groups` 不是同一棵树。
    pub iommu: Vec<String>,
    /// HID：先 `bus/hid/devices`，再 `class/hid`。只列名，不读 report。
    pub hid: Vec<String>,
    /// 内存热插拔块：先 `bus/memory/devices`，再 `class/memory`。最多 8 个名。
    pub memory: Vec<String>,
    /// IEEE 1394：先 `bus/firewire/devices`，再 `class/firewire`。
    pub firewire: Vec<String>,
    /// Greybus：先 `bus/greybus/devices`，再 `class/greybus`。
    pub greybus: Vec<String>,
    /// RapidIO：先 `bus/rapidio/devices`，再 `class/rapidio`。
    pub rapidio: Vec<String>,
    /// ULPI USB PHY：先 `bus/ulpi/devices`，再 `class/ulpi`。
    pub ulpi: Vec<String>,
    /// SPMI：先 `bus/spmi/devices`，再 `class/spmi`。
    pub spmi: Vec<String>,
    /// PCIe endpoint controller（`class/pci_epc`）。
    pub pci_epc: Vec<String>,
    /// IEEE 1588 PTP 时钟（`class/ptp`）。
    pub ptp: Vec<String>,
    /// Pulse Per Second（`class/pps`）。
    pub pps: Vec<String>,
    /// TPM：`class/tpm`，资源管理器另见 `class/tpmrm`。
    pub tpm: Vec<String>,
    /// BIOS WMI 固件属性（`class/firmware-attributes`，ThinkLMI/Dell sysman）。
    pub firmware_attributes: Vec<String>,
    /// PCIe endpoint function：先 `bus/pci-epf/devices`，再 `class/pci_epf`。
    pub pci_epf: Vec<String>,
    /// MIPI Slimbus：先 `bus/slimbus/devices`，再 `class/slimbus`。
    pub slimbus: Vec<String>,
    /// Memory Stick：先 `bus/memstick/devices`，再 `class/memstick_host`。
    pub memstick: Vec<String>,
    /// SIOX 串行 IO 扩展：先 `bus/siox/devices`，再 `class/siox`。
    pub siox: Vec<String>,
    /// MIPI HSI：先 `bus/hsi/devices`，再 `class/hsi`。
    pub hsi: Vec<String>,
    /// ARM AMBA：先 `bus/amba/devices`，再 `class/amba`。
    pub amba: Vec<String>,
    /// IBM FSI：合并 `bus/fsi/devices` 与 `class/fsi-master`。
    pub fsi: Vec<String>,
    /// 并口用户态（`class/ppdev`）。
    pub ppdev: Vec<String>,
    /// PCMCIA / CardBus：先 `bus/pcmcia/devices`，再 `class/pcmcia`。
    pub pcmcia: Vec<String>,
    /// Hyper-V VMBus：先 `bus/vmbus/devices`，再 `class/vmbus`。
    pub vmbus: Vec<String>,
    /// Broadcom AMBA（bcma）：先 `bus/bcma/devices`，再 `class/bcma`。
    pub bcma: Vec<String>,
    /// Intel Trace Hub：先 `bus/intel_th/devices`，再 `class/intel_th`。
    pub intel_th: Vec<String>,
    /// ARM CoreSight：先 `bus/coresight/devices`，再 `class/coresight`。
    pub coresight: Vec<String>,
    /// PCI Non-Transparent Bridge：先 `bus/ntb/devices`，再 `class/ntb`。
    pub ntb: Vec<String>,
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
    let typec = list_optional_names(
        ctx.sys_path("class/typec"),
        8,
        "typec",
        &mut notes,
        &mut missing,
    );
    let udc = list_optional_names(
        ctx.sys_path("class/udc"),
        8,
        "udc",
        &mut notes,
        &mut missing,
    );
    let dax = list_optional_names(
        ctx.sys_path("class/dax"),
        8,
        "dax",
        &mut notes,
        &mut missing,
    );
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
    let ubi = list_optional_names(
        ctx.sys_path("class/ubi"),
        8,
        "ubi",
        &mut notes,
        &mut missing,
    );
    let scsi_generic = list_optional_names(
        ctx.sys_path("class/scsi_generic"),
        8,
        "scsi_generic",
        &mut notes,
        &mut missing,
    );
    let wwan = list_optional_names(
        ctx.sys_path("class/wwan"),
        8,
        "wwan",
        &mut notes,
        &mut missing,
    );
    let ppp = list_optional_names(
        ctx.sys_path("class/ppp"),
        8,
        "ppp",
        &mut notes,
        &mut missing,
    );
    let phy = list_optional_names(
        ctx.sys_path("class/phy"),
        8,
        "phy",
        &mut notes,
        &mut missing,
    );
    let remoteproc = list_optional_names(
        ctx.sys_path("class/remoteproc"),
        8,
        "remoteproc",
        &mut notes,
        &mut missing,
    );
    let extcon = list_optional_names(
        ctx.sys_path("class/extcon"),
        8,
        "extcon",
        &mut notes,
        &mut missing,
    );
    let tee = list_optional_names(
        ctx.sys_path("class/tee"),
        8,
        "tee",
        &mut notes,
        &mut missing,
    );
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
    let macvtap = list_optional_names(
        ctx.sys_path("class/macvtap"),
        8,
        "macvtap",
        &mut notes,
        &mut missing,
    );
    let tun = list_misc_device(ctx, "tun", Some("net/tun"), &mut notes, &mut missing);
    let nvme_generic = list_optional_names(
        ctx.sys_path("class/nvme-generic"),
        8,
        "nvme-generic",
        &mut notes,
        &mut missing,
    );
    let nvme_fabrics = list_misc_device(
        ctx,
        "nvme-fabrics",
        Some("nvme-fabrics"),
        &mut notes,
        &mut missing,
    );
    let iscsi_endpoint = list_optional_names(
        ctx.sys_path("class/iscsi_endpoint"),
        8,
        "iscsi_endpoint",
        &mut notes,
        &mut missing,
    );
    let iscsi_iface = list_optional_names(
        ctx.sys_path("class/iscsi_iface"),
        8,
        "iscsi_iface",
        &mut notes,
        &mut missing,
    );
    let iscsi_connection = list_optional_names(
        ctx.sys_path("class/iscsi_connection"),
        8,
        "iscsi_connection",
        &mut notes,
        &mut missing,
    );
    let container = list_optional_names(
        ctx.sys_path("bus/container/devices"),
        8,
        "container",
        &mut notes,
        &mut missing,
    );
    let iscsi_flashnode = list_class_or_bus(
        ctx,
        "class/iscsi_flashnode",
        "bus/iscsi_flashnode/devices",
        8,
        "iscsi_flashnode",
        &mut notes,
        &mut missing,
    );
    let nd = list_optional_names(ctx.sys_path("class/nd"), 8, "nd", &mut notes, &mut missing);
    let dma_heap = list_optional_names(
        ctx.sys_path("class/dma_heap"),
        8,
        "dma_heap",
        &mut notes,
        &mut missing,
    );
    let cxl = list_alt_dirs(
        ctx,
        "bus/cxl/devices",
        "class/cxl",
        8,
        "cxl",
        &mut notes,
        &mut missing,
    );
    let devfreq = list_optional_names(
        ctx.sys_path("class/devfreq"),
        8,
        "devfreq",
        &mut notes,
        &mut missing,
    );
    let fpga = list_prefixed_classes(
        ctx,
        &[
            ("class/fpga_manager", "fpga_manager"),
            ("class/fpga_bridge", "fpga_bridge"),
            ("class/fpga_region", "fpga_region"),
        ],
        8,
        "fpga",
        &mut notes,
        &mut missing,
    );
    let gnss = list_optional_names(
        ctx.sys_path("class/gnss"),
        8,
        "gnss",
        &mut notes,
        &mut missing,
    );
    let rpmsg = list_optional_names(
        ctx.sys_path("class/rpmsg"),
        8,
        "rpmsg",
        &mut notes,
        &mut missing,
    );
    let devcoredump = list_optional_names(
        ctx.sys_path("class/devcoredump"),
        8,
        "devcoredump",
        &mut notes,
        &mut missing,
    );
    let scsi_disk = list_optional_names(
        ctx.sys_path("class/scsi_disk"),
        8,
        "scsi_disk",
        &mut notes,
        &mut missing,
    );
    let scsi_tape = list_optional_names(
        ctx.sys_path("class/scsi_tape"),
        8,
        "scsi_tape",
        &mut notes,
        &mut missing,
    );
    let graphics = list_optional_names(
        ctx.sys_path("class/graphics"),
        8,
        "graphics",
        &mut notes,
        &mut missing,
    );
    let cec = list_optional_names(
        ctx.sys_path("class/cec"),
        8,
        "cec",
        &mut notes,
        &mut missing,
    );
    let media = list_optional_names(
        ctx.sys_path("class/media"),
        8,
        "media",
        &mut notes,
        &mut missing,
    );
    let nbd = list_block_prefixed(ctx, "nbd", 16, "nbd", &mut notes, &mut missing);
    // 旧 VFIO group class 是 `class/vfio`；cdev/IOMMUFD 是 `class/vfio-dev`。两边都缺失才 leftover。
    let vfio = list_prefixed_classes(
        ctx,
        &[("class/vfio", "vfio"), ("class/vfio-dev", "vfio-dev")],
        8,
        "vfio",
        &mut notes,
        &mut missing,
    );
    // mdev 真实 ABI 是 bus；没有独立 `class/mdev` 时不要当成缺失。
    let mdev = list_alt_dirs(
        ctx,
        "bus/mdev/devices",
        "class/mdev",
        8,
        "mdev",
        &mut notes,
        &mut missing,
    );
    // vhost-net/vsock/vdpa 注册为 misc + /dev 节点，没有 `class/vhost`。
    let vhost = list_misc_prefix(ctx, "vhost-", 8, "vhost", &mut notes, &mut missing);
    let fc = list_prefixed_classes(
        ctx,
        &[
            ("class/fc_host", "fc_host"),
            ("class/fc_remote_ports", "fc_remote_ports"),
            ("class/fc_vports", "fc_vports"),
        ],
        8,
        "fc",
        &mut notes,
        &mut missing,
    );
    let accel = list_optional_names(
        ctx.sys_path("class/accel"),
        8,
        "accel",
        &mut notes,
        &mut missing,
    );
    let vdpa = list_alt_dirs(
        ctx,
        "bus/vdpa/devices",
        "class/vdpa",
        8,
        "vdpa",
        &mut notes,
        &mut missing,
    );
    let uio = list_optional_names(
        ctx.sys_path("class/uio"),
        8,
        "uio",
        &mut notes,
        &mut missing,
    );
    let auxiliary = list_alt_dirs(
        ctx,
        "bus/auxiliary/devices",
        "class/auxiliary",
        8,
        "auxiliary",
        &mut notes,
        &mut missing,
    );
    let usbmon = list_optional_names(
        ctx.sys_path("class/usbmon"),
        8,
        "usbmon",
        &mut notes,
        &mut missing,
    );
    // Generic Counter 真实 ABI 是 bus（sysfs-bus-counter）；没有独立 `class/counter` 时不要当成缺失。
    let counter = list_alt_dirs(
        ctx,
        "bus/counter/devices",
        "class/counter",
        8,
        "counter",
        &mut notes,
        &mut missing,
    );
    let drm_dp_aux_dev = list_optional_names(
        ctx.sys_path("class/drm_dp_aux_dev"),
        8,
        "drm_dp_aux_dev",
        &mut notes,
        &mut missing,
    );
    // MHI 真实 ABI 是 bus；没有独立 `class/mhi` 时不要当成缺失。
    let mhi = list_alt_dirs(
        ctx,
        "bus/mhi/devices",
        "class/mhi",
        8,
        "mhi",
        &mut notes,
        &mut missing,
    );
    // IPMI 消息接口是 `class/ipmi`；OpenBMC 还有 `class/ipmi_bmc`。两边都缺失才 leftover。
    let ipmi = list_prefixed_classes(
        ctx,
        &[("class/ipmi", "ipmi"), ("class/ipmi_bmc", "ipmi_bmc")],
        8,
        "ipmi",
        &mut notes,
        &mut missing,
    );
    let usb_role = list_optional_names(
        ctx.sys_path("class/usb_role"),
        8,
        "usb_role",
        &mut notes,
        &mut missing,
    );
    // I3C 真实 ABI 是 bus；没有独立 `class/i3c` 时不要当成缺失。
    let i3c = list_alt_dirs(
        ctx,
        "bus/i3c/devices",
        "class/i3c",
        8,
        "i3c",
        &mut notes,
        &mut missing,
    );
    let vduse = list_optional_names(
        ctx.sys_path("class/vduse"),
        8,
        "vduse",
        &mut notes,
        &mut missing,
    );
    let mux = list_optional_names(
        ctx.sys_path("class/mux"),
        8,
        "mux",
        &mut notes,
        &mut missing,
    );
    // SoundWire 真实 ABI 是 bus；没有独立 `class/soundwire` 时不要当成缺失。
    let soundwire = list_alt_dirs(
        ctx,
        "bus/soundwire/devices",
        "class/soundwire",
        8,
        "soundwire",
        &mut notes,
        &mut missing,
    );
    let rc = list_optional_names(ctx.sys_path("class/rc"), 8, "rc", &mut notes, &mut missing);
    // STM 设备是 `class/stm`；console/heartbeat 等源在 `class/stm_source`。两边都缺失才 leftover。
    let stm = list_prefixed_classes(
        ctx,
        &[("class/stm", "stm"), ("class/stm_source", "stm_source")],
        8,
        "stm",
        &mut notes,
        &mut missing,
    );
    // PECI 真实 ABI 是 bus；没有独立 `class/peci` 时不要当成缺失。
    let peci = list_alt_dirs(
        ctx,
        "bus/peci/devices",
        "class/peci",
        8,
        "peci",
        &mut notes,
        &mut missing,
    );
    let wakeup = list_optional_names(
        ctx.sys_path("class/wakeup"),
        8,
        "wakeup",
        &mut notes,
        &mut missing,
    );
    let msr = list_optional_names(ctx.sys_path("class/msr"), 8, "msr", &mut notes, &mut missing);
    let dpll = list_optional_names(
        ctx.sys_path("class/dpll"),
        8,
        "dpll",
        &mut notes,
        &mut missing,
    );
    let iommu = list_optional_names(
        ctx.sys_path("class/iommu"),
        8,
        "iommu",
        &mut notes,
        &mut missing,
    );
    // HID 真实 ABI 是 bus；没有独立 `class/hid` 时不要当成缺失。
    let hid = list_alt_dirs(
        ctx,
        "bus/hid/devices",
        "class/hid",
        8,
        "hid",
        &mut notes,
        &mut missing,
    );
    // 内存块在 `bus/memory`；没有独立 `class/memory` 时不要当成缺失。
    let memory = list_alt_dirs(
        ctx,
        "bus/memory/devices",
        "class/memory",
        8,
        "memory",
        &mut notes,
        &mut missing,
    );
    let firewire = list_alt_dirs(
        ctx,
        "bus/firewire/devices",
        "class/firewire",
        8,
        "firewire",
        &mut notes,
        &mut missing,
    );
    let greybus = list_alt_dirs(
        ctx,
        "bus/greybus/devices",
        "class/greybus",
        8,
        "greybus",
        &mut notes,
        &mut missing,
    );
    let rapidio = list_alt_dirs(
        ctx,
        "bus/rapidio/devices",
        "class/rapidio",
        8,
        "rapidio",
        &mut notes,
        &mut missing,
    );
    let ulpi = list_alt_dirs(
        ctx,
        "bus/ulpi/devices",
        "class/ulpi",
        8,
        "ulpi",
        &mut notes,
        &mut missing,
    );
    let spmi = list_alt_dirs(
        ctx,
        "bus/spmi/devices",
        "class/spmi",
        8,
        "spmi",
        &mut notes,
        &mut missing,
    );
    let pci_epc = list_optional_names(
        ctx.sys_path("class/pci_epc"),
        8,
        "pci_epc",
        &mut notes,
        &mut missing,
    );
    let ptp = list_optional_names(
        ctx.sys_path("class/ptp"),
        8,
        "ptp",
        &mut notes,
        &mut missing,
    );
    let pps = list_optional_names(
        ctx.sys_path("class/pps"),
        8,
        "pps",
        &mut notes,
        &mut missing,
    );
    // TPM 字符设备是 `class/tpm`；资源管理器是 `class/tpmrm`。两边都缺失才 leftover。
    let tpm = list_prefixed_classes(
        ctx,
        &[("class/tpm", "tpm"), ("class/tpmrm", "tpmrm")],
        8,
        "tpm",
        &mut notes,
        &mut missing,
    );
    let firmware_attributes = list_optional_names(
        ctx.sys_path("class/firmware-attributes"),
        8,
        "firmware_attributes",
        &mut notes,
        &mut missing,
    );
    let pci_epf = list_alt_dirs(
        ctx,
        "bus/pci-epf/devices",
        "class/pci_epf",
        8,
        "pci_epf",
        &mut notes,
        &mut missing,
    );
    let slimbus = list_alt_dirs(
        ctx,
        "bus/slimbus/devices",
        "class/slimbus",
        8,
        "slimbus",
        &mut notes,
        &mut missing,
    );
    let memstick = list_merge_dirs(
        ctx,
        "bus/memstick/devices",
        "class/memstick_host",
        8,
        "memstick",
        &mut notes,
        &mut missing,
    );
    let siox = list_alt_dirs(
        ctx,
        "bus/siox/devices",
        "class/siox",
        8,
        "siox",
        &mut notes,
        &mut missing,
    );
    let hsi = list_alt_dirs(
        ctx,
        "bus/hsi/devices",
        "class/hsi",
        8,
        "hsi",
        &mut notes,
        &mut missing,
    );
    let amba = list_alt_dirs(
        ctx,
        "bus/amba/devices",
        "class/amba",
        8,
        "amba",
        &mut notes,
        &mut missing,
    );
    // FSI master class 是连字符 `fsi-master`；空 bus 仍回退 class。
    let fsi = list_merge_dirs(
        ctx,
        "bus/fsi/devices",
        "class/fsi-master",
        8,
        "fsi",
        &mut notes,
        &mut missing,
    );
    let ppdev = list_optional_names(
        ctx.sys_path("class/ppdev"),
        8,
        "ppdev",
        &mut notes,
        &mut missing,
    );
    let pcmcia = list_alt_dirs(
        ctx,
        "bus/pcmcia/devices",
        "class/pcmcia",
        8,
        "pcmcia",
        &mut notes,
        &mut missing,
    );
    let vmbus = list_alt_dirs(
        ctx,
        "bus/vmbus/devices",
        "class/vmbus",
        8,
        "vmbus",
        &mut notes,
        &mut missing,
    );
    let bcma = list_alt_dirs(
        ctx,
        "bus/bcma/devices",
        "class/bcma",
        8,
        "bcma",
        &mut notes,
        &mut missing,
    );
    let intel_th = list_alt_dirs(
        ctx,
        "bus/intel_th/devices",
        "class/intel_th",
        8,
        "intel_th",
        &mut notes,
        &mut missing,
    );
    let coresight = list_alt_dirs(
        ctx,
        "bus/coresight/devices",
        "class/coresight",
        8,
        "coresight",
        &mut notes,
        &mut missing,
    );
    let ntb = list_alt_dirs(
        ctx,
        "bus/ntb/devices",
        "class/ntb",
        8,
        "ntb",
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
        macvtap,
        tun,
        nvme_generic,
        nvme_fabrics,
        iscsi_endpoint,
        iscsi_iface,
        iscsi_connection,
        container,
        iscsi_flashnode,
        nd,
        dma_heap,
        cxl,
        devfreq,
        fpga,
        gnss,
        rpmsg,
        devcoredump,
        scsi_disk,
        scsi_tape,
        graphics,
        cec,
        media,
        nbd,
        vfio,
        mdev,
        vhost,
        fc,
        accel,
        vdpa,
        uio,
        auxiliary,
        usbmon,
        counter,
        drm_dp_aux_dev,
        mhi,
        ipmi,
        usb_role,
        i3c,
        vduse,
        mux,
        soundwire,
        rc,
        stm,
        peci,
        wakeup,
        msr,
        dpll,
        iommu,
        hid,
        memory,
        firewire,
        greybus,
        rapidio,
        ulpi,
        spmi,
        pci_epc,
        ptp,
        pps,
        tpm,
        firmware_attributes,
        pci_epf,
        slimbus,
        memstick,
        siox,
        hsi,
        amba,
        fsi,
        ppdev,
        pcmcia,
        vmbus,
        bcma,
        intel_th,
        coresight,
        ntb,
        notes,
    }
}

/// 只刷新瞬时的 `class/devcoredump`，不重扫整份 buses。
/// 权限/leftover note 同步改写，避免 GUI 一直显示启动时的空列表或已消失的 `devcdN`。
pub fn refresh_devcoredump(report: &mut BusesReport, ctx: &ProbeCtx) {
    let mut extra = Vec::new();
    let mut missing = Vec::new();
    report.devcoredump = list_optional_names(
        ctx.sys_path("class/devcoredump"),
        8,
        "devcoredump",
        &mut extra,
        &mut missing,
    );
    report
        .notes
        .retain(|n| leftover_note(n).is_some() || !n.contains("devcoredump"));
    set_leftover_label(&mut report.notes, "devcoredump", !missing.is_empty());
    report.notes.extend(extra);
}

const LEFTOVER_SUFFIX: &str = "（云主机/无对应硬件时常见）。";

fn leftover_note(n: &str) -> Option<&str> {
    n.strip_prefix("无 ")?.strip_suffix(LEFTOVER_SUFFIX)
}

fn set_leftover_label(notes: &mut Vec<String>, label: &str, missing: bool) {
    if let Some(idx) = notes.iter().position(|n| leftover_note(n).is_some()) {
        let inner = leftover_note(&notes[idx]).unwrap_or("").to_string();
        let mut labels: Vec<String> = inner
            .split('/')
            .filter(|s| !s.is_empty() && *s != label)
            .map(str::to_string)
            .collect();
        if missing {
            labels.push(label.to_string());
        }
        if labels.is_empty() {
            notes.remove(idx);
        } else {
            notes[idx] = format!("无 {}{LEFTOVER_SUFFIX}", labels.join("/"));
        }
    } else if missing {
        notes.push(format!("无 {label}{LEFTOVER_SUFFIX}"));
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

/// iSCSI flashnode 在较新内核从 bus 改成 class。先看 `class/`，没有再看 `bus/.../devices`。
/// 两边都缺失才记 leftover；权限不足写 note，不要当成缺失。
fn list_class_or_bus(
    ctx: &ProbeCtx,
    class_rel: &str,
    bus_rel: &str,
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    list_alt_dirs(ctx, class_rel, bus_rel, cap, label, notes, missing)
}

/// 先试 `first_rel`，NotFound 再试 `second_rel`。两边都缺失才记 leftover。
fn list_alt_dirs(
    ctx: &ProbeCtx,
    first_rel: &str,
    second_rel: &str,
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    match dir_list(ctx.sys_path(first_rel)) {
        DirList::Names(mut n) => {
            n.sort();
            n.truncate(cap);
            return n;
        }
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
        DirList::Missing => {}
    }
    match dir_list(ctx.sys_path(second_rel)) {
        DirList::Names(mut n) => {
            n.sort();
            n.truncate(cap);
            n
        }
        DirList::Failed(l) => {
            notes.push(l);
            Vec::new()
        }
        DirList::Missing => {
            missing.push(label);
            Vec::new()
        }
    }
}

/// 合并两个目录的名字；空目录仍算存在。两边都 Missing 才 leftover。
fn list_merge_dirs(
    ctx: &ProbeCtx,
    first_rel: &str,
    second_rel: &str,
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut saw = false;
    for rel in [first_rel, second_rel] {
        match dir_list(ctx.sys_path(rel)) {
            DirList::Names(n) => {
                saw = true;
                out.extend(n);
            }
            DirList::Failed(l) => {
                saw = true;
                notes.push(l);
            }
            DirList::Missing => {}
        }
    }
    out.sort();
    out.dedup();
    out.truncate(cap);
    if !saw {
        missing.push(label);
    }
    out
}

/// FPGA 没有统一的 `class/fpga`。分别看 manager / bridge / region，名前加 class 前缀。
/// 三个 class 都缺失才记 leftover；权限不足写 note，不要当成缺失。
fn list_prefixed_classes(
    ctx: &ProbeCtx,
    classes: &[(&str, &str)],
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut saw_any = false;
    for (rel, prefix) in classes {
        match dir_list(ctx.sys_path(rel)) {
            DirList::Names(n) => {
                saw_any = true;
                for name in n {
                    out.push(format!("{prefix}/{name}"));
                }
            }
            DirList::Failed(l) => {
                saw_any = true;
                notes.push(l);
            }
            DirList::Missing => {}
        }
    }
    out.sort();
    out.truncate(cap);
    if !saw_any {
        missing.push(label);
    }
    out
}

/// vhost-net/vsock/vdpa 注册为 misc（`class/misc/vhost-*`）和 `/dev/vhost-*`，没有 `class/vhost`。
/// 列出 misc 与 /dev 里匹配前缀的名字；权限不足写 note，不要当成缺失。两边都没有才 leftover。
fn list_misc_prefix(
    ctx: &ProbeCtx,
    prefix: &'static str,
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut saw_misc = false;
    match dir_list(ctx.sys_path("class/misc")) {
        DirList::Names(n) => {
            saw_misc = true;
            for name in n {
                if name.starts_with(prefix) {
                    out.push(name);
                }
            }
        }
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
        DirList::Missing => {}
    }
    match dir_list(ctx.dev_path("")) {
        DirList::Names(n) => {
            for name in n {
                if name.starts_with(prefix) && !out.iter().any(|e| e == &name) {
                    out.push(name);
                }
            }
        }
        DirList::Failed(l) => {
            if !saw_misc {
                notes.push(l);
                return Vec::new();
            }
        }
        DirList::Missing => {}
    }
    out.sort();
    out.truncate(cap);
    if out.is_empty() {
        missing.push(label);
    }
    out
}

/// TUN / nvme-fabrics 注册为 misc 设备，没有独立 `/sys/class/{tun,nvme-fabrics}`。
/// 先看 `class/misc/<name>`，没有再看对应 `/dev` 节点；权限不足写 note，不要当成缺失。
fn list_misc_device(
    ctx: &ProbeCtx,
    misc_name: &'static str,
    dev_rel: Option<&str>,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    let misc_path = ctx.sys_path(format!("class/misc/{misc_name}"));
    match access::list_dir_names(&misc_path) {
        Sample {
            access: AccessKind::Ok,
            ..
        } => return vec![misc_name.to_string()],
        s if matches!(s.access, AccessKind::PermissionDenied | AccessKind::Error) => {
            notes.push(s.access_label());
            return Vec::new();
        }
        _ => {}
    }
    if let Some(rel) = dev_rel {
        let node = ctx.dev_path(rel);
        let source = node.display().to_string();
        match std::fs::metadata(&node) {
            Ok(_) => return vec![misc_name.to_string()],
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                notes.push(Sample::<String>::denied(source).access_label());
                return Vec::new();
            }
            Err(_) => {}
        }
    }
    missing.push(misc_name);
    Vec::new()
}

/// NBD 挂在通用 block class（`class/block/nbdN`），没有独立的 `class/nbd`。
/// 先看 `class/block`，没有再看 `block`；只保留 `nbd`+数字（不要 `nbd0p1`）。
/// 能列出 block 目录但没有 nbd 节点时返回空列表，不要当成 class 缺失。
/// 两边目录都 Missing 才记 leftover；权限不足写 note。
fn list_block_prefixed(
    ctx: &ProbeCtx,
    prefix: &'static str,
    cap: usize,
    label: &'static str,
    notes: &mut Vec<String>,
    missing: &mut Vec<&'static str>,
) -> Vec<String> {
    match dir_list(ctx.sys_path("class/block")) {
        DirList::Names(n) => return filter_prefix_disks(n, prefix, cap),
        DirList::Failed(l) => {
            notes.push(l);
            return Vec::new();
        }
        DirList::Missing => {}
    }
    match dir_list(ctx.sys_path("block")) {
        DirList::Names(n) => filter_prefix_disks(n, prefix, cap),
        DirList::Failed(l) => {
            notes.push(l);
            Vec::new()
        }
        DirList::Missing => {
            missing.push(label);
            Vec::new()
        }
    }
}

fn filter_prefix_disks(names: Vec<String>, prefix: &str, cap: usize) -> Vec<String> {
    let mut n: Vec<String> = names
        .into_iter()
        .filter(|name| {
            name.strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    n.sort();
    n.truncate(cap);
    n
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
    for name in names
        .into_iter()
        .filter(|n| n.starts_with("video") || n.starts_with("media"))
    {
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
        let card = children.into_iter().find(|c| {
            c.starts_with(&format!("{name}:")) || (c.starts_with("mmc") && c.contains(':'))
        });
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
        fs::write(
            hid.join("uevent"),
            "HID_NAME=Test Keyboard\nHID_ID=0003:0000:0000\n",
        )
        .unwrap();
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
        fs::create_dir_all(root.join("sys/class/macvtap/tap0")).unwrap();
        fs::create_dir_all(root.join("sys/class/nvme-generic/ng0n1")).unwrap();
        fs::create_dir_all(root.join("sys/class/misc/tun")).unwrap();
        fs::create_dir_all(root.join("sys/class/iscsi_endpoint/ep0")).unwrap();
        fs::create_dir_all(root.join("sys/class/iscsi_iface/iface0")).unwrap();
        fs::create_dir_all(root.join("sys/class/iscsi_connection/connection0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/container/devices/ACPI0004:00")).unwrap();
        fs::create_dir_all(root.join("sys/bus/iscsi_flashnode/devices/flashnode0")).unwrap();
        fs::create_dir_all(root.join("sys/class/nd/nmem0")).unwrap();
        fs::create_dir_all(root.join("sys/class/dma_heap/system")).unwrap();
        fs::create_dir_all(root.join("sys/bus/cxl/devices/mem0")).unwrap();
        fs::create_dir_all(root.join("sys/class/devfreq/devfreq0")).unwrap();
        fs::create_dir_all(root.join("sys/class/fpga_manager/fpga0")).unwrap();
        fs::create_dir_all(root.join("sys/class/fpga_bridge/br0")).unwrap();
        fs::create_dir_all(root.join("sys/class/fpga_region/region0")).unwrap();
        fs::create_dir_all(root.join("sys/class/gnss/gnss0")).unwrap();
        fs::create_dir_all(root.join("sys/class/rpmsg/rpmsg0")).unwrap();
        fs::create_dir_all(root.join("sys/class/devcoredump/devcd0")).unwrap();
        fs::create_dir_all(root.join("sys/class/scsi_disk/0:0:0:0")).unwrap();
        fs::create_dir_all(root.join("sys/class/scsi_tape/st0")).unwrap();
        fs::create_dir_all(root.join("sys/class/graphics/fb0")).unwrap();
        fs::create_dir_all(root.join("sys/class/cec/cec0")).unwrap();
        fs::create_dir_all(root.join("sys/class/media/media0")).unwrap();
        fs::create_dir_all(root.join("sys/class/block/nbd0")).unwrap();
        fs::create_dir_all(root.join("sys/class/block/nbd0p1")).unwrap();
        fs::create_dir_all(root.join("sys/class/vfio/vfio0")).unwrap();
        fs::create_dir_all(root.join("sys/class/vfio-dev/vfio1")).unwrap();
        fs::create_dir_all(root.join("sys/bus/mdev/devices/mdev0")).unwrap();
        fs::create_dir_all(root.join("sys/class/misc/vhost-net")).unwrap();
        fs::create_dir_all(root.join("sys/class/fc_host/host0")).unwrap();
        fs::create_dir_all(root.join("sys/class/accel/accel0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/vdpa/devices/vdpa0")).unwrap();
        fs::create_dir_all(root.join("sys/class/uio/uio0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/auxiliary/devices/intel_vsec.telemetry.0")).unwrap();
        fs::create_dir_all(root.join("sys/class/usbmon/usbmon0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/counter/devices/counter0")).unwrap();
        fs::create_dir_all(root.join("sys/class/drm_dp_aux_dev/drm_dp_aux0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/mhi/devices/mhi0")).unwrap();
        fs::create_dir_all(root.join("sys/class/ipmi/ipmi0")).unwrap();
        fs::create_dir_all(root.join("sys/class/usb_role/dual-role-switch0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/i3c/devices/i3c-0")).unwrap();
        fs::create_dir_all(root.join("sys/class/vduse/vduse0")).unwrap();
        fs::create_dir_all(root.join("sys/class/mux/muxchip0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/soundwire/devices/sdw-master-0")).unwrap();
        fs::create_dir_all(root.join("sys/class/rc/rc0")).unwrap();
        fs::create_dir_all(root.join("sys/class/stm/dummy_stm.0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/peci/devices/0-30")).unwrap();
        fs::create_dir_all(root.join("sys/class/wakeup/wakeup0")).unwrap();
        fs::create_dir_all(root.join("sys/class/msr/msr0")).unwrap();
        fs::create_dir_all(root.join("sys/class/dpll/dev0")).unwrap();
        fs::create_dir_all(root.join("sys/class/iommu/dmar0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/hid/devices/0003:046D:C52B.0001")).unwrap();
        fs::create_dir_all(root.join("sys/bus/memory/devices/memory0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/firewire/devices/fw0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/greybus/devices/1-1")).unwrap();
        fs::create_dir_all(root.join("sys/bus/rapidio/devices/00:00")).unwrap();
        fs::create_dir_all(root.join("sys/bus/ulpi/devices/ulpi-1")).unwrap();
        fs::create_dir_all(root.join("sys/bus/spmi/devices/0-00")).unwrap();
        fs::create_dir_all(root.join("sys/class/pci_epc/pci_epc0")).unwrap();
        fs::create_dir_all(root.join("sys/class/ptp/ptp0")).unwrap();
        fs::create_dir_all(root.join("sys/class/pps/pps0")).unwrap();
        fs::create_dir_all(root.join("sys/class/tpm/tpm0")).unwrap();
        fs::create_dir_all(root.join("sys/class/firmware-attributes/thinklmi")).unwrap();
        fs::create_dir_all(root.join("sys/bus/pci-epf/devices/pci_epf_test.0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/slimbus/devices/slim-0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/memstick/devices/ms0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/siox/devices/siox-0-0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/hsi/devices/hsi_char.0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/amba/devices/e0000000.uart")).unwrap();
        fs::create_dir_all(root.join("sys/bus/fsi/devices/00:00:00:06")).unwrap();
        fs::create_dir_all(root.join("sys/class/ppdev/parport0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/pcmcia/devices/0.0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/vmbus/devices/vmbus_0_1")).unwrap();
        fs::create_dir_all(root.join("sys/bus/bcma/devices/bcma0:0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/intel_th/devices/0-gth")).unwrap();
        fs::create_dir_all(root.join("sys/bus/coresight/devices/tmc_etf0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/ntb/devices/ntb0")).unwrap();
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
        assert_eq!(r.macvtap, vec!["tap0".to_string()]);
        assert_eq!(r.nvme_generic, vec!["ng0n1".to_string()]);
        assert_eq!(r.tun, vec!["tun".to_string()]);
        assert!(r.nvme_fabrics.is_empty());
        assert_eq!(r.iscsi_endpoint, vec!["ep0".to_string()]);
        assert_eq!(r.iscsi_iface, vec!["iface0".to_string()]);
        assert_eq!(r.iscsi_connection, vec!["connection0".to_string()]);
        assert_eq!(r.container, vec!["ACPI0004:00".to_string()]);
        assert_eq!(r.iscsi_flashnode, vec!["flashnode0".to_string()]);
        assert_eq!(r.nd, vec!["nmem0".to_string()]);
        assert_eq!(r.dma_heap, vec!["system".to_string()]);
        assert_eq!(r.cxl, vec!["mem0".to_string()]);
        assert_eq!(r.devfreq, vec!["devfreq0".to_string()]);
        assert_eq!(
            r.fpga,
            vec![
                "fpga_bridge/br0".to_string(),
                "fpga_manager/fpga0".to_string(),
                "fpga_region/region0".to_string(),
            ]
        );
        assert_eq!(r.gnss, vec!["gnss0".to_string()]);
        assert_eq!(r.rpmsg, vec!["rpmsg0".to_string()]);
        assert_eq!(r.devcoredump, vec!["devcd0".to_string()]);
        assert_eq!(r.scsi_disk, vec!["0:0:0:0".to_string()]);
        assert_eq!(r.scsi_tape, vec!["st0".to_string()]);
        assert_eq!(r.graphics, vec!["fb0".to_string()]);
        assert_eq!(r.cec, vec!["cec0".to_string()]);
        assert_eq!(r.media, vec!["media0".to_string()]);
        assert_eq!(r.nbd, vec!["nbd0".to_string()]);
        assert_eq!(
            r.vfio,
            vec!["vfio-dev/vfio1".to_string(), "vfio/vfio0".to_string()]
        );
        assert_eq!(r.mdev, vec!["mdev0".to_string()]);
        assert_eq!(r.vhost, vec!["vhost-net".to_string()]);
        assert_eq!(r.fc, vec!["fc_host/host0".to_string()]);
        assert_eq!(r.accel, vec!["accel0".to_string()]);
        assert_eq!(r.vdpa, vec!["vdpa0".to_string()]);
        assert_eq!(r.uio, vec!["uio0".to_string()]);
        assert_eq!(r.auxiliary, vec!["intel_vsec.telemetry.0".to_string()]);
        assert_eq!(r.usbmon, vec!["usbmon0".to_string()]);
        assert_eq!(r.counter, vec!["counter0".to_string()]);
        assert_eq!(r.drm_dp_aux_dev, vec!["drm_dp_aux0".to_string()]);
        assert_eq!(r.mhi, vec!["mhi0".to_string()]);
        assert_eq!(r.ipmi, vec!["ipmi/ipmi0".to_string()]);
        assert_eq!(r.usb_role, vec!["dual-role-switch0".to_string()]);
        assert_eq!(r.i3c, vec!["i3c-0".to_string()]);
        assert_eq!(r.vduse, vec!["vduse0".to_string()]);
        assert_eq!(r.mux, vec!["muxchip0".to_string()]);
        assert_eq!(r.soundwire, vec!["sdw-master-0".to_string()]);
        assert_eq!(r.rc, vec!["rc0".to_string()]);
        assert_eq!(r.stm, vec!["stm/dummy_stm.0".to_string()]);
        assert_eq!(r.peci, vec!["0-30".to_string()]);
        assert_eq!(r.wakeup, vec!["wakeup0".to_string()]);
        assert_eq!(r.msr, vec!["msr0".to_string()]);
        assert_eq!(r.dpll, vec!["dev0".to_string()]);
        assert_eq!(r.iommu, vec!["dmar0".to_string()]);
        assert_eq!(r.hid, vec!["0003:046D:C52B.0001".to_string()]);
        assert_eq!(r.memory, vec!["memory0".to_string()]);
        assert_eq!(r.firewire, vec!["fw0".to_string()]);
        assert_eq!(r.greybus, vec!["1-1".to_string()]);
        assert_eq!(r.rapidio, vec!["00:00".to_string()]);
        assert_eq!(r.ulpi, vec!["ulpi-1".to_string()]);
        assert_eq!(r.spmi, vec!["0-00".to_string()]);
        assert_eq!(r.pci_epc, vec!["pci_epc0".to_string()]);
        assert_eq!(r.ptp, vec!["ptp0".to_string()]);
        assert_eq!(r.pps, vec!["pps0".to_string()]);
        assert_eq!(r.tpm, vec!["tpm/tpm0".to_string()]);
        assert_eq!(r.firmware_attributes, vec!["thinklmi".to_string()]);
        assert_eq!(r.pci_epf, vec!["pci_epf_test.0".to_string()]);
        assert_eq!(r.slimbus, vec!["slim-0".to_string()]);
        assert_eq!(r.memstick, vec!["ms0".to_string()]);
        assert_eq!(r.siox, vec!["siox-0-0".to_string()]);
        assert_eq!(r.hsi, vec!["hsi_char.0".to_string()]);
        assert_eq!(r.amba, vec!["e0000000.uart".to_string()]);
        assert_eq!(r.fsi, vec!["00:00:00:06".to_string()]);
        assert_eq!(r.ppdev, vec!["parport0".to_string()]);
        assert_eq!(r.pcmcia, vec!["0.0".to_string()]);
        assert_eq!(r.vmbus, vec!["vmbus_0_1".to_string()]);
        assert_eq!(r.bcma, vec!["bcma0:0".to_string()]);
        assert_eq!(r.intel_th, vec!["0-gth".to_string()]);
        assert_eq!(r.coresight, vec!["tmc_etf0".to_string()]);
        assert_eq!(r.ntb, vec!["ntb0".to_string()]);
        assert_eq!(r.spi, vec!["spi0.0".to_string()]);
        assert_eq!(r.serio, vec!["serio0".to_string()]);
        assert!(
            r.notes
                .iter()
                .any(|n| n.contains("typec") && n.contains("nvme-fabrics")),
            "missing typec/udc/dax/wmi/ubi/nvme-fabrics should share one note: {:?}",
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
            r.notes
                .iter()
                .any(|n| n.contains("权限") || n.contains("失败")),
            "{:?}",
            r.notes
        );
        assert!(!r.notes.iter().any(|n| n.contains("无 rfkill")));
    }

    #[test]
    fn tun_from_dev_node_without_misc_class() {
        let root = std::env::temp_dir().join(format!("aida-buses-tun-{}", std::process::id()));
        fs::create_dir_all(root.join("dev/net")).unwrap();
        fs::write(root.join("dev/net/tun"), "").unwrap();
        fs::create_dir_all(root.join("sys/class")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.tun, vec!["tun".to_string()]);
        assert!(
            !r.notes
                .iter()
                .any(|n| n.contains("无") && n.contains("tun")),
            "present tun via /dev/net/tun must not be listed as missing: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_misc_tun_is_not_missing() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aida-buses-tun-deny-{}", std::process::id()));
        let tun = root.join("sys/class/misc/tun");
        fs::create_dir_all(&tun).unwrap();
        fs::set_permissions(&tun, fs::Permissions::from_mode(0o000)).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let _ = fs::set_permissions(&tun, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&root);
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        assert!(
            r.notes
                .iter()
                .any(|n| n.contains("权限") || n.contains("失败")),
            "{:?}",
            r.notes
        );
        assert!(
            !r.notes
                .iter()
                .any(|n| n.contains("无") && n.contains("tun")),
            "denied misc/tun must not look like missing: {:?}",
            r.notes
        );
    }

    #[test]
    fn flashnode_from_class_without_bus() {
        let root =
            std::env::temp_dir().join(format!("aida-flashnode-class-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/iscsi_flashnode/flashnode_sess-0:0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.iscsi_flashnode, vec!["flashnode_sess-0:0".to_string()]);
        assert!(
            r.notes.iter().all(|n| !n.contains("iscsi_flashnode")),
            "class flashnode must not be reported missing: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cxl_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-cxl-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/cxl/devices/mem0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.cxl, vec!["mem0".to_string()]);
        assert!(
            r.notes.iter().all(|n| !n.contains("cxl")),
            "bus cxl must not be reported missing: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fpga_from_framework_classes_without_class_fpga() {
        let root = std::env::temp_dir().join(format!("aida-fpga-class-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/fpga_manager/fpga0")).unwrap();
        fs::create_dir_all(root.join("sys/class/fpga_bridge/br0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(
            r.fpga,
            vec![
                "fpga_bridge/br0".to_string(),
                "fpga_manager/fpga0".to_string(),
            ]
        );
        assert!(
            r.notes.iter().all(|n| !n.contains("fpga")),
            "fpga_manager/bridge must not be reported missing: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn nbd_from_block_without_class_nbd() {
        let root = std::env::temp_dir().join(format!("aida-nbd-block-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/block/nbd0")).unwrap();
        fs::create_dir_all(root.join("sys/class/block/nbd0p1")).unwrap();
        fs::create_dir_all(root.join("sys/class/block/vda")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.nbd, vec!["nbd0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "nbd"))),
            "class/block nbd0 must not be leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refresh_devcoredump_picks_up_and_drops_nodes() {
        let root = std::env::temp_dir().join(format!("aida-devcd-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let mut r = collect(&ctx);
        assert!(r.devcoredump.is_empty());
        assert!(
            r.notes.iter().any(|n| leftover_note(n)
                .is_some_and(|inner| inner.split('/').any(|s| s == "devcoredump"))),
            "missing class should be leftover: {:?}",
            r.notes
        );
        fs::create_dir_all(root.join("sys/class/devcoredump/devcd0")).unwrap();
        refresh_devcoredump(&mut r, &ctx);
        assert_eq!(r.devcoredump, vec!["devcd0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n)
                .is_none_or(|inner| inner.split('/').all(|s| s != "devcoredump"))),
            "present class must not stay leftover: {:?}",
            r.notes
        );
        fs::remove_dir_all(root.join("sys/class/devcoredump")).unwrap();
        refresh_devcoredump(&mut r, &ctx);
        assert!(r.devcoredump.is_empty());
        assert!(
            r.notes.iter().any(|n| leftover_note(n)
                .is_some_and(|inner| inner.split('/').any(|s| s == "devcoredump"))),
            "removed class should return to leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vfio_from_vfio_dev_without_legacy_class() {
        let root = std::env::temp_dir().join(format!("aida-vfio-dev-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/vfio-dev/vfio0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.vfio, vec!["vfio-dev/vfio0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "vfio"))),
            "present vfio-dev must not leftover vfio: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mdev_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-mdev-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/mdev/devices/83b8f4f2-509f-4275-a0a0-000000000001"))
            .unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(
            r.mdev,
            vec!["83b8f4f2-509f-4275-a0a0-000000000001".to_string()]
        );
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "mdev"))),
            "bus/mdev must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vhost_from_misc_without_class_vhost() {
        let root = std::env::temp_dir().join(format!("aida-vhost-misc-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/misc/vhost-net")).unwrap();
        fs::create_dir_all(root.join("sys/class/misc/vhost-vsock")).unwrap();
        fs::create_dir_all(root.join("sys/class/vhost/ignored")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(
            r.vhost,
            vec!["vhost-net".to_string(), "vhost-vsock".to_string()]
        );
        assert!(
            r.notes.iter().all(
                |n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "vhost"))
            ),
            "misc vhost-* must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vhost_from_dev_node_without_misc() {
        let root = std::env::temp_dir().join(format!("aida-vhost-dev-{}", std::process::id()));
        fs::create_dir_all(root.join("dev")).unwrap();
        fs::write(root.join("dev/vhost-vdpa"), b"").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.vhost, vec!["vhost-vdpa".to_string()]);
        assert!(
            r.notes.iter().all(
                |n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "vhost"))
            ),
            "/dev/vhost-* must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fake_class_vhost_still_leftover() {
        let root = std::env::temp_dir().join(format!("aida-vhost-fake-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/vhost/vhost0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert!(r.vhost.is_empty());
        assert!(
            r.notes
                .iter()
                .any(|n| leftover_note(n)
                    .is_some_and(|inner| inner.split('/').any(|s| s == "vhost"))),
            "class/vhost is not the ABI and should leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vdpa_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-vdpa-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/vdpa/devices/vdpa0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.vdpa, vec!["vdpa0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "vdpa"))),
            "bus/vdpa must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fc_from_class_hosts_without_unified_fc() {
        let root = std::env::temp_dir().join(format!("aida-fc-host-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/fc_host/host2")).unwrap();
        fs::create_dir_all(root.join("sys/class/fc_remote_ports/rport-2:0-0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(
            r.fc,
            vec![
                "fc_host/host2".to_string(),
                "fc_remote_ports/rport-2:0-0".to_string()
            ]
        );
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "fc"))),
            "fc_host must not leftover fc: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vdpa_from_class_without_bus() {
        let root = std::env::temp_dir().join(format!("aida-vdpa-class-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/vdpa/vdpa1")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.vdpa, vec!["vdpa1".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "vdpa"))),
            "class/vdpa must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_fc_accel_vdpa_when_classes_missing() {
        let root =
            std::env::temp_dir().join(format!("aida-fc-accel-vdpa-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"fc") && labels.contains(&"accel") && labels.contains(&"vdpa"),
            "missing fc/accel/vdpa must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn auxiliary_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-aux-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/auxiliary/devices/mlx5_core.eth.0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.auxiliary, vec!["mlx5_core.eth.0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n)
                    .is_none_or(|inner| inner.split('/').all(|s| s != "auxiliary"))),
            "bus/auxiliary must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_uio_aux_usbmon_when_classes_missing() {
        let root =
            std::env::temp_dir().join(format!("aida-uio-aux-usbmon-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"uio") && labels.contains(&"auxiliary") && labels.contains(&"usbmon"),
            "missing uio/auxiliary/usbmon must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn mhi_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-mhi-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/mhi/devices/mhi0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.mhi, vec!["mhi0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "mhi"))),
            "bus/mhi must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn counter_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-counter-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/counter/devices/counter0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.counter, vec!["counter0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n)
                .is_none_or(|inner| inner.split('/').all(|s| s != "counter"))),
            "bus/counter must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_counter_dpaux_mhi_when_classes_missing() {
        let root = std::env::temp_dir().join(format!(
            "aida-counter-dpaux-mhi-miss-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"counter")
                && labels.contains(&"drm_dp_aux_dev")
                && labels.contains(&"mhi"),
            "missing counter/drm_dp_aux_dev/mhi must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn i3c_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-i3c-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/i3c/devices/i3c-0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.i3c, vec!["i3c-0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "i3c"))),
            "bus/i3c must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ipmi_bmc_without_msghandler_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-ipmi-bmc-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/ipmi_bmc/ipmi-bmc0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ipmi, vec!["ipmi_bmc/ipmi-bmc0".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "ipmi"))),
            "class/ipmi_bmc must not leftover ipmi: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_ipmi_usbrole_i3c_when_classes_missing() {
        let root = std::env::temp_dir().join(format!(
            "aida-ipmi-usbrole-i3c-miss-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"ipmi") && labels.contains(&"usb_role") && labels.contains(&"i3c"),
            "missing ipmi/usb_role/i3c must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn soundwire_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-sdw-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/soundwire/devices/sdw-master-0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.soundwire, vec!["sdw-master-0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n)
                .is_none_or(|inner| inner.split('/').all(|s| s != "soundwire"))),
            "bus/soundwire must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_vduse_mux_soundwire_when_classes_missing() {
        let root = std::env::temp_dir().join(format!(
            "aida-vduse-mux-sdw-miss-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"vduse")
                && labels.contains(&"mux")
                && labels.contains(&"soundwire"),
            "missing vduse/mux/soundwire must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn peci_from_bus_without_class() {
        let root = std::env::temp_dir().join(format!("aida-peci-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/peci/devices/0-30")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.peci, vec!["0-30".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "peci"))),
            "bus/peci must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn stm_source_without_stm_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-stm-source-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/stm_source/stm_console")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.stm, vec!["stm_source/stm_console".to_string()]);
        assert!(
            r.notes
                .iter()
                .all(|n| leftover_note(n).is_none_or(|inner| inner.split('/').all(|s| s != "stm"))),
            "class/stm_source must not leftover stm: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_rc_stm_peci_when_classes_missing() {
        let root = std::env::temp_dir().join(format!("aida-rc-stm-peci-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"rc") && labels.contains(&"stm") && labels.contains(&"peci"),
            "missing rc/stm/peci must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_wakeup_msr_dpll_when_classes_missing() {
        let root =
            std::env::temp_dir().join(format!("aida-wakeup-msr-dpll-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"wakeup") && labels.contains(&"msr") && labels.contains(&"dpll"),
            "missing wakeup/msr/dpll must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn wakeup_msr_present_are_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-wakeup-msr-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/wakeup/wakeup3")).unwrap();
        fs::create_dir_all(root.join("sys/class/msr/msr1")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.wakeup, vec!["wakeup3".to_string()]);
        assert_eq!(r.msr, vec!["msr1".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "wakeup" && s != "msr")
            })),
            "present wakeup/msr must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_iommu_hid_memory_when_classes_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-iommu-hid-mem-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"iommu") && labels.contains(&"hid") && labels.contains(&"memory"),
            "missing iommu/hid/memory must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn iommu_hid_memory_present_are_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-iommu-hid-mem-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/iommu/dmar0")).unwrap();
        fs::create_dir_all(root.join("sys/bus/hid/devices/0003:0001:0001.0001")).unwrap();
        fs::create_dir_all(root.join("sys/bus/memory/devices/memory3")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.iommu, vec!["dmar0".to_string()]);
        assert_eq!(r.hid, vec!["0003:0001:0001.0001".to_string()]);
        assert_eq!(r.memory, vec!["memory3".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner
                    .split('/')
                    .all(|s| s != "iommu" && s != "hid" && s != "memory")
            })),
            "present iommu/hid/memory must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn memory_block_names_capped_at_eight() {
        let root = std::env::temp_dir().join(format!("aida-mem-cap-{}", std::process::id()));
        for i in 0..12 {
            fs::create_dir_all(root.join(format!("sys/bus/memory/devices/memory{i}"))).unwrap();
        }
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.memory.len(), 8, "memory blocks must cap at 8: {:?}", r.memory);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "memory")
            })),
            "present memory bus must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_firewire_greybus_rapidio_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-fw-gb-rio-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"firewire")
                && labels.contains(&"greybus")
                && labels.contains(&"rapidio"),
            "missing firewire/greybus/rapidio must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn firewire_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-fw-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/firewire/devices/fw1")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.firewire, vec!["fw1".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "firewire")
            })),
            "present firewire must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn greybus_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-gb-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/greybus/devices/1-2")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.greybus, vec!["1-2".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "greybus")
            })),
            "present greybus must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_ulpi_spmi_pciepc_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-ulpi-spmi-epc-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"ulpi")
                && labels.contains(&"spmi")
                && labels.contains(&"pci_epc"),
            "missing ulpi/spmi/pci_epc must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ulpi_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-ulpi-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/ulpi/devices/phy0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ulpi, vec!["phy0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "ulpi")
            })),
            "present ulpi must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_ptp_pps_tpm_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-ptp-pps-tpm-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"ptp") && labels.contains(&"pps") && labels.contains(&"tpm"),
            "missing ptp/pps/tpm must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ptp_from_class_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-ptp-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/ptp/ptp0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ptp, vec!["ptp0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "ptp")
            })),
            "present ptp must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tpmrm_without_tpm_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-tpmrm-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/tpmrm/tpmrm0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.tpm, vec!["tpmrm/tpmrm0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "tpm")
            })),
            "class/tpmrm must not leftover tpm: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_fwattr_epf_slimbus_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-fwattr-epf-slim-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"firmware_attributes")
                && labels.contains(&"pci_epf")
                && labels.contains(&"slimbus"),
            "missing firmware_attributes/pci_epf/slimbus must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn firmware_attributes_hyphen_class_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-fwattr-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/firmware-attributes/thinklmi")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.firmware_attributes, vec!["thinklmi".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "firmware_attributes")
            })),
            "class/firmware-attributes must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn firmware_attributes_underscore_class_still_leftover() {
        let root = std::env::temp_dir().join(format!("aida-fwattr-underscore-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/firmware_attributes/thinklmi")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert!(r.firmware_attributes.is_empty());
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        assert!(
            inner.split('/').any(|s| s == "firmware_attributes"),
            "underscore class is not the ABI and should leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn slimbus_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-slimbus-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/slimbus/devices/slim-0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.slimbus, vec!["slim-0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "slimbus")
            })),
            "present slimbus must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_memstick_siox_hsi_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-memstick-siox-hsi-miss-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class")).unwrap();
        fs::create_dir_all(root.join("sys/bus")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"memstick") && labels.contains(&"siox") && labels.contains(&"hsi"),
            "missing memstick/siox/hsi must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn memstick_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-memstick-present-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/memstick/devices/ms0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.memstick, vec!["ms0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "memstick")
            })),
            "present memstick must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn memstick_host_class_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-memstick-host-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/memstick_host/memstick0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.memstick, vec!["memstick0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "memstick")
            })),
            "class/memstick_host must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn memstick_empty_bus_falls_back_to_host_class() {
        let root = std::env::temp_dir().join(format!("aida-memstick-empty-bus-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/bus/memstick/devices")).unwrap();
        fs::create_dir_all(root.join("sys/class/memstick_host/memstick0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.memstick, vec!["memstick0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "memstick")
            })),
            "empty bus plus class/memstick_host must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_amba_fsi_ppdev_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-amba-fsi-ppdev-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"amba") && labels.contains(&"fsi") && labels.contains(&"ppdev"),
            "missing amba/fsi/ppdev must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn amba_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-amba-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/amba/devices/e0000000.uart")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.amba, vec!["e0000000.uart".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "amba")
            })),
            "present amba must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fsi_master_class_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-fsi-master-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class/fsi-master/fsi0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.fsi, vec!["fsi0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "fsi")
            })),
            "class/fsi-master must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fsi_underscore_class_still_leftover() {
        let root = std::env::temp_dir().join(format!("aida-fsi-underscore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class/fsi_master/fsi0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert!(r.fsi.is_empty());
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        assert!(
            inner.split('/').any(|s| s == "fsi"),
            "underscore class/fsi_master must still leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fsi_empty_bus_falls_back_to_master_class() {
        let root = std::env::temp_dir().join(format!("aida-fsi-empty-bus-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/fsi/devices")).unwrap();
        fs::create_dir_all(root.join("sys/class/fsi-master/fsi0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.fsi, vec!["fsi0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "fsi")
            })),
            "empty bus plus class/fsi-master must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ppdev_class_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-ppdev-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class/ppdev/parport0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ppdev, vec!["parport0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "ppdev")
            })),
            "present ppdev must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_pcmcia_vmbus_bcma_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-pcmcia-vmbus-bcma-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"pcmcia") && labels.contains(&"vmbus") && labels.contains(&"bcma"),
            "missing pcmcia/vmbus/bcma must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pcmcia_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-pcmcia-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/pcmcia/devices/0.0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.pcmcia, vec!["0.0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "pcmcia")
            })),
            "present pcmcia must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vmbus_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-vmbus-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/vmbus/devices/vmbus_0_1")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.vmbus, vec!["vmbus_0_1".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "vmbus")
            })),
            "present vmbus must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bcma_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-bcma-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/bcma/devices/bcma0:0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.bcma, vec!["bcma0:0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "bcma")
            })),
            "present bcma must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_intel_th_coresight_ntb_when_missing() {
        let root = std::env::temp_dir()
            .join(format!("aida-th-coresight-ntb-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let inner = r.notes.iter().find_map(|n| leftover_note(n)).unwrap_or("");
        let labels: Vec<&str> = inner.split('/').collect();
        assert!(
            labels.contains(&"intel_th")
                && labels.contains(&"coresight")
                && labels.contains(&"ntb"),
            "missing intel_th/coresight/ntb must leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn intel_th_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-intel-th-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/intel_th/devices/0-gth")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.intel_th, vec!["0-gth".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "intel_th")
            })),
            "present intel_th must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coresight_from_bus_is_not_leftover() {
        let root =
            std::env::temp_dir().join(format!("aida-coresight-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/coresight/devices/tmc_etf0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.coresight, vec!["tmc_etf0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "coresight")
            })),
            "present coresight must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ntb_from_bus_is_not_leftover() {
        let root = std::env::temp_dir().join(format!("aida-ntb-present-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/bus/ntb/devices/ntb0")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.ntb, vec!["ntb0".to_string()]);
        assert!(
            r.notes.iter().all(|n| leftover_note(n).is_none_or(|inner| {
                inner.split('/').all(|s| s != "ntb")
            })),
            "present ntb must not leftover: {:?}",
            r.notes
        );
        let _ = fs::remove_dir_all(&root);
    }
}
