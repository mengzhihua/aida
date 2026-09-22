//! DMI / SMBIOS。
//!
//! 优先读 `/sys/class/dmi/id/*`（内核已解码的字段），无权限或缺目录时
//! 再尝试解析 `/sys/firmware/dmi/tables/DMI`。不调用 `dmidecode`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct DmiInfo {
    pub bios_vendor: Sample<String>,
    pub bios_version: Sample<String>,
    pub bios_date: Sample<String>,
    pub sys_vendor: Sample<String>,
    pub product_name: Sample<String>,
    pub product_version: Sample<String>,
    pub product_serial: Sample<String>,
    pub product_uuid: Sample<String>,
    pub product_family: Sample<String>,
    pub board_vendor: Sample<String>,
    pub board_name: Sample<String>,
    pub board_version: Sample<String>,
    pub board_serial: Sample<String>,
    pub chassis_vendor: Sample<String>,
    pub chassis_type: Sample<String>,
    pub smbios_records: Vec<SmbiosRecord>,
    /// SMBIOS Type 16 物理内存阵列（容量上限 / ECC / 槽位数）。
    pub memory_arrays: Vec<MemoryArray>,
    pub memory_devices: Vec<MemoryDevice>,
    /// SMBIOS Type 4 处理器（插座 / 额定频率 / 核心数）。对标 AIDA64 CPU/主板。
    pub processors: Vec<ProcessorDevice>,
    /// SMBIOS Type 7 缓存（与 sysfs cpu0/cache 互补）。
    pub caches: Vec<CacheDevice>,
    /// SMBIOS Type 9 系统插槽（PCI/PCIe）。
    pub slots: Vec<SystemSlot>,
    /// SMBIOS Type 8 端口连接器（USB / RJ-45 等，对标 AIDA64 主板端口）。
    pub ports: Vec<PortConnector>,
    /// SMBIOS Type 41 板载设备（网卡 / SATA / NVMe）。
    pub onboard: Vec<OnboardDevice>,
    /// SMBIOS Type 39 电源（额定功率 / 厂商）。
    pub power_supplies: Vec<PowerSupply>,
    /// SMBIOS Type 11 OEM 字符串。
    pub oem_strings: Vec<String>,
    /// SMBIOS Type 13 当前 BIOS 语言。
    pub bios_language: Option<String>,
    /// Type 13 可安装语言（最多 16）。
    pub bios_languages: Vec<String>,
    /// SMBIOS Type 32 系统启动状态。
    pub boot_status: Option<String>,
    /// SMBIOS Type 43 TPM 设备（与 sysfs class/tpm 互补）。
    pub tpm_devices: Vec<TpmSmbios>,
    /// SMBIOS Type 12 系统配置选项（跳线/开关字符串）。
    pub config_options: Vec<String>,
    /// SMBIOS Type 22 便携电池。
    pub batteries: Vec<PortableBattery>,
    /// SMBIOS Type 23 系统复位 / 看门狗。
    pub system_reset: Option<SystemReset>,
    /// SMBIOS Type 24 硬件安全（开机/管理员密码状态）。
    pub hardware_security: Option<HardwareSecurity>,
    /// SMBIOS Type 26 电压探头（毫伏；`0x8000` 未知）。
    pub voltage_probes: Vec<VoltageProbe>,
    /// SMBIOS Type 27 冷却装置（风扇/热管；转速 `0x8000` 未知）。
    pub cooling_devices: Vec<CoolingDevice>,
    /// SMBIOS Type 28 温度探头（十分之一摄氏度；`0x8000` 未知）。
    pub temperature_probes: Vec<TemperatureProbe>,
    /// SMBIOS Type 3 机箱（类型 / 序列号 / 高度；对标 AIDA64 主板机箱）。
    pub chassis: Option<ChassisEnclosure>,
    /// SMBIOS Type 25 下次定时开机（BCD；全 0 未排程）。
    pub power_controls: Option<SystemPowerControls>,
    /// SMBIOS Type 29 电流探头（毫安；`0x8000` 未知）。
    pub current_probes: Vec<CurrentProbe>,
    /// SMBIOS Type 38 IPMI 设备（KCS/SMIC/BT/SSIF）。
    pub ipmi_devices: Vec<IpmiDevice>,
    /// SMBIOS Type 18 32 位内存错误（ECC；`0x80000000` 地址未知）。
    pub memory_errors: Vec<MemoryError32>,
    /// SMBIOS Type 19 物理内存阵列映射地址（对标 AIDA64 内存映射）。
    pub mapped_addresses: Vec<MemoryMappedAddress>,
    /// SMBIOS Type 21 板载指针设备（鼠标/触控板）。
    pub pointing_devices: Vec<PointingDevice>,
    /// Type 0 BIOS ROM 大小（KiB）。
    pub bios_rom_kb: Option<u64>,
    /// Type 0 BIOS 版本号 major.minor（有则显示）。
    pub bios_release: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SmbiosRecord {
    pub kind: u8,
    pub kind_name: String,
    pub handle: u16,
    pub strings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryArray {
    pub location: Option<String>,
    pub ecc: Option<String>,
    pub max_capacity_mb: Option<u64>,
    pub devices: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryDevice {
    pub locator: Option<String>,
    pub bank: Option<String>,
    /// Size=0 未安装；Size=0xFFFF 已安装但容量未知（`installed` 仍为 true）。
    pub installed: bool,
    pub size_mb: Option<u64>,
    pub r#type: Option<String>,
    pub form_factor: Option<String>,
    pub speed_mts: Option<u32>,
    pub configured_mts: Option<u32>,
    pub data_width: Option<u16>,
    pub total_width: Option<u16>,
    pub rank: Option<u8>,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub part: Option<String>,
}

impl MemoryDevice {
    pub fn size_label(&self) -> String {
        if !self.installed {
            "empty".into()
        } else {
            self.size_mb
                .map(|n| format!("{n} MB"))
                .unwrap_or_else(|| "unknown".into())
        }
    }
}

pub fn collect(ctx: &ProbeCtx) -> DmiInfo {
    let id = ctx.sys_path("class/dmi/id");
    let id_exists = access::dir_exists(&id);

    let mut info = DmiInfo {
        bios_vendor: field(&id, "bios_vendor", id_exists),
        bios_version: field(&id, "bios_version", id_exists),
        bios_date: field(&id, "bios_date", id_exists),
        sys_vendor: field(&id, "sys_vendor", id_exists),
        product_name: field(&id, "product_name", id_exists),
        product_version: field(&id, "product_version", id_exists),
        product_serial: field(&id, "product_serial", id_exists),
        product_uuid: field(&id, "product_uuid", id_exists),
        product_family: field(&id, "product_family", id_exists),
        board_vendor: field(&id, "board_vendor", id_exists),
        board_name: field(&id, "board_name", id_exists),
        board_version: field(&id, "board_version", id_exists),
        board_serial: field(&id, "board_serial", id_exists),
        chassis_vendor: field(&id, "chassis_vendor", id_exists),
        chassis_type: field(&id, "chassis_type", id_exists),
        smbios_records: Vec::new(),
        memory_arrays: Vec::new(),
        memory_devices: Vec::new(),
        processors: Vec::new(),
        caches: Vec::new(),
        slots: Vec::new(),
        ports: Vec::new(),
        onboard: Vec::new(),
        power_supplies: Vec::new(),
        oem_strings: Vec::new(),
        bios_language: None,
        bios_languages: Vec::new(),
        boot_status: None,
        tpm_devices: Vec::new(),
        config_options: Vec::new(),
        batteries: Vec::new(),
        system_reset: None,
        hardware_security: None,
        voltage_probes: Vec::new(),
        cooling_devices: Vec::new(),
        temperature_probes: Vec::new(),
        chassis: None,
        power_controls: None,
        current_probes: Vec::new(),
        ipmi_devices: Vec::new(),
        memory_errors: Vec::new(),
        mapped_addresses: Vec::new(),
        pointing_devices: Vec::new(),
        bios_rom_kb: None,
        bios_release: None,
        notes: Vec::new(),
    };

    let table = access::read_bytes(ctx.sys_path("firmware/dmi/tables/DMI"));
    match table.access {
        AccessKind::Ok => {
            if let Some(bytes) = table.value.as_deref() {
                let parsed = parse_smbios(bytes);
                info.memory_arrays = parsed
                    .iter()
                    .filter(|r| r.kind == 16)
                    .filter_map(|r| array_from_raw(bytes, r))
                    .collect();
                info.memory_devices = parsed
                    .iter()
                    .filter(|r| r.kind == 17)
                    .filter_map(|r| memory_from_raw(bytes, r))
                    .collect();
                info.processors = parsed
                    .iter()
                    .filter(|r| r.kind == 4)
                    .filter_map(|r| processor_from_raw(bytes, r))
                    .collect();
                info.caches = parsed
                    .iter()
                    .filter(|r| r.kind == 7)
                    .filter_map(|r| cache_from_raw(bytes, r))
                    .collect();
                info.slots = parsed
                    .iter()
                    .filter(|r| r.kind == 9)
                    .filter_map(|r| slot_from_raw(bytes, r))
                    .take(16)
                    .collect();
                info.ports = parsed
                    .iter()
                    .filter(|r| r.kind == 8)
                    .filter_map(|r| port_from_raw(bytes, r))
                    .take(16)
                    .collect();
                info.onboard = parsed
                    .iter()
                    .filter(|r| r.kind == 41)
                    .filter_map(|r| onboard_from_raw(bytes, r))
                    .take(16)
                    .collect();
                info.power_supplies = parsed
                    .iter()
                    .filter(|r| r.kind == 39)
                    .filter_map(|r| psu_from_raw(bytes, r))
                    .take(8)
                    .collect();
                info.oem_strings = parsed
                    .iter()
                    .filter(|r| r.kind == 11)
                    .flat_map(|r| oem_from_raw(bytes, r))
                    .take(16)
                    .collect();
                if let Some(lang) = parsed
                    .iter()
                    .find(|r| r.kind == 13)
                    .and_then(|r| language_from_raw(bytes, r))
                {
                    info.bios_language = lang.0;
                    info.bios_languages = lang.1;
                }
                info.boot_status = parsed
                    .iter()
                    .find(|r| r.kind == 32)
                    .and_then(|r| boot_from_raw(bytes, r));
                info.tpm_devices = parsed
                    .iter()
                    .filter(|r| r.kind == 43)
                    .filter_map(|r| tpm_from_raw(bytes, r))
                    .take(4)
                    .collect();
                info.config_options = parsed
                    .iter()
                    .filter(|r| r.kind == 12)
                    .flat_map(|r| config_from_raw(bytes, r))
                    .take(16)
                    .collect();
                info.batteries = parsed
                    .iter()
                    .filter(|r| r.kind == 22)
                    .filter_map(|r| battery_from_raw(bytes, r))
                    .take(4)
                    .collect();
                info.system_reset = parsed
                    .iter()
                    .find(|r| r.kind == 23)
                    .and_then(|r| reset_from_raw(bytes, r));
                info.hardware_security = parsed
                    .iter()
                    .find(|r| r.kind == 24)
                    .and_then(|r| hwsec_from_raw(bytes, r));
                info.voltage_probes = parsed
                    .iter()
                    .filter(|r| r.kind == 26)
                    .filter_map(|r| voltage_from_raw(bytes, r))
                    .take(8)
                    .collect();
                info.cooling_devices = parsed
                    .iter()
                    .filter(|r| r.kind == 27)
                    .filter_map(|r| cooling_from_raw(bytes, r))
                    .take(8)
                    .collect();
                info.temperature_probes = parsed
                    .iter()
                    .filter(|r| r.kind == 28)
                    .filter_map(|r| temperature_from_raw(bytes, r))
                    .take(8)
                    .collect();
                info.chassis = parsed
                    .iter()
                    .find(|r| r.kind == 3)
                    .and_then(|r| chassis_from_raw(bytes, r));
                info.power_controls = parsed
                    .iter()
                    .find(|r| r.kind == 25)
                    .and_then(|r| power_controls_from_raw(bytes, r));
                info.current_probes = parsed
                    .iter()
                    .filter(|r| r.kind == 29)
                    .filter_map(|r| current_from_raw(bytes, r))
                    .take(8)
                    .collect();
                info.ipmi_devices = parsed
                    .iter()
                    .filter(|r| r.kind == 38)
                    .filter_map(|r| ipmi_from_raw(bytes, r))
                    .take(4)
                    .collect();
                info.memory_errors = parsed
                    .iter()
                    .filter(|r| r.kind == 18)
                    .filter_map(|r| memory_error_from_raw(bytes, r))
                    .take(8)
                    .collect();
                info.mapped_addresses = parsed
                    .iter()
                    .filter(|r| r.kind == 19)
                    .filter_map(|r| mapped_from_raw(bytes, r))
                    .take(16)
                    .collect();
                info.pointing_devices = parsed
                    .iter()
                    .filter(|r| r.kind == 21)
                    .filter_map(|r| pointing_from_raw(bytes, r))
                    .take(4)
                    .collect();
                if let Some(bios) = parsed.iter().find(|r| r.kind == 0) {
                    let (rom, rel) = bios_extras(bytes, bios);
                    info.bios_rom_kb = rom;
                    info.bios_release = rel;
                }
                info.smbios_records = parsed;
                if info.sys_vendor.value.is_none() {
                    fill_from_smbios(&mut info);
                }
            }
        }
        AccessKind::PermissionDenied => {
            info.notes.push(format!(
                "SMBIOS 原始表权限不足：{}。{}",
                table.source,
                table.hint.as_deref().unwrap_or("需要 root")
            ));
        }
        AccessKind::NotFound => {
            if !id_exists {
                info.notes.push(
                    "本机未导出 /sys/class/dmi 与 /sys/firmware/dmi（常见于容器或裁剪内核）。".into(),
                );
            }
        }
        _ => {
            if let Some(h) = table.hint {
                info.notes.push(h);
            }
        }
    }

    let denied = [
        &info.product_serial,
        &info.product_uuid,
        &info.board_serial,
    ]
    .iter()
    .any(|s| s.access == AccessKind::PermissionDenied);
    if denied && !info.notes.iter().any(|n| n.contains("序列号")) {
        info.notes.push(
            "部分 DMI 字段对普通用户不可读（序列号/UUID）。界面会显示“权限不足”，而不是编造空值。"
                .into(),
        );
    }
    info
}

fn field(dir: &std::path::Path, name: &str, dir_exists: bool) -> Sample<String> {
    if !dir_exists {
        return Sample::missing(dir.join(name).display().to_string());
    }
    access::read_trimmed(dir.join(name))
}

fn fill_from_smbios(info: &mut DmiInfo) {
    // 仅在 sysfs 文本字段缺失时，用 Type 0/1/2 的字符串补齐。
    for rec in &info.smbios_records {
        match rec.kind {
            0 => {
                take_if_empty(&mut info.bios_vendor, rec.strings.first());
                take_if_empty(&mut info.bios_version, rec.strings.get(1));
                take_if_empty(&mut info.bios_date, rec.strings.get(2));
            }
            1 => {
                take_if_empty(&mut info.sys_vendor, rec.strings.first());
                take_if_empty(&mut info.product_name, rec.strings.get(1));
                take_if_empty(&mut info.product_version, rec.strings.get(2));
                take_if_empty(&mut info.product_serial, rec.strings.get(3));
            }
            2 => {
                take_if_empty(&mut info.board_vendor, rec.strings.first());
                take_if_empty(&mut info.board_name, rec.strings.get(1));
                take_if_empty(&mut info.board_version, rec.strings.get(2));
                take_if_empty(&mut info.board_serial, rec.strings.get(3));
            }
            3 => {
                take_if_empty(&mut info.chassis_vendor, rec.strings.first());
            }
            _ => {}
        }
    }
}

fn take_if_empty(slot: &mut Sample<String>, s: Option<&String>) {
    if slot.value.is_none() {
        if let Some(v) = s {
            if !v.is_empty() {
                *slot = Sample::ok(v.clone(), "smbios-table");
            }
        }
    }
}

fn kind_name(kind: u8) -> String {
    match kind {
        0 => "BIOS",
        1 => "System",
        2 => "Baseboard",
        3 => "Chassis",
        4 => "Processor",
        7 => "Cache",
        8 => "Port Connector",
        9 => "System Slot",
        11 => "OEM Strings",
        12 => "System Configuration Options",
        13 => "BIOS Language",
        22 => "Portable Battery",
        23 => "System Reset",
        24 => "Hardware Security",
        25 => "System Power Controls",
        26 => "Voltage Probe",
        27 => "Cooling Device",
        28 => "Temperature Probe",
        29 => "Electrical Current Probe",
        38 => "IPMI Device",
        16 => "Memory Array",
        17 => "Memory Device",
        18 => "32-bit Memory Error",
        19 => "Memory Mapped Address",
        21 => "Built-in Pointing Device",
        32 => "System Boot",
        39 => "Power Supply",
        41 => "Onboard Device",
        43 => "TPM Device",
        127 => "End of Table",
        n => return format!("Type {n}"),
    }
    .into()
}

/// 解析 SMBIOS 结构表（32 位 entry 指向的 _DMI_ blob）。
pub fn parse_smbios(buf: &[u8]) -> Vec<SmbiosRecord> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        let kind = buf[i];
        let length = buf[i + 1] as usize;
        if length < 4 || i + length > buf.len() {
            break;
        }
        let handle = u16::from_le_bytes([buf[i + 2], buf[i + 3]]);
        let mut s = i + length;
        let mut strings = Vec::new();
        if s >= buf.len() {
            break;
        }
        loop {
            if s >= buf.len() {
                break;
            }
            if buf[s] == 0 {
                // 空字符串或表结束：连续两个 NUL。
                if strings.is_empty() || (s + 1 < buf.len() && buf[s + 1] == 0) || s + 1 >= buf.len()
                {
                    s += if s + 1 < buf.len() && buf[s + 1] == 0 {
                        2
                    } else {
                        1
                    };
                    break;
                }
            }
            let start = s;
            while s < buf.len() && buf[s] != 0 {
                s += 1;
            }
            if s > start {
                strings.push(String::from_utf8_lossy(&buf[start..s]).into_owned());
            }
            s += 1; // skip NUL
            if s < buf.len() && buf[s] == 0 {
                s += 1;
                break;
            }
        }
        out.push(SmbiosRecord {
            kind,
            kind_name: kind_name(kind),
            handle,
            strings,
        });
        if kind == 127 {
            break;
        }
        i = s;
    }
    out
}

