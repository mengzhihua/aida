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
    /// `/proc/key-users` 行数。不要 dump `/proc/keys`。失败不是 0。
    pub key_users: Sample<usize>,
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
    /// `0` 表示改用 `dirty_ratio`。
    pub dirty_bytes: Sample<u64>,
    pub dirty_background_bytes: Sample<u64>,
    /// `0` 表示改用 `overcommit_ratio`。
    pub overcommit_kbytes: Sample<u64>,
    pub pipe_user_pages_soft: Sample<u64>,
    pub pipe_user_pages_hard: Sample<u64>,
    pub compact_unevictable_allowed: Sample<String>,
    pub watermark_boost_factor: Sample<u64>,
    pub unknown_nmi_panic: Sample<String>,
    /// `0` 表示不限制 core dump 管道。
    pub core_pipe_limit: Sample<u64>,
    pub printk_devkmsg: Sample<String>,
    pub task_delayacct: Sample<String>,
    /// 三个 token：highwater / lowwater / frequency。
    pub acct: Sample<String>,
    pub zone_reclaim_mode: Sample<u64>,
    pub mount_max: Sample<u64>,
    pub write_wakeup_threshold: Sample<u64>,
    pub urandom_min_reseed_secs: Sample<u64>,
    pub shm_rmid_forced: Sample<String>,
    /// `0` 不限制；`1` 仅 dumpable；`2` 一律禁止可执行 memfd。
    pub memfd_noexec: Sample<String>,
    pub dirtytime_expire_seconds: Sample<u64>,
    pub soft_watchdog: Sample<String>,
    pub watchdog_cpumask: Sample<String>,
    pub panic_on_rcu_stall: Sample<String>,
    /// `0` 表示不限制 warn 次数。
    pub warn_limit: Sample<u64>,
    /// `-1` 表示不限制 kexec load（panic 路径）。
    pub kexec_load_limit_panic: Sample<i64>,
    pub split_lock_mitigate: Sample<String>,
    /// 可为负；不要当无符号解析。
    pub hung_task_warnings: Sample<i64>,
    pub hung_task_check_count: Sample<u64>,
    /// `0` 表示沿用 `hung_task_timeout_secs`。
    pub hung_task_check_interval_secs: Sample<u64>,
    /// `-1` 表示不限制 kexec load（reboot 路径）。
    pub kexec_load_limit_reboot: Sample<i64>,
    /// `0` 表示不把 RCU stall 升级为 panic。
    pub max_rcu_stall_to_panic: Sample<u64>,
    /// panic 附加信息位图，保留原文。
    pub panic_print: Sample<String>,
    pub panic_on_io_nmi: Sample<String>,
    pub hung_task_all_cpu_backtrace: Sample<String>,
    pub panic_on_unrecovered_nmi: Sample<String>,
    pub oops_all_cpu_backtrace: Sample<String>,
    pub compaction_proactiveness: Sample<u64>,
    pub page_lock_unfairness: Sample<u64>,
    pub sched_deadline_period_max_us: Sample<u64>,
    pub sched_deadline_period_min_us: Sample<u64>,
    pub hardlockup_all_cpu_backtrace: Sample<String>,
    pub print_fatal_signals: Sample<String>,
    pub bpf_stats_enabled: Sample<String>,
    pub core_sort_vma: Sample<String>,
    pub min_slab_ratio: Sample<u64>,
    pub min_unmapped_ratio: Sample<u64>,
    pub softlockup_all_cpu_backtrace: Sample<String>,
    pub io_delay_type: Sample<u64>,
    pub extfrag_threshold: Sample<u64>,
    pub stat_interval: Sample<u64>,
    /// `0` 表示 printk 后不加额外延迟。
    pub printk_delay: Sample<u64>,
    pub max_lock_depth: Sample<u64>,
    pub perf_event_mlock_kb: Sample<u64>,
    /// `0` 表示不压缩 hugetlb vmemmap。
    pub hugetlb_optimize_vmemmap: Sample<String>,
    pub perf_event_max_stack: Sample<u64>,
    pub perf_event_max_contexts_per_stack: Sample<u64>,
    /// `0` 表示使用内核默认 per-cpu 高水位。
    pub percpu_pagelist_high_fraction: Sample<u64>,
    /// `1` 表示采集 NUMA VM 计数。
    pub numa_stat: Sample<String>,
    /// NUMA 热页提升速率上限，单位 MB/s。
    pub numa_balancing_promote_rate_limit_mbps: Sample<u64>,
    /// `0` 表示新的 64-bit VA 布局。
    pub legacy_va_layout: Sample<String>,
    /// `0` 表示没有组可通过 shm 分配 hugetlb。
    pub hugetlb_shm_group: Sample<u64>,
    pub core_file_note_size_limit: Sample<u64>,
    /// `0` 表示不自动重算 `msgmni`。
    pub auto_msgmni: Sample<String>,
    pub numa_zonelist_order: Sample<String>,
    /// 各 zone 的 lowmem 预留比例，空白分隔。
    pub lowmem_reserve_ratio: Sample<String>,
    /// `0` 表示不额外 overcommit hugepage。
    pub nr_overcommit_hugepages: Sample<u64>,
    /// `0` 表示 hugepage 计数不按 mempolicy 节点拆。
    pub nr_hugepages_mempolicy: Sample<u64>,
    /// `0` 表示默认大小 hugepage 池没有静态预留页。
    pub nr_hugepages: Sample<u64>,
    /// `0` 表示不启用 ACPI video 特殊标志。
    pub acpi_video_flags: Sample<u64>,
    /// x86 启动协议里的 bootloader 类型；`0` 表示未声明。
    pub bootloader_type: Sample<u64>,
    pub bootloader_version: Sample<u64>,
    /// `1` 强制走 sysfs 固件回退（调试用）。
    pub firmware_force_sysfs_fallback: Sample<String>,
    /// `1` 忽略 sysfs 固件回退。
    pub firmware_ignore_sysfs_fallback: Sample<String>,
    /// initramfs 声明的根设备号。`0` 表示未设。
    pub real_root_dev: Sample<u64>,
    /// `0` 表示不向 `/proc/schedstat` 额外导出调度统计。无 `CONFIG_SCHEDSTATS` 时 NotFound。
    pub sched_schedstats: Sample<String>,
    /// `0` 表示 WARN 时不关掉 tracing。无 tracing 时 NotFound。
    pub traceoff_on_warning: Sample<String>,
    /// `kernel.arch`，与 `uname -m` 同类，不调用 uname。
    pub kernel_arch: Sample<String>,
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
    let dentry_state = access::read_trimmed(ctx.proc_path("sys/fs/dentry-state"));
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
        inotify_max_user_watches: access::read_u64(
            ctx.proc_path("sys/fs/inotify/max_user_watches"),
        ),
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
        hung_task_timeout_secs: access::read_u64(
            ctx.proc_path("sys/kernel/hung_task_timeout_secs"),
        ),
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
        printk_ratelimit_burst: access::read_u64(
            ctx.proc_path("sys/kernel/printk_ratelimit_burst"),
        ),
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
        sysctl_writes_strict: access::read_trimmed(
            ctx.proc_path("sys/kernel/sysctl_writes_strict"),
        ),
        dentry_nr: parse_state_nth(&dentry_state, 0),
        dentry_unused: parse_state_nth(&dentry_state, 1),
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
        dirty_bytes: access::read_u64(ctx.proc_path("sys/vm/dirty_bytes")),
        dirty_background_bytes: access::read_u64(ctx.proc_path("sys/vm/dirty_background_bytes")),
        overcommit_kbytes: access::read_u64(ctx.proc_path("sys/vm/overcommit_kbytes")),
        pipe_user_pages_soft: access::read_u64(ctx.proc_path("sys/fs/pipe-user-pages-soft")),
        pipe_user_pages_hard: access::read_u64(ctx.proc_path("sys/fs/pipe-user-pages-hard")),
        compact_unevictable_allowed: access::read_trimmed(
            ctx.proc_path("sys/vm/compact_unevictable_allowed"),
        ),
        watermark_boost_factor: access::read_u64(ctx.proc_path("sys/vm/watermark_boost_factor")),
        unknown_nmi_panic: access::read_trimmed(ctx.proc_path("sys/kernel/unknown_nmi_panic")),
        core_pipe_limit: access::read_u64(ctx.proc_path("sys/kernel/core_pipe_limit")),
        printk_devkmsg: access::read_trimmed(ctx.proc_path("sys/kernel/printk_devkmsg")),
        task_delayacct: access::read_trimmed(ctx.proc_path("sys/kernel/task_delayacct")),
        acct: access::read_trimmed(ctx.proc_path("sys/kernel/acct")),
        zone_reclaim_mode: access::read_u64(ctx.proc_path("sys/vm/zone_reclaim_mode")),
        mount_max: access::read_u64(ctx.proc_path("sys/fs/mount-max")),
        write_wakeup_threshold: access::read_u64(
            ctx.proc_path("sys/kernel/random/write_wakeup_threshold"),
        ),
        urandom_min_reseed_secs: access::read_u64(
            ctx.proc_path("sys/kernel/random/urandom_min_reseed_secs"),
        ),
        shm_rmid_forced: access::read_trimmed(ctx.proc_path("sys/kernel/shm_rmid_forced")),
        memfd_noexec: access::read_trimmed(ctx.proc_path("sys/vm/memfd_noexec")),
        dirtytime_expire_seconds: access::read_u64(
            ctx.proc_path("sys/vm/dirtytime_expire_seconds"),
        ),
        soft_watchdog: access::read_trimmed(ctx.proc_path("sys/kernel/soft_watchdog")),
        watchdog_cpumask: access::read_trimmed(ctx.proc_path("sys/kernel/watchdog_cpumask")),
        panic_on_rcu_stall: access::read_trimmed(ctx.proc_path("sys/kernel/panic_on_rcu_stall")),
        warn_limit: access::read_u64(ctx.proc_path("sys/kernel/warn_limit")),
        kexec_load_limit_panic: access::read_i64(
            ctx.proc_path("sys/kernel/kexec_load_limit_panic"),
        ),
        split_lock_mitigate: access::read_trimmed(ctx.proc_path("sys/kernel/split_lock_mitigate")),
        hung_task_warnings: access::read_i64(ctx.proc_path("sys/kernel/hung_task_warnings")),
        hung_task_check_count: access::read_u64(ctx.proc_path("sys/kernel/hung_task_check_count")),
        hung_task_check_interval_secs: access::read_u64(
            ctx.proc_path("sys/kernel/hung_task_check_interval_secs"),
        ),
        kexec_load_limit_reboot: access::read_i64(
            ctx.proc_path("sys/kernel/kexec_load_limit_reboot"),
        ),
        max_rcu_stall_to_panic: access::read_u64(
            ctx.proc_path("sys/kernel/max_rcu_stall_to_panic"),
        ),
        panic_print: access::read_trimmed(ctx.proc_path("sys/kernel/panic_print")),
        panic_on_io_nmi: access::read_trimmed(ctx.proc_path("sys/kernel/panic_on_io_nmi")),
        hung_task_all_cpu_backtrace: access::read_trimmed(
            ctx.proc_path("sys/kernel/hung_task_all_cpu_backtrace"),
        ),
        panic_on_unrecovered_nmi: access::read_trimmed(
            ctx.proc_path("sys/kernel/panic_on_unrecovered_nmi"),
        ),
        oops_all_cpu_backtrace: access::read_trimmed(
            ctx.proc_path("sys/kernel/oops_all_cpu_backtrace"),
        ),
        compaction_proactiveness: access::read_u64(
            ctx.proc_path("sys/vm/compaction_proactiveness"),
        ),
        page_lock_unfairness: access::read_u64(ctx.proc_path("sys/vm/page_lock_unfairness")),
        sched_deadline_period_max_us: access::read_u64(
            ctx.proc_path("sys/kernel/sched_deadline_period_max_us"),
        ),
        sched_deadline_period_min_us: access::read_u64(
            ctx.proc_path("sys/kernel/sched_deadline_period_min_us"),
        ),
        hardlockup_all_cpu_backtrace: access::read_trimmed(
            ctx.proc_path("sys/kernel/hardlockup_all_cpu_backtrace"),
        ),
        print_fatal_signals: access::read_trimmed(ctx.proc_path("sys/kernel/print-fatal-signals")),
        bpf_stats_enabled: access::read_trimmed(ctx.proc_path("sys/kernel/bpf_stats_enabled")),
        core_sort_vma: access::read_trimmed(ctx.proc_path("sys/kernel/core_sort_vma")),
        min_slab_ratio: access::read_u64(ctx.proc_path("sys/vm/min_slab_ratio")),
        min_unmapped_ratio: access::read_u64(ctx.proc_path("sys/vm/min_unmapped_ratio")),
        softlockup_all_cpu_backtrace: access::read_trimmed(
            ctx.proc_path("sys/kernel/softlockup_all_cpu_backtrace"),
        ),
        io_delay_type: access::read_u64(ctx.proc_path("sys/kernel/io_delay_type")),
        extfrag_threshold: access::read_u64(ctx.proc_path("sys/vm/extfrag_threshold")),
        stat_interval: access::read_u64(ctx.proc_path("sys/vm/stat_interval")),
        printk_delay: access::read_u64(ctx.proc_path("sys/kernel/printk_delay")),
        max_lock_depth: access::read_u64(ctx.proc_path("sys/kernel/max_lock_depth")),
        perf_event_mlock_kb: access::read_u64(ctx.proc_path("sys/kernel/perf_event_mlock_kb")),
        hugetlb_optimize_vmemmap: access::read_trimmed(
            ctx.proc_path("sys/vm/hugetlb_optimize_vmemmap"),
        ),
        perf_event_max_stack: access::read_u64(ctx.proc_path("sys/kernel/perf_event_max_stack")),
        perf_event_max_contexts_per_stack: access::read_u64(
            ctx.proc_path("sys/kernel/perf_event_max_contexts_per_stack"),
        ),
        percpu_pagelist_high_fraction: access::read_u64(
            ctx.proc_path("sys/vm/percpu_pagelist_high_fraction"),
        ),
        numa_stat: access::read_trimmed(ctx.proc_path("sys/vm/numa_stat")),
        numa_balancing_promote_rate_limit_mbps: access::read_u64(
            ctx.proc_path("sys/kernel/numa_balancing_promote_rate_limit_MBps"),
        ),
        legacy_va_layout: access::read_trimmed(ctx.proc_path("sys/vm/legacy_va_layout")),
        hugetlb_shm_group: access::read_u64(ctx.proc_path("sys/vm/hugetlb_shm_group")),
        core_file_note_size_limit: access::read_u64(
            ctx.proc_path("sys/kernel/core_file_note_size_limit"),
        ),
        auto_msgmni: access::read_trimmed(ctx.proc_path("sys/kernel/auto_msgmni")),
        numa_zonelist_order: access::read_trimmed(ctx.proc_path("sys/vm/numa_zonelist_order")),
        lowmem_reserve_ratio: access::read_trimmed(ctx.proc_path("sys/vm/lowmem_reserve_ratio")),
        nr_overcommit_hugepages: access::read_u64(ctx.proc_path("sys/vm/nr_overcommit_hugepages")),
        nr_hugepages_mempolicy: access::read_u64(ctx.proc_path("sys/vm/nr_hugepages_mempolicy")),
        nr_hugepages: access::read_u64(ctx.proc_path("sys/vm/nr_hugepages")),
        acpi_video_flags: access::read_u64(ctx.proc_path("sys/kernel/acpi_video_flags")),
        bootloader_type: access::read_u64(ctx.proc_path("sys/kernel/bootloader_type")),
        bootloader_version: access::read_u64(ctx.proc_path("sys/kernel/bootloader_version")),
        firmware_force_sysfs_fallback: access::read_trimmed(
            ctx.proc_path("sys/kernel/firmware_config/force_sysfs_fallback"),
        ),
        firmware_ignore_sysfs_fallback: access::read_trimmed(
            ctx.proc_path("sys/kernel/firmware_config/ignore_sysfs_fallback"),
        ),
        real_root_dev: access::read_u64(ctx.proc_path("sys/kernel/real-root-dev")),
        sched_schedstats: access::read_trimmed(ctx.proc_path("sys/kernel/sched_schedstats")),
        traceoff_on_warning: access::read_trimmed(ctx.proc_path("sys/kernel/traceoff_on_warning")),
        kernel_arch: access::read_trimmed(ctx.proc_path("sys/kernel/arch")),
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

