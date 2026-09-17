//! 权限与 sysfs/procfs 读取原语。
//!
//! 设计要点：
//! - 每个字段都带 `AccessKind`，无权限时保留路径和提示，而不是静默丢弃。
//! - 探测代码只读文件，不调用 `dmidecode` / `lspci` / `smartctl`。
//! - `ProbeCtx` 把 `/proc` `/sys` `/dev` 做成可替换根，便于夹具测试。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// 单次读取结果：值可缺，原因必须可解释。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sample<T: Serialize> {
    pub value: Option<T>,
    pub access: AccessKind,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessKind {
    Ok,
    PermissionDenied,
    NotFound,
    Unsupported,
    Error,
}

impl<T: Serialize> Sample<T> {
    pub fn ok(value: T, source: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            access: AccessKind::Ok,
            source: source.into(),
            hint: None,
        }
    }

    pub fn missing(source: impl Into<String>) -> Self {
        let source = source.into();
        let hint = hint_for(&source, AccessKind::NotFound);
        Self {
            value: None,
            access: AccessKind::NotFound,
            source,
            hint,
        }
    }

    pub fn denied(source: impl Into<String>) -> Self {
        let source = source.into();
        let hint = hint_for(&source, AccessKind::PermissionDenied);
        Self {
            value: None,
            access: AccessKind::PermissionDenied,
            source,
            hint,
        }
    }

    pub fn unsupported(source: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            value: None,
            access: AccessKind::Unsupported,
            source: source.into(),
            hint: Some(hint.into()),
        }
    }

    pub fn error(source: impl Into<String>, err: impl Into<String>) -> Self {
        Self {
            value: None,
            access: AccessKind::Error,
            source: source.into(),
            hint: Some(err.into()),
        }
    }

    pub fn as_str(&self) -> Option<&str>
    where
        T: AsRef<str>,
    {
        self.value.as_ref().map(|v| v.as_ref())
    }

    pub fn display(&self) -> String
    where
        T: std::fmt::Display,
    {
        match (&self.value, self.access) {
            (Some(v), AccessKind::Ok) => v.to_string(),
            _ => self.access_label(),
        }
    }

    pub fn access_label(&self) -> String {
        match self.access {
            AccessKind::Ok => self
                .value
                .as_ref()
                .map(|_| String::from("ok"))
                .unwrap_or_else(|| String::from("empty")),
            AccessKind::PermissionDenied => {
                format!("[权限不足] {}", self.hint.as_deref().unwrap_or(&self.source))
            }
            AccessKind::NotFound => {
                format!("[不存在] {}", self.hint.as_deref().unwrap_or(&self.source))
            }
            AccessKind::Unsupported => {
                format!("[不支持] {}", self.hint.as_deref().unwrap_or(&self.source))
            }
            AccessKind::Error => format!("[读取失败] {}", self.hint.as_deref().unwrap_or("unknown")),
        }
    }
}

/// 文件系统根。生产环境用 `/`，测试时可指向临时夹具目录。
#[derive(Clone, Debug)]
pub struct ProbeCtx {
    pub proc: PathBuf,
    pub sys: PathBuf,
    pub dev: PathBuf,
    pub etc: PathBuf,
    pub usr_share: PathBuf,
}

impl Default for ProbeCtx {
    fn default() -> Self {
        Self::live()
    }
}

impl ProbeCtx {
    pub fn live() -> Self {
        Self {
            proc: PathBuf::from("/proc"),
            sys: PathBuf::from("/sys"),
            dev: PathBuf::from("/dev"),
            etc: PathBuf::from("/etc"),
            usr_share: PathBuf::from("/usr/share"),
        }
    }

    pub fn proc_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.proc.join(rel)
    }

    pub fn sys_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.sys.join(rel)
    }

    pub fn dev_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.dev.join(rel)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Privilege {
    pub uid: u32,
    pub euid: u32,
    pub is_root: bool,
    pub username: Option<String>,
    pub groups: Vec<GroupRef>,
    /// 面向 UI 的总提示：当前身份能看到什么、缺什么。
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupRef {
    pub gid: u32,
    pub name: Option<String>,
}

impl Privilege {
    pub fn detect() -> Self {
        let uid = unsafe { libc::getuid() };
        let euid = unsafe { libc::geteuid() };
        let username = username_for(uid);
        let groups = current_groups();
        let is_root = euid == 0;
        let in_disk = groups.iter().any(|g| g.name.as_deref() == Some("disk"));
        let summary = if is_root {
            "当前为 root，DMI/SMBIOS、NVMe SMART、部分传感器应可完整读取。".to_string()
        } else {
            let disk_hint = if in_disk {
                "已在 disk 组，块设备节点通常可读。"
            } else {
                "不在 disk 组，/dev/nvme* 的 SMART ioctl 可能失败。"
            };
            format!(
                "当前为普通用户 uid={uid} ({})。DMI 序列号/UUID、SMBIOS 表、NVMe Admin 命令可能被内核隐藏。{disk_hint} 完整检测请使用 sudo / pkexec。",
                username.as_deref().unwrap_or("unknown")
            )
        };
        Self {
            uid,
            euid,
            is_root,
            username,
            groups,
            summary,
        }
    }

    pub fn in_group(&self, name: &str) -> bool {
        self.groups.iter().any(|g| g.name.as_deref() == Some(name))
    }
}

pub fn read_trimmed(path: impl AsRef<Path>) -> Sample<String> {
    let path = path.as_ref();
    let source = path.display().to_string();
    match fs::read_to_string(path) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                Sample {
                    value: None,
                    access: AccessKind::Ok,
                    source,
                    hint: Some("文件存在但内容为空".into()),
                }
            } else {
                Sample::ok(t.to_string(), source)
            }
        }
        Err(e) => map_io_err(source, e),
    }
}

