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
    pub memory_devices: Vec<MemoryDevice>,
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
pub struct MemoryDevice {
    pub locator: Option<String>,
    pub bank: Option<String>,
    pub size_mb: Option<u64>,
    pub r#type: Option<String>,
    pub speed_mts: Option<u16>,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub part: Option<String>,
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
        memory_devices: Vec::new(),
        notes: Vec::new(),
    };

    let table = access::read_bytes(ctx.sys_path("firmware/dmi/tables/DMI"));
    match table.access {
        AccessKind::Ok => {
            if let Some(bytes) = table.value.as_deref() {
                let parsed = parse_smbios(bytes);
                info.memory_devices = parsed
                    .iter()
                    .filter(|r| r.kind == 17)
                    .filter_map(|r| memory_from_raw(bytes, r))
                    .collect();
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

fn memory_from_raw(buf: &[u8], rec: &SmbiosRecord) -> Option<MemoryDevice> {
    // 再次扫描以拿到 formatted area。对 Type 17：
    // 0x0c size u16, 0x12 form factor, 0x15 locator string#, 0x17 bank#,
    // 0x12 speed u16 at 0x15? SMBIOS 2.1+ Memory Device:
    // offset 0x0C size, 0x0E extended size later
    // 0x12 form factor, 0x13 device set, 0x14 device locator string
    // 0x15 bank locator string, 0x16 memory type, 0x17 type detail
    // 0x18 speed u16
    // 0x1A manufacturer string, 0x1B serial, 0x1C asset, 0x1D part
    // 为稳妥起见只使用已经切出来的 strings，size 需要 formatted bytes。
    // 这里做一次轻量重扫：按 handle 找结构。
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        let kind = buf[i];
        let length = buf[i + 1] as usize;
        if length < 4 || i + length > buf.len() {
            break;
        }
        let handle = u16::from_le_bytes([buf[i + 2], buf[i + 3]]);
        if kind == 17 && handle == rec.handle && length >= 0x1A {
            let size_raw = u16::from_le_bytes([buf[i + 0x0C], buf[i + 0x0D]]);
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
                Some((size_raw & 0x7FFF) as u64 / 1024) // KB -> MB
            } else {
                Some(size_raw as u64)
            };
            let speed = if length >= 0x1A {
                let sp = u16::from_le_bytes([buf[i + 0x18], buf[i + 0x19]]);
                if sp == 0 { None } else { Some(sp) }
            } else {
                None
            };
            let mem_type = if length > 0x16 {
                Some(memory_type_name(buf[i + 0x16]).to_string())
            } else {
                None
            };
            return Some(MemoryDevice {
                locator: rec.strings.first().cloned(),
                bank: rec.strings.get(1).cloned(),
                size_mb,
                r#type: mem_type,
                speed_mts: speed,
                manufacturer: rec.strings.get(2).cloned(),
                serial: rec.strings.get(3).cloned(),
                part: rec.strings.get(5).cloned(),
            });
        }
        // skip strings like parse_smbios
        let mut s = i + length;
        loop {
            if s >= buf.len() {
                i = s;
                break;
            }
            if buf[s] == 0 {
                s += 1;
                if s >= buf.len() || buf[s] == 0 {
                    i = s + 1;
                    break;
                }
                continue;
            }
            while s < buf.len() && buf[s] != 0 {
                s += 1;
            }
            s += 1;
            if s < buf.len() && buf[s] == 0 {
                i = s + 1;
                break;
            }
        }
    }
    None
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
}