fn smbios_str(strings: &[String], idx: u8) -> Option<String> {
    if idx == 0 {
        return None;
    }
    strings
        .get((idx as usize).saturating_sub(1))
        .cloned()
        .filter(|s| !s.is_empty())
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessorDevice {
    pub socket: Option<String>,
    pub manufacturer: Option<String>,
    pub version: Option<String>,
    pub max_mhz: Option<u16>,
    pub current_mhz: Option<u16>,
    pub cores: Option<u16>,
    pub threads: Option<u16>,
    pub populated: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheDevice {
    pub socket: Option<String>,
    pub level: Option<u8>,
    pub kind: Option<String>,
    pub size_kb: Option<u64>,
    pub associativity: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemSlot {
    pub designation: Option<String>,
    pub kind: Option<String>,
    pub usage: Option<String>,
    pub bus: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PortConnector {
    pub internal: Option<String>,
    pub external: Option<String>,
    pub connector: Option<String>,
    pub port: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardDevice {
    pub designation: Option<String>,
    pub kind: Option<String>,
    pub enabled: bool,
    pub bus: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PowerSupply {
    pub location: Option<String>,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub max_watts: Option<u16>,
    pub present: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TpmSmbios {
    pub vendor: Option<String>,
    pub spec: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PortableBattery {
    pub location: Option<String>,
    pub manufacturer: Option<String>,
    pub name: Option<String>,
    pub chemistry: Option<String>,
    /// 设计容量 mWh；DSP0134 仅 `0` 表示未知。
    pub design_capacity_mwh: Option<u32>,
    /// 设计电压 mV；`0` 表示未知。
    pub design_voltage_mv: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemReset {
    pub enabled: bool,
    pub watchdog: bool,
    pub boot_option: String,
    pub reset_count: Option<u16>,
    pub reset_limit: Option<u16>,
    pub timeout_min: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HardwareSecurity {
    pub power_on_password: String,
    pub keyboard_password: String,
    pub administrator_password: String,
    pub front_panel_reset: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VoltageProbe {
    pub description: Option<String>,
    pub location: String,
    pub status: String,
    /// 最大可读电压 mV；DSP0134 仅 `0x8000` 表示未知。
    pub max_mv: Option<u16>,
    pub min_mv: Option<u16>,
    pub nominal_mv: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CoolingDevice {
    pub description: Option<String>,
    pub kind: String,
    pub status: String,
    pub group: u8,
    pub probe_handle: u16,
    /// 额定转速 rpm；`0x8000` 表示未知或非旋转装置。
    pub nominal_rpm: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TemperatureProbe {
    pub description: Option<String>,
    pub location: String,
    pub status: String,
    /// 最大可读温度，单位 0.1 °C；仅 `0x8000` 未知。
    pub max_tenth_c: Option<i16>,
    pub min_tenth_c: Option<i16>,
    pub nominal_tenth_c: Option<i16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChassisEnclosure {
    pub manufacturer: Option<String>,
    pub kind: String,
    pub locked: bool,
    pub version: Option<String>,
    pub serial: Option<String>,
    pub asset_tag: Option<String>,
    pub boot_state: Option<String>,
    pub power_state: Option<String>,
    pub thermal_state: Option<String>,
    pub security: Option<String>,
    /// 机架单位高度；`0` 未指定。
    pub height_u: Option<u8>,
    /// 电源线数量；`0` 未指定。
    pub power_cords: Option<u8>,
    pub sku: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemPowerControls {
    /// `None` 表示未排程（月=0）。
    pub next_power_on: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CurrentProbe {
    pub description: Option<String>,
    pub location: String,
    pub status: String,
    /// 最大可读电流 mA；仅 `0x8000` 未知。
    pub max_ma: Option<u16>,
    pub min_ma: Option<u16>,
    pub nominal_ma: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IpmiDevice {
    pub interface: String,
    pub spec: Option<String>,
    pub i2c_address: u8,
    pub nv_storage: Option<u8>,
    pub base_address: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryError32 {
    pub error_type: String,
    pub granularity: String,
    pub operation: String,
    /// Vendor Syndrome；`0` 未知。
    pub syndrome: Option<u32>,
    /// 阵列物理地址；仅 `0x80000000` 未知。
    pub array_address: Option<u32>,
    /// 设备内地址；仅 `0x80000000` 未知。
    pub device_address: Option<u32>,
    /// 错误分辨字节数；仅 `0x80000000` 未知。
    pub resolution: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryMappedAddress {
    /// 起始字节地址（DWORD KB×1024，或 2.7 扩展 QWORD）。
    pub start: Option<u64>,
    /// 结束字节地址（DWORD 含该 KB 的最后一字节，或扩展 QWORD）。
    pub end: Option<u64>,
    pub array_handle: u16,
    pub partition_width: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct PointingDevice {
    pub kind: String,
    pub interface: String,
    pub buttons: u8,
}

pub fn hex_range(start: Option<u64>, end: Option<u64>) -> String {
    match (start, end) {
        (Some(s), Some(e)) => format!("0x{s:X}-0x{e:X}"),
        (Some(s), None) => format!("0x{s:X}-—"),
        (None, Some(e)) => format!("—-0x{e:X}"),
        _ => "—".into(),
    }
}

pub fn opt_hex_u32(v: Option<u32>) -> String {
    v.map(|n| format!("0x{n:X}")).unwrap_or_else(|| "—".into())
}

pub fn tenth_c_label(v: Option<i16>) -> String {
    v.map(|n| format!("{:.1} °C", n as f32 / 10.0))
        .unwrap_or_else(|| "—".into())
}

fn next_smbios_struct(buf: &[u8], i: usize) -> Option<usize> {
    let mut s = i;
    loop {
        if s >= buf.len() {
            return Some(s);
        }
        if buf[s] == 0 {
            s += 1;
            if s >= buf.len() || buf[s] == 0 {
                return Some(s + 1);
            }
            continue;
        }
        while s < buf.len() && buf[s] != 0 {
            s += 1;
        }
        s += 1;
        if s < buf.len() && buf[s] == 0 {
            return Some(s + 1);
        }
    }
}

fn array_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<MemoryArray> {
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        let kind = buf[i];
        let length = buf[i + 1] as usize;
        if length < 4 || i + length > buf.len() {
            break;
        }
        let handle = u16::from_le_bytes([buf[i + 2], buf[i + 3]]);
        if kind == 16 && handle == rec.handle && length >= 0x0F {
            let location = match buf[i + 0x04] {
                0x03 => Some("System board".into()),
                0x04 => Some("ISA add-on".into()),
                0x05 => Some("EISA add-on".into()),
                0x06 => Some("PCI add-on".into()),
                other => Some(format!("0x{other:02x}")),
            };
            let ecc = match buf[i + 0x06] {
                0x02 => Some("Unknown".into()),
                0x03 => Some("None".into()),
                0x04 => Some("Parity".into()),
                0x05 => Some("Single-bit ECC".into()),
                0x06 => Some("Multi-bit ECC".into()),
                other => Some(format!("0x{other:02x}")),
            };
            let cap_raw = u32::from_le_bytes([
                buf[i + 0x07],
                buf[i + 0x08],
                buf[i + 0x09],
                buf[i + 0x0A],
            ]);
            let max_capacity_mb = if cap_raw == 0x8000_0000 && length >= 0x17 {
                let ext = u64::from_le_bytes([
                    buf[i + 0x0F],
                    buf[i + 0x10],
                    buf[i + 0x11],
                    buf[i + 0x12],
                    buf[i + 0x13],
                    buf[i + 0x14],
                    buf[i + 0x15],
                    buf[i + 0x16],
                ]);
                Some(ext / 1024 / 1024)
            } else if cap_raw == 0 {
                None
            } else {
                Some(cap_raw as u64 / 1024)
            };
            let devices = u16::from_le_bytes([buf[i + 0x0D], buf[i + 0x0E]]);
            return Some(MemoryArray {
                location,
                ecc,
                max_capacity_mb,
                devices: if devices == 0 { None } else { Some(devices) },
            });
        }
        i = next_smbios_struct(buf, i + length)?;
    }
    None
}

fn memory_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<MemoryDevice> {
    // Type 17（DSP0134）：0x0C size、0x0E form factor、0x10/0x11 locator 字符串号、
    // 0x12 type、0x15 speed、0x17+ 厂商/序列/料号。不要按 strings 数组下标猜。
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        let kind = buf[i];
        let length = buf[i + 1] as usize;
        if length < 4 || i + length > buf.len() {
            break;
        }
        let handle = u16::from_le_bytes([buf[i + 2], buf[i + 3]]);
        if kind == 17 && handle == rec.handle && length >= 0x0F {
            let size_raw = u16::from_le_bytes([buf[i + 0x0C], buf[i + 0x0D]]);
            let installed = size_raw != 0;
            let size_mb = if size_raw == 0 || size_raw == 0xFFFF {
                None
            } else if size_raw == 0x7FFF && length >= 0x20 {
                let ext = u32::from_le_bytes([
                    buf[i + 0x1C],
                    buf[i + 0x1D],
                    buf[i + 0x1E],
                    buf[i + 0x1F],
                ]);
                Some(ext as u64)
            } else if size_raw & 0x8000 != 0 {
                Some((size_raw & 0x7FFF) as u64 / 1024)
            } else {
                Some(size_raw as u64)
            };
            let form_factor = Some(form_factor_name(buf[i + 0x0E]).to_string());
            let locator = if length > 0x10 {
                smbios_str(&rec.strings, buf[i + 0x10])
            } else {
                None
            };
            let bank = if length > 0x11 {
                smbios_str(&rec.strings, buf[i + 0x11])
            } else {
                None
            };
            let mem_type = if length > 0x12 {
                Some(memory_type_name(buf[i + 0x12]).to_string())
            } else {
                None
            };
            let speed_mts = type17_speed_mts(buf, i, length, 0x15, 0x54);
            let manufacturer = if length > 0x17 {
                smbios_str(&rec.strings, buf[i + 0x17])
            } else {
                None
            };
            let serial = if length > 0x18 {
                smbios_str(&rec.strings, buf[i + 0x18])
            } else {
                None
            };
            let part = if length > 0x1A {
                smbios_str(&rec.strings, buf[i + 0x1A])
            } else {
                None
            };
            let total_width = if length >= 0x0A {
                let w = u16::from_le_bytes([buf[i + 0x08], buf[i + 0x09]]);
                if w == 0 || w == 0xFFFF { None } else { Some(w) }
            } else {
                None
            };
            let data_width = if length >= 0x0C {
                let w = u16::from_le_bytes([buf[i + 0x0A], buf[i + 0x0B]]);
                if w == 0 || w == 0xFFFF { None } else { Some(w) }
            } else {
                None
            };
            let rank = if length > 0x1B {
                let r = buf[i + 0x1B] & 0x0F;
                if r == 0 { None } else { Some(r) }
            } else {
                None
            };
            let configured_mts = type17_speed_mts(buf, i, length, 0x20, 0x58);
            return Some(MemoryDevice {
                locator,
                bank,
                installed,
                size_mb,
                r#type: mem_type,
                form_factor,
                speed_mts,
                configured_mts,
                data_width,
                total_width,
                rank,
                manufacturer,
                serial,
                part,
            });
        }
        i = next_smbios_struct(buf, i + length)?;
    }
    None
}

fn rec_offset(buf: &[u8], rec: &SmbiosRecord) -> Option<usize> {
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        let kind = buf[i];
        let length = buf[i + 1] as usize;
        if length < 4 || i + length > buf.len() {
            break;
        }
        let handle = u16::from_le_bytes([buf[i + 2], buf[i + 3]]);
        if kind == rec.kind && handle == rec.handle {
            return Some(i);
        }
        i = next_smbios_struct(buf, i + length)?;
    }
    None
}

fn word(buf: &[u8], i: usize, off: usize) -> u16 {
    u16::from_le_bytes([buf[i + off], buf[i + off + 1]])
}

fn qword(buf: &[u8], i: usize, off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[i + off..i + off + 8]);
    u64::from_le_bytes(b)
}

fn dword(buf: &[u8], i: usize, off: usize) -> u32 {
    u32::from_le_bytes([
        buf[i + off],
        buf[i + off + 1],
        buf[i + off + 2],
        buf[i + off + 3],
    ])
}

fn bios_extras(buf: &[u8], rec: &SmbiosRecord) -> (Option<u64>, Option<String>) {
    let Some(i) = rec_offset(buf, rec) else {
        return (None, None);
    };
    let length = buf[i + 1] as usize;
    if i + length > buf.len() || length < 0x0A {
        return (None, None);
    }
    let rom = buf[i + 0x09];
    let bios_rom_kb = if rom == 0xFF {
        if length >= 0x1A {
            // DSP0134 Extended BIOS ROM Size：bits 13:0 数值，bits 15:14 单位 00b=MiB、01b=GiB。
            let ext = word(buf, i, 0x18) as u64;
            let amount = ext & 0x3FFF;
            match ext >> 14 {
                0 if amount != 0 => Some(amount * 1024),
                1 if amount != 0 => Some(amount * 1024 * 1024),
                _ => None,
            }
        } else {
            None
        }
    } else {
        Some((rom as u64 + 1) * 64)
    };
    let bios_release = if length >= 0x16 {
        let maj = buf[i + 0x14];
        let min = buf[i + 0x15];
        if maj == 0xFF && min == 0xFF {
            None
        } else {
            Some(format!("{maj}.{min}"))
        }
    } else {
        None
    };
    (bios_rom_kb, bios_release)
}

fn processor_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<ProcessorDevice> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x1A || i + length > buf.len() {
        return None;
    }
    let max_mhz = {
        let v = word(buf, i, 0x14);
        if v == 0 { None } else { Some(v) }
    };
    let current_mhz = {
        let v = word(buf, i, 0x16);
        if v == 0 { None } else { Some(v) }
    };
    let status = buf[i + 0x18];
    let populated = status & 0x40 != 0;
    let enabled = (status & 0x07) == 0x01;
    // Core Count / Thread Count：BYTE `0xFF` 才读 3.0 WORD（`0x2A` / `0x2E`）。
    // 长度够长但 BYTE 仍有效时，WORD 可能是 0（保留），不能当未知。
    let cores = type4_count(buf, i, length, 0x23, 0x2A, 0x2C);
    let threads = type4_count(buf, i, length, 0x25, 0x2E, 0x30);
    Some(ProcessorDevice {
        socket: smbios_str(&rec.strings, buf[i + 0x04]),
        manufacturer: if length > 0x07 {
            smbios_str(&rec.strings, buf[i + 0x07])
        } else {
            None
        },
        version: if length > 0x10 {
            smbios_str(&rec.strings, buf[i + 0x10])
        } else {
            None
        },
        max_mhz,
        current_mhz,
        cores,
        threads,
        populated,
        enabled,
    })
}

fn type4_count(
    buf: &[u8],
    i: usize,
    length: usize,
    byte_off: usize,
    word_off: usize,
    word_len: usize,
) -> Option<u16> {
    if length <= byte_off {
        return None;
    }
    match buf[i + byte_off] {
        0 => None,
        0xFF => {
            if length < word_len {
                return None;
            }
            let n = word(buf, i, word_off);
            if n == 0 { None } else { Some(n) }
        }
        n => Some(n as u16),
    }
}

fn cache_size_kb(raw: u16, ext: Option<u32>) -> Option<u64> {
    if raw == 0xFFFF {
        let ext = ext?;
        if ext == 0 {
            return None;
        }
        let granules = (ext & 0x7FFF_FFFF) as u64;
        Some(if ext & 0x8000_0000 != 0 {
            granules * 64
        } else {
            granules
        })
    } else if raw == 0 {
        None
    } else {
        let granules = (raw & 0x7FFF) as u64;
        Some(if raw & 0x8000 != 0 {
            granules * 64
        } else {
            granules
        })
    }
}

fn cache_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<CacheDevice> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x13 || i + length > buf.len() {
        return None;
    }
    let cfg = word(buf, i, 0x05);
    let level = ((cfg & 0x07) + 1) as u8;
    let installed = word(buf, i, 0x09);
    let ext = if length >= 0x1B {
        Some(u32::from_le_bytes([
            buf[i + 0x17],
            buf[i + 0x18],
            buf[i + 0x19],
            buf[i + 0x1A],
        ]))
    } else {
        None
    };
    let kind = match buf[i + 0x11] {
        0x03 => Some("Instruction".into()),
        0x04 => Some("Data".into()),
        0x05 => Some("Unified".into()),
        other => Some(format!("0x{other:02x}")),
    };
    let associativity = match buf[i + 0x12] {
        0x02 => Some("Unknown".into()),
        0x03 => Some("Direct".into()),
        0x04 => Some("2-way".into()),
        0x05 => Some("4-way".into()),
        0x06 => Some("Fully".into()),
        0x07 => Some("8-way".into()),
        0x08 => Some("16-way".into()),
        other => Some(format!("0x{other:02x}")),
    };
    Some(CacheDevice {
        socket: smbios_str(&rec.strings, buf[i + 0x04]),
        level: Some(level),
        kind,
        size_kb: cache_size_kb(installed, ext),
        associativity,
    })
}

fn slot_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<SystemSlot> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x0C || i + length > buf.len() {
        return None;
    }
    let kind = slot_type_name(buf[i + 0x05]);
    // DSP0134 7.10.3 Current Usage：01h Other / 02h Unknown / 03h Available / 04h In use。
    let usage = match buf[i + 0x07] {
        0x01 => Some("Other".into()),
        0x02 => Some("Unknown".into()),
        0x03 => Some("Available".into()),
        0x04 => Some("In use".into()),
        other => Some(format!("0x{other:02x}")),
    };
    let bus = if length >= 0x11 {
        let seg = word(buf, i, 0x0D);
        let busn = buf[i + 0x0F];
        let df = buf[i + 0x10];
        if seg == 0xFFFF && busn == 0xFF && df == 0xFF {
            None
        } else {
            Some(format!("{seg:04x}:{busn:02x}:{:02x}.{}", df >> 3, df & 7))
        }
    } else {
        None
    };
    Some(SystemSlot {
        designation: smbios_str(&rec.strings, buf[i + 0x04]),
        kind: Some(kind.into()),
        usage,
        bus,
    })
}

fn port_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<PortConnector> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x09 || i + length > buf.len() {
        return None;
    }
    let internal = smbios_str(&rec.strings, buf[i + 0x04]);
    let external = smbios_str(&rec.strings, buf[i + 0x06]);
    let conn = buf[i + 0x07];
    let connector = Some(connector_type_name(if conn == 0 { buf[i + 0x05] } else { conn }));
    Some(PortConnector {
        internal,
        external,
        connector,
        port: Some(port_type_name(buf[i + 0x08])),
    })
}

/// DSP0134 Table 42 Connector Types。未知编号保留十六进制，不折叠成 Other。
fn connector_type_name(t: u8) -> String {
    match t {
        0x00 => "None".into(),
        0x08 => "DB-9 male".into(),
        0x09 => "DB-9 female".into(),
        0x0A => "RJ-11".into(),
        0x0B => "RJ-45".into(),
        0x0F => "PS/2".into(),
        0x12 => "USB".into(),
        0x1F => "Mini-jack".into(),
        0x21 => "IEEE 1394".into(),
        0x22 => "SAS/SATA".into(),
        0x23 => "USB-C".into(),
        0xFF => "Other".into(),
        n => format!("0x{n:02X}"),
    }
}

/// DSP0134 Table 43 Port Types。未知编号保留十六进制。
fn port_type_name(t: u8) -> String {
    match t {
        0x00 => "None".into(),
        0x01 => "Parallel".into(),
        0x08 => "Serial 16550".into(),
        0x09 => "Serial 16550A".into(),
        0x0D => "Keyboard".into(),
        0x0E => "Mouse".into(),
        0x10 => "USB".into(),
        0x11 => "FireWire".into(),
        0x16 => "Access Bus".into(),
        0x1D => "Audio".into(),
        0x1F => "Network".into(),
        0x20 => "SATA".into(),
        0x21 => "SAS".into(),
        0x23 => "Thunderbolt".into(),
        0xFF => "Other".into(),
        n => format!("0x{n:02X}"),
    }
}

fn onboard_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<OnboardDevice> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x0B || i + length > buf.len() {
        return None;
    }
    let raw = buf[i + 0x05];
    let enabled = raw & 0x80 != 0;
    let kind = onboard_type_name(raw & 0x7F);
    let bus = {
        let seg = word(buf, i, 0x07);
        let busn = buf[i + 0x09];
        let df = buf[i + 0x0A];
        if seg == 0xFFFF && busn == 0xFF && df == 0xFF {
            None
        } else {
            Some(format!("{seg:04x}:{busn:02x}:{:02x}.{}", df >> 3, df & 7))
        }
    };
    Some(OnboardDevice {
        designation: smbios_str(&rec.strings, buf[i + 0x04]),
        kind: Some(kind.into()),
        enabled,
        bus,
    })
}

fn onboard_type_name(t: u8) -> &'static str {
    match t {
        0x01 => "Other",
        0x03 => "Video",
        0x05 => "Ethernet",
        0x07 => "Sound",
        0x09 => "SATA",
        0x0A => "SAS",
        0x0B => "WLAN",
        0x0C => "Bluetooth",
        0x0D => "WWAN",
        0x0F => "NVMe",
        0x10 => "UFS",
        _ => "Other",
    }
}

fn psu_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<PowerSupply> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x10 || i + length > buf.len() {
        return None;
    }
    let max = word(buf, i, 0x0C);
    // DSP0134：仅 8000h 表示未知；0 仍是 0 W。
    let max_watts = if max == 0x8000 { None } else { Some(max) };
    let ch = word(buf, i, 0x0E);
    Some(PowerSupply {
        location: smbios_str(&rec.strings, buf[i + 0x05]),
        name: smbios_str(&rec.strings, buf[i + 0x06]),
        manufacturer: smbios_str(&rec.strings, buf[i + 0x07]),
        max_watts,
        present: ch & 0x02 != 0,
    })
}

fn oem_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Vec<String> {
    let Some(i) = rec_offset(buf, rec) else {
        return Vec::new();
    };
    let length = buf[i + 1] as usize;
    if length < 0x05 || i + length > buf.len() {
        return Vec::new();
    }
    let n = buf[i + 0x04] as usize;
    rec.strings.iter().take(n.min(16)).cloned().collect()
}

fn language_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<(Option<String>, Vec<String>)> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x16 || i + length > buf.len() {
        return None;
    }
    let n = buf[i + 0x04] as usize;
    Some((
        smbios_str(&rec.strings, buf[i + 0x15]),
        rec.strings.iter().take(n.min(16)).cloned().collect(),
    ))
}

fn boot_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<String> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x0B || i + length > buf.len() {
        return None;
    }
    Some(boot_status_name(buf[i + 0x0A]).into())
}

fn boot_status_name(t: u8) -> String {
    match t {
        0 => "No errors".into(),
        1 => "No bootable media".into(),
        2 => "OS failed to load".into(),
        3 => "Firmware hardware failure".into(),
        4 => "OS hardware failure".into(),
        5 => "User-requested boot".into(),
        6 => "Security violation".into(),
        7 => "Previously requested image".into(),
        8 => "Watchdog expired".into(),
        n => format!("0x{n:02X}"),
    }
}

fn tpm_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<TpmSmbios> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x1F || i + length > buf.len() {
        return None;
    }
    let raw = &buf[i + 0x04..i + 0x08];
    let vendor = {
        let s: String = raw
            .iter()
            .copied()
            .take_while(|&b| b != 0 && b.is_ascii_graphic())
            .map(|b| b as char)
            .collect();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };
    Some(TpmSmbios {
        vendor,
        spec: Some(format!("{}.{}", buf[i + 0x08], buf[i + 0x09])),
        description: smbios_str(&rec.strings, buf[i + 0x12]),
    })
}

fn config_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Vec<String> {
    let Some(i) = rec_offset(buf, rec) else {
        return Vec::new();
    };
    let length = buf[i + 1] as usize;
    if length < 0x05 || i + length > buf.len() {
        return Vec::new();
    }
    let n = buf[i + 0x04] as usize;
    rec.strings.iter().take(n.min(16)).cloned().collect()
}

fn battery_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<PortableBattery> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x10 || i + length > buf.len() {
        return None;
    }
    let raw_cap = word(buf, i, 0x0A);
    let mul = if length >= 0x16 {
        let m = buf[i + 0x15];
        if m == 0 { 1u32 } else { m as u32 }
    } else {
        1
    };
    let chem = buf[i + 0x09];
    let chemistry = if chem == 0x02 && length >= 0x15 {
        smbios_str(&rec.strings, buf[i + 0x14])
            .or_else(|| Some(battery_chemistry_name(chem)))
    } else {
        Some(battery_chemistry_name(chem))
    };
    Some(PortableBattery {
        location: smbios_str(&rec.strings, buf[i + 0x04]),
        manufacturer: smbios_str(&rec.strings, buf[i + 0x05]),
        name: smbios_str(&rec.strings, buf[i + 0x08]),
        chemistry,
        design_capacity_mwh: if raw_cap == 0 {
            None
        } else {
            Some(raw_cap as u32 * mul)
        },
        design_voltage_mv: {
            let v = word(buf, i, 0x0C);
            if v == 0 { None } else { Some(v) }
        },
    })
}

fn battery_chemistry_name(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Lead Acid".into(),
        0x04 => "Nickel Cadmium".into(),
        0x05 => "Nickel metal hydride".into(),
        0x06 => "Lithium-ion".into(),
        0x07 => "Zinc air".into(),
        0x08 => "Lithium Polymer".into(),
        n => format!("0x{n:02X}"),
    }
}

fn reset_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<SystemReset> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x0D || i + length > buf.len() {
        return None;
    }
    let cap = buf[i + 0x04];
    Some(SystemReset {
        enabled: cap & 0x01 != 0,
        watchdog: cap & 0x20 != 0,
        boot_option: reset_boot_option((cap >> 1) & 0x03).into(),
        reset_count: word_unknown(word(buf, i, 0x05)),
        reset_limit: word_unknown(word(buf, i, 0x07)),
        timeout_min: word_unknown(word(buf, i, 0x0B)),
    })
}

fn word_unknown(v: u16) -> Option<u16> {
    if v == 0xFFFF { None } else { Some(v) }
}

fn reset_boot_option(t: u8) -> &'static str {
    match t {
        0 => "Reserved",
        1 => "Operating System",
        2 => "System utilities",
        _ => "Do not reboot",
    }
}

fn hwsec_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<HardwareSecurity> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x05 || i + length > buf.len() {
        return None;
    }
    let s = buf[i + 0x04];
    Some(HardwareSecurity {
        power_on_password: hw_sec_status(s >> 6).into(),
        keyboard_password: hw_sec_status(s >> 4).into(),
        administrator_password: hw_sec_status(s >> 2).into(),
        front_panel_reset: hw_sec_status(s).into(),
    })
}

fn hw_sec_status(v: u8) -> &'static str {
    match v & 0x03 {
        0 => "Disabled",
        1 => "Enabled",
        2 => "Not Implemented",
        _ => "Unknown",
    }
}