fn count_data_lines(sample: &Sample<String>) -> Sample<usize> {
    match sample.value.as_deref() {
        Some(text) => Sample::ok(
            text.lines().filter(|l| !l.trim().is_empty()).count(),
            sample.source.clone(),
        ),
        None => Sample {
            value: None,
            access: sample.access,
            source: sample.source.clone(),
            hint: sample.hint.clone(),
        },
    }
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
    match text
        .split_whitespace()
        .nth(idx)
        .and_then(|s| s.parse().ok())
    {
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
        fs::write(
            root.join("proc/sys/kernel/sched_cfs_bandwidth_slice_us"),
            "5000\n",
        )
        .unwrap();
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
        fs::write(
            root.join("proc/sys/kernel/perf_event_max_sample_rate"),
            "100000\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/kernel/perf_cpu_time_max_percent"),
            "25\n",
        )
        .unwrap();
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
        fs::write(
            root.join("proc/sys/kernel/shmall"),
            "18446744073692774399\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/msgmnb"), "16384\n").unwrap();
        fs::write(root.join("proc/sys/kernel/msgmni"), "32000\n").unwrap();
        fs::write(root.join("proc/sys/fs/overflowgid"), "65534\n").unwrap();
        fs::write(root.join("proc/sys/kernel/io_uring_disabled"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/io_uring_group"), "-1\n").unwrap();
        fs::write(root.join("proc/sys/vm/dirty_bytes"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/dirty_background_bytes"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/overcommit_kbytes"), "0\n").unwrap();
        fs::write(root.join("proc/sys/fs/pipe-user-pages-soft"), "16384\n").unwrap();
        fs::write(root.join("proc/sys/fs/pipe-user-pages-hard"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/compact_unevictable_allowed"), "1\n").unwrap();
        fs::write(root.join("proc/sys/vm/watermark_boost_factor"), "15000\n").unwrap();
        fs::write(root.join("proc/sys/kernel/unknown_nmi_panic"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/core_pipe_limit"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/printk_devkmsg"), "on\n").unwrap();
        fs::write(root.join("proc/sys/kernel/task_delayacct"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/acct"), "4\t2\t30\n").unwrap();
        fs::write(root.join("proc/sys/vm/zone_reclaim_mode"), "0\n").unwrap();
        fs::write(root.join("proc/sys/fs/mount-max"), "100000\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/random/write_wakeup_threshold"),
            "256\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/kernel/random/urandom_min_reseed_secs"),
            "60\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/shm_rmid_forced"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/memfd_noexec"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/dirtytime_expire_seconds"), "43200\n").unwrap();
        fs::write(root.join("proc/sys/kernel/soft_watchdog"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/watchdog_cpumask"), "0-3\n").unwrap();
        fs::write(root.join("proc/sys/kernel/panic_on_rcu_stall"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/warn_limit"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/kexec_load_limit_panic"), "-1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/split_lock_mitigate"), "1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/hung_task_warnings"), "10\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/hung_task_check_count"),
            "4194304\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/kernel/hung_task_check_interval_secs"),
            "0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/kexec_load_limit_reboot"), "-1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/max_rcu_stall_to_panic"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/panic_print"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/panic_on_io_nmi"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/hung_task_all_cpu_backtrace"),
            "0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/panic_on_unrecovered_nmi"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/oops_all_cpu_backtrace"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/compaction_proactiveness"), "20\n").unwrap();
        fs::write(root.join("proc/sys/vm/page_lock_unfairness"), "5\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/sched_deadline_period_max_us"),
            "4194304\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/kernel/sched_deadline_period_min_us"),
            "100\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/kernel/hardlockup_all_cpu_backtrace"),
            "0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/print-fatal-signals"), "1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/bpf_stats_enabled"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/core_sort_vma"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/min_slab_ratio"), "5\n").unwrap();
        fs::write(root.join("proc/sys/vm/min_unmapped_ratio"), "1\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/softlockup_all_cpu_backtrace"),
            "0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/io_delay_type"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/extfrag_threshold"), "500\n").unwrap();
        fs::write(root.join("proc/sys/vm/stat_interval"), "1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/printk_delay"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/max_lock_depth"), "1024\n").unwrap();
        fs::write(root.join("proc/sys/kernel/perf_event_mlock_kb"), "516\n").unwrap();
        fs::write(root.join("proc/sys/vm/hugetlb_optimize_vmemmap"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/perf_event_max_stack"), "127\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/perf_event_max_contexts_per_stack"),
            "8\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/vm/percpu_pagelist_high_fraction"),
            "0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/vm/numa_stat"), "1\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/numa_balancing_promote_rate_limit_MBps"),
            "65536\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/vm/legacy_va_layout"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/hugetlb_shm_group"), "0\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/core_file_note_size_limit"),
            "4194304\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/auto_msgmni"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/numa_zonelist_order"), "Node\n").unwrap();
        fs::write(
            root.join("proc/sys/vm/lowmem_reserve_ratio"),
            "256 256 32 0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/vm/nr_overcommit_hugepages"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/nr_hugepages_mempolicy"), "0\n").unwrap();
        fs::write(root.join("proc/sys/vm/nr_hugepages"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/acpi_video_flags"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/bootloader_type"), "176\n").unwrap();
        fs::write(root.join("proc/sys/kernel/bootloader_version"), "0\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel/firmware_config")).unwrap();
        fs::write(
            root.join("proc/sys/kernel/firmware_config/force_sysfs_fallback"),
            "0\n",
        )
        .unwrap();
        fs::write(
            root.join("proc/sys/kernel/firmware_config/ignore_sysfs_fallback"),
            "0\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/real-root-dev"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/sched_schedstats"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/traceoff_on_warning"), "0\n").unwrap();
        fs::write(root.join("proc/sys/kernel/arch"), "x86_64\n").unwrap();
        fs::write(
            root.join("proc/sys/kernel/shmmax"),
            "18446744073692774399\n",
        )
        .unwrap();
        fs::write(root.join("proc/sys/kernel/shmmni"), "4096\n").unwrap();
        fs::create_dir_all(root.join("proc/sys/fs/mqueue")).unwrap();
        fs::write(root.join("proc/sys/fs/mqueue/queues_max"), "256\n").unwrap();
        fs::create_dir_all(root.join("proc/sysvipc")).unwrap();
        fs::write(root.join("proc/sysvipc/shm"), "key shmid\n0 7\n0 10\n").unwrap();
        fs::write(root.join("proc/sys/fs/suid_dumpable"), "0\n").unwrap();
        fs::write(
            root.join("proc/consoles"),
            "tty0                 -WU (E    )    4:1\n",
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
        assert_eq!(r.key_users.value, Some(2));
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
        assert_eq!(r.dirty_bytes.value, Some(0));
        assert_eq!(r.dirty_background_bytes.value, Some(0));
        assert_eq!(r.overcommit_kbytes.value, Some(0));
        assert_eq!(r.pipe_user_pages_soft.value, Some(16384));
        assert_eq!(r.pipe_user_pages_hard.value, Some(0));
        assert_eq!(r.compact_unevictable_allowed.value.as_deref(), Some("1"));
        assert_eq!(r.watermark_boost_factor.value, Some(15000));
        assert_eq!(r.unknown_nmi_panic.value.as_deref(), Some("0"));
        assert_eq!(r.core_pipe_limit.value, Some(0));
        assert_eq!(r.printk_devkmsg.value.as_deref(), Some("on"));
        assert_eq!(r.task_delayacct.value.as_deref(), Some("0"));
        assert_eq!(r.acct.value.as_deref(), Some("4\t2\t30"));
        assert_eq!(r.zone_reclaim_mode.value, Some(0));
        assert_eq!(r.mount_max.value, Some(100000));
        assert_eq!(r.write_wakeup_threshold.value, Some(256));
        assert_eq!(r.urandom_min_reseed_secs.value, Some(60));
        assert_eq!(r.shm_rmid_forced.value.as_deref(), Some("0"));
        assert_eq!(r.memfd_noexec.value.as_deref(), Some("0"));
        assert_eq!(r.dirtytime_expire_seconds.value, Some(43200));
        assert_eq!(r.soft_watchdog.value.as_deref(), Some("0"));
        assert_eq!(r.watchdog_cpumask.value.as_deref(), Some("0-3"));
        assert_eq!(r.panic_on_rcu_stall.value.as_deref(), Some("0"));
        assert_eq!(r.warn_limit.value, Some(0));
        assert_eq!(r.kexec_load_limit_panic.value, Some(-1));
        assert_eq!(r.split_lock_mitigate.value.as_deref(), Some("1"));
        assert_eq!(r.hung_task_warnings.value, Some(10));
        assert_eq!(r.hung_task_check_count.value, Some(4_194_304));
        assert_eq!(r.hung_task_check_interval_secs.value, Some(0));
        assert_eq!(r.kexec_load_limit_reboot.value, Some(-1));
        assert_eq!(r.max_rcu_stall_to_panic.value, Some(0));
        assert_eq!(r.panic_print.value.as_deref(), Some("0"));
        assert_eq!(r.panic_on_io_nmi.value.as_deref(), Some("0"));
        assert_eq!(r.hung_task_all_cpu_backtrace.value.as_deref(), Some("0"));
        assert_eq!(r.panic_on_unrecovered_nmi.value.as_deref(), Some("0"));
        assert_eq!(r.oops_all_cpu_backtrace.value.as_deref(), Some("0"));
        assert_eq!(r.compaction_proactiveness.value, Some(20));
        assert_eq!(r.page_lock_unfairness.value, Some(5));
        assert_eq!(r.sched_deadline_period_max_us.value, Some(4_194_304));
        assert_eq!(r.sched_deadline_period_min_us.value, Some(100));
        assert_eq!(r.hardlockup_all_cpu_backtrace.value.as_deref(), Some("0"));
        assert_eq!(r.print_fatal_signals.value.as_deref(), Some("1"));
        assert_eq!(r.bpf_stats_enabled.value.as_deref(), Some("0"));
        assert_eq!(r.core_sort_vma.value.as_deref(), Some("0"));
        assert_eq!(r.min_slab_ratio.value, Some(5));
        assert_eq!(r.min_unmapped_ratio.value, Some(1));
        assert_eq!(r.softlockup_all_cpu_backtrace.value.as_deref(), Some("0"));
        assert_eq!(r.io_delay_type.value, Some(0));
        assert_eq!(r.extfrag_threshold.value, Some(500));
        assert_eq!(r.stat_interval.value, Some(1));
        assert_eq!(r.printk_delay.value, Some(0));
        assert_eq!(r.max_lock_depth.value, Some(1024));
        assert_eq!(r.perf_event_mlock_kb.value, Some(516));
        assert_eq!(r.hugetlb_optimize_vmemmap.value.as_deref(), Some("0"));
        assert_eq!(r.perf_event_max_stack.value, Some(127));
        assert_eq!(r.perf_event_max_contexts_per_stack.value, Some(8));
        assert_eq!(r.percpu_pagelist_high_fraction.value, Some(0));
        assert_eq!(r.numa_stat.value.as_deref(), Some("1"));
        assert_eq!(r.numa_balancing_promote_rate_limit_mbps.value, Some(65536));
        assert_eq!(r.legacy_va_layout.value.as_deref(), Some("0"));
        assert_eq!(r.hugetlb_shm_group.value, Some(0));
        assert_eq!(r.core_file_note_size_limit.value, Some(4_194_304));
        assert_eq!(r.auto_msgmni.value.as_deref(), Some("0"));
        assert_eq!(r.numa_zonelist_order.value.as_deref(), Some("Node"));
        assert_eq!(
            r.lowmem_reserve_ratio.value.as_deref(),
            Some("256 256 32 0")
        );
        assert_eq!(r.nr_overcommit_hugepages.value, Some(0));
        assert_eq!(r.nr_hugepages_mempolicy.value, Some(0));
        assert_eq!(r.nr_hugepages.value, Some(0));
        assert_eq!(r.acpi_video_flags.value, Some(0));
        assert_eq!(r.bootloader_type.value, Some(176));
        assert_eq!(r.bootloader_version.value, Some(0));
        assert_eq!(r.firmware_force_sysfs_fallback.value.as_deref(), Some("0"));
        assert_eq!(r.firmware_ignore_sysfs_fallback.value.as_deref(), Some("0"));
        assert_eq!(r.real_root_dev.value, Some(0));
        assert_eq!(r.sched_schedstats.value.as_deref(), Some("0"));
        assert_eq!(r.traceoff_on_warning.value.as_deref(), Some("0"));
        assert_eq!(r.kernel_arch.value.as_deref(), Some("x86_64"));
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
        let (inuse, free) =
            parse_inode_state(&Sample::ok("80 12 45 0 0 0 0".into(), "inode-state"));
        assert_eq!(inuse.value, Some(68));
        assert_eq!(free.value, Some(12));
        let (bad_inuse, bad_free) = parse_inode_state(&Sample::ok("10 12".into(), "inode-state"));
        assert_eq!(bad_inuse.access, crate::access::AccessKind::Error);
        assert_eq!(bad_free.access, crate::access::AccessKind::Error);
        assert!(bad_inuse.value.is_none());
    }

    #[test]
    fn missing_key_users_is_not_zero() {
        let root = std::env::temp_dir().join(format!("aida-key-users-{}", std::process::id()));
        fs::create_dir_all(root.join("proc")).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert!(r.key_users.value.is_none());
        assert_eq!(r.key_users.access, crate::access::AccessKind::NotFound);
        let _ = fs::remove_dir_all(&root);
    }
}
