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
    pub conntrack_count: Sample<u64>,
    pub conntrack_max: Sample<u64>,
    pub tcp_congestion: Sample<String>,
    pub tcp_available_congestion: Sample<String>,
    pub tcpext: TcpExt,
    pub snmp6: Snmp6,
    pub sockstat6: SockStat6,
    pub somaxconn: Sample<u64>,
    pub netdev_max_backlog: Sample<u64>,
    pub rmem_max: Sample<u64>,
    pub wmem_max: Sample<u64>,
    pub arp_entries: usize,
    pub route_entries: usize,
    pub unix_sockets: usize,
    pub ipv6_routes: usize,
    pub inet6_addrs: usize,
    pub packet_sockets: usize,
    pub tcp_fastopen: Sample<String>,
    pub tcp_syncookies: Sample<String>,
    pub ip_local_port_range: Sample<String>,
    pub tcp: TcpTune,
    pub default_qdisc: Sample<String>,
    pub ipv6_disable: Sample<String>,
    pub ipv6_forwarding: Sample<String>,
    pub protocols: Vec<String>,
    pub igmp_ifaces: usize,
    pub netdev_budget: Sample<u64>,
    pub rp_filter: Sample<String>,
    pub icmp_echo_ignore_broadcasts: Sample<String>,
    pub ipv6_use_tempaddr: Sample<String>,
    /// 与 `conf/all` 不同的接口值，例如 `eth0:1`。
    pub rp_filter_dev: Vec<String>,
    pub ipv6_use_tempaddr_dev: Vec<String>,
    pub rt6_entries: Sample<u64>,
    pub optmem_max: Sample<u64>,
    pub netdev_budget_usecs: Sample<u64>,
    pub accept_redirects: Sample<String>,
    pub accept_source_route: Sample<String>,
    pub log_martians: Sample<String>,
    pub icmp_ignore_bogus: Sample<String>,
    pub netlink_sockets: usize,
    pub tcp_socks: usize,
    pub udp_socks: usize,
    pub tcp6_socks: usize,
    pub udp6_socks: usize,
    pub raw_socks: usize,
    pub udplite_socks: usize,
    pub xfrm_in_no_states: Option<u64>,
    pub xfrm_out_no_states: Option<u64>,
    pub ptypes: Vec<String>,
    pub fib_trie_leaves: Option<u64>,
    pub busy_poll: Sample<u64>,
    pub dev_weight: Sample<u64>,
    pub unix_max_dgram_qlen: Sample<u64>,
    pub igmp6_ifaces: usize,
    pub raw6_socks: usize,
    pub udplite6_socks: usize,
    pub iptables: Vec<String>,
    pub ip6tables: Vec<String>,
    pub connectors: Vec<String>,
    pub ipv6_accept_ra: Sample<String>,
    pub ipv6_autoconf: Sample<String>,
    pub ipv6_hop_limit: Sample<u64>,
    pub conntrack_tcp_established: Sample<u64>,
    pub conntrack_buckets: Sample<u64>,
    pub tcp_max_tw_buckets: Sample<u64>,
    pub busy_read: Sample<u64>,
    pub icmp_ratelimit: Sample<u64>,
    pub ip_default_ttl: Sample<u64>,
    /// 三个页数：min / pressure / max。
    pub tcp_mem: Sample<String>,
    pub udp_mem: Sample<String>,
    pub tcp_max_orphans: Sample<u64>,
    pub tcp_dsack: Sample<String>,
    pub tcp_autocorking: Sample<String>,
    pub ipv6_accept_dad: Sample<String>,
    /// `0` EUI64，`1` none，`2` stable-privacy，`3` random。
    pub ipv6_addr_gen_mode: Sample<String>,
    /// 与 `conf/all` 不同的接口值。
    pub ipv6_accept_dad_dev: Vec<String>,
    pub ipv6_addr_gen_mode_dev: Vec<String>,
    pub ip6frag_high_thresh: Sample<u64>,
    pub rps_sock_flow_entries: Sample<u64>,
    pub ipfrag_high_thresh: Sample<u64>,
    pub ipfrag_low_thresh: Sample<u64>,
    pub tcp_early_retrans: Sample<u64>,
    pub ip_no_pmtu_disc: Sample<String>,
    pub fib_multipath_hash_policy: Sample<u64>,
    pub ipv6_max_addresses: Sample<u64>,
    pub tcp_frto: Sample<String>,
    pub tcp_invalid_ratelimit: Sample<u64>,
    pub tcp_min_tso_segs: Sample<u64>,
    pub tcp_pacing_ss_ratio: Sample<u64>,
    pub netdev_tstamp_prequeue: Sample<String>,
    /// `0` 关闭内核网络日志限速。
    pub message_cost: Sample<u64>,
    pub ipv6_accept_ra_defrtr: Sample<String>,
    /// `-1` 表示使用 RFC 默认次数。
    pub ipv6_router_solicitations: Sample<i64>,
    pub ipv6_accept_ra_defrtr_dev: Vec<String>,
    pub ipv6_router_solicitations_dev: Vec<String>,
    pub ip6frag_low_thresh: Sample<u64>,
    pub ip_unprivileged_port_start: Sample<u64>,
    pub tcp_pacing_ca_ratio: Sample<u64>,
    pub tcp_orphan_retries: Sample<u64>,
    pub tcp_rfc1337: Sample<String>,
    pub bindv6only: Sample<String>,
    pub ipfrag_time: Sample<u64>,
    pub ipv6_dad_transmits: Sample<u64>,
    /// 与 `conf/all` 不同的接口。
    pub ipv6_dad_transmits_dev: Vec<String>,
    /// 与 `message_cost` 成对；`cost=0` 时 burst 不生效。
    pub message_burst: Sample<u64>,
    pub tcp_ecn_fallback: Sample<String>,
    pub ip_nonlocal_bind: Sample<String>,
    pub icmp_echo_ignore_all: Sample<String>,
    pub ipfrag_max_dist: Sample<u64>,
    pub tcp_abort_on_overflow: Sample<String>,
    pub tcp_no_metrics_save: Sample<String>,
    /// 默认 `2147483647`（INT_MAX）表示不额外收紧。
    pub tcp_challenge_ack_limit: Sample<u64>,
    pub ip_dynaddr: Sample<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TcpTune {
    pub keepalive_time: Sample<u64>,
    pub fin_timeout: Sample<u64>,
    pub max_syn_backlog: Sample<u64>,
    pub timestamps: Sample<String>,
    pub sack: Sample<String>,
    pub window_scaling: Sample<String>,
    pub ecn: Sample<String>,
    pub tw_reuse: Sample<String>,
    pub retries2: Sample<u64>,
    pub retries1: Sample<u64>,
    pub slow_start_after_idle: Sample<String>,
    pub syn_retries: Sample<u64>,
    pub synack_retries: Sample<u64>,
    pub keepalive_probes: Sample<u64>,
    pub keepalive_intvl: Sample<u64>,
    pub rmem: Sample<String>,
    pub wmem: Sample<String>,
    pub mtu_probing: Sample<String>,
    /// `u32::MAX`（4294967295）表示未限制。
    pub notsent_lowat: Sample<u64>,
    pub adv_win_scale: Sample<i64>,
    pub moderate_rcvbuf: Sample<String>,
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
pub struct TcpExt {
    pub timewait: Option<u64>,
    pub listen_overflows: Option<u64>,
    pub listen_drops: Option<u64>,
    pub timeouts: Option<u64>,
    pub abort_on_timeout: Option<u64>,
    pub orig_data_sent: Option<u64>,
    pub delivered: Option<u64>,
    pub rcv_coalesce: Option<u64>,
    pub in_octets: Option<u64>,
    pub out_octets: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snmp6 {
    pub in_receives: Option<u64>,
    pub in_delivers: Option<u64>,
    pub out_requests: Option<u64>,
    pub in_octets: Option<u64>,
    pub out_octets: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SockStat6 {
    pub tcp_inuse: Option<u64>,
    pub udp_inuse: Option<u64>,
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
    let tcpext = parse_netstat(&access::read_trimmed(ctx.proc_path("net/netstat")));
    let conntrack_count = access::read_u64(ctx.proc_path("sys/net/netfilter/nf_conntrack_count"));
    let conntrack_max = access::read_u64(ctx.proc_path("sys/net/netfilter/nf_conntrack_max"));
    let tcp_congestion = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_congestion_control"));
    let tcp_available_congestion =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_available_congestion_control"));
    let snmp6 = parse_snmp6(&access::read_trimmed(ctx.proc_path("net/snmp6")));
    let sockstat6 = parse_sockstat6(&access::read_trimmed(ctx.proc_path("net/sockstat6")));
    let somaxconn = access::read_u64(ctx.proc_path("sys/net/core/somaxconn"));
    let netdev_max_backlog = access::read_u64(ctx.proc_path("sys/net/core/netdev_max_backlog"));
    let rmem_max = access::read_u64(ctx.proc_path("sys/net/core/rmem_max"));
    let wmem_max = access::read_u64(ctx.proc_path("sys/net/core/wmem_max"));
    let arp_entries = count_table_rows(&access::read_trimmed(ctx.proc_path("net/arp")));
    let route_entries = count_table_rows(&access::read_trimmed(ctx.proc_path("net/route")));
    let unix_sockets = count_table_rows(&access::read_trimmed(ctx.proc_path("net/unix")));
    let ipv6_routes = count_data_lines(&access::read_trimmed(ctx.proc_path("net/ipv6_route")));
    let inet6_addrs = count_data_lines(&access::read_trimmed(ctx.proc_path("net/if_inet6")));
    let packet_sockets = count_table_rows(&access::read_trimmed(ctx.proc_path("net/packet")));
    let tcp_fastopen = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_fastopen"));
    let tcp_syncookies = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_syncookies"));
    let ip_local_port_range =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/ip_local_port_range"));
    let tcp = TcpTune {
        keepalive_time: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_keepalive_time")),
        fin_timeout: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_fin_timeout")),
        max_syn_backlog: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_max_syn_backlog")),
        timestamps: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_timestamps")),
        sack: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_sack")),
        window_scaling: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_window_scaling")),
        ecn: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_ecn")),
        tw_reuse: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_tw_reuse")),
        retries2: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_retries2")),
        retries1: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_retries1")),
        slow_start_after_idle: access::read_trimmed(
            ctx.proc_path("sys/net/ipv4/tcp_slow_start_after_idle"),
        ),
        syn_retries: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_syn_retries")),
        synack_retries: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_synack_retries")),
        keepalive_probes: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_keepalive_probes")),
        keepalive_intvl: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_keepalive_intvl")),
        rmem: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_rmem")),
        wmem: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_wmem")),
        mtu_probing: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_mtu_probing")),
        notsent_lowat: access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_notsent_lowat")),
        adv_win_scale: access::read_i64(ctx.proc_path("sys/net/ipv4/tcp_adv_win_scale")),
        moderate_rcvbuf: access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_moderate_rcvbuf")),
    };
    let default_qdisc = access::read_trimmed(ctx.proc_path("sys/net/core/default_qdisc"));
    let ipv6_disable = access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/disable_ipv6"));
    let ipv6_forwarding = access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/forwarding"));
    let protocols = parse_protocols(&access::read_trimmed(ctx.proc_path("net/protocols")));
    let igmp_ifaces = count_igmp_ifaces(&access::read_trimmed(ctx.proc_path("net/igmp")));
    let netdev_budget = access::read_u64(ctx.proc_path("sys/net/core/netdev_budget"));
    let rp_filter = access::read_trimmed(ctx.proc_path("sys/net/ipv4/conf/all/rp_filter"));
    let icmp_echo_ignore_broadcasts =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/icmp_echo_ignore_broadcasts"));
    let ipv6_use_tempaddr =
        access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/use_tempaddr"));
    let rp_filter_dev = conf_dev_diffs(ctx, "ipv4", "rp_filter", rp_filter.value.as_deref());
    let ipv6_use_tempaddr_dev = conf_dev_diffs(
        ctx,
        "ipv6",
        "use_tempaddr",
        ipv6_use_tempaddr.value.as_deref(),
    );
    let rt6_entries = parse_rt6_stats(&access::read_trimmed(ctx.proc_path("net/rt6_stats")));
    let optmem_max = access::read_u64(ctx.proc_path("sys/net/core/optmem_max"));
    let netdev_budget_usecs = access::read_u64(ctx.proc_path("sys/net/core/netdev_budget_usecs"));
    let accept_redirects =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/conf/all/accept_redirects"));
    let accept_source_route =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/conf/all/accept_source_route"));
    let log_martians = access::read_trimmed(ctx.proc_path("sys/net/ipv4/conf/all/log_martians"));
    let icmp_ignore_bogus =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/icmp_ignore_bogus_error_responses"));
    let netlink_sockets = count_table_rows(&access::read_trimmed(ctx.proc_path("net/netlink")));
    let tcp_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/tcp")));
    let udp_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/udp")));
    let tcp6_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/tcp6")));
    let udp6_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/udp6")));
    let raw_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/raw")));
    let udplite_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/udplite")));
    let (xfrm_in_no_states, xfrm_out_no_states) =
        parse_xfrm_stat(&access::read_trimmed(ctx.proc_path("net/xfrm_stat")));
    let ptypes = parse_ptype(&access::read_trimmed(ctx.proc_path("net/ptype")));
    let fib_trie_leaves =
        parse_fib_leaves(&access::read_trimmed(ctx.proc_path("net/fib_triestat")));
    let busy_poll = access::read_u64(ctx.proc_path("sys/net/core/busy_poll"));
    let dev_weight = access::read_u64(ctx.proc_path("sys/net/core/dev_weight"));
    let unix_max_dgram_qlen = access::read_u64(ctx.proc_path("sys/net/unix/max_dgram_qlen"));
    let igmp6_ifaces = count_igmp6_ifaces(&access::read_trimmed(ctx.proc_path("net/igmp6")));
    let raw6_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/raw6")));
    let udplite6_socks = count_table_rows(&access::read_trimmed(ctx.proc_path("net/udplite6")));
    let iptables = tables_from(
        &access::read_trimmed(ctx.proc_path("net/ip_tables_names")),
        &mut notes,
    );
    let ip6tables = tables_from(
        &access::read_trimmed(ctx.proc_path("net/ip6_tables_names")),
        &mut notes,
    );
    let connectors = parse_connector(&access::read_trimmed(ctx.proc_path("net/connector")));
    let ipv6_accept_ra = access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/accept_ra"));
    let ipv6_autoconf = access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/autoconf"));
    let ipv6_hop_limit = access::read_u64(ctx.proc_path("sys/net/ipv6/conf/all/hop_limit"));
    let conntrack_tcp_established =
        access::read_u64(ctx.proc_path("sys/net/netfilter/nf_conntrack_tcp_timeout_established"));
    let conntrack_buckets =
        access::read_u64(ctx.proc_path("sys/net/netfilter/nf_conntrack_buckets"));
    let tcp_max_tw_buckets = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_max_tw_buckets"));
    let busy_read = access::read_u64(ctx.proc_path("sys/net/core/busy_read"));
    let icmp_ratelimit = access::read_u64(ctx.proc_path("sys/net/ipv4/icmp_ratelimit"));
    let ip_default_ttl = access::read_u64(ctx.proc_path("sys/net/ipv4/ip_default_ttl"));
    let tcp_mem = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_mem"));
    let udp_mem = access::read_trimmed(ctx.proc_path("sys/net/ipv4/udp_mem"));
    let tcp_max_orphans = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_max_orphans"));
    let tcp_dsack = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_dsack"));
    let tcp_autocorking = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_autocorking"));
    let ipv6_accept_dad = access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/accept_dad"));
    let ipv6_addr_gen_mode =
        access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/addr_gen_mode"));
    let ipv6_accept_dad_dev =
        conf_dev_diffs(ctx, "ipv6", "accept_dad", ipv6_accept_dad.value.as_deref());
    let ipv6_addr_gen_mode_dev = conf_dev_diffs(
        ctx,
        "ipv6",
        "addr_gen_mode",
        ipv6_addr_gen_mode.value.as_deref(),
    );
    let ip6frag_high_thresh = access::read_u64(ctx.proc_path("sys/net/ipv6/ip6frag_high_thresh"));
    let rps_sock_flow_entries =
        access::read_u64(ctx.proc_path("sys/net/core/rps_sock_flow_entries"));
    let ipfrag_high_thresh = access::read_u64(ctx.proc_path("sys/net/ipv4/ipfrag_high_thresh"));
    let ipfrag_low_thresh = access::read_u64(ctx.proc_path("sys/net/ipv4/ipfrag_low_thresh"));
    let tcp_early_retrans = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_early_retrans"));
    let ip_no_pmtu_disc = access::read_trimmed(ctx.proc_path("sys/net/ipv4/ip_no_pmtu_disc"));
    let fib_multipath_hash_policy =
        access::read_u64(ctx.proc_path("sys/net/ipv4/fib_multipath_hash_policy"));
    let ipv6_max_addresses = access::read_u64(ctx.proc_path("sys/net/ipv6/conf/all/max_addresses"));
    let tcp_frto = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_frto"));
    let tcp_invalid_ratelimit =
        access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_invalid_ratelimit"));
    let tcp_min_tso_segs = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_min_tso_segs"));
    let tcp_pacing_ss_ratio = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_pacing_ss_ratio"));
    let netdev_tstamp_prequeue =
        access::read_trimmed(ctx.proc_path("sys/net/core/netdev_tstamp_prequeue"));
    let message_cost = access::read_u64(ctx.proc_path("sys/net/core/message_cost"));
    let ipv6_accept_ra_defrtr =
        access::read_trimmed(ctx.proc_path("sys/net/ipv6/conf/all/accept_ra_defrtr"));
    let ipv6_router_solicitations =
        access::read_i64(ctx.proc_path("sys/net/ipv6/conf/all/router_solicitations"));
    let ipv6_accept_ra_defrtr_dev = conf_dev_diffs(
        ctx,
        "ipv6",
        "accept_ra_defrtr",
        ipv6_accept_ra_defrtr.value.as_deref(),
    );
    let rs_all = ipv6_router_solicitations.value.map(|v| v.to_string());
    let ipv6_router_solicitations_dev =
        conf_dev_diffs(ctx, "ipv6", "router_solicitations", rs_all.as_deref());
    let ip6frag_low_thresh = access::read_u64(ctx.proc_path("sys/net/ipv6/ip6frag_low_thresh"));
    let ip_unprivileged_port_start =
        access::read_u64(ctx.proc_path("sys/net/ipv4/ip_unprivileged_port_start"));
    let tcp_pacing_ca_ratio = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_pacing_ca_ratio"));
    let tcp_orphan_retries = access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_orphan_retries"));
    let tcp_rfc1337 = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_rfc1337"));
    let bindv6only = access::read_trimmed(ctx.proc_path("sys/net/ipv6/bindv6only"));
    let ipfrag_time = access::read_u64(ctx.proc_path("sys/net/ipv4/ipfrag_time"));
    let ipv6_dad_transmits = access::read_u64(ctx.proc_path("sys/net/ipv6/conf/all/dad_transmits"));
    let dad_all = ipv6_dad_transmits.value.map(|v| v.to_string());
    let ipv6_dad_transmits_dev = conf_dev_diffs(ctx, "ipv6", "dad_transmits", dad_all.as_deref());
    let message_burst = access::read_u64(ctx.proc_path("sys/net/core/message_burst"));
    let tcp_ecn_fallback = access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_ecn_fallback"));
    let ip_nonlocal_bind = access::read_trimmed(ctx.proc_path("sys/net/ipv4/ip_nonlocal_bind"));
    let icmp_echo_ignore_all =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/icmp_echo_ignore_all"));
    let ipfrag_max_dist = access::read_u64(ctx.proc_path("sys/net/ipv4/ipfrag_max_dist"));
    let tcp_abort_on_overflow =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_abort_on_overflow"));
    let tcp_no_metrics_save =
        access::read_trimmed(ctx.proc_path("sys/net/ipv4/tcp_no_metrics_save"));
    let tcp_challenge_ack_limit =
        access::read_u64(ctx.proc_path("sys/net/ipv4/tcp_challenge_ack_limit"));
    let ip_dynaddr = access::read_trimmed(ctx.proc_path("sys/net/ipv4/ip_dynaddr"));
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
                conntrack_count,
                conntrack_max,
                tcp_congestion,
                tcp_available_congestion,
                tcpext,
                snmp6,
                sockstat6,
                somaxconn,
                netdev_max_backlog,
                rmem_max,
                wmem_max,
                arp_entries,
                route_entries,
                unix_sockets,
                ipv6_routes,
                inet6_addrs,
                packet_sockets,
                tcp_fastopen,
                tcp_syncookies,
                ip_local_port_range,
                tcp,
                default_qdisc,
                ipv6_disable,
                ipv6_forwarding,
                protocols,
                igmp_ifaces,
                netdev_budget,
                rp_filter,
                icmp_echo_ignore_broadcasts,
                ipv6_use_tempaddr,
                rp_filter_dev,
                ipv6_use_tempaddr_dev,
                rt6_entries,
                optmem_max,
                netdev_budget_usecs,
                accept_redirects,
                accept_source_route,
                log_martians,
                icmp_ignore_bogus,
                netlink_sockets,
                tcp_socks,
                udp_socks,
                tcp6_socks,
                udp6_socks,
                raw_socks,
                udplite_socks,
                xfrm_in_no_states,
                xfrm_out_no_states,
                ptypes,
                fib_trie_leaves,
                busy_poll,
                dev_weight,
                unix_max_dgram_qlen,
                igmp6_ifaces,
                raw6_socks,
                udplite6_socks,
                iptables,
                ip6tables,
                connectors,
                ipv6_accept_ra,
                ipv6_autoconf,
                ipv6_hop_limit,
                conntrack_tcp_established,
                conntrack_buckets,
                tcp_max_tw_buckets,
                busy_read,
                icmp_ratelimit,
                ip_default_ttl,
                tcp_mem,
                udp_mem,
                tcp_max_orphans,
                tcp_dsack,
                tcp_autocorking,
                ipv6_accept_dad,
                ipv6_addr_gen_mode,
                ipv6_accept_dad_dev,
                ipv6_addr_gen_mode_dev,
                ip6frag_high_thresh,
                rps_sock_flow_entries,
                ipfrag_high_thresh,
                ipfrag_low_thresh,
                tcp_early_retrans,
                ip_no_pmtu_disc,
                fib_multipath_hash_policy,
                ipv6_max_addresses,
                tcp_frto,
                tcp_invalid_ratelimit,
                tcp_min_tso_segs,
                tcp_pacing_ss_ratio,
                netdev_tstamp_prequeue,
                message_cost,
                ipv6_accept_ra_defrtr,
                ipv6_router_solicitations,
                ipv6_accept_ra_defrtr_dev,
                ipv6_router_solicitations_dev,
                ip6frag_low_thresh,
                ip_unprivileged_port_start,
                tcp_pacing_ca_ratio,
                tcp_orphan_retries,
                tcp_rfc1337,
                bindv6only,
                ipfrag_time,
                ipv6_dad_transmits,
                ipv6_dad_transmits_dev,
                message_burst,
                tcp_ecn_fallback,
                ip_nonlocal_bind,
                icmp_echo_ignore_all,
                ipfrag_max_dist,
                tcp_abort_on_overflow,
                tcp_no_metrics_save,
                tcp_challenge_ack_limit,
                ip_dynaddr,
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
        conntrack_count,
        conntrack_max,
        tcp_congestion,
        tcp_available_congestion,
        tcpext,
        snmp6,
        sockstat6,
        somaxconn,
        netdev_max_backlog,
        rmem_max,
        wmem_max,
        arp_entries,
        route_entries,
        unix_sockets,
        ipv6_routes,
        inet6_addrs,
        packet_sockets,
        tcp_fastopen,
        tcp_syncookies,
        ip_local_port_range,
        tcp,
        default_qdisc,
        ipv6_disable,
        ipv6_forwarding,
        protocols,
        igmp_ifaces,
        netdev_budget,
        rp_filter,
        icmp_echo_ignore_broadcasts,
        ipv6_use_tempaddr,
        rp_filter_dev,
        ipv6_use_tempaddr_dev,
        rt6_entries,
        optmem_max,
        netdev_budget_usecs,
        accept_redirects,
        accept_source_route,
        log_martians,
        icmp_ignore_bogus,
        netlink_sockets,
        tcp_socks,
        udp_socks,
        tcp6_socks,
        udp6_socks,
        raw_socks,
        udplite_socks,
        xfrm_in_no_states,
        xfrm_out_no_states,
        ptypes,
        fib_trie_leaves,
        busy_poll,
        dev_weight,
        unix_max_dgram_qlen,
        igmp6_ifaces,
        raw6_socks,
        udplite6_socks,
        iptables,
        ip6tables,
        connectors,
        ipv6_accept_ra,
        ipv6_autoconf,
        ipv6_hop_limit,
        conntrack_tcp_established,
        conntrack_buckets,
        tcp_max_tw_buckets,
        busy_read,
        icmp_ratelimit,
        ip_default_ttl,
        tcp_mem,
        udp_mem,
        tcp_max_orphans,
        tcp_dsack,
        tcp_autocorking,
        ipv6_accept_dad,
        ipv6_addr_gen_mode,
        ipv6_accept_dad_dev,
        ipv6_addr_gen_mode_dev,
        ip6frag_high_thresh,
        rps_sock_flow_entries,
        ipfrag_high_thresh,
        ipfrag_low_thresh,
        tcp_early_retrans,
        ip_no_pmtu_disc,
        fib_multipath_hash_policy,
        ipv6_max_addresses,
        tcp_frto,
        tcp_invalid_ratelimit,
        tcp_min_tso_segs,
        tcp_pacing_ss_ratio,
        netdev_tstamp_prequeue,
        message_cost,
        ipv6_accept_ra_defrtr,
        ipv6_router_solicitations,
        ipv6_accept_ra_defrtr_dev,
        ipv6_router_solicitations_dev,
        ip6frag_low_thresh,
        ip_unprivileged_port_start,
        tcp_pacing_ca_ratio,
        tcp_orphan_retries,
        tcp_rfc1337,
        bindv6only,
        ipfrag_time,
        ipv6_dad_transmits,
        ipv6_dad_transmits_dev,
        message_burst,
        tcp_ecn_fallback,
        ip_nonlocal_bind,
        icmp_echo_ignore_all,
        ipfrag_max_dist,
        tcp_abort_on_overflow,
        tcp_no_metrics_save,
        tcp_challenge_ack_limit,
        ip_dynaddr,
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

/// `/proc/net/snmp6`：每行 `Key  Value`，与 IPv4 snmp 两行组不同。
pub fn parse_snmp6(sample: &Sample<String>) -> Snmp6 {
    let Some(text) = sample.value.as_deref() else {
        return Snmp6::default();
    };
    let mut map = std::collections::BTreeMap::<String, u64>::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(k) = it.next() else { continue };
        if let Some(v) = it.next().and_then(|s| s.parse::<u64>().ok()) {
            map.insert(k.to_string(), v);
        }
    }
    let pick = |k: &str| map.get(k).copied();
    Snmp6 {
        in_receives: pick("Ip6InReceives"),
        in_delivers: pick("Ip6InDelivers"),
        out_requests: pick("Ip6OutRequests"),
        in_octets: pick("Ip6InOctets"),
        out_octets: pick("Ip6OutOctets"),
    }
}

pub fn parse_sockstat6(sample: &Sample<String>) -> SockStat6 {
    let Some(text) = sample.value.as_deref() else {
        return SockStat6::default();
    };
    let mut out = SockStat6::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(kind) = it.next() else { continue };
        let kind = kind.trim_end_matches(':');
        let mut map = std::collections::BTreeMap::new();
        while let (Some(k), Some(v)) = (it.next(), it.next()) {
            if let Ok(n) = v.parse::<u64>() {
                map.insert(k, n);
            }
        }
        match kind {
            "TCP6" => out.tcp_inuse = map.get("inuse").copied(),
            "UDP6" => out.udp_inuse = map.get("inuse").copied(),
            _ => {}
        }
    }
    out
}

fn count_table_rows(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// `/proc/net/xfrm_stat`：每行 `Key  Value`，与 snmp6 相同。
pub fn parse_xfrm_stat(sample: &Sample<String>) -> (Option<u64>, Option<u64>) {
    let Some(text) = sample.value.as_deref() else {
        return (None, None);
    };
    let mut inn = None;
    let mut out = None;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(k) = it.next() else { continue };
        let Some(v) = it.next().and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        match k {
            "XfrmInNoStates" => inn = Some(v),
            "XfrmOutNoStates" => out = Some(v),
            _ => {}
        }
    }
    (inn, out)
}

/// `/proc/net/ptype`：跳过表头，最多 8 条 `type:function`。
pub fn parse_ptype(sample: &Sample<String>) -> Vec<String> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let mut it = line.split_whitespace();
        let Some(typ) = it.next() else { continue };
        let Some(func) = it.next() else { continue };
        // 中间 Device 列常空，function 是最后一个 token。
        let func = it.last().unwrap_or(func);
        out.push(format!("{typ}:{func}"));
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// `/proc/net/fib_triestat` 第一段 `Leaves:` 是主表。不要读巨大的 `fib_trie`。
pub fn parse_fib_leaves(sample: &Sample<String>) -> Option<u64> {
    let text = sample.value.as_deref()?;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Leaves:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn count_data_lines(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// `/proc/net/protocols`：有 socket 的协议 `NAME:count`，最多 16 条。
pub fn parse_protocols(sample: &Sample<String>) -> Vec<String> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let Some(socks) = it.nth(1).and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        if socks == 0 {
            continue;
        }
        out.push(format!("{name}:{socks}"));
        if out.len() >= 16 {
            break;
        }
    }
    out
}

/// `/proc/net/igmp`：只计接口头行（行首是 Idx + 设备名）。组记录定时器也含冒号，不能当接口。
pub fn count_igmp_ifaces(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    text.lines()
        .filter(|l| {
            if l.starts_with(char::is_whitespace) {
                return false;
            }
            let mut it = l.split_whitespace();
            let Some(idx) = it.next() else {
                return false;
            };
            idx.parse::<u32>().is_ok() && it.next().is_some() && l.contains(':')
        })
        .count()
}

/// `/proc/net/igmp6`：每行 `ifindex iface group …`，按接口名去重，不要把组地址当接口。
pub fn count_igmp6_ifaces(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    let mut names = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(idx) = it.next() else { continue };
        if idx.parse::<u32>().is_err() {
            continue;
        }
        let Some(name) = it.next() else { continue };
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names.len()
}

/// `/proc/net/ip_tables_names`：一行一个表名。空文件表示未加载 iptables，不是失败。
pub fn parse_name_lines(sample: &Sample<String>) -> Vec<String> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(8)
        .map(|s| s.to_string())
        .collect()
}

/// `PermissionDenied`/`Error` 记入 notes，不要当成「未加载的空表」。
fn tables_from(sample: &Sample<String>, notes: &mut Vec<String>) -> Vec<String> {
    if matches!(
        sample.access,
        AccessKind::PermissionDenied | AccessKind::Error
    ) {
        notes.push(sample.access_label());
        Vec::new()
    } else {
        parse_name_lines(sample)
    }
}

/// `/proc/net/connector`：跳过表头，取 Name 列，最多 8 条。
pub fn parse_connector(sample: &Sample<String>) -> Vec<String> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        out.push(name.to_string());
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// `/proc/net/rt6_stats` 十六进制 7 列：fib_nodes … dst cache … discarded。
/// 第 6 列才是 destination cache entries，不是第一列。
pub fn parse_rt6_stats(sample: &Sample<String>) -> Sample<u64> {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return miss();
    };
    match text
        .split_whitespace()
        .nth(5)
        .and_then(|s| u64::from_str_radix(s, 16).ok())
    {
        Some(v) => Sample::ok(v, sample.source.clone()),
        None => Sample::error(sample.source.clone(), "无法解析 rt6_stats"),
    }
}

