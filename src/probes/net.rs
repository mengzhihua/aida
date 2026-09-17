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
    pub sockstat: SockStat,
    pub snmp: Snmp,
    pub softnet: Softnet,
    pub bridges: Vec<Bridge>,
    pub bonds: Vec<Bond>,
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
    pub rx_queues: usize,
    pub tx_queues: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SockStat {
    pub sockets_used: Option<u64>,
    pub tcp_inuse: Option<u64>,
    pub tcp_tw: Option<u64>,
    pub udp_inuse: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snmp {
    pub ip_forwarding: Option<u64>,
    pub ip_in_receives: Option<u64>,
    pub ip_in_delivers: Option<u64>,
    pub ip_out_requests: Option<u64>,
    pub tcp_curr_estab: Option<u64>,
    pub tcp_in_segs: Option<u64>,
    pub tcp_out_segs: Option<u64>,
    pub tcp_retrans: Option<u64>,
    pub udp_in: Option<u64>,
    pub udp_out: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Softnet {
    pub processed: u64,
    pub dropped: u64,
    pub time_squeeze: u64,
    pub cpus: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Bridge {
    pub name: String,
    pub bridge_id: Sample<String>,
    pub stp_state: Sample<String>,
    pub forward_delay: Sample<String>,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Bond {
    pub name: String,
    pub mode: Sample<String>,
    pub slaves: Sample<String>,
    pub active_slave: Sample<String>,
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
    let snmp = parse_snmp(&access::read_trimmed(ctx.proc_path("net/snmp")));
    let softnet = parse_softnet(&access::read_trimmed(ctx.proc_path("net/softnet_stat")));
    let sockstat = parse_sockstat(&access::read_trimmed(ctx.proc_path("net/sockstat")));
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
                sockstat,
                snmp,
                softnet,
                bridges: Vec::new(),
                bonds: Vec::new(),
                notes,
            };
        }
    };
    let mut interfaces = Vec::new();
    let mut bridges = Vec::new();
    let mut bonds = Vec::new();
    for name in names {
        let dir = root.join(&name);
        let kind_code = access::read_trimmed(dir.join("type"));
        let kind = {
            let mut k = match kind_code
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
            if dir.join("bridge").is_dir() {
                k = "Bridge".into();
            } else if dir.join("bonding").is_dir() {
                k = "Bond".into();
            }
            k
        };
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
        let (rx_queues, tx_queues) = count_queues(&dir.join("queues"));
        if dir.join("bridge").is_dir() {
            let br = dir.join("bridge");
            let members = access::list_dir_names(dir.join("brif"))
                .value
                .unwrap_or_default();
            bridges.push(Bridge {
                bridge_id: access::read_trimmed(br.join("bridge_id")),
                stp_state: access::read_trimmed(br.join("stp_state")),
                forward_delay: access::read_trimmed(br.join("forward_delay")),
                members,
                name: name.clone(),
            });
        }
        if dir.join("bonding").is_dir() {
            let b = dir.join("bonding");
            bonds.push(Bond {
                mode: access::read_trimmed(b.join("mode")),
                slaves: access::read_trimmed(b.join("slaves")),
                active_slave: access::read_trimmed(b.join("active_slave")),
                name: name.clone(),
            });
        }
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
            rx_queues,
            tx_queues,
            name,
        });
    }
    NetReport {
        interfaces,
        sockstat,
        snmp,
        softnet,
        bridges,
        bonds,
        notes,
    }
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

fn count_queues(dir: &std::path::Path) -> (usize, usize) {
    let names = match access::list_dir_names(dir).value {
        Some(n) => n,
        None => return (0, 0),
    };
    let rx = names.iter().filter(|n| n.starts_with("rx-")).count();
    let tx = names.iter().filter(|n| n.starts_with("tx-")).count();
    (rx, tx)
}

pub fn parse_sockstat(sample: &Sample<String>) -> SockStat {
    let Some(text) = sample.value.as_deref() else {
        return SockStat::default();
    };
    let mut out = SockStat::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(kind) = it.next() else {
            continue;
        };
        let kind = kind.trim_end_matches(':');
        let mut map = std::collections::BTreeMap::new();
        while let (Some(k), Some(v)) = (it.next(), it.next()) {
            if let Ok(n) = v.parse::<u64>() {
                map.insert(k, n);
            }
        }
        match kind {
            "sockets" => out.sockets_used = map.get("used").copied(),
            "TCP" => {
                out.tcp_inuse = map.get("inuse").copied();
                out.tcp_tw = map.get("tw").copied();
            }
            "UDP" => out.udp_inuse = map.get("inuse").copied(),
            _ => {}
        }
    }
    out
}