/// Type 26/27/28 探头 WORD：仅 `0x8000` 表示未知（`0` 仍是 0）。
fn probe_word_unknown(v: u16) -> Option<u16> {
    if v == 0x8000 { None } else { Some(v) }
}

fn probe_temp_unknown(v: u16) -> Option<i16> {
    if v == 0x8000 { None } else { Some(v as i16) }
}

fn probe_status(v: u8) -> String {
    match (v >> 5) & 0x07 {
        1 => "Other".into(),
        2 => "Unknown".into(),
        3 => "OK".into(),
        4 => "Non-critical".into(),
        5 => "Critical".into(),
        6 => "Non-recoverable".into(),
        n => format!("0x{n:02X}"),
    }
}

/// Type 26 Table 96 + Type 28 Table 100 位置枚举（后者更完整）。
fn probe_location(v: u8) -> String {
    match v & 0x1F {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Processor".into(),
        0x04 => "Disk".into(),
        0x05 => "Peripheral Bay".into(),
        0x06 => "System Management Module".into(),
        0x07 => "Motherboard".into(),
        0x08 => "Memory Module".into(),
        0x09 => "Processor Module".into(),
        0x0A => "Power Unit".into(),
        0x0B => "Add-in Card".into(),
        0x0C => "Front Panel Board".into(),
        0x0D => "Back Panel Board".into(),
        0x0E => "Power System Board".into(),
        0x0F => "Drive Back Plane".into(),
        n => format!("0x{n:02X}"),
    }
}