pub fn read_bytes(path: impl AsRef<Path>) -> Sample<Vec<u8>> {
    let path = path.as_ref();
    let source = path.display().to_string();
    match fs::read(path) {
        Ok(buf) if buf.is_empty() => Sample {
            value: None,
            access: AccessKind::Ok,
            source,
            hint: Some("文件存在但内容为空".into()),
        },
        Ok(buf) => Sample::ok(buf, source),
        Err(e) => map_io_err(source, e),
    }
}

pub fn dir_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().is_dir()
}

pub fn list_dir_names(path: impl AsRef<Path>) -> Sample<Vec<String>> {
    let path = path.as_ref();
    let source = path.display().to_string();
    match fs::read_dir(path) {
        Ok(rd) => {
            let mut names: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            names.sort();
            Sample::ok(names, source)
        }
        Err(e) => map_io_err(source, e),
    }
}

pub fn parse_u64_str(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn map_io_err<T: Serialize>(source: String, e: std::io::Error) -> Sample<T> {
    match e.kind() {
        ErrorKind::PermissionDenied => Sample::denied(source),
        ErrorKind::NotFound => Sample::missing(source),
        _ => Sample::error(source, e.to_string()),
    }
}

fn hint_for(source: &str, kind: AccessKind) -> Option<String> {
    match kind {
        AccessKind::PermissionDenied if source.contains("/sys/class/dmi") => Some(
            "多数发行版把 product_serial / product_uuid 设为 0400。用 root 运行，或读取 /sys/firmware/dmi/tables/DMI（通常也需要 root）。"
                .into(),
        ),
        AccessKind::PermissionDenied if source.contains("/sys/firmware/dmi") => {
            Some("SMBIOS 原始表默认仅 root 可读，这是为了保护机器序列号。".into())
        }
        AccessKind::PermissionDenied if source.contains("/dev/nvme") => Some(
            "NVMe Admin ioctl 需要 /dev/nvmeX 的读写权限。把用户加入 disk 组，或使用 sudo。".into(),
        ),
        AccessKind::NotFound if source.contains("/sys/class/dmi") => Some(
            "容器或精简虚拟机可能不导出 DMI。裸机/KVM 通常存在 /sys/class/dmi/id 或 /sys/firmware/dmi/tables。"
                .into(),
        ),
        AccessKind::NotFound if source.contains("/sys/class/hwmon") => Some(
            "无 hwmon 节点：常见于容器、部分云主机，或内核未加载核心温度驱动（coretemp/k10temp/zenpower）。"
                .into(),
        ),
        AccessKind::NotFound if source.contains("/sys/class/nvme") => {
            Some("未发现 NVMe 控制器。SATA/Virtio 盘走 /sys/block，不走 nvme 类。".into())
        }
        _ => None,
    }
}

fn username_for(uid: u32) -> Option<String> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let mut parts = line.split(':');
        let name = parts.next()?;
        let _pw = parts.next()?;
        let id = parts.next()?.parse::<u32>().ok()?;
        if id == uid {
            return Some(name.to_string());
        }
    }
    None
}

fn current_groups() -> Vec<GroupRef> {
    // NGROUPS_MAX 在 Linux 上通常 65536，这里取一个实际够用的缓冲。
    let mut buf = vec![0 as libc::gid_t; 64];
    let n = unsafe { libc::getgroups(buf.len() as libc::c_int, buf.as_mut_ptr()) };
    if n < 0 {
        return Vec::new();
    }
    buf.truncate(n as usize);
    let names = group_names();
    buf.into_iter()
        .map(|gid| GroupRef {
            gid,
            name: names.iter().find(|(id, _)| *id == gid).map(|(_, n)| n.clone()),
        })
        .collect()
}

fn group_names() -> Vec<(u32, String)> {
    let Ok(text) = fs::read_to_string("/etc/group") else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split(':');
            let name = parts.next()?.to_string();
            let _pw = parts.next()?;
            let gid = parts.next()?.parse::<u32>().ok()?;
            Some((gid, name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn read_ok_and_missing() {
        let dir = std::env::temp_dir().join(format!("aida-access-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("name");
        fs::write(&f, "  hwmon0 \n").unwrap();
        let s = read_trimmed(&f);
        assert_eq!(s.access, AccessKind::Ok);
        assert_eq!(s.value.as_deref(), Some("hwmon0"));
        let missing = read_trimmed(dir.join("nope"));
        assert_eq!(missing.access, AccessKind::NotFound);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_denied() {
        let dir = std::env::temp_dir().join(format!("aida-access-deny-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("secret");
        fs::write(&f, "serial\n").unwrap();
        let mut perms = fs::metadata(&f).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&f, perms).unwrap();
        // root 会绕过 mode bit，非 root 才应得到 PermissionDenied。
        let s = read_trimmed(&f);
        if unsafe { libc::geteuid() } == 0 {
            assert_eq!(s.access, AccessKind::Ok);
        } else {
            assert_eq!(s.access, AccessKind::PermissionDenied);
            assert!(s.hint.is_some() || s.source.contains("secret"));
        }
        let mut perms = fs::metadata(&f).unwrap().permissions();
        perms.set_mode(0o644);
        let _ = fs::set_permissions(&f, perms);
        let _ = fs::remove_dir_all(&dir);
    }
}