/// `/proc/net/snmp` 两行一组：字段名行 + 数值行。
pub fn parse_snmp(sample: &Sample<String>) -> Snmp {
    let Some(text) = sample.value.as_deref() else {
        return Snmp::default();
    };
    let mut map = std::collections::BTreeMap::<String, i64>::new();
    let mut lines = text.lines().peekable();
    while let Some(header) = lines.next() {
        let Some(values) = lines.next() else { break };
        let mut h = header.split_whitespace();
        let mut v = values.split_whitespace();
        let Some(proto) = h.next() else { continue };
        let _ = v.next(); // same proto token
        let proto = proto.trim_end_matches(':');
        for (key, val) in h.zip(v) {
            if let Ok(n) = val.parse::<i64>() {
                map.insert(format!("{proto}.{key}"), n);
            }
        }
    }
    let pick = |k: &str| map.get(k).copied().map(|n| n.max(0) as u64);
    Snmp {
        ip_forwarding: pick("Ip.Forwarding"),
        ip_in_receives: pick("Ip.InReceives"),
        ip_in_delivers: pick("Ip.InDelivers"),
        ip_out_requests: pick("Ip.OutRequests"),
        tcp_curr_estab: pick("Tcp.CurrEstab"),
        tcp_in_segs: pick("Tcp.InSegs"),
        tcp_out_segs: pick("Tcp.OutSegs"),
        tcp_retrans: pick("Tcp.RetransSegs"),
        udp_in: pick("Udp.InDatagrams"),
        udp_out: pick("Udp.OutDatagrams"),
    }
}

/// `/proc/net/softnet_stat`：每 CPU 一行十六进制，列 0 processed、1 dropped、2 time_squeeze。
pub fn parse_softnet(sample: &Sample<String>) -> Softnet {
    let Some(text) = sample.value.as_deref() else {
        return Softnet::default();
    };
    let mut out = Softnet::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(processed) = it.next().and_then(|s| u64::from_str_radix(s, 16).ok()) else {
            continue;
        };
        let dropped = it
            .next()
            .and_then(|s| u64::from_str_radix(s, 16).ok())
            .unwrap_or(0);
        let squeeze = it
            .next()
            .and_then(|s| u64::from_str_radix(s, 16).ok())
            .unwrap_or(0);
        out.processed += processed;
        out.dropped += dropped;
        out.time_squeeze += squeeze;
        out.cpus += 1;
    }
    out
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
        fs::create_dir_all(iface.join("queues/rx-0")).unwrap();
        fs::create_dir_all(iface.join("queues/tx-0")).unwrap();
        fs::create_dir_all(root.join("proc/net")).unwrap();
        fs::write(
            root.join("proc/net/sockstat"),
            "sockets: used 12\nTCP: inuse 3 orphan 0 tw 1 alloc 4 mem 0\nUDP: inuse 2 mem 0\n",
        )
        .unwrap();
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
        assert_eq!(r.interfaces[0].rx_queues, 1);
        assert_eq!(r.interfaces[0].tx_queues, 1);
        assert_eq!(r.sockstat.tcp_inuse, Some(3));
        assert_eq!(r.sockstat.udp_inuse, Some(2));
        fs::create_dir_all(iface.join("wireless")).unwrap();
        let r2 = collect(&ctx);
        assert!(r2.interfaces[0].wireless);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snmp_softnet_and_bridge() {
        let snmp = parse_snmp(&Sample::ok(
            "Ip: Forwarding InReceives InDelivers OutRequests\nIp: 1 10 9 8\nTcp: CurrEstab InSegs OutSegs RetransSegs\nTcp: 3 100 90 1\nUdp: InDatagrams OutDatagrams\nUdp: 4 5\n".into(),
            "snmp",
        ));
        assert_eq!(snmp.ip_forwarding, Some(1));
        assert_eq!(snmp.tcp_curr_estab, Some(3));
        assert_eq!(snmp.udp_out, Some(5));
        let sn = parse_softnet(&Sample::ok(
            "0000000a 00000001 00000002 0 0 0\n00000005 00000000 00000000 0 0 0\n".into(),
            "softnet",
        ));
        assert_eq!(sn.processed, 15);
        assert_eq!(sn.dropped, 1);
        assert_eq!(sn.time_squeeze, 2);
        assert_eq!(sn.cpus, 2);

        let root = std::env::temp_dir().join(format!("aida-net-br-{}", std::process::id()));
        let br = root.join("sys/class/net/br0");
        fs::create_dir_all(br.join("statistics")).unwrap();
        fs::create_dir_all(br.join("bridge")).unwrap();
        fs::create_dir_all(br.join("brif/eth0")).unwrap();
        fs::write(br.join("type"), "1\n").unwrap();
        fs::write(br.join("operstate"), "up\n").unwrap();
        fs::write(br.join("bridge/bridge_id"), "8000.aabb\n").unwrap();
        fs::write(br.join("bridge/stp_state"), "0\n").unwrap();
        fs::write(br.join("statistics/rx_bytes"), "0\n").unwrap();
        fs::write(br.join("statistics/tx_bytes"), "0\n").unwrap();
        fs::write(br.join("statistics/rx_packets"), "0\n").unwrap();
        fs::write(br.join("statistics/tx_packets"), "0\n").unwrap();
        fs::write(br.join("statistics/rx_errors"), "0\n").unwrap();
        fs::write(br.join("statistics/tx_errors"), "0\n").unwrap();
        fs::create_dir_all(root.join("proc/net")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.interfaces[0].kind, "Bridge");
        assert_eq!(r.bridges[0].name, "br0");
        assert_eq!(r.bridges[0].members, vec!["eth0"]);
        let _ = fs::remove_dir_all(&root);
    }
}