fn cooling_type_name(v: u8) -> String {
    match v & 0x1F {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Fan".into(),
        0x04 => "Centrifugal Blower".into(),
        0x05 => "Chip Fan".into(),
        0x06 => "Cabinet Fan".into(),
        0x07 => "Power Supply Fan".into(),
        0x08 => "Heat Pipe".into(),
        0x09 => "Integrated Refrigeration".into(),
        0x10 => "Active Cooling".into(),
        0x11 => "Passive Cooling".into(),
        n => format!("0x{n:02X}"),
    }
}

fn voltage_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<VoltageProbe> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x14 || i + length > buf.len() {
        return None;
    }
    let loc = buf[i + 0x05];
    Some(VoltageProbe {
        description: smbios_str(&rec.strings, buf[i + 0x04]),
        location: probe_location(loc),
        status: probe_status(loc),
        max_mv: probe_word_unknown(word(buf, i, 0x06)),
        min_mv: probe_word_unknown(word(buf, i, 0x08)),
        nominal_mv: if length > 0x14 {
            probe_word_unknown(word(buf, i, 0x14))
        } else {
            None
        },
    })
}

fn cooling_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<CoolingDevice> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x0C || i + length > buf.len() {
        return None;
    }
    let ts = buf[i + 0x06];
    Some(CoolingDevice {
        description: if length >= 0x0F {
            smbios_str(&rec.strings, buf[i + 0x0E])
        } else {
            None
        },
        kind: cooling_type_name(ts),
        status: probe_status(ts),
        group: buf[i + 0x07],
        probe_handle: word(buf, i, 0x04),
        nominal_rpm: if length > 0x0C {
            probe_word_unknown(word(buf, i, 0x0C))
        } else {
            None
        },
    })
}

fn temperature_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<TemperatureProbe> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x14 || i + length > buf.len() {
        return None;
    }
    let loc = buf[i + 0x05];
    Some(TemperatureProbe {
        description: smbios_str(&rec.strings, buf[i + 0x04]),
        location: probe_location(loc),
        status: probe_status(loc),
        max_tenth_c: probe_temp_unknown(word(buf, i, 0x06)),
        min_tenth_c: probe_temp_unknown(word(buf, i, 0x08)),
        nominal_tenth_c: if length > 0x14 {
            probe_temp_unknown(word(buf, i, 0x14))
        } else {
            None
        },
    })
}

fn chassis_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<ChassisEnclosure> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x09 || i + length > buf.len() {
        return None;
    }
    let type_byte = buf[i + 0x05];
    let contained_n = if length >= 0x14 { buf[i + 0x13] as usize } else { 0 };
    let contained_m = if length >= 0x15 { buf[i + 0x14] as usize } else { 0 };
    let sku_off = 0x15 + contained_n.saturating_mul(contained_m);
    Some(ChassisEnclosure {
        manufacturer: smbios_str(&rec.strings, buf[i + 0x04]),
        kind: chassis_type_name(type_byte & 0x7F),
        locked: type_byte & 0x80 != 0,
        version: smbios_str(&rec.strings, buf[i + 0x06]),
        serial: smbios_str(&rec.strings, buf[i + 0x07]),
        asset_tag: smbios_str(&rec.strings, buf[i + 0x08]),
        boot_state: if length >= 0x0A {
            Some(enclosure_state(buf[i + 0x09]))
        } else {
            None
        },
        power_state: if length >= 0x0B {
            Some(enclosure_state(buf[i + 0x0A]))
        } else {
            None
        },
        thermal_state: if length >= 0x0C {
            Some(enclosure_state(buf[i + 0x0B]))
        } else {
            None
        },
        security: if length >= 0x0D {
            Some(chassis_security(buf[i + 0x0C]))
        } else {
            None
        },
        height_u: if length >= 0x12 {
            let h = buf[i + 0x11];
            if h == 0 { None } else { Some(h) }
        } else {
            None
        },
        power_cords: if length >= 0x13 {
            let n = buf[i + 0x12];
            if n == 0 { None } else { Some(n) }
        } else {
            None
        },
        sku: if length > sku_off {
            smbios_str(&rec.strings, buf[i + sku_off])
        } else {
            None
        },
    })
}

fn chassis_type_name(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Desktop".into(),
        0x04 => "Low Profile Desktop".into(),
        0x05 => "Pizza Box".into(),
        0x06 => "Mini Tower".into(),
        0x07 => "Tower".into(),
        0x08 => "Portable".into(),
        0x09 => "Laptop".into(),
        0x0A => "Notebook".into(),
        0x0B => "Hand Held".into(),
        0x0C => "Docking Station".into(),
        0x0D => "All in One".into(),
        0x0E => "Sub Notebook".into(),
        0x0F => "Space-saving".into(),
        0x10 => "Lunch Box".into(),
        0x11 => "Main Server Chassis".into(),
        0x12 => "Expansion Chassis".into(),
        0x13 => "SubChassis".into(),
        0x14 => "Bus Expansion Chassis".into(),
        0x15 => "Peripheral Chassis".into(),
        0x16 => "RAID Chassis".into(),
        0x17 => "Rack Mount Chassis".into(),
        0x18 => "Sealed-case PC".into(),
        0x19 => "Multi-system Chassis".into(),
        0x1A => "Compact PCI".into(),
        0x1B => "Advanced TCA".into(),
        0x1C => "Blade".into(),
        0x1D => "Blade Enclosure".into(),
        0x1E => "Tablet".into(),
        0x1F => "Convertible".into(),
        0x20 => "Detachable".into(),
        0x21 => "IoT Gateway".into(),
        0x22 => "Embedded PC".into(),
        0x23 => "Mini PC".into(),
        0x24 => "Stick PC".into(),
        n => format!("0x{n:02X}"),
    }
}

fn enclosure_state(v: u8) -> String {
    match v {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Safe".into(),
        0x04 => "Warning".into(),
        0x05 => "Critical".into(),
        0x06 => "Non-recoverable".into(),
        n => format!("0x{n:02X}"),
    }
}

fn chassis_security(v: u8) -> String {
    match v {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "None".into(),
        0x04 => "External interface locked out".into(),
        0x05 => "External interface enabled".into(),
        n => format!("0x{n:02X}"),
    }
}

fn power_controls_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<SystemPowerControls> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x09 || i + length > buf.len() {
        return None;
    }
    Some(SystemPowerControls {
        next_power_on: bcd_power_on(
            buf[i + 0x04],
            buf[i + 0x05],
            buf[i + 0x06],
            buf[i + 0x07],
            buf[i + 0x08],
        ),
    })
}

fn bcd_nibble_pair(v: u8) -> Option<u8> {
    let hi = v >> 4;
    let lo = v & 0x0F;
    if hi > 9 || lo > 9 {
        return None;
    }
    Some(hi * 10 + lo)
}

fn bcd_power_on(month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<String> {
    let month = bcd_nibble_pair(month)?;
    if month == 0 {
        return None;
    }
    let day = bcd_nibble_pair(day).unwrap_or(0);
    let hour = bcd_nibble_pair(hour).unwrap_or(0);
    let minute = bcd_nibble_pair(minute).unwrap_or(0);
    let second = bcd_nibble_pair(second).unwrap_or(0);
    Some(format!("{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"))
}

fn current_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<CurrentProbe> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x14 || i + length > buf.len() {
        return None;
    }
    let loc = buf[i + 0x05];
    Some(CurrentProbe {
        description: smbios_str(&rec.strings, buf[i + 0x04]),
        location: probe_location(loc),
        status: probe_status(loc),
        max_ma: probe_word_unknown(word(buf, i, 0x06)),
        min_ma: probe_word_unknown(word(buf, i, 0x08)),
        nominal_ma: if length > 0x14 {
            probe_word_unknown(word(buf, i, 0x14))
        } else {
            None
        },
    })
}

fn ipmi_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<IpmiDevice> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x10 || i + length > buf.len() {
        return None;
    }
    let spec = buf[i + 0x05];
    let nv = buf[i + 0x07];
    let raw = qword(buf, i, 0x08);
    let io = raw & 1 != 0;
    let addr = raw & !1;
    Some(IpmiDevice {
        interface: ipmi_interface_name(buf[i + 0x04]),
        spec: Some(format!("{}.{}", spec >> 4, spec & 0x0F)),
        i2c_address: buf[i + 0x06],
        nv_storage: if nv == 0xFF { None } else { Some(nv) },
        base_address: format!(
            "{} 0x{addr:X}",
            if io { "I/O" } else { "MEM" }
        ),
    })
}

fn ipmi_interface_name(t: u8) -> String {
    match t {
        0x00 => "Unknown".into(),
        0x01 => "KCS".into(),
        0x02 => "SMIC".into(),
        0x03 => "BT".into(),
        0x04 => "SSIF".into(),
        n => format!("0x{n:02X}"),
    }
}

fn mem_addr_unknown(v: u32) -> Option<u32> {
    if v == 0x8000_0000 { None } else { Some(v) }
}

fn memory_error_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<MemoryError32> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x17 || i + length > buf.len() {
        return None;
    }
    let syn = dword(buf, i, 0x07);
    Some(MemoryError32 {
        error_type: memory_error_type(buf[i + 0x04]),
        granularity: memory_error_granularity(buf[i + 0x05]),
        operation: memory_error_operation(buf[i + 0x06]),
        syndrome: if syn == 0 { None } else { Some(syn) },
        array_address: mem_addr_unknown(dword(buf, i, 0x0B)),
        device_address: mem_addr_unknown(dword(buf, i, 0x0F)),
        resolution: mem_addr_unknown(dword(buf, i, 0x13)),
    })
}