fn conf_dev_diffs(ctx: &ProbeCtx, family: &str, attr: &str, all: Option<&str>) -> Vec<String> {
    let root = ctx.proc_path(format!("sys/net/{family}/conf"));
    let names = match access::list_dir_names(&root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in names {
        if name == "all" || name == "default" {
            continue;
        }
        let s = access::read_trimmed(root.join(&name).join(attr));
        if let Some(v) = s.value.as_deref() {
            if all != Some(v) {
                out.push(format!("{name}:{v}"));
            }
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// `/proc/net/netstat` 与 snmp 相同：两行一组。不调用 `netstat`。
pub fn parse_netstat(sample: &Sample<String>) -> TcpExt {
    let Some(text) = sample.value.as_deref() else {
        return TcpExt::default();
    };
    let mut map = std::collections::BTreeMap::<String, u64>::new();
    let mut lines = text.lines().peekable();
    while let Some(header) = lines.next() {
        let Some(values) = lines.next() else { break };
        let mut h = header.split_whitespace();
        let mut v = values.split_whitespace();
        let Some(proto) = h.next() else { continue };
        let _ = v.next();
        let proto = proto.trim_end_matches(':');
        for (key, val) in h.zip(v) {
            if let Ok(n) = val.parse::<u64>() {
                map.insert(format!("{proto}.{key}"), n);
            }
        }
    }
    let pick = |k: &str| map.get(k).copied();
    TcpExt {
        timewait: pick("TcpExt.TW"),
        listen_overflows: pick("TcpExt.ListenOverflows"),
        listen_drops: pick("TcpExt.ListenDrops"),
        timeouts: pick("TcpExt.TCPTimeouts"),
        abort_on_timeout: pick("TcpExt.TCPAbortOnTimeout"),
        orig_data_sent: pick("TcpExt.TCPOrigDataSent"),
        delivered: pick("TcpExt.TCPDelivered"),
        rcv_coalesce: pick("TcpExt.TCPRcvCoalesce"),
        in_octets: pick("IpExt.InOctets"),
        out_octets: pick("IpExt.OutOctets"),
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

    #[test]
    fn netstat_and_conntrack_fixture() {
        let ext = parse_netstat(&Sample::ok(
            "TcpExt: TW ListenOverflows ListenDrops TCPTimeouts TCPAbortOnTimeout TCPOrigDataSent TCPDelivered TCPRcvCoalesce\nTcpExt: 10 1 2 3 4 100 90 50\nIpExt: InOctets OutOctets\nIpExt: 1000 2000\n".into(),
            "netstat",
        ));
        assert_eq!(ext.timewait, Some(10));
        assert_eq!(ext.listen_overflows, Some(1));
        assert_eq!(ext.in_octets, Some(1000));
        assert_eq!(ext.out_octets, Some(2000));
        assert_eq!(ext.delivered, Some(90));

        let root = std::env::temp_dir().join(format!("aida-net-ct-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/net")).unwrap();
        fs::create_dir_all(root.join("proc/net")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/netfilter")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4")).unwrap();
        fs::write(
            root.join("proc/sys/net/netfilter/nf_conntrack_count"),
            "53\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/netfilter/nf_conntrack_max"),
            "262144\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_congestion_control"),
            "cubic\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_available_congestion_control"),
            "reno cubic\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("proc/sys/net/core")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4/conf/all")).unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_retries2"), "15\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_syn_retries"), "6\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_synack_retries"), "5\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_rmem"),
            "4096\t131072\t6291456\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/core/optmem_max"), "131072\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/conf/all/accept_redirects"),
            "0\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/net/tcp"),
            "sl local rem\n 0: 0 0\n 1: 0 0\n",
        )
        .unwrap();
        fs::write(root.join("proc/net/tcp6"), "sl local rem\n 0: 0 0\n").unwrap();
        fs::write(root.join("proc/net/udp6"), "sl local rem\n").unwrap();
        fs::write(root.join("proc/net/raw"), "sl local rem\n").unwrap();
        fs::write(root.join("proc/net/udplite"), "sl local rem\n").unwrap();
        fs::write(
            root.join("proc/net/xfrm_stat"),
            "XfrmInNoStates\t3\nXfrmOutNoStates\t1\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/net/ptype"),
            "Type Device      Function\n0800          ip_rcv\n0806          arp_rcv\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/net/fib_triestat"),
            "Basic info:\nMain:\n\tLeaves:         3\nLocal:\n\tLeaves:         7\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/core/busy_poll"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/core/dev_weight"), "64\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/net/unix")).unwrap();
        fs::write(root.join("proc/sys/net/unix/max_dgram_qlen"), "512\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_notsent_lowat"),
            "4294967295\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/net/igmp6"),
            "1    lo              ff020000000000000000000000000001     1 0000000C 0\n1    lo              ff010000000000000000000000000001     1 00000008 0\n2    eth0            ff020000000000000000000000000001     1 0000000C 0\n",
        )
        .unwrap();
        fs::write(root.join("proc/net/raw6"), "sl local rem\n").unwrap();
        fs::write(root.join("proc/net/ip_tables_names"), "filter\nnat\n").unwrap();
        fs::write(
            root.join("proc/net/connector"),
            "Name            ID\ncn_proc         1:1\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_adv_win_scale"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_moderate_rcvbuf"), "1\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv6/conf/all")).unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/accept_ra"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/autoconf"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/hop_limit"), "64\n").unwrap();
        fs::write(
            root.join("proc/sys/net/netfilter/nf_conntrack_tcp_timeout_established"),
            "432000\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/netfilter/nf_conntrack_buckets"),
            "65536\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_max_tw_buckets"), "65536\n").unwrap();
        fs::write(root.join("proc/sys/net/core/busy_read"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/icmp_ratelimit"), "1000\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/ip_default_ttl"), "64\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_mem"),
            "181818\t242425\t363636\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/udp_mem"),
            "363636\t484848\t727272\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_max_orphans"), "16384\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_dsack"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_autocorking"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/accept_dad"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/addr_gen_mode"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/ip6frag_high_thresh"),
            "4194304\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/core/rps_sock_flow_entries"),
            "32768\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/ipfrag_high_thresh"),
            "4194304\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/ipfrag_low_thresh"),
            "3145728\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_retries1"), "3\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_early_retrans"), "3\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/ip_no_pmtu_disc"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/fib_multipath_hash_policy"),
            "0\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/all/max_addresses"),
            "16\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_frto"), "2\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_invalid_ratelimit"),
            "500\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_min_tso_segs"), "2\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_pacing_ss_ratio"), "200\n").unwrap();
        fs::write(root.join("proc/sys/net/core/netdev_tstamp_prequeue"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/core/message_cost"), "5\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/all/accept_ra_defrtr"),
            "1\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/all/router_solicitations"),
            "-1\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/ip6frag_low_thresh"),
            "3145728\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/ip_unprivileged_port_start"),
            "1024\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_pacing_ca_ratio"), "120\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_orphan_retries"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_rfc1337"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/bindv6only"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/ipfrag_time"), "30\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/dad_transmits"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/core/message_burst"), "10\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_ecn_fallback"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/ip_nonlocal_bind"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/icmp_echo_ignore_all"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/ipfrag_max_dist"), "64\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_abort_on_overflow"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/tcp_no_metrics_save"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_challenge_ack_limit"),
            "2147483647\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv4/ip_dynaddr"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/tcp_slow_start_after_idle"),
            "1\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/core/netdev_budget"), "300\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/conf/all/rp_filter"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv4/icmp_echo_ignore_broadcasts"),
            "1\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/net/rt6_stats"),
            "0005 0004 001b 0004 0000 0007 0000\n",
        )
        .unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.conntrack_count.value, Some(53));
        assert_eq!(r.conntrack_max.value, Some(262144));
        assert_eq!(r.tcp_congestion.value.as_deref(), Some("cubic"));
        assert_eq!(r.tcp.retries2.value, Some(15));
        assert_eq!(r.tcp.syn_retries.value, Some(6));
        assert_eq!(r.tcp.synack_retries.value, Some(5));
        assert_eq!(r.tcp.rmem.value.as_deref(), Some("4096\t131072\t6291456"));
        assert_eq!(r.optmem_max.value, Some(131072));
        assert_eq!(r.accept_redirects.value.as_deref(), Some("0"));
        assert_eq!(r.tcp_socks, 2);
        assert_eq!(r.tcp6_socks, 1);
        assert_eq!(r.udp6_socks, 0);
        assert_eq!(r.raw_socks, 0);
        assert_eq!(r.udplite_socks, 0);
        assert_eq!(r.xfrm_in_no_states, Some(3));
        assert_eq!(r.xfrm_out_no_states, Some(1));
        assert_eq!(
            r.ptypes,
            vec!["0800:ip_rcv".to_string(), "0806:arp_rcv".to_string()]
        );
        assert_eq!(r.fib_trie_leaves, Some(3));
        assert_eq!(r.busy_poll.value, Some(0));
        assert_eq!(r.dev_weight.value, Some(64));
        assert_eq!(r.unix_max_dgram_qlen.value, Some(512));
        assert_eq!(r.tcp.notsent_lowat.value, Some(4_294_967_295));
        assert_eq!(r.igmp6_ifaces, 2);
        assert_eq!(r.raw6_socks, 0);
        assert_eq!(r.iptables, vec!["filter".to_string(), "nat".to_string()]);
        assert_eq!(r.connectors, vec!["cn_proc".to_string()]);
        assert_eq!(r.tcp.adv_win_scale.value, Some(1));
        assert_eq!(r.tcp.moderate_rcvbuf.value.as_deref(), Some("1"));
        assert_eq!(r.ipv6_accept_ra.value.as_deref(), Some("1"));
        assert_eq!(r.ipv6_autoconf.value.as_deref(), Some("1"));
        assert_eq!(r.ipv6_hop_limit.value, Some(64));
        assert_eq!(r.conntrack_tcp_established.value, Some(432000));
        assert_eq!(r.conntrack_buckets.value, Some(65536));
        assert_eq!(r.tcp_max_tw_buckets.value, Some(65536));
        assert_eq!(r.busy_read.value, Some(0));
        assert_eq!(r.icmp_ratelimit.value, Some(1000));
        assert_eq!(r.ip_default_ttl.value, Some(64));
        assert_eq!(r.tcp_mem.value.as_deref(), Some("181818\t242425\t363636"));
        assert_eq!(r.udp_mem.value.as_deref(), Some("363636\t484848\t727272"));
        assert_eq!(r.tcp_max_orphans.value, Some(16384));
        assert_eq!(r.tcp_dsack.value.as_deref(), Some("1"));
        assert_eq!(r.tcp_autocorking.value.as_deref(), Some("1"));
        assert_eq!(r.ipv6_accept_dad.value.as_deref(), Some("1"));
        assert_eq!(r.ipv6_addr_gen_mode.value.as_deref(), Some("0"));
        assert_eq!(r.ip6frag_high_thresh.value, Some(4_194_304));
        assert_eq!(r.rps_sock_flow_entries.value, Some(32768));
        assert_eq!(r.ipfrag_high_thresh.value, Some(4_194_304));
        assert_eq!(r.ipfrag_low_thresh.value, Some(3_145_728));
        assert_eq!(r.tcp.retries1.value, Some(3));
        assert_eq!(r.tcp_early_retrans.value, Some(3));
        assert_eq!(r.ip_no_pmtu_disc.value.as_deref(), Some("0"));
        assert_eq!(r.fib_multipath_hash_policy.value, Some(0));
        assert_eq!(r.ipv6_max_addresses.value, Some(16));
        assert_eq!(r.tcp_frto.value.as_deref(), Some("2"));
        assert_eq!(r.tcp_invalid_ratelimit.value, Some(500));
        assert_eq!(r.tcp_min_tso_segs.value, Some(2));
        assert_eq!(r.tcp_pacing_ss_ratio.value, Some(200));
        assert_eq!(r.netdev_tstamp_prequeue.value.as_deref(), Some("1"));
        assert_eq!(r.message_cost.value, Some(5));
        assert_eq!(r.ipv6_accept_ra_defrtr.value.as_deref(), Some("1"));
        assert_eq!(r.ipv6_router_solicitations.value, Some(-1));
        assert_eq!(r.ip6frag_low_thresh.value, Some(3_145_728));
        assert_eq!(r.ip_unprivileged_port_start.value, Some(1024));
        assert_eq!(r.tcp_pacing_ca_ratio.value, Some(120));
        assert_eq!(r.tcp_orphan_retries.value, Some(0));
        assert_eq!(r.tcp_rfc1337.value.as_deref(), Some("0"));
        assert_eq!(r.bindv6only.value.as_deref(), Some("0"));
        assert_eq!(r.ipfrag_time.value, Some(30));
        assert_eq!(r.ipv6_dad_transmits.value, Some(1));
        assert_eq!(r.message_burst.value, Some(10));
        assert_eq!(r.tcp_ecn_fallback.value.as_deref(), Some("1"));
        assert_eq!(r.ip_nonlocal_bind.value.as_deref(), Some("0"));
        assert_eq!(r.icmp_echo_ignore_all.value.as_deref(), Some("0"));
        assert_eq!(r.ipfrag_max_dist.value, Some(64));
        assert_eq!(r.tcp_abort_on_overflow.value.as_deref(), Some("0"));
        assert_eq!(r.tcp_no_metrics_save.value.as_deref(), Some("0"));
        assert_eq!(r.tcp_challenge_ack_limit.value, Some(2_147_483_647));
        assert_eq!(r.ip_dynaddr.value.as_deref(), Some("0"));
        assert_eq!(r.tcp.slow_start_after_idle.value.as_deref(), Some("1"));
        assert_eq!(r.netdev_budget.value, Some(300));
        assert_eq!(r.rp_filter.value.as_deref(), Some("0"));
        assert_eq!(r.icmp_echo_ignore_broadcasts.value.as_deref(), Some("1"));
        assert_eq!(r.rt6_entries.value, Some(7));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn per_iface_rp_filter_and_tempaddr_diffs() {
        let root = std::env::temp_dir().join(format!("aida-net-rp-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/class/net")).unwrap();
        fs::create_dir_all(root.join("proc/net")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4/conf/all")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4/conf/eth0")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4/conf/lo")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv6/conf/all")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv6/conf/lo")).unwrap();
        fs::write(root.join("proc/sys/net/ipv4/conf/all/rp_filter"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/conf/eth0/rp_filter"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv4/conf/lo/rp_filter"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/use_tempaddr"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/lo/use_tempaddr"), "-1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/accept_dad"), "0\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/lo/accept_dad"), "-1\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv6/conf/eth0")).unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/addr_gen_mode"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/eth0/addr_gen_mode"),
            "3\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/all/accept_ra_defrtr"),
            "1\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/eth0/accept_ra_defrtr"),
            "0\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/all/router_solicitations"),
            "-1\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/net/ipv6/conf/lo/router_solicitations"),
            "3\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/all/dad_transmits"), "1\n").unwrap();
        fs::write(root.join("proc/sys/net/ipv6/conf/lo/dad_transmits"), "0\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.rp_filter.value.as_deref(), Some("0"));
        assert!(
            r.rp_filter_dev.iter().any(|s| s == "eth0:1"),
            "eth0 rp_filter=1 must differ from conf/all: {:?}",
            r.rp_filter_dev
        );
        assert!(
            !r.rp_filter_dev.iter().any(|s| s.starts_with("lo:")),
            "lo matching all must not appear: {:?}",
            r.rp_filter_dev
        );
        assert_eq!(r.ipv6_use_tempaddr.value.as_deref(), Some("0"));
        assert!(
            r.ipv6_use_tempaddr_dev.iter().any(|s| s == "lo:-1"),
            "lo use_tempaddr=-1 must differ from conf/all: {:?}",
            r.ipv6_use_tempaddr_dev
        );
        assert_eq!(r.ipv6_accept_dad.value.as_deref(), Some("0"));
        assert!(
            r.ipv6_accept_dad_dev.iter().any(|s| s == "lo:-1"),
            "lo accept_dad=-1 must differ from conf/all: {:?}",
            r.ipv6_accept_dad_dev
        );
        assert_eq!(r.ipv6_addr_gen_mode.value.as_deref(), Some("0"));
        assert!(
            r.ipv6_addr_gen_mode_dev.iter().any(|s| s == "eth0:3"),
            "eth0 addr_gen_mode=3 must differ from conf/all: {:?}",
            r.ipv6_addr_gen_mode_dev
        );
        assert_eq!(r.ipv6_accept_ra_defrtr.value.as_deref(), Some("1"));
        assert!(
            r.ipv6_accept_ra_defrtr_dev.iter().any(|s| s == "eth0:0"),
            "eth0 accept_ra_defrtr=0 must differ from conf/all: {:?}",
            r.ipv6_accept_ra_defrtr_dev
        );
        assert_eq!(r.ipv6_router_solicitations.value, Some(-1));
        assert!(
            r.ipv6_router_solicitations_dev.iter().any(|s| s == "lo:3"),
            "lo router_solicitations=3 must differ from conf/all: {:?}",
            r.ipv6_router_solicitations_dev
        );
        assert_eq!(r.ipv6_dad_transmits.value, Some(1));
        assert!(
            r.ipv6_dad_transmits_dev.iter().any(|s| s == "lo:0"),
            "lo dad_transmits=0 must differ from conf/all: {:?}",
            r.ipv6_dad_transmits_dev
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snmp6_sockstat6_and_unix_rows() {
        let s6 = parse_snmp6(&Sample::ok(
            "Ip6InReceives                    \t110\nIp6InDelivers                    \t100\nIp6OutRequests                   \t90\nIp6InOctets                      \t1000\nIp6OutOctets                     \t2000\n".into(),
            "snmp6",
        ));
        assert_eq!(s6.in_receives, Some(110));
        assert_eq!(s6.out_octets, Some(2000));
        let sk = parse_sockstat6(&Sample::ok(
            "TCP6: inuse 9\nUDP6: inuse 2\n".into(),
            "sockstat6",
        ));
        assert_eq!(sk.tcp_inuse, Some(9));
        assert_eq!(sk.udp_inuse, Some(2));
        assert_eq!(
            count_table_rows(&Sample::ok("Num RefCount\na 1\nb 2\n".into(), "unix")),
            2
        );
        assert_eq!(
            count_data_lines(&Sample::ok(
                "00000000000000000000000000000001 01 80 10 80       lo\nfe80... 02 40 20 80   eth0\n".into(),
                "if_inet6",
            )),
            2
        );
        let proto = parse_protocols(&Sample::ok(
            "protocol  size sockets  memory\nTCP  1  13  0\nUNIX  1  0  0\nUDP  1  2  0\n".into(),
            "protocols",
        ));
        assert_eq!(proto, vec!["TCP:13".to_string(), "UDP:2".to_string()]);
        assert_eq!(
            count_igmp_ifaces(&Sample::ok(
                "Idx\tDevice    : Count\n1\tlo        :     1      V3\n\t\t010000E0     1 0:00000000\t0\n2\teth0      :     1      V3\n\t\t010000E0     1 0:00000000\t0\n".into(),
                "igmp",
            )),
            2
        );
        let rt6 = parse_rt6_stats(&Sample::ok(
            "0005 0004 001b 0004 0000 0007 0000\n".into(),
            "rt6_stats",
        ));
        assert_eq!(rt6.value, Some(7));
    }

    #[test]
    fn iptables_denied_is_not_unloaded() {
        let mut notes = Vec::new();
        let denied = tables_from(&Sample::denied("/proc/net/ip_tables_names"), &mut notes);
        assert!(denied.is_empty());
        assert!(
            notes.iter().any(|n| n.contains("权限不足")),
            "PermissionDenied must be noted, not look unloaded: {notes:?}"
        );
        let mut ok_notes = Vec::new();
        let empty = tables_from(&Sample::ok(String::new(), "ip_tables_names"), &mut ok_notes);
        assert!(empty.is_empty());
        assert!(
            ok_notes.is_empty(),
            "empty Ok file is unloaded, not a read failure: {ok_notes:?}"
        );
        assert_eq!(
            parse_name_lines(&Sample::ok("filter\nnat\n".into(), "ip_tables_names")),
            vec!["filter".to_string(), "nat".to_string()]
        );
    }
}
