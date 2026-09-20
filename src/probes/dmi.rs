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
        9 => "System Slot",
        16 => "Memory Array",
        17 => "Memory Device",
        19 => "Memory Mapped Address",
        32 => "Boot",
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
}