fn memory_error_type(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "OK".into(),
        0x04 => "Bad read".into(),
        0x05 => "Parity error".into(),
        0x06 => "Single-bit error".into(),
        0x07 => "Double-bit error".into(),
        0x08 => "Multi-bit error".into(),
        0x09 => "Nibble error".into(),
        0x0A => "Checksum error".into(),
        0x0B => "CRC error".into(),
        0x0C => "Corrected single-bit error".into(),
        0x0D => "Corrected error".into(),
        0x0E => "Uncorrectable error".into(),
        n => format!("0x{n:02X}"),
    }
}

fn memory_error_granularity(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Device level".into(),
        0x04 => "Memory partition level".into(),
        n => format!("0x{n:02X}"),
    }
}

fn memory_error_operation(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Read".into(),
        0x04 => "Write".into(),
        0x05 => "Partial write".into(),
        n => format!("0x{n:02X}"),
    }
}

fn mapped_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<MemoryMappedAddress> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x0F || i + length > buf.len() {
        return None;
    }
    let start_d = dword(buf, i, 0x04);
    let end_d = dword(buf, i, 0x08);
    let start = if start_d == 0xFFFF_FFFF {
        if length >= 0x1F {
            Some(qword(buf, i, 0x0F))
        } else {
            None
        }
    } else {
        Some((start_d as u64) * 1024)
    };
    let end = if end_d == 0xFFFF_FFFF {
        if length >= 0x1F {
            Some(qword(buf, i, 0x17))
        } else {
            None
        }
    } else {
        Some((end_d as u64) * 1024 + 1023)
    };
    Some(MemoryMappedAddress {
        start,
        end,
        array_handle: word(buf, i, 0x0C),
        partition_width: buf[i + 0x0E],
    })
}

fn pointing_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<PointingDevice> {
    let i = rec_offset(buf, rec)?;
    let length = buf[i + 1] as usize;
    if length < 0x07 || i + length > buf.len() {
        return None;
    }
    Some(PointingDevice {
        kind: pointing_type_name(buf[i + 0x04]),
        interface: pointing_interface_name(buf[i + 0x05]),
        buttons: buf[i + 0x06],
    })
}

fn pointing_type_name(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Mouse".into(),
        0x04 => "Track Ball".into(),
        0x05 => "Track Point".into(),
        0x06 => "Glide Point".into(),
        0x07 => "Touch Pad".into(),
        0x08 => "Touch Screen".into(),
        0x09 => "Optical Sensor".into(),
        n => format!("0x{n:02X}"),
    }
}

fn pointing_interface_name(t: u8) -> String {
    match t {
        0x01 => "Other".into(),
        0x02 => "Unknown".into(),
        0x03 => "Serial".into(),
        0x04 => "PS/2".into(),
        0x05 => "Infrared".into(),
        0x06 => "HP-HIL".into(),
        0x07 => "Bus mouse".into(),
        0x08 => "ADB".into(),
        0xA0 => "Bus mouse DB-9".into(),
        0xA1 => "Bus mouse micro-DIN".into(),
        0xA2 => "USB".into(),
        n => format!("0x{n:02X}"),
    }
}

fn slot_type_name(t: u8) -> &'static str {
    match t {
        0x03 => "ISA",
        0x06 => "PCI",
        0x09 => "Proprietary",
        0x0F => "AGP",
        0xA5 => "PCI Express",
        0xA6 => "PCIe x1",
        0xA7 => "PCIe x2",
        0xA8 => "PCIe x4",
        0xA9 => "PCIe x8",
        0xAA => "PCIe x16",
        0xAB => "PCIe Gen 2",
        0xAC => "PCIe Gen 2 x1",
        0xAD => "PCIe Gen 2 x2",
        0xAE => "PCIe Gen 2 x4",
        0xAF => "PCIe Gen 2 x8",
        0xB0 => "PCIe Gen 2 x16",
        0xB1 => "PCIe Gen 3",
        0xB2 => "PCIe Gen 3 x1",
        0xB3 => "PCIe Gen 3 x2",
        0xB4 => "PCIe Gen 3 x4",
        0xB5 => "PCIe Gen 3 x8",
        0xB6 => "PCIe Gen 3 x16",
        0xB8 => "PCIe Gen 4",
        0xB9 => "PCIe Gen 4 x1",
        0xBA => "PCIe Gen 4 x2",
        0xBB => "PCIe Gen 4 x4",
        0xBC => "PCIe Gen 4 x8",
        0xBD => "PCIe Gen 4 x16",
        0xBE => "PCIe Gen 5",
        0xBF => "PCIe Gen 5 x1",
        0xC0 => "PCIe Gen 5 x2",
        0xC1 => "PCIe Gen 5 x4",
        0xC2 => "PCIe Gen 5 x8",
        0xC3 => "PCIe Gen 5 x16",
        _ => "Other",
    }
}

/// Speed / Configured Speed：0 未知，0xFFFF 读 32 位扩展字段（3.3+ 的 0x54 / 0x58）。
fn type17_speed_mts(
    buf: &[u8],
    i: usize,
    length: usize,
    word_off: usize,
    ext_off: usize,
) -> Option<u32> {
    if length < word_off + 2 {
        return None;
    }
    let sp = u16::from_le_bytes([buf[i + word_off], buf[i + word_off + 1]]);
    match sp {
        0 => None,
        0xFFFF => {
            if length < ext_off + 4 {
                return None;
            }
            let ext = u32::from_le_bytes([
                buf[i + ext_off],
                buf[i + ext_off + 1],
                buf[i + ext_off + 2],
                buf[i + ext_off + 3],
            ]);
            if ext == 0 { None } else { Some(ext) }
        }
        n => Some(n as u32),
    }
}

fn form_factor_name(t: u8) -> &'static str {
    match t {
        0x01 => "Other",
        0x02 => "Unknown",
        0x08 => "Proprietary Card",
        0x09 => "DIMM",
        0x0C => "RIMM",
        0x0D => "SODIMM",
        0x0E => "SRIMM",
        0x0F => "FB-DIMM",
        0x10 => "Die",
        _ => "Unknown",
    }
}

