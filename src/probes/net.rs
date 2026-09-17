//! 网络接口：`/sys/class/net` + `/proc/net/dev` 计数，地址用 `getifaddrs`（不是 `ip`/`ifconfig`）。

use std::collections::BTreeMap;
use std::ffi::CStr;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ptr;

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct NetReport {
    pub interfaces: Vec<NetIface>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetIface {
    pub name: String,
    pub operstate: Sample<String>,
    pub mac: Sample<String>,
    pub mtu: Sample<u64>,
    pub speed_mbps: Sample<i64>,
    pub duplex: Sample<String>,
    pub carrier: Sample<String>,
    pub kind: String,
    pub driver: Sample<String>,
    pub rx_bytes: Sample<u64>,
    pub tx_bytes: Sample<u64>,
    pub rx_packets: Sample<u64>,
    pub tx_packets: Sample<u64>,
    pub rx_errors: Sample<u64>,
    pub tx_errors: Sample<u64>,
    /// 两次采样之间的速率；单次 collect 未差分时为 None。
    pub rx_bps: Option<f64>,
    pub tx_bps: Option<f64>,
    pub addresses: Vec<String>,
    pub wireless: bool,
}

#[derive(Clone, Debug)]
pub struct NetSnap {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

pub fn collect(ctx: &ProbeCtx) -> NetReport {
    collect_with_prev(ctx, None, 0.0)
}

pub fn collect_with_prev(ctx: &ProbeCtx, prev: Option<&[NetSnap]>, dt_sec: f64) -> NetReport {
    let mut notes = Vec::new();
    let addrs = interface_addresses();
    let root = ctx.sys_path("class/net");
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        s => {
            notes.push(s.access_label());
            return NetReport {
                interfaces: Vec::new(),
                notes,
            };
        }
    };
    let mut interfaces = Vec::new();
    for name in names {
        let dir = root.join(&name);
        let kind_code = access::read_trimmed(dir.join("type"));
        let kind = match kind_code
            .value
            .as_deref()
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(1) => "Ethernet",
            Some(772) => "Loopback",
            Some(776) => "Sit / tunnel",
            Some(778) => "GRE",
            Some(803) => "IEEE 802.11",
            Some(823) => "WireGuard",
            Some(n) => {
                let _ = n;
                "Other"
            }
            None => "Unknown",
        }
        .to_string();
        let speed = match access::read_trimmed(dir.join("speed")) {
            Sample {
                access: AccessKind::Ok,
                value: Some(s),
                source,
                ..
            } => match s.parse::<i64>() {
                Ok(v) if v >= 0 => Sample::ok(v, source),
                Ok(_) => Sample::unsupported(source, "speed=-1 表示内核未知（虚拟网卡常见）"),
                Err(_) => Sample::error(source, "无法解析 speed"),
            },
            s => Sample {
                value: None,
                access: s.access,
                source: s.source,
                hint: s.hint,
            },
        };
        let stats = dir.join("statistics");
        let rx = access::read_u64(stats.join("rx_bytes"));
        let tx = access::read_u64(stats.join("tx_bytes"));
        let (rx_bps, tx_bps) = match (prev, rx.value, tx.value) {
            (Some(p), Some(rxb), Some(txb)) if dt_sec > 0.0 => {
                if let Some(old) = p.iter().find(|x| x.name == name) {
                    (
                        Some((rxb.saturating_sub(old.rx_bytes) as f64) / dt_sec),
                        Some((txb.saturating_sub(old.tx_bytes) as f64) / dt_sec),
                    )
                } else {
                    (None, None)
                }
            }
            _ => (None, None),
        };
        interfaces.push(NetIface {
            kind,
            driver: read_driver(&dir),
            operstate: access::read_trimmed(dir.join("operstate")),
            mac: access::read_trimmed(dir.join("address")),
            mtu: access::read_u64(dir.join("mtu")),
            speed_mbps: speed,
            duplex: access::read_trimmed(dir.join("duplex")),
            carrier: access::read_trimmed(dir.join("carrier")),
            rx_packets: access::read_u64(stats.join("rx_packets")),
            tx_packets: access::read_u64(stats.join("tx_packets")),
            rx_errors: access::read_u64(stats.join("rx_errors")),
            tx_errors: access::read_u64(stats.join("tx_errors")),
            rx_bytes: rx,
            tx_bytes: tx,
            rx_bps,
            tx_bps,
            addresses: addrs.get(&name).cloned().unwrap_or_default(),
            wireless: dir.join("wireless").exists(),
            name,
        });
    }
    NetReport { interfaces, notes }
}

