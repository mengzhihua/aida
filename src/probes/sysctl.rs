//! 内核可调参数与控制台：`/proc/sys/*`、`/proc/consoles`。不调用 `sysctl`。

use serde::Serialize;

use crate::access::{self, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct SysctlReport {
    pub file_nr_alloc: Sample<u64>,
    pub file_nr_max: Sample<u64>,
    pub inode_nr: Sample<u64>,
    pub pid_max: Sample<u64>,
    pub threads_max: Sample<u64>,
    pub swappiness: Sample<u64>,
    pub overcommit_memory: Sample<String>,
    pub overcommit_ratio: Sample<u64>,
    pub dirty_ratio: Sample<u64>,
    pub dirty_background_ratio: Sample<u64>,
    pub aslr: Sample<String>,
    pub core_pattern: Sample<String>,
    pub printk: Sample<String>,
    pub ip_forward: Sample<String>,
    pub aio_nr: Sample<u64>,
    pub aio_max_nr: Sample<u64>,
    pub inotify_max_user_watches: Sample<u64>,
    pub inotify_max_user_instances: Sample<u64>,
    pub nr_open: Sample<u64>,
    pub max_map_count: Sample<u64>,
    pub mmap_min_addr: Sample<u64>,
    pub boot_id: Sample<String>,
    pub nmi_watchdog: Sample<String>,
    pub watchdog: Sample<String>,
    pub watchdog_thresh: Sample<u64>,
    pub pipe_max_size: Sample<u64>,
    pub file_max: Sample<u64>,
    /// 负数表示 panic 后立即重启，不是读取失败。
    pub panic: Sample<i64>,
    pub sysrq: Sample<String>,
    pub min_free_kbytes: Sample<u64>,
    pub vfs_cache_pressure: Sample<u64>,
    pub watermark_scale_factor: Sample<u64>,
    pub dirty_expire_centisecs: Sample<u64>,
    pub ctrl_alt_del: Sample<String>,
    pub random_poolsize: Sample<u64>,
    pub sched_autogroup: Sample<String>,
    pub leases_enable: Sample<String>,
    pub suid_dumpable: Sample<String>,
    pub cap_last_cap: Sample<u64>,
    pub keys_maxkeys: Sample<u64>,
    pub keys_maxbytes: Sample<u64>,
    pub hung_task_timeout_secs: Sample<u64>,
    pub panic_on_oops: Sample<String>,
    pub panic_on_warn: Sample<String>,
    pub core_uses_pid: Sample<String>,
    pub shmmax: Sample<String>,
    pub shmmni: Sample<u64>,
    pub msgmax: Sample<u64>,
    pub sem: Sample<String>,
    pub mqueue_queues_max: Sample<u64>,
    pub inotify_max_queued_events: Sample<u64>,
    pub epoll_max_user_watches: Sample<u64>,
    pub dirty_writeback_centisecs: Sample<u64>,
    pub page_cluster: Sample<u64>,
    /// `-1` 表示 RT 运行时不限（占满 period）。
    pub sched_rt_runtime_us: Sample<i64>,
    pub sched_rt_period_us: Sample<u64>,
    pub sched_rr_timeslice_ms: Sample<u64>,
    pub numa_balancing: Sample<String>,
    pub timer_migration: Sample<String>,
    pub panic_on_oom: Sample<String>,
    pub oom_kill_allocating_task: Sample<String>,
    pub laptop_mode: Sample<String>,
    pub kexec_load_disabled: Sample<String>,
    pub hung_task_panic: Sample<String>,
    pub printk_ratelimit: Sample<u64>,
    pub printk_ratelimit_burst: Sample<u64>,
    pub sched_cfs_bandwidth_slice_us: Sample<u64>,
    pub oops_limit: Sample<u64>,
    pub hardlockup_panic: Sample<String>,
    pub softlockup_panic: Sample<String>,
    pub oom_dump_tasks: Sample<String>,
    pub user_reserve_kbytes: Sample<u64>,
    pub unprivileged_userfaultfd: Sample<String>,
    pub ngroups_max: Sample<u64>,
    pub overflowuid: Sample<u64>,
    pub dir_notify_enable: Sample<String>,
    pub lease_break_time: Sample<u64>,
    pub sysctl_writes_strict: Sample<String>,
    pub dentry_nr: Sample<u64>,
    pub dentry_unused: Sample<u64>,
    pub admin_reserve_kbytes: Sample<u64>,
    pub perf_event_max_sample_rate: Sample<u64>,
    pub perf_cpu_time_max_percent: Sample<u64>,
    pub keys_gc_delay: Sample<u64>,
    /// `/proc/key-users` 行数。不要 dump `/proc/keys`。
    pub key_users: usize,
    pub vsyscall32: Sample<String>,
    pub ldisc_autoload: Sample<String>,
    /// `inode-state`：`inode_inuse = nr_inodes - nr_unused`，`inode_free` 为第 2 列。
    pub inode_inuse: Sample<u64>,
    pub inode_free: Sample<u64>,
    pub pty_max: Sample<u64>,
    pub pty_nr: Sample<u64>,
    /// 与 shmmax 一样，64 位上常接近 `u64::MAX`。
    pub shmall: Sample<String>,
    pub msgmnb: Sample<u64>,
    pub msgmni: Sample<u64>,
    pub overflowgid: Sample<u64>,
    /// `0` 允许；`1` 仅特权；`2` 完全禁止。
    pub io_uring_disabled: Sample<String>,
    /// `-1` 表示未绑定用户组。
    pub io_uring_group: Sample<i64>,
    pub sysvipc_shm: usize,
    pub sysvipc_sem: usize,
    pub sysvipc_msg: usize,
    pub consoles: Vec<Console>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Console {
    pub name: String,
    pub flags: String,
    pub extra: String,
}

pub fn collect(ctx: &ProbeCtx) -> SysctlReport {
    let mut notes = Vec::new();
    let file_nr = access::read_trimmed(ctx.proc_path("sys/fs/file-nr"));
    let (file_nr_alloc, file_nr_max) = parse_file_nr(&file_nr);
    let inode = access::read_trimmed(ctx.proc_path("sys/fs/inode-nr"));
    let inode_nr = match inode.value.as_deref() {
        Some(t) => t
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .map(|v| Sample::ok(v, inode.source.clone()))
            .unwrap_or_else(|| Sample::error(inode.source.clone(), "无法解析 inode-nr")),
        None => Sample {
            value: None,
            access: inode.access,
            source: inode.source,
            hint: inode.hint,
        },
    };
    let consoles = parse_consoles(&access::read_trimmed(ctx.proc_path("consoles")));
    if consoles.is_empty() {
        notes.push("无 /proc/consoles 行（容器里常见）。".into());
    }
    let (inode_inuse, inode_free) =
        parse_inode_state(&access::read_trimmed(ctx.proc_path("sys/fs/inode-state")));
    SysctlReport {
        file_nr_alloc,
        file_nr_max,
        inode_nr,
        pid_max: access::read_u64(ctx.proc_path("sys/kernel/pid_max")),
        threads_max: access::read_u64(ctx.proc_path("sys/kernel/threads-max")),
        swappiness: access::read_u64(ctx.proc_path("sys/vm/swappiness")),
        overcommit_memory: access::read_trimmed(ctx.proc_path("sys/vm/overcommit_memory")),
        overcommit_ratio: access::read_u64(ctx.proc_path("sys/vm/overcommit_ratio")),
        dirty_ratio: access::read_u64(ctx.proc_path("sys/vm/dirty_ratio")),
        dirty_background_ratio: access::read_u64(ctx.proc_path("sys/vm/dirty_background_ratio")),
        aslr: access::read_trimmed(ctx.proc_path("sys/kernel/randomize_va_space")),
        core_pattern: access::read_trimmed(ctx.proc_path("sys/kernel/core_pattern")),
        printk: access::read_trimmed(ctx.proc_path("sys/kernel/printk")),
        ip_forward: access::read_trimmed(ctx.proc_path("sys/net/ipv4/ip_forward")),
        aio_nr: access::read_u64(ctx.proc_path("sys/fs/aio-nr")),
        aio_max_nr: access::read_u64(ctx.proc_path("sys/fs/aio-max-nr")),
        inotify_max_user_watches: access::read_u64(ctx.proc_path("sys/fs/inotify/max_user_watches")),
        inotify_max_user_instances: access::read_u64(
            ctx.proc_path("sys/fs/inotify/max_user_instances"),
        ),
        nr_open: access::read_u64(ctx.proc_path("sys/fs/nr_open")),
        max_map_count: access::read_u64(ctx.proc_path("sys/vm/max_map_count")),
        mmap_min_addr: access::read_u64(ctx.proc_path("sys/vm/mmap_min_addr")),
        boot_id: access::read_trimmed(ctx.proc_path("sys/kernel/random/boot_id")),
        nmi_watchdog: access::read_trimmed(ctx.proc_path("sys/kernel/nmi_watchdog")),
        watchdog: access::read_trimmed(ctx.proc_path("sys/kernel/watchdog")),
        watchdog_thresh: access::read_u64(ctx.proc_path("sys/kernel/watchdog_thresh")),
        pipe_max_size: access::read_u64(ctx.proc_path("sys/fs/pipe-max-size")),
        file_max: access::read_u64(ctx.proc_path("sys/fs/file-max")),
        panic: access::read_i64(ctx.proc_path("sys/kernel/panic")),
        sysrq: access::read_trimmed(ctx.proc_path("sys/kernel/sysrq")),
        min_free_kbytes: access::read_u64(ctx.proc_path("sys/vm/min_free_kbytes")),
        vfs_cache_pressure: access::read_u64(ctx.proc_path("sys/vm/vfs_cache_pressure")),
        watermark_scale_factor: access::read_u64(ctx.proc_path("sys/vm/watermark_scale_factor")),
        dirty_expire_centisecs: access::read_u64(ctx.proc_path("sys/vm/dirty_expire_centisecs")),
        ctrl_alt_del: access::read_trimmed(ctx.proc_path("sys/kernel/ctrl-alt-del")),
        random_poolsize: access::read_u64(ctx.proc_path("sys/kernel/random/poolsize")),
        sched_autogroup: access::read_trimmed(ctx.proc_path("sys/kernel/sched_autogroup_enabled")),
        leases_enable: access::read_trimmed(ctx.proc_path("sys/fs/leases-enable")),
        suid_dumpable: access::read_trimmed(ctx.proc_path("sys/fs/suid_dumpable")),
        cap_last_cap: access::read_u64(ctx.proc_path("sys/kernel/cap_last_cap")),
        keys_maxkeys: access::read_u64(ctx.proc_path("sys/kernel/keys/maxkeys")),
        keys_maxbytes: access::read_u64(ctx.proc_path("sys/kernel/keys/maxbytes")),
        hung_task_timeout_secs: access::read_u64(ctx.proc_path("sys/kernel/hung_task_timeout_secs")),
        panic_on_oops: access::read_trimmed(ctx.proc_path("sys/kernel/panic_on_oops")),
        panic_on_warn: access::read_trimmed(ctx.proc_path("sys/kernel/panic_on_warn")),
        core_uses_pid: access::read_trimmed(ctx.proc_path("sys/kernel/core_uses_pid")),
        shmmax: access::read_trimmed(ctx.proc_path("sys/kernel/shmmax")),
        shmmni: access::read_u64(ctx.proc_path("sys/kernel/shmmni")),
        msgmax: access::read_u64(ctx.proc_path("sys/kernel/msgmax")),
        sem: access::read_trimmed(ctx.proc_path("sys/kernel/sem")),
        mqueue_queues_max: access::read_u64(ctx.proc_path("sys/fs/mqueue/queues_max")),
        inotify_max_queued_events: access::read_u64(
            ctx.proc_path("sys/fs/inotify/max_queued_events"),
        ),
        epoll_max_user_watches: access::read_u64(ctx.proc_path("sys/fs/epoll/max_user_watches")),
        dirty_writeback_centisecs: access::read_u64(
            ctx.proc_path("sys/vm/dirty_writeback_centisecs"),
        ),
        page_cluster: access::read_u64(ctx.proc_path("sys/vm/page-cluster")),
        sched_rt_runtime_us: access::read_i64(ctx.proc_path("sys/kernel/sched_rt_runtime_us")),
        sched_rt_period_us: access::read_u64(ctx.proc_path("sys/kernel/sched_rt_period_us")),
        sched_rr_timeslice_ms: access::read_u64(ctx.proc_path("sys/kernel/sched_rr_timeslice_ms")),
        numa_balancing: access::read_trimmed(ctx.proc_path("sys/kernel/numa_balancing")),
        timer_migration: access::read_trimmed(ctx.proc_path("sys/kernel/timer_migration")),
        panic_on_oom: access::read_trimmed(ctx.proc_path("sys/vm/panic_on_oom")),
        oom_kill_allocating_task: access::read_trimmed(
            ctx.proc_path("sys/vm/oom_kill_allocating_task"),
        ),
        laptop_mode: access::read_trimmed(ctx.proc_path("sys/vm/laptop_mode")),
        kexec_load_disabled: access::read_trimmed(ctx.proc_path("sys/kernel/kexec_load_disabled")),
        hung_task_panic: access::read_trimmed(ctx.proc_path("sys/kernel/hung_task_panic")),
        printk_ratelimit: access::read_u64(ctx.proc_path("sys/kernel/printk_ratelimit")),
        printk_ratelimit_burst: access::read_u64(ctx.proc_path("sys/kernel/printk_ratelimit_burst")),
        sched_cfs_bandwidth_slice_us: access::read_u64(
            ctx.proc_path("sys/kernel/sched_cfs_bandwidth_slice_us"),
        ),
        oops_limit: access::read_u64(ctx.proc_path("sys/kernel/oops_limit")),
        hardlockup_panic: access::read_trimmed(ctx.proc_path("sys/kernel/hardlockup_panic")),
        softlockup_panic: access::read_trimmed(ctx.proc_path("sys/kernel/softlockup_panic")),
        oom_dump_tasks: access::read_trimmed(ctx.proc_path("sys/vm/oom_dump_tasks")),
        user_reserve_kbytes: access::read_u64(ctx.proc_path("sys/vm/user_reserve_kbytes")),
        unprivileged_userfaultfd: access::read_trimmed(
            ctx.proc_path("sys/vm/unprivileged_userfaultfd"),
        ),
        ngroups_max: access::read_u64(ctx.proc_path("sys/kernel/ngroups_max")),
        overflowuid: access::read_u64(ctx.proc_path("sys/fs/overflowuid")),
        dir_notify_enable: access::read_trimmed(ctx.proc_path("sys/fs/dir-notify-enable")),
        lease_break_time: access::read_u64(ctx.proc_path("sys/fs/lease-break-time")),
        sysctl_writes_strict: access::read_trimmed(ctx.proc_path("sys/kernel/sysctl_writes_strict")),
        dentry_nr: parse_state_nth(&access::read_trimmed(ctx.proc_path("sys/fs/dentry-state")), 0),
        dentry_unused: parse_state_nth(
            &access::read_trimmed(ctx.proc_path("sys/fs/dentry-state")),
            1,
        ),
        admin_reserve_kbytes: access::read_u64(ctx.proc_path("sys/vm/admin_reserve_kbytes")),
        perf_event_max_sample_rate: access::read_u64(
            ctx.proc_path("sys/kernel/perf_event_max_sample_rate"),
        ),
        perf_cpu_time_max_percent: access::read_u64(
            ctx.proc_path("sys/kernel/perf_cpu_time_max_percent"),
        ),
        keys_gc_delay: access::read_u64(ctx.proc_path("sys/kernel/keys/gc_delay")),
        key_users: count_data_lines(&access::read_trimmed(ctx.proc_path("key-users"))),
        vsyscall32: access::read_trimmed(ctx.proc_path("sys/abi/vsyscall32")),
        ldisc_autoload: access::read_trimmed(ctx.proc_path("sys/dev/tty/ldisc_autoload")),
        inode_inuse,
        inode_free,
        pty_max: access::read_u64(ctx.proc_path("sys/kernel/pty/max")),
        pty_nr: access::read_u64(ctx.proc_path("sys/kernel/pty/nr")),
        shmall: access::read_trimmed(ctx.proc_path("sys/kernel/shmall")),
        msgmnb: access::read_u64(ctx.proc_path("sys/kernel/msgmnb")),
        msgmni: access::read_u64(ctx.proc_path("sys/kernel/msgmni")),
        overflowgid: access::read_u64(ctx.proc_path("sys/fs/overflowgid")),
        io_uring_disabled: access::read_trimmed(ctx.proc_path("sys/kernel/io_uring_disabled")),
        io_uring_group: access::read_i64(ctx.proc_path("sys/kernel/io_uring_group")),
        sysvipc_shm: count_table_rows(&access::read_trimmed(ctx.proc_path("sysvipc/shm"))),
        sysvipc_sem: count_table_rows(&access::read_trimmed(ctx.proc_path("sysvipc/sem"))),
        sysvipc_msg: count_table_rows(&access::read_trimmed(ctx.proc_path("sysvipc/msg"))),
        consoles,
        notes,
    }
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

fn count_data_lines(sample: &Sample<String>) -> usize {
    let Some(text) = sample.value.as_deref() else {
        return 0;
    };
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

fn parse_state_nth(sample: &Sample<String>, idx: usize) -> Sample<u64> {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return miss();
    };
    match text.split_whitespace().nth(idx).and_then(|s| s.parse().ok()) {
        Some(v) => Sample::ok(v, sample.source.clone()),
        None => miss(),
    }
}

/// `inode-state`：第 1 列 nr_inodes（已分配），第 2 列 nr_unused。
/// `inode_inuse = nr_inodes - nr_unused`，空闲大于已分配时记为读取失败。
pub fn parse_inode_state(sample: &Sample<String>) -> (Sample<u64>, Sample<u64>) {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return (miss(), miss());
    };
    let mut it = text.split_whitespace();
    let nr = it.next().and_then(|s| s.parse::<u64>().ok());
    let free = it.next().and_then(|s| s.parse::<u64>().ok());
    match (nr, free) {
        (Some(nr), Some(free)) if free <= nr => (
            Sample::ok(nr - free, sample.source.clone()),
            Sample::ok(free, sample.source.clone()),
        ),
        (Some(_), Some(_)) => (
            Sample::error(sample.source.clone(), "inode-state 空闲大于已分配"),
            Sample::error(sample.source.clone(), "inode-state 空闲大于已分配"),
        ),
        _ => (miss(), miss()),
    }
}

/// `file-nr`：已分配、未用、上限。
pub fn parse_file_nr(sample: &Sample<String>) -> (Sample<u64>, Sample<u64>) {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return (miss(), miss());
    };
    let mut it = text.split_whitespace();
    let alloc = it.next().and_then(|s| s.parse().ok());
    let _unused = it.next();
    let max = it.next().and_then(|s| s.parse().ok());
    (
        alloc
            .map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
        max.map(|v| Sample::ok(v, sample.source.clone()))
            .unwrap_or_else(miss),
    )
}

pub fn parse_consoles(sample: &Sample<String>) -> Vec<Console> {
    let Some(text) = sample.value.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let flags = it.next().unwrap_or("").to_string();
        let extra = it.collect::<Vec<_>>().join(" ");
        out.push(Console {
            name: name.to_string(),
            flags,
            extra,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn file_nr_and_console_lines() {
        let nr = Sample::ok("864\t0\t9223372036854775807".into(), "file-nr");
        let (a, m) = parse_file_nr(&nr);
        assert_eq!(a.value, Some(864));
        assert_eq!(m.value, Some(9223372036854775807));
        let cons = parse_consoles(&Sample::ok(
            "ttyS0                -W- (EC    a)    4:64\n".into(),
            "consoles",
        ));
        assert_eq!(cons[0].name, "ttyS0");
        assert_eq!(cons[0].flags, "-W-");
        assert!(cons[0].extra.contains("4:64"));
    }

    #[test]
    fn collect_sysctl_fixture() {
        let root = std::env::temp_dir().join(format!("aida-sysctl-{}", std::process::id()));
        fs::create_dir_all(root.join("proc/sys/fs")).unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel")).unwrap();
        fs::create_dir_all(root.join("proc/sys/vm")).unwrap();
        fs::create_dir_all(root.join("proc/sys/net/ipv4")).unwrap();
        fs::write(root.join("proc/sys/fs/file-nr"), "10\t0\t100\n").unwrap();
        fs::write(root.join("proc/sys/fs/inode-nr"), "20\t5\n").unwrap();
        fs::write(root.join("proc/sys/kernel/pid_max"), "32768\n").unwrap();
        fs::write(root.join("proc/sys/kernel/threads-max"), "1000\n").unwrap();
        fs::write(root.join("proc/sys/vm/swappiness"), "60\n").unwrap();
        fs::write(root.join("proc/sys/vm/overcommit_memory"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/randomize_va_space"), "2\n").unwrap();
        fs::write(root.join("proc/sys/kernel/core_pattern"), "core\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/fs/inotify")).unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel/random")).unwrap();
        fs::write(root.join("proc/sys/fs/aio-nr"), "0\n").unwrap();
        fs::write(root.join("proc/sys/fs/aio-max-nr"), "65536\n").unwrap();
        fs::write(root.join("proc/sys/fs/inotify/max_user_watches"), "8192\n").unwrap();
        fs::write(root.join("proc/sys/kernel/random/boot_id"), "aaaa-bbbb\n").unwrap();
        fs::write(root.join("proc/sys/kernel/nmi_watchdog"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/panic"), "1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sysrq"), "1\n").unwrap();
        fs::write(root.join("proc/sys/vm/min_free_kbytes"), "67584\n").unwrap();
        fs::write(root.join("proc/sys/fs/file-max"), "100000\n").unwrap();
        fs::write(root.join("proc/sys/vm/watermark_scale_factor"), "10\n").unwrap();
        fs::write(root.join("proc/sys/kernel/ctrl-alt-del"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/random/poolsize"), "256\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel/keys")).unwrap();
        fs::write(root.join("proc/sys/kernel/keys/maxkeys"), "200\n").unwrap();
        fs::write(root.join("proc/sys/kernel/keys/maxbytes"), "20000\n").unwrap();
        fs::write(root.join("proc/sys/kernel/hung_task_timeout_secs"), "120\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sched_rt_runtime_us"), "-1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sched_rt_period_us"), "1000000\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sched_rr_timeslice_ms"), "100\n").unwrap();
        fs::write(root.join("proc/sys/kernel/numa_balancing"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/timer_migration"), "1\n").unwrap();
        fs::write(root.join("proc/sys/vm/panic_on_oom"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/oom_kill_allocating_task"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/laptop_mode"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/kexec_load_disabled"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/hung_task_panic"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/printk_ratelimit"), "5\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sched_cfs_bandwidth_slice_us"), "5000\n").unwrap();
        fs::write(root.join("proc/sys/kernel/oops_limit"), "10000\n").unwrap();
        fs::write(root.join("proc/sys/kernel/hardlockup_panic"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/oom_dump_tasks"), "1\n").unwrap();
        fs::write(root.join("proc/sys/vm/user_reserve_kbytes"), "131072\n").unwrap();
        fs::write(root.join("proc/sys/vm/unprivileged_userfaultfd"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/ngroups_max"), "65536\n").unwrap();
        fs::write(root.join("proc/sys/fs/overflowuid"), "65534\n").unwrap();
        fs::write(root.join("proc/sys/fs/dir-notify-enable"), "1\n").unwrap();
        fs::write(root.join("proc/sys/fs/lease-break-time"), "45\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sysctl_writes_strict"), "1\n").unwrap();
        fs::write(root.join("proc/sys/fs/dentry-state"), "100 40 45 0 0 0\n").unwrap();
        fs::write(root.join("proc/sys/vm/admin_reserve_kbytes"), "8192\n").unwrap();
        fs::write(root.join("proc/sys/kernel/perf_event_max_sample_rate"), "100000\n").unwrap();
        fs::write(root.join("proc/sys/kernel/perf_cpu_time_max_percent"), "25\n").unwrap();
        fs::write(root.join("proc/sys/kernel/keys/gc_delay"), "300\n").unwrap();
        fs::write(
            root.join("proc/key-users"),
            "    0:    32 31/31 25/1000000 505/25000000\n  997:     1 1/1 1/200 9/20000\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("proc/sys/abi")).unwrap();
        fs::write(root.join("proc/sys/abi/vsyscall32"), "1\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/dev/tty")).unwrap();
        fs::write(root.join("proc/sys/dev/tty/ldisc_autoload"), "1\n").unwrap();
        fs::write(root.join("proc/sys/fs/inode-state"), "80 12 45 0 0 0 0\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel/pty")).unwrap();
        fs::write(root.join("proc/sys/kernel/pty/max"), "4096\n").unwrap();
        fs::write(root.join("proc/sys/kernel/pty/nr"), "3\n").unwrap();
        fs::write(root.join("proc/sys/kernel/shmall"), "18446744073692774399\n").unwrap();
        fs::write(root.join("proc/sys/kernel/msgmnb"), "16384\n").unwrap();
        fs::write(root.join("proc/sys/kernel/msgmni"), "32000\n").unwrap();
        fs::write(root.join("proc/sys/fs/overflowgid"), "65534\n").unwrap();
        fs::write(root.join("proc/sys/kernel/io_uring_disabled"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/io_uring_group"), "-1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/shmmax"), "18446744073692774399\n").unwrap();
        fs::write(root.join("proc/sys/kernel/shmmni"), "4096\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/fs/mqueue")).unwrap();
        fs::write(root.join("proc/sys/fs/mqueue/queues_max"), "256\n").unwrap();
        fs::create_dir_all(root.join("proc/sysvipc")).unwrap();
        fs::write(
            root.join("proc/sysvipc/shm"),
            "key shmid\n0 7\n0 10\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/fs/suid_dumpable"), "0\n").unwrap();
        fs::write(root.join("proc/consoles"), "tty0                 -WU (E    )    4:1\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.file_nr_alloc.value, Some(10));
        assert_eq!(r.pid_max.value, Some(32768));
        assert_eq!(r.aslr.value.as_deref(), Some("2"));
        assert_eq!(r.consoles[0].name, "tty0");
        assert_eq!(r.aio_max_nr.value, Some(65536));
        assert_eq!(r.boot_id.value.as_deref(), Some("aaaa-bbbb"));
        assert_eq!(r.nmi_watchdog.value.as_deref(), Some("0"));
        assert_eq!(r.panic.value, Some(1));
        assert_eq!(r.sysrq.value.as_deref(), Some("1"));
        assert_eq!(r.file_max.value, Some(100000));
        assert_eq!(r.watermark_scale_factor.value, Some(10));
        assert_eq!(r.random_poolsize.value, Some(256));
        assert_eq!(r.keys_maxkeys.value, Some(200));
        assert_eq!(r.keys_maxbytes.value, Some(20000));
        assert_eq!(r.hung_task_timeout_secs.value, Some(120));
        assert_eq!(r.sched_rt_runtime_us.value, Some(-1));
        assert_eq!(r.sched_rt_period_us.value, Some(1_000_000));
        assert_eq!(r.sched_rr_timeslice_ms.value, Some(100));
        assert_eq!(r.numa_balancing.value.as_deref(), Some("0"));
        assert_eq!(r.timer_migration.value.as_deref(), Some("1"));
        assert_eq!(r.panic_on_oom.value.as_deref(), Some("0"));
        assert_eq!(r.oom_kill_allocating_task.value.as_deref(), Some("0"));
        assert_eq!(r.laptop_mode.value.as_deref(), Some("0"));
        assert_eq!(r.kexec_load_disabled.value.as_deref(), Some("0"));
        assert_eq!(r.hung_task_panic.value.as_deref(), Some("0"));
        assert_eq!(r.printk_ratelimit.value, Some(5));
        assert_eq!(r.sched_cfs_bandwidth_slice_us.value, Some(5000));
        assert_eq!(r.oops_limit.value, Some(10_000));
        assert_eq!(r.hardlockup_panic.value.as_deref(), Some("0"));
        assert_eq!(r.oom_dump_tasks.value.as_deref(), Some("1"));
        assert_eq!(r.user_reserve_kbytes.value, Some(131072));
        assert_eq!(r.unprivileged_userfaultfd.value.as_deref(), Some("0"));
        assert_eq!(r.ngroups_max.value, Some(65536));
        assert_eq!(r.overflowuid.value, Some(65534));
        assert_eq!(r.dir_notify_enable.value.as_deref(), Some("1"));
        assert_eq!(r.lease_break_time.value, Some(45));
        assert_eq!(r.dentry_nr.value, Some(100));
        assert_eq!(r.dentry_unused.value, Some(40));
        assert_eq!(r.admin_reserve_kbytes.value, Some(8192));
        assert_eq!(r.perf_event_max_sample_rate.value, Some(100_000));
        assert_eq!(r.key_users, 2);
        assert_eq!(r.vsyscall32.value.as_deref(), Some("1"));
        assert_eq!(r.ldisc_autoload.value.as_deref(), Some("1"));
        assert_eq!(r.inode_inuse.value, Some(68));
        assert_eq!(r.inode_free.value, Some(12));
        assert_eq!(r.pty_max.value, Some(4096));
        assert_eq!(r.pty_nr.value, Some(3));
        assert_eq!(r.shmall.value.as_deref(), Some("18446744073692774399"));
        assert_eq!(r.msgmnb.value, Some(16384));
        assert_eq!(r.msgmni.value, Some(32000));
        assert_eq!(r.overflowgid.value, Some(65534));
        assert_eq!(r.io_uring_disabled.value.as_deref(), Some("0"));
        assert_eq!(r.io_uring_group.value, Some(-1));
        assert_eq!(r.shmmax.value.as_deref(), Some("18446744073692774399"));
        assert_eq!(r.shmmni.value, Some(4096));
        assert_eq!(r.mqueue_queues_max.value, Some(256));
        assert_eq!(r.sysvipc_shm, 2);
        assert_eq!(r.suid_dumpable.value.as_deref(), Some("0"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn panic_timeout_accepts_negative() {
        let root = std::env::temp_dir().join(format!("aida-panic-neg-{}", std::process::id()));
        fs::create_dir_all(root.join("proc/sys/kernel")).unwrap();
        fs::write(root.join("proc/sys/kernel/panic"), "-1\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.panic.value, Some(-1));
        assert_eq!(r.panic.access, crate::access::AccessKind::Ok);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn inode_state_inuse_subtracts_unused() {
        let (inuse, free) = parse_inode_state(&Sample::ok("80 12 45 0 0 0 0".into(), "inode-state"));
        assert_eq!(inuse.value, Some(68));
        assert_eq!(free.value, Some(12));
        let (bad_inuse, bad_free) =
            parse_inode_state(&Sample::ok("10 12".into(), "inode-state"));
        assert_eq!(bad_inuse.access, crate::access::AccessKind::Error);
        assert_eq!(bad_free.access, crate::access::AccessKind::Error);
        assert!(bad_inuse.value.is_none());
    }
}