fn memory_type_name(t: u8) -> &'static str {
    match t {
        0x01 => "Other",
        0x02 => "Unknown",
        0x03 => "DRAM",
        0x12 => "DDR",
        0x13 => "DDR2",
        0x18 => "DDR3",
        0x1A => "DDR4",
        0x22 => "DDR5",
        0x1E => "LPDDR3",
        0x1F => "LPDDR4",
        0x23 => "LPDDR5",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_bios_and_end() {
        // Type 0, length 18 (0x12), handle 0, then two strings + double NUL; Type 127.
        let mut buf = vec![0u8, 0x12, 0x00, 0x00];
        buf.resize(0x12, 0);
        buf.extend_from_slice(b"Vendor\0Version\0\0");
        buf.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&buf);
        assert!(recs.iter().any(|r| r.kind == 0 && r.strings.first().map(|s| s.as_str()) == Some("Vendor")));
        assert!(recs.iter().any(|r| r.kind == 127));
    }

    #[test]
    fn type17_uses_string_numbers_and_spec_offsets() {
        let mut rec = vec![0u8; 0x22];
        rec[0] = 17;
        rec[1] = 0x22;
        rec[2] = 1;
        rec[0x08] = 72;
        rec[0x0A] = 64;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x20;
        rec[0x0E] = 0x09;
        rec[0x10] = 1;
        rec[0x11] = 2;
        rec[0x12] = 0x1A;
        rec[0x15] = 0x80;
        rec[0x16] = 0x0C;
        rec[0x17] = 3;
        rec[0x18] = 4;
        rec[0x1A] = 5;
        rec[0x1B] = 0x02;
        rec[0x20] = 0x6A;
        rec[0x21] = 0x0A;
        rec.extend_from_slice(b"DIMM_A1\0BANK 0\0Samsung\0SN1\0M393A\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let mem = recs
            .iter()
            .find(|r| r.kind == 17)
            .and_then(|r| memory_from_raw(&rec, r))
            .expect("type 17");
        assert_eq!(mem.locator.as_deref(), Some("DIMM_A1"));
        assert_eq!(mem.bank.as_deref(), Some("BANK 0"));
        assert_eq!(mem.size_mb, Some(8192));
        assert!(mem.installed);
        assert_eq!(mem.r#type.as_deref(), Some("DDR4"));
        assert_eq!(mem.form_factor.as_deref(), Some("DIMM"));
        assert_eq!(mem.speed_mts, Some(3200));
        assert_eq!(mem.configured_mts, Some(2666));
        assert_eq!(mem.rank, Some(2));
        assert_eq!(mem.data_width, Some(64));
        assert_eq!(mem.total_width, Some(72));
        assert_eq!(mem.manufacturer.as_deref(), Some("Samsung"));
        assert_eq!(mem.serial.as_deref(), Some("SN1"));
        assert_eq!(mem.part.as_deref(), Some("M393A"));
    }

    #[test]
    fn type16_memory_array_capacity_and_ecc() {
        let mut rec = vec![0u8; 0x0F];
        rec[0] = 16;
        rec[1] = 0x0F;
        rec[0x04] = 0x03;
        rec[0x06] = 0x05;
        rec[0x07] = 0x00;
        rec[0x08] = 0x00;
        rec[0x09] = 0x00;
        rec[0x0A] = 0x02;
        rec[0x0D] = 4;
        rec[0x0E] = 0;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let arr = recs
            .iter()
            .find(|r| r.kind == 16)
            .and_then(|r| array_from_raw(&rec, r))
            .expect("type 16");
        assert_eq!(arr.location.as_deref(), Some("System board"));
        assert_eq!(arr.ecc.as_deref(), Some("Single-bit ECC"));
        assert_eq!(arr.max_capacity_mb, Some(32 * 1024));
        assert_eq!(arr.devices, Some(4));
    }

    #[test]
    fn type16_pci_addon_is_location_0x06() {
        let mut rec = vec![0u8; 0x0F];
        rec[0] = 16;
        rec[1] = 0x0F;
        rec[0x04] = 0x06;
        rec[0x06] = 0x03;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let arr = recs
            .iter()
            .find(|r| r.kind == 16)
            .and_then(|r| array_from_raw(&rec, r))
            .expect("type 16");
        assert_eq!(arr.location.as_deref(), Some("PCI add-on"));
    }

    #[test]
    fn type17_ffff_size_is_installed_unknown() {
        let mut rec = vec![0u8; 0x1B];
        rec[0] = 17;
        rec[1] = 0x1B;
        rec[0x0C] = 0xFF;
        rec[0x0D] = 0xFF;
        rec[0x0E] = 0x09;
        rec[0x10] = 1;
        rec.extend_from_slice(b"DIMM_B1\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let mem = recs
            .iter()
            .find(|r| r.kind == 17)
            .and_then(|r| memory_from_raw(&rec, r))
            .expect("type 17");
        assert!(mem.installed);
        assert_eq!(mem.size_mb, None);
        assert_eq!(mem.size_label(), "unknown");
        assert_eq!(mem.locator.as_deref(), Some("DIMM_B1"));
    }

    #[test]
    fn type17_zero_size_is_empty_slot() {
        let mut rec = vec![0u8; 0x1B];
        rec[0] = 17;
        rec[1] = 0x1B;
        rec[0x10] = 1;
        rec.extend_from_slice(b"DIMM_C1\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let mem = recs
            .iter()
            .find(|r| r.kind == 17)
            .and_then(|r| memory_from_raw(&rec, r))
            .expect("type 17");
        assert!(!mem.installed);
        assert_eq!(mem.size_label(), "empty");
    }

    #[test]
    fn type17_ffff_speed_reads_extended_dwords() {
        let mut rec = vec![0u8; 0x5E];
        rec[0] = 17;
        rec[1] = 0x5E;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x20;
        rec[0x0E] = 0x09;
        rec[0x15] = 0xFF;
        rec[0x16] = 0xFF;
        rec[0x1B] = 0x02;
        rec[0x20] = 0xFF;
        rec[0x21] = 0xFF;
        // Extended Speed / Configured Memory Speed 在 0x54 / 0x58，不是 0x56 / 0x5A。
        rec[0x54] = 0x80;
        rec[0x55] = 0x38;
        rec[0x56] = 0x01;
        rec[0x58] = 0x40;
        rec[0x59] = 0x0D;
        rec[0x5A] = 0x03;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let mem = recs
            .iter()
            .find(|r| r.kind == 17)
            .and_then(|r| memory_from_raw(&rec, r))
            .expect("type 17");
        assert_eq!(mem.speed_mts, Some(80_000));
        assert_eq!(mem.configured_mts, Some(200_000));
        assert_eq!(mem.rank, Some(2));
        assert_eq!(mem.size_mb, Some(8192));
    }

    #[test]
    fn type4_processor_socket_speed_and_cores() {
        let mut rec = vec![0u8; 0x30];
        rec[0] = 4;
        rec[1] = 0x30;
        rec[0x04] = 1;
        rec[0x07] = 2;
        rec[0x10] = 3;
        rec[0x14] = 0x88;
        rec[0x15] = 0x13;
        rec[0x16] = 0x10;
        rec[0x17] = 0x0E;
        rec[0x18] = 0x41;
        rec[0x23] = 0xFF;
        rec[0x25] = 0xFF;
        rec[0x2A] = 8;
        rec[0x2E] = 16;
        rec.extend_from_slice(b"LGA1700\0Intel\0Core i7\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 4)
            .and_then(|r| processor_from_raw(&rec, r))
            .expect("type 4");
        assert_eq!(p.socket.as_deref(), Some("LGA1700"));
        assert_eq!(p.manufacturer.as_deref(), Some("Intel"));
        assert_eq!(p.version.as_deref(), Some("Core i7"));
        assert_eq!(p.max_mhz, Some(5000));
        assert_eq!(p.current_mhz, Some(3600));
        assert_eq!(p.cores, Some(8));
        assert_eq!(p.threads, Some(16));
        assert!(p.populated && p.enabled);
    }

    #[test]
    fn type7_cache_level_and_size() {
        let mut rec = vec![0u8; 0x13];
        rec[0] = 7;
        rec[1] = 0x13;
        rec[0x04] = 1;
        rec[0x05] = 0x02;
        rec[0x09] = 0x00;
        rec[0x0A] = 0x20;
        rec[0x11] = 0x05;
        rec[0x12] = 0x07;
        rec.extend_from_slice(b"L3 Cache\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 7)
            .and_then(|r| cache_from_raw(&rec, r))
            .expect("type 7");
        assert_eq!(c.socket.as_deref(), Some("L3 Cache"));
        assert_eq!(c.level, Some(3));
        assert_eq!(c.kind.as_deref(), Some("Unified"));
        assert_eq!(c.size_kb, Some(8192));
        assert_eq!(c.associativity.as_deref(), Some("8-way"));
    }

    #[test]
    fn type9_pcie_slot_in_use() {
        let mut rec = vec![0u8; 0x11];
        rec[0] = 9;
        rec[1] = 0x11;
        rec[0x04] = 1;
        rec[0x05] = 0xAA;
        rec[0x07] = 0x04;
        rec[0x0F] = 0x01;
        rec.extend_from_slice(b"PCIe x16\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let s = recs
            .iter()
            .find(|r| r.kind == 9)
            .and_then(|r| slot_from_raw(&rec, r))
            .expect("type 9");
        assert_eq!(s.designation.as_deref(), Some("PCIe x16"));
        assert_eq!(s.kind.as_deref(), Some("PCIe x16"));
        assert_eq!(s.usage.as_deref(), Some("In use"));
        assert_eq!(s.bus.as_deref(), Some("0000:01:00.0"));
        assert_eq!(
            recs.iter().find(|r| r.kind == 9).map(|r| r.kind_name.as_str()),
            Some("System Slot")
        );
    }

    #[test]
    fn type9_available_without_pci_address() {
        let mut rec = vec![0u8; 0x11];
        rec[0] = 9;
        rec[1] = 0x11;
        rec[0x04] = 1;
        rec[0x05] = 0x06;
        rec[0x07] = 0x03;
        rec[0x0D] = 0xFF;
        rec[0x0E] = 0xFF;
        rec[0x0F] = 0xFF;
        rec[0x10] = 0xFF;
        rec.extend_from_slice(b"PCI Slot 1\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let s = recs
            .iter()
            .find(|r| r.kind == 9)
            .and_then(|r| slot_from_raw(&rec, r))
            .expect("type 9");
        assert_eq!(s.designation.as_deref(), Some("PCI Slot 1"));
        assert_eq!(s.kind.as_deref(), Some("PCI"));
        assert_eq!(s.usage.as_deref(), Some("Available"));
        assert_eq!(s.bus, None);
    }

    #[test]
    fn type9_pcie_gen5_and_proprietary() {
        let mut gen5 = vec![0u8; 0x11];
        gen5[0] = 9;
        gen5[1] = 0x11;
        gen5[0x04] = 1;
        gen5[0x05] = 0xBE;
        gen5[0x07] = 0x04;
        gen5.extend_from_slice(b"Slot0\0\0");
        gen5.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&gen5);
        let s = recs
            .iter()
            .find(|r| r.kind == 9)
            .and_then(|r| slot_from_raw(&gen5, r))
            .expect("type 9 gen5");
        assert_eq!(s.kind.as_deref(), Some("PCIe Gen 5"));

        let mut prop = vec![0u8; 0x0C];
        prop[0] = 9;
        prop[1] = 0x0C;
        prop[0x04] = 1;
        prop[0x05] = 0x09;
        prop[0x07] = 0x02;
        prop.extend_from_slice(b"OEM\0\0");
        prop.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&prop);
        let s = recs
            .iter()
            .find(|r| r.kind == 9)
            .and_then(|r| slot_from_raw(&prop, r))
            .expect("type 9 proprietary");
        assert_eq!(s.kind.as_deref(), Some("Proprietary"));
        assert_eq!(s.usage.as_deref(), Some("Unknown"));
    }

    #[test]
    fn type4_byte_core_count_when_no_word() {
        let mut rec = vec![0u8; 0x26];
        rec[0] = 4;
        rec[1] = 0x26;
        rec[0x04] = 1;
        rec[0x07] = 2;
        rec[0x10] = 3;
        rec[0x14] = 0x20;
        rec[0x15] = 0x0C;
        rec[0x16] = 0xE8;
        rec[0x17] = 0x07;
        rec[0x18] = 0x41;
        rec[0x23] = 4;
        rec[0x25] = 8;
        rec.extend_from_slice(b"Socket 0\0AMD\0Ryzen\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 4)
            .and_then(|r| processor_from_raw(&rec, r))
            .expect("type 4");
        assert_eq!(p.socket.as_deref(), Some("Socket 0"));
        assert_eq!(p.max_mhz, Some(3104));
        assert_eq!(p.current_mhz, Some(2024));
        assert_eq!(p.cores, Some(4));
        assert_eq!(p.threads, Some(8));
        assert!(p.populated && p.enabled);
    }

    #[test]
    fn type4_byte_count_beats_zero_word() {
        let mut rec = vec![0u8; 0x30];
        rec[0] = 4;
        rec[1] = 0x30;
        rec[0x04] = 1;
        rec[0x07] = 2;
        rec[0x10] = 3;
        rec[0x14] = 0x88;
        rec[0x15] = 0x13;
        rec[0x16] = 0x10;
        rec[0x17] = 0x0E;
        rec[0x18] = 0x41;
        rec[0x23] = 8;
        rec[0x25] = 16;
        rec.extend_from_slice(b"LGA1700\0Intel\0Core i7\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 4)
            .and_then(|r| processor_from_raw(&rec, r))
            .expect("type 4");
        assert_eq!(p.cores, Some(8));
        assert_eq!(p.threads, Some(16));
    }

    #[test]
    fn type0_bios_rom_and_release() {
        let mut rec = vec![0u8; 0x18];
        rec[0] = 0;
        rec[1] = 0x18;
        rec[0x09] = 0x7F;
        rec[0x14] = 5;
        rec[0x15] = 17;
        rec.extend_from_slice(b"Vendor\0Ver\0Date\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let bios = recs.iter().find(|r| r.kind == 0).expect("type 0");
        let (rom, rel) = bios_extras(&rec, bios);
        assert_eq!(rom, Some(8192));
        assert_eq!(rel.as_deref(), Some("5.17"));
    }

    #[test]
    fn type0_extended_rom_is_mib() {
        let mut rec = vec![0u8; 0x1A];
        rec[0] = 0;
        rec[1] = 0x1A;
        rec[0x09] = 0xFF;
        rec[0x14] = 0xFF;
        rec[0x15] = 0xFF;
        rec[0x18] = 16;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let bios = recs.iter().find(|r| r.kind == 0).expect("type 0");
        let (rom, rel) = bios_extras(&rec, bios);
        assert_eq!(rom, Some(16 * 1024));
        assert_eq!(rel, None);
    }

    #[test]
    fn type0_extended_rom_is_gib() {
        let mut rec = vec![0u8; 0x1A];
        rec[0] = 0;
        rec[1] = 0x1A;
        rec[0x09] = 0xFF;
        rec[0x14] = 0xFF;
        rec[0x15] = 0xFF;
        rec[0x18] = 0x01;
        rec[0x19] = 0x40;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let bios = recs.iter().find(|r| r.kind == 0).expect("type 0");
        let (rom, rel) = bios_extras(&rec, bios);
        assert_eq!(rom, Some(1_048_576));
        assert_eq!(rel, None);
    }

    #[test]
    fn type7_extended_size_when_ffff() {
        let mut rec = vec![0u8; 0x1B];
        rec[0] = 7;
        rec[1] = 0x1B;
        rec[0x04] = 1;
        rec[0x05] = 0x01;
        rec[0x09] = 0xFF;
        rec[0x0A] = 0xFF;
        rec[0x11] = 0x04;
        rec[0x12] = 0x08;
        rec[0x17] = 0x00;
        rec[0x18] = 0x80;
        rec.extend_from_slice(b"L2 Cache\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 7)
            .and_then(|r| cache_from_raw(&rec, r))
            .expect("type 7");
        assert_eq!(c.socket.as_deref(), Some("L2 Cache"));
        assert_eq!(c.level, Some(2));
        assert_eq!(c.kind.as_deref(), Some("Data"));
        assert_eq!(c.size_kb, Some(32768));
        assert_eq!(c.associativity.as_deref(), Some("16-way"));
    }

    #[test]
    fn type8_usb_and_rj45_ports() {
        let mut rec = Vec::new();
        let mut usb = vec![0u8; 0x09];
        usb[0] = 8;
        usb[1] = 0x09;
        usb[2] = 1;
        usb[0x04] = 1;
        usb[0x05] = 0x12;
        usb[0x06] = 2;
        usb[0x07] = 0x12;
        usb[0x08] = 0x10;
        usb.extend_from_slice(b"JUSB1\0USB3_1\0\0");
        rec.extend(usb);
        let mut lan = vec![0u8; 0x09];
        lan[0] = 8;
        lan[1] = 0x09;
        lan[2] = 2;
        lan[0x04] = 1;
        lan[0x06] = 2;
        lan[0x07] = 0x0B;
        lan[0x08] = 0x1F;
        lan.extend_from_slice(b"JLAN1\0LAN1\0\0");
        rec.extend(lan);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let ports: Vec<_> = recs
            .iter()
            .filter(|r| r.kind == 8)
            .filter_map(|r| port_from_raw(&rec, r))
            .collect();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].internal.as_deref(), Some("JUSB1"));
        assert_eq!(ports[0].external.as_deref(), Some("USB3_1"));
        assert_eq!(ports[0].connector.as_deref(), Some("USB"));
        assert_eq!(ports[0].port.as_deref(), Some("USB"));
        assert_eq!(ports[1].internal.as_deref(), Some("JLAN1"));
        assert_eq!(ports[1].external.as_deref(), Some("LAN1"));
        assert_eq!(ports[1].connector.as_deref(), Some("RJ-45"));
        assert_eq!(ports[1].port.as_deref(), Some("Network"));
        assert_eq!(
            recs.iter().find(|r| r.kind == 8).map(|r| r.kind_name.as_str()),
            Some("Port Connector")
        );
    }

    #[test]
    fn type8_prefers_internal_connector_when_external_none() {
        let mut rec = vec![0u8; 0x09];
        rec[0] = 8;
        rec[1] = 0x09;
        rec[0x04] = 1;
        rec[0x05] = 0x12;
        rec[0x07] = 0x00;
        rec[0x08] = 0x10;
        rec.extend_from_slice(b"JUSB2\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 8)
            .and_then(|r| port_from_raw(&rec, r))
            .expect("type 8");
        assert_eq!(p.internal.as_deref(), Some("JUSB2"));
        assert!(p.external.is_none());
        assert_eq!(p.connector.as_deref(), Some("USB"));
        assert_eq!(p.port.as_deref(), Some("USB"));
    }

    #[test]
    fn type8_spec_codes_are_not_shifted() {
        assert_eq!(connector_type_name(0x0B), "RJ-45");
        assert_eq!(connector_type_name(0x1F), "Mini-jack");
        assert_eq!(connector_type_name(0x23), "USB-C");
        assert_eq!(port_type_name(0x08), "Serial 16550");
        assert_eq!(port_type_name(0x10), "USB");
        assert_eq!(port_type_name(0x1F), "Network");
        assert_eq!(connector_type_name(0x40), "0x40");
        assert_eq!(port_type_name(0x30), "0x30");
    }

    #[test]
    fn type41_onboard_ethernet_enabled() {
        let mut rec = vec![0u8; 0x0B];
        rec[0] = 41;
        rec[1] = 0x0B;
        rec[0x04] = 1;
        rec[0x05] = 0x85;
        rec[0x09] = 0x02;
        rec[0x0A] = 0x00;
        rec.extend_from_slice(b"Onboard LAN\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let d = recs
            .iter()
            .find(|r| r.kind == 41)
            .and_then(|r| onboard_from_raw(&rec, r))
            .expect("type 41");
        assert_eq!(d.designation.as_deref(), Some("Onboard LAN"));
        assert_eq!(d.kind.as_deref(), Some("Ethernet"));
        assert!(d.enabled);
        assert_eq!(d.bus.as_deref(), Some("0000:02:00.0"));
    }

    #[test]
    fn type39_psu_max_watts_and_present() {
        let mut rec = vec![0u8; 0x10];
        rec[0] = 39;
        rec[1] = 0x10;
        rec[0x05] = 1;
        rec[0x06] = 2;
        rec[0x07] = 3;
        rec[0x0C] = 0x2C;
        rec[0x0D] = 0x01;
        rec[0x0E] = 0x02;
        rec.extend_from_slice(b"PSU Bay\0PS-650\0Corsair\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 39)
            .and_then(|r| psu_from_raw(&rec, r))
            .expect("type 39");
        assert_eq!(p.location.as_deref(), Some("PSU Bay"));
        assert_eq!(p.name.as_deref(), Some("PS-650"));
        assert_eq!(p.manufacturer.as_deref(), Some("Corsair"));
        assert_eq!(p.max_watts, Some(300));
        assert!(p.present);
    }

    #[test]
    fn type39_zero_watts_is_zero_not_unknown() {
        let mut rec = vec![0u8; 0x10];
        rec[0] = 39;
        rec[1] = 0x10;
        rec[0x06] = 1;
        rec.extend_from_slice(b"PS-0\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 39)
            .and_then(|r| psu_from_raw(&rec, r))
            .expect("type 39");
        assert_eq!(p.max_watts, Some(0));
    }

    #[test]
    fn type39_8000h_watts_is_unknown() {
        let mut rec = vec![0u8; 0x10];
        rec[0] = 39;
        rec[1] = 0x10;
        rec[0x06] = 1;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x80;
        rec.extend_from_slice(b"PS-U\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 39)
            .and_then(|r| psu_from_raw(&rec, r))
            .expect("type 39");
        assert_eq!(p.max_watts, None);
    }

    #[test]
    fn type11_oem_strings() {
        let mut rec = vec![0u8; 0x05];
        rec[0] = 11;
        rec[1] = 0x05;
        rec[0x04] = 2;
        rec.extend_from_slice(b"Board-REV-A\0https://oem.example\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let oem = recs
            .iter()
            .find(|r| r.kind == 11)
            .map(|r| oem_from_raw(&rec, r))
            .expect("type 11");
        assert_eq!(oem, vec!["Board-REV-A".to_string(), "https://oem.example".to_string()]);
        assert_eq!(
            recs.iter().find(|r| r.kind == 11).map(|r| r.kind_name.as_str()),
            Some("OEM Strings")
        );
    }

    #[test]
    fn type13_current_language_abbreviated() {
        let mut rec = vec![0u8; 0x16];
        rec[0] = 13;
        rec[1] = 0x16;
        rec[0x04] = 2;
        rec[0x05] = 0x01;
        rec[0x15] = 2;
        rec.extend_from_slice(b"enUS\0frCA\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let (cur, langs) = recs
            .iter()
            .find(|r| r.kind == 13)
            .and_then(|r| language_from_raw(&rec, r))
            .expect("type 13");
        assert_eq!(cur.as_deref(), Some("frCA"));
        assert_eq!(langs, vec!["enUS".to_string(), "frCA".to_string()]);
        assert_eq!(
            recs.iter().find(|r| r.kind == 13).map(|r| r.kind_name.as_str()),
            Some("BIOS Language")
        );
    }

    #[test]
    fn type32_no_errors_boot() {
        let mut rec = vec![0u8; 0x0B];
        rec[0] = 32;
        rec[1] = 0x0B;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let st = recs
            .iter()
            .find(|r| r.kind == 32)
            .and_then(|r| boot_from_raw(&rec, r))
            .expect("type 32");
        assert_eq!(st, "No errors");
        assert_eq!(
            recs.iter().find(|r| r.kind == 32).map(|r| r.kind_name.as_str()),
            Some("System Boot")
        );
    }

    #[test]
    fn type43_tpm2_vendor_and_spec() {
        let mut rec = vec![0u8; 0x1F];
        rec[0] = 43;
        rec[1] = 0x1F;
        rec[0x04] = b'I';
        rec[0x05] = b'F';
        rec[0x06] = b'X';
        rec[0x08] = 2;
        rec[0x09] = 0;
        rec[0x12] = 1;
        rec.extend_from_slice(b"TPM 2.0\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let t = recs
            .iter()
            .find(|r| r.kind == 43)
            .and_then(|r| tpm_from_raw(&rec, r))
            .expect("type 43");
        assert_eq!(t.vendor.as_deref(), Some("IFX"));
        assert_eq!(t.spec.as_deref(), Some("2.0"));
        assert_eq!(t.description.as_deref(), Some("TPM 2.0"));
        assert_eq!(
            recs.iter().find(|r| r.kind == 43).map(|r| r.kind_name.as_str()),
            Some("TPM Device")
        );
    }

    #[test]
    fn type12_config_options() {
        let mut rec = vec![0u8; 0x05];
        rec[0] = 12;
        rec[1] = 0x05;
        rec[0x04] = 2;
        rec.extend_from_slice(b"Jumper J1: closed\0SW1: on\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let opts = recs
            .iter()
            .find(|r| r.kind == 12)
            .map(|r| config_from_raw(&rec, r))
            .expect("type 12");
        assert_eq!(
            opts,
            vec!["Jumper J1: closed".to_string(), "SW1: on".to_string()]
        );
        assert_eq!(
            recs.iter().find(|r| r.kind == 12).map(|r| r.kind_name.as_str()),
            Some("System Configuration Options")
        );
    }

    #[test]
    fn type22_lithium_ion_capacity_and_voltage() {
        let mut rec = vec![0u8; 0x1A];
        rec[0] = 22;
        rec[1] = 0x1A;
        rec[0x04] = 1;
        rec[0x05] = 2;
        rec[0x08] = 3;
        rec[0x09] = 0x06;
        rec[0x0A] = 0xC0;
        rec[0x0B] = 0x12; // 4800 mWh
        rec[0x0C] = 0x5C;
        rec[0x0D] = 0x2B; // 11100 mV
        rec[0x15] = 1;
        rec.extend_from_slice(b"BAT0\0SMP\0DELL 1F22\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let b = recs
            .iter()
            .find(|r| r.kind == 22)
            .and_then(|r| battery_from_raw(&rec, r))
            .expect("type 22");
        assert_eq!(b.location.as_deref(), Some("BAT0"));
        assert_eq!(b.manufacturer.as_deref(), Some("SMP"));
        assert_eq!(b.name.as_deref(), Some("DELL 1F22"));
        assert_eq!(b.chemistry.as_deref(), Some("Lithium-ion"));
        assert_eq!(b.design_capacity_mwh, Some(4800));
        assert_eq!(b.design_voltage_mv, Some(11100));
        assert_eq!(
            recs.iter().find(|r| r.kind == 22).map(|r| r.kind_name.as_str()),
            Some("Portable Battery")
        );
    }

    #[test]
    fn type22_zero_capacity_is_unknown() {
        let mut rec = vec![0u8; 0x10];
        rec[0] = 22;
        rec[1] = 0x10;
        rec[0x09] = 0x02;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let b = recs
            .iter()
            .find(|r| r.kind == 22)
            .and_then(|r| battery_from_raw(&rec, r))
            .expect("type 22");
        assert_eq!(b.design_capacity_mwh, None);
        assert_eq!(b.design_voltage_mv, None);
        assert_eq!(b.chemistry.as_deref(), Some("Unknown"));
    }

    #[test]
    fn type22_multiplier_is_at_0x15_not_oem() {
        let mut rec = vec![0u8; 0x1A];
        rec[0] = 22;
        rec[1] = 0x1A;
        rec[0x0A] = 0xC0;
        rec[0x0B] = 0x12; // 4800
        rec[0x15] = 2;
        rec[0x16] = 0x80; // OEM, must not multiply
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let b = recs
            .iter()
            .find(|r| r.kind == 22)
            .and_then(|r| battery_from_raw(&rec, r))
            .expect("type 22");
        assert_eq!(b.design_capacity_mwh, Some(9600));
    }

    #[test]
    fn type22_unknown_chemistry_uses_sbds_string() {
        let mut rec = vec![0u8; 0x1A];
        rec[0] = 22;
        rec[1] = 0x1A;
        rec[0x04] = 1;
        rec[0x05] = 2;
        rec[0x08] = 3;
        rec[0x09] = 0x02;
        rec[0x14] = 4;
        rec.extend_from_slice(b"BAT0\0SMP\0NAME\0LION\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let b = recs
            .iter()
            .find(|r| r.kind == 22)
            .and_then(|r| battery_from_raw(&rec, r))
            .expect("type 22");
        assert_eq!(b.chemistry.as_deref(), Some("LION"));
    }

    #[test]
    fn type23_watchdog_enabled_os_boot() {
        let mut rec = vec![0u8; 0x0D];
        rec[0] = 23;
        rec[1] = 0x0D;
        rec[0x04] = 0x23; // enabled + OS boot + watchdog
        rec[0x05] = 3;
        rec[0x06] = 0;
        rec[0x07] = 5;
        rec[0x08] = 0;
        rec[0x0B] = 10;
        rec[0x0C] = 0;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let r = recs
            .iter()
            .find(|r| r.kind == 23)
            .and_then(|r| reset_from_raw(&rec, r))
            .expect("type 23");
        assert!(r.enabled);
        assert!(r.watchdog);
        assert_eq!(r.boot_option, "Operating System");
        assert_eq!(r.reset_count, Some(3));
        assert_eq!(r.reset_limit, Some(5));
        assert_eq!(r.timeout_min, Some(10));
        assert_eq!(
            recs.iter().find(|r| r.kind == 23).map(|r| r.kind_name.as_str()),
            Some("System Reset")
        );
    }

    #[test]
    fn type23_ffff_count_is_unknown() {
        let mut rec = vec![0u8; 0x0D];
        rec[0] = 23;
        rec[1] = 0x0D;
        rec[0x05] = 0xFF;
        rec[0x06] = 0xFF;
        rec[0x07] = 0xFF;
        rec[0x08] = 0xFF;
        rec[0x0B] = 0xFF;
        rec[0x0C] = 0xFF;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let r = recs
            .iter()
            .find(|r| r.kind == 23)
            .and_then(|r| reset_from_raw(&rec, r))
            .expect("type 23");
        assert_eq!(r.reset_count, None);
        assert_eq!(r.reset_limit, None);
        assert_eq!(r.timeout_min, None);
    }

    #[test]
    fn type24_password_status_nibbles() {
        let mut rec = vec![0u8; 0x05];
        rec[0] = 24;
        rec[1] = 0x05;
        rec[0x04] = 0x64; // power-on Enabled, keyboard Not Implemented, admin Enabled, front Disabled
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let h = recs
            .iter()
            .find(|r| r.kind == 24)
            .and_then(|r| hwsec_from_raw(&rec, r))
            .expect("type 24");
        assert_eq!(h.power_on_password, "Enabled");
        assert_eq!(h.keyboard_password, "Not Implemented");
        assert_eq!(h.administrator_password, "Enabled");
        assert_eq!(h.front_panel_reset, "Disabled");
        assert_eq!(
            recs.iter().find(|r| r.kind == 24).map(|r| r.kind_name.as_str()),
            Some("Hardware Security")
        );
    }

    #[test]
    fn type26_motherboard_ok_nominal_mv() {
        let mut rec = vec![0u8; 0x16];
        rec[0] = 26;
        rec[1] = 0x16;
        rec[0x04] = 1;
        rec[0x05] = 0x67; // OK + Motherboard
        rec[0x06] = 0xE0;
        rec[0x07] = 0x2E; // 12000 mV
        rec[0x08] = 0x00;
        rec[0x09] = 0x80; // min unknown
        rec[0x0A] = 0x00;
        rec[0x0B] = 0x80;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x80;
        rec[0x0E] = 0x00;
        rec[0x0F] = 0x80;
        rec[0x14] = 0xE0;
        rec[0x15] = 0x2E;
        rec.extend_from_slice(b"VCORE\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let v = recs
            .iter()
            .find(|r| r.kind == 26)
            .and_then(|r| voltage_from_raw(&rec, r))
            .expect("type 26");
        assert_eq!(v.description.as_deref(), Some("VCORE"));
        assert_eq!(v.location, "Motherboard");
        assert_eq!(v.status, "OK");
        assert_eq!(v.max_mv, Some(12000));
        assert_eq!(v.min_mv, None);
        assert_eq!(v.nominal_mv, Some(12000));
        assert_eq!(
            recs.iter().find(|r| r.kind == 26).map(|r| r.kind_name.as_str()),
            Some("Voltage Probe")
        );
    }

    #[test]
    fn type26_zero_mv_is_zero_not_unknown() {
        let mut rec = vec![0u8; 0x14];
        rec[0] = 26;
        rec[1] = 0x14;
        rec[0x05] = 0x83; // Non-critical + Processor
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let v = recs
            .iter()
            .find(|r| r.kind == 26)
            .and_then(|r| voltage_from_raw(&rec, r))
            .expect("type 26");
        assert_eq!(v.max_mv, Some(0));
        assert_eq!(v.min_mv, Some(0));
        assert_eq!(v.nominal_mv, None);
        assert_eq!(v.location, "Processor");
        assert_eq!(v.status, "Non-critical");
    }

    #[test]
    fn type27_fan_nominal_rpm_and_description() {
        let mut rec = vec![0u8; 0x0F];
        rec[0] = 27;
        rec[1] = 0x0F;
        rec[0x04] = 0x28;
        rec[0x05] = 0x00; // probe handle 0x0028
        rec[0x06] = 0x63; // OK + Fan
        rec[0x07] = 1;
        rec[0x0C] = 0xB8;
        rec[0x0D] = 0x0B; // 3000 rpm
        rec[0x0E] = 1;
        rec.extend_from_slice(b"CPU Fan\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 27)
            .and_then(|r| cooling_from_raw(&rec, r))
            .expect("type 27");
        assert_eq!(c.description.as_deref(), Some("CPU Fan"));
        assert_eq!(c.kind, "Fan");
        assert_eq!(c.status, "OK");
        assert_eq!(c.group, 1);
        assert_eq!(c.probe_handle, 0x0028);
        assert_eq!(c.nominal_rpm, Some(3000));
        assert_eq!(
            recs.iter().find(|r| r.kind == 27).map(|r| r.kind_name.as_str()),
            Some("Cooling Device")
        );
    }

    #[test]
    fn type27_8000_rpm_is_unknown_and_desc_needs_0x0f() {
        let mut rec = vec![0u8; 0x0E];
        rec[0] = 27;
        rec[1] = 0x0E;
        rec[0x06] = 0xA1; // Critical + Other
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x80;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 27)
            .and_then(|r| cooling_from_raw(&rec, r))
            .expect("type 27");
        assert_eq!(c.nominal_rpm, None);
        assert_eq!(c.description, None);
        assert_eq!(c.kind, "Other");
        assert_eq!(c.status, "Critical");
    }

    #[test]
    fn type28_processor_temp_tenths() {
        let mut rec = vec![0u8; 0x16];
        rec[0] = 28;
        rec[1] = 0x16;
        rec[0x04] = 1;
        rec[0x05] = 0x63; // OK + Processor
        rec[0x06] = 0x90;
        rec[0x07] = 0x01; // 40.0 °C
        rec[0x08] = 0xCE;
        rec[0x09] = 0xFF; // -5.0 °C
        rec[0x0A] = 0x00;
        rec[0x0B] = 0x80;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x80;
        rec[0x0E] = 0x00;
        rec[0x0F] = 0x80;
        rec[0x14] = 0x7C;
        rec[0x15] = 0x01; // 38.0 °C
        rec.extend_from_slice(b"CPU\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let t = recs
            .iter()
            .find(|r| r.kind == 28)
            .and_then(|r| temperature_from_raw(&rec, r))
            .expect("type 28");
        assert_eq!(t.description.as_deref(), Some("CPU"));
        assert_eq!(t.location, "Processor");
        assert_eq!(t.status, "OK");
        assert_eq!(t.max_tenth_c, Some(400));
        assert_eq!(t.min_tenth_c, Some(-50));
        assert_eq!(t.nominal_tenth_c, Some(380));
        assert_eq!(
            recs.iter().find(|r| r.kind == 28).map(|r| r.kind_name.as_str()),
            Some("Temperature Probe")
        );
    }

    #[test]
    fn type28_8000_is_unknown() {
        let mut rec = vec![0u8; 0x16];
        rec[0] = 28;
        rec[1] = 0x16;
        rec[0x05] = 0x87; // Non-critical + Motherboard
        rec[0x06] = 0x00;
        rec[0x07] = 0x80;
        rec[0x08] = 0x00;
        rec[0x09] = 0x80;
        rec[0x14] = 0x00;
        rec[0x15] = 0x80;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let t = recs
            .iter()
            .find(|r| r.kind == 28)
            .and_then(|r| temperature_from_raw(&rec, r))
            .expect("type 28");
        assert_eq!(t.max_tenth_c, None);
        assert_eq!(t.min_tenth_c, None);
        assert_eq!(t.nominal_tenth_c, None);
        assert_eq!(t.location, "Motherboard");
        assert_eq!(t.status, "Non-critical");
    }

    #[test]
    fn type3_rack_mount_lock_height_and_sku() {
        let mut rec = vec![0u8; 0x16];
        rec[0] = 3;
        rec[1] = 0x16;
        rec[0x04] = 1;
        rec[0x05] = 0x97; // lock + Rack Mount Chassis 0x17
        rec[0x06] = 2;
        rec[0x07] = 3;
        rec[0x08] = 4;
        rec[0x09] = 0x03; // Safe
        rec[0x0A] = 0x03;
        rec[0x0B] = 0x04; // Warning
        rec[0x0C] = 0x03; // None
        rec[0x11] = 2; // 2U
        rec[0x12] = 2;
        rec[0x13] = 0;
        rec[0x14] = 0;
        rec[0x15] = 5;
        rec.extend_from_slice(b"Dell\0R1\0ABC123\0TAG1\0SKU-9\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 3)
            .and_then(|r| chassis_from_raw(&rec, r))
            .expect("type 3");
        assert_eq!(c.manufacturer.as_deref(), Some("Dell"));
        assert_eq!(c.kind, "Rack Mount Chassis");
        assert!(c.locked);
        assert_eq!(c.version.as_deref(), Some("R1"));
        assert_eq!(c.serial.as_deref(), Some("ABC123"));
        assert_eq!(c.asset_tag.as_deref(), Some("TAG1"));
        assert_eq!(c.boot_state.as_deref(), Some("Safe"));
        assert_eq!(c.thermal_state.as_deref(), Some("Warning"));
        assert_eq!(c.security.as_deref(), Some("None"));
        assert_eq!(c.height_u, Some(2));
        assert_eq!(c.power_cords, Some(2));
        assert_eq!(c.sku.as_deref(), Some("SKU-9"));
        assert_eq!(
            recs.iter().find(|r| r.kind == 3).map(|r| r.kind_name.as_str()),
            Some("Chassis")
        );
    }

    #[test]
    fn type3_zero_height_is_unspecified() {
        let mut rec = vec![0u8; 0x13];
        rec[0] = 3;
        rec[1] = 0x13;
        rec[0x05] = 0x03; // Desktop, unlocked
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 3)
            .and_then(|r| chassis_from_raw(&rec, r))
            .expect("type 3");
        assert_eq!(c.kind, "Desktop");
        assert!(!c.locked);
        assert_eq!(c.height_u, None);
        assert_eq!(c.power_cords, None);
        assert_eq!(c.sku, None);
    }

    #[test]
    fn type25_bcd_schedule_and_unspecified_month() {
        let mut rec = vec![0u8; 0x09];
        rec[0] = 25;
        rec[1] = 0x09;
        rec[0x04] = 0x12; // December
        rec[0x05] = 0x31;
        rec[0x06] = 0x23;
        rec[0x07] = 0x59;
        rec[0x08] = 0x00;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 25)
            .and_then(|r| power_controls_from_raw(&rec, r))
            .expect("type 25");
        assert_eq!(p.next_power_on.as_deref(), Some("12-31 23:59:00"));
        assert_eq!(
            recs.iter().find(|r| r.kind == 25).map(|r| r.kind_name.as_str()),
            Some("System Power Controls")
        );

        let mut rec = vec![0u8; 0x09];
        rec[0] = 25;
        rec[1] = 0x09;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 25)
            .and_then(|r| power_controls_from_raw(&rec, r))
            .expect("type 25 zero");
        assert_eq!(p.next_power_on, None);
    }

    #[test]
    fn type29_psu_ok_nominal_ma() {
        let mut rec = vec![0u8; 0x16];
        rec[0] = 29;
        rec[1] = 0x16;
        rec[0x04] = 1;
        rec[0x05] = 0x6A; // OK + Power Unit
        rec[0x06] = 0x10;
        rec[0x07] = 0x27; // 10000 mA
        rec[0x08] = 0x00;
        rec[0x09] = 0x80;
        rec[0x0A] = 0x00;
        rec[0x0B] = 0x80;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x80;
        rec[0x0E] = 0x00;
        rec[0x0F] = 0x80;
        rec[0x14] = 0xE8;
        rec[0x15] = 0x03; // 1000 mA
        rec.extend_from_slice(b"PSU\0\0");
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 29)
            .and_then(|r| current_from_raw(&rec, r))
            .expect("type 29");
        assert_eq!(c.description.as_deref(), Some("PSU"));
        assert_eq!(c.location, "Power Unit");
        assert_eq!(c.status, "OK");
        assert_eq!(c.max_ma, Some(10000));
        assert_eq!(c.min_ma, None);
        assert_eq!(c.nominal_ma, Some(1000));
        assert_eq!(
            recs.iter().find(|r| r.kind == 29).map(|r| r.kind_name.as_str()),
            Some("Electrical Current Probe")
        );
    }

    #[test]
    fn type29_zero_ma_is_zero_not_unknown() {
        let mut rec = vec![0u8; 0x14];
        rec[0] = 29;
        rec[1] = 0x14;
        rec[0x05] = 0x67; // OK + Motherboard
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let c = recs
            .iter()
            .find(|r| r.kind == 29)
            .and_then(|r| current_from_raw(&rec, r))
            .expect("type 29");
        assert_eq!(c.max_ma, Some(0));
        assert_eq!(c.min_ma, Some(0));
        assert_eq!(c.nominal_ma, None);
    }

    #[test]
    fn type38_kcs_io_and_no_nv() {
        let mut rec = vec![0u8; 0x10];
        rec[0] = 38;
        rec[1] = 0x10;
        rec[0x04] = 0x01; // KCS
        rec[0x05] = 0x20; // spec 2.0
        rec[0x06] = 0x20;
        rec[0x07] = 0xFF;
        rec[0x08] = 0xA3; // I/O 0xCA2 (bit0=1)
        rec[0x09] = 0x0C;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let d = recs
            .iter()
            .find(|r| r.kind == 38)
            .and_then(|r| ipmi_from_raw(&rec, r))
            .expect("type 38");
        assert_eq!(d.interface, "KCS");
        assert_eq!(d.spec.as_deref(), Some("2.0"));
        assert_eq!(d.i2c_address, 0x20);
        assert_eq!(d.nv_storage, None);
        assert_eq!(d.base_address, "I/O 0xCA2");
        assert_eq!(
            recs.iter().find(|r| r.kind == 38).map(|r| r.kind_name.as_str()),
            Some("IPMI Device")
        );
    }

    #[test]
    fn type18_ok_read_and_unknown_addrs() {
        let mut rec = vec![0u8; 0x17];
        rec[0] = 18;
        rec[1] = 0x17;
        rec[0x04] = 0x03; // OK
        rec[0x05] = 0x03; // Device level
        rec[0x06] = 0x03; // Read
        rec[0x07] = 0x78;
        rec[0x08] = 0x56;
        rec[0x09] = 0x34;
        rec[0x0A] = 0x12;
        rec[0x0B] = 0x00;
        rec[0x0C] = 0x10;
        rec[0x0D] = 0x00;
        rec[0x0E] = 0x00; // array 0x1000
        rec[0x0F] = 0x00;
        rec[0x10] = 0x00;
        rec[0x11] = 0x00;
        rec[0x12] = 0x80; // device 0x80000000 unknown
        rec[0x13] = 0x08;
        rec[0x14] = 0x00;
        rec[0x15] = 0x00;
        rec[0x16] = 0x00; // resolution 8
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let e = recs
            .iter()
            .find(|r| r.kind == 18)
            .and_then(|r| memory_error_from_raw(&rec, r))
            .expect("type 18");
        assert_eq!(e.error_type, "OK");
        assert_eq!(e.granularity, "Device level");
        assert_eq!(e.operation, "Read");
        assert_eq!(e.syndrome, Some(0x1234_5678));
        assert_eq!(e.array_address, Some(0x1000));
        assert_eq!(e.device_address, None);
        assert_eq!(e.resolution, Some(8));
        assert_eq!(
            recs.iter().find(|r| r.kind == 18).map(|r| r.kind_name.as_str()),
            Some("32-bit Memory Error")
        );
    }

    #[test]
    fn type18_zero_syndrome_and_80000000_unknown() {
        let mut rec = vec![0u8; 0x17];
        rec[0] = 18;
        rec[1] = 0x17;
        rec[0x04] = 0x06; // Single-bit error
        rec[0x05] = 0x04;
        rec[0x06] = 0x04;
        rec[0x0B] = 0x00;
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x00;
        rec[0x0E] = 0x80;
        rec[0x0F] = 0x00;
        rec[0x10] = 0x00;
        rec[0x11] = 0x00;
        rec[0x12] = 0x80;
        rec[0x13] = 0x00;
        rec[0x14] = 0x00;
        rec[0x15] = 0x00;
        rec[0x16] = 0x80;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let e = recs
            .iter()
            .find(|r| r.kind == 18)
            .and_then(|r| memory_error_from_raw(&rec, r))
            .expect("type 18");
        assert_eq!(e.error_type, "Single-bit error");
        assert_eq!(e.granularity, "Memory partition level");
        assert_eq!(e.operation, "Write");
        assert_eq!(e.syndrome, None);
        assert_eq!(e.array_address, None);
        assert_eq!(e.device_address, None);
        assert_eq!(e.resolution, None);
    }

    #[test]
    fn type19_kb_range_inclusive_end() {
        let mut rec = vec![0u8; 0x0F];
        rec[0] = 19;
        rec[1] = 0x0F;
        rec[0x08] = 0xFF;
        rec[0x09] = 0xFF;
        rec[0x0A] = 0x0F;
        rec[0x0B] = 0x00; // end 0x000FFFFF KB
        rec[0x0C] = 0x00;
        rec[0x0D] = 0x10; // array handle 0x1000
        rec[0x0E] = 2;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let m = recs
            .iter()
            .find(|r| r.kind == 19)
            .and_then(|r| mapped_from_raw(&rec, r))
            .expect("type 19");
        assert_eq!(m.start, Some(0));
        assert_eq!(m.end, Some(0x3FFF_FFFF));
        assert_eq!(m.array_handle, 0x1000);
        assert_eq!(m.partition_width, 2);
        assert_eq!(hex_range(m.start, m.end), "0x0-0x3FFFFFFF");
        assert_eq!(
            recs.iter().find(|r| r.kind == 19).map(|r| r.kind_name.as_str()),
            Some("Memory Mapped Address")
        );
    }

    #[test]
    fn type19_ffffffff_reads_extended_qwords() {
        let mut rec = vec![0u8; 0x1F];
        rec[0] = 19;
        rec[1] = 0x1F;
        rec[0x04] = 0xFF;
        rec[0x05] = 0xFF;
        rec[0x06] = 0xFF;
        rec[0x07] = 0xFF;
        rec[0x08] = 0xFF;
        rec[0x09] = 0xFF;
        rec[0x0A] = 0xFF;
        rec[0x0B] = 0xFF;
        rec[0x0C] = 0x10;
        rec[0x0D] = 0x00;
        rec[0x0E] = 1;
        rec[0x0F] = 0x00;
        rec[0x10] = 0x00;
        rec[0x11] = 0x00;
        rec[0x12] = 0x00;
        rec[0x13] = 0x01; // start 0x1_0000_0000
        rec[0x17] = 0xFF;
        rec[0x18] = 0xFF;
        rec[0x19] = 0xFF;
        rec[0x1A] = 0xFF;
        rec[0x1B] = 0x01; // end 0x1_FFFF_FFFF
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let m = recs
            .iter()
            .find(|r| r.kind == 19)
            .and_then(|r| mapped_from_raw(&rec, r))
            .expect("type 19 ext");
        assert_eq!(m.start, Some(0x1_0000_0000));
        assert_eq!(m.end, Some(0x1_FFFF_FFFF));
        assert_eq!(m.array_handle, 0x0010);
        assert_eq!(m.partition_width, 1);
    }

    #[test]
    fn type21_ps2_mouse_and_usb_touchpad() {
        let mut rec = vec![0u8; 0x07];
        rec[0] = 21;
        rec[1] = 0x07;
        rec[0x04] = 0x03; // Mouse
        rec[0x05] = 0x04; // PS/2
        rec[0x06] = 3;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 21)
            .and_then(|r| pointing_from_raw(&rec, r))
            .expect("type 21");
        assert_eq!(p.kind, "Mouse");
        assert_eq!(p.interface, "PS/2");
        assert_eq!(p.buttons, 3);
        assert_eq!(
            recs.iter().find(|r| r.kind == 21).map(|r| r.kind_name.as_str()),
            Some("Built-in Pointing Device")
        );

        let mut rec = vec![0u8; 0x07];
        rec[0] = 21;
        rec[1] = 0x07;
        rec[0x04] = 0x07; // Touch Pad
        rec[0x05] = 0xA2; // USB
        rec[0x06] = 2;
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&[127u8, 4, 0, 0, 0, 0]);
        let recs = parse_smbios(&rec);
        let p = recs
            .iter()
            .find(|r| r.kind == 21)
            .and_then(|r| pointing_from_raw(&rec, r))
            .expect("type 21 usb");
        assert_eq!(p.kind, "Touch Pad");
        assert_eq!(p.interface, "USB");
        assert_eq!(p.buttons, 2);
    }
}