pub fn counters(report: &NetReport) -> Vec<NetSnap> {
    report
        .interfaces
        .iter()
        .filter_map(|i| {
            Some(NetSnap {
                name: i.name.clone(),
                rx_bytes: i.rx_bytes.value?,
                tx_bytes: i.tx_bytes.value?,
            })
        })
        .collect()
}

fn read_driver(dir: &std::path::Path) -> Sample<String> {
    let link = dir.join("device/driver");
    match fs::read_link(&link) {
        Ok(p) => Sample::ok(
            p.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            link.display().to_string(),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Sample::missing(link.display().to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Sample::denied(link.display().to_string())
        }
        Err(e) => Sample::error(link.display().to_string(), e.to_string()),
    }
}

fn interface_addresses() -> BTreeMap<String, Vec<String>> {
    let mut head: *mut libc::ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return BTreeMap::new();
    }
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut p = head;
    while !p.is_null() {
        let ifa = unsafe { &*p };
        if !ifa.ifa_name.is_null() && !ifa.ifa_addr.is_null() {
            let name = unsafe { CStr::from_ptr(ifa.ifa_name) }
                .to_string_lossy()
                .into_owned();
            if let Some(addr) = sockaddr_text(ifa.ifa_addr) {
                let list = map.entry(name).or_default();
                if !list.contains(&addr) {
                    list.push(addr);
                }
            }
        }
        p = ifa.ifa_next;
    }
    unsafe { libc::freeifaddrs(head) };
    map
}

fn sockaddr_text(sa: *const libc::sockaddr) -> Option<String> {
    if sa.is_null() {
        return None;
    }
    let family = unsafe { (*sa).sa_family as i32 };
    match family {
        libc::AF_INET => {
            let sin = unsafe { &*(sa as *const libc::sockaddr_in) };
            let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
            Some(ip.to_string())
        }
        libc::AF_INET6 => {
            let sin6 = unsafe { &*(sa as *const libc::sockaddr_in6) };
            let ip = Ipv6Addr::from(sin6.sin6_addr.s6_addr);
            Some(ip.to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reads_sysfs_fixture() {
        let root = std::env::temp_dir().join(format!("aida-net-{}", std::process::id()));
        let iface = root.join("sys/class/net/eth0");
        fs::create_dir_all(iface.join("statistics")).unwrap();
        fs::write(iface.join("operstate"), "up\n").unwrap();
        fs::write(iface.join("address"), "aa:bb:cc:dd:ee:ff\n").unwrap();
        fs::write(iface.join("mtu"), "1500\n").unwrap();
        fs::write(iface.join("type"), "1\n").unwrap();
        fs::write(iface.join("speed"), "1000\n").unwrap();
        fs::write(iface.join("statistics/rx_bytes"), "1000\n").unwrap();
        fs::write(iface.join("statistics/tx_bytes"), "2000\n").unwrap();
        fs::write(iface.join("statistics/rx_packets"), "10\n").unwrap();
        fs::write(iface.join("statistics/tx_packets"), "20\n").unwrap();
        fs::write(iface.join("statistics/rx_errors"), "0\n").unwrap();
        fs::write(iface.join("statistics/tx_errors"), "0\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let prev = [NetSnap {
            name: "eth0".into(),
            rx_bytes: 500,
            tx_bytes: 500,
        }];
        let r = collect_with_prev(&ctx, Some(&prev), 1.0);
        assert_eq!(r.interfaces.len(), 1);
        assert_eq!(r.interfaces[0].kind, "Ethernet");
        assert!(!r.interfaces[0].wireless);
        assert_eq!(r.interfaces[0].rx_bps, Some(500.0));
        assert_eq!(r.interfaces[0].tx_bps, Some(1500.0));
        fs::create_dir_all(iface.join("wireless")).unwrap();
        let r2 = collect(&ctx);
        assert!(r2.interfaces[0].wireless);
        let _ = fs::remove_dir_all(&root);
    }
}
