//! 睡眠 / 休眠：`/sys/power`。不调用 `systemctl suspend`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct PmReport {
    pub state: Sample<String>,
    pub mem_sleep: Sample<String>,
    pub disk: Sample<String>,
    pub wakeup_count: Sample<String>,
    pub suspend_success: Sample<u64>,
    pub suspend_fail: Sample<u64>,
    pub wakeups: usize,
    pub notes: Vec<String>,
}

pub fn collect(ctx: &ProbeCtx) -> PmReport {
    let mut notes = Vec::new();
    let root = ctx.sys_path("power");
    let state = access::read_trimmed(root.join("state"));
    if state.access == AccessKind::NotFound {
        notes.push("无 /sys/power（容器未挂 sysfs 时常见）。".into());
    }
    let stats = root.join("suspend_stats");
    let wakeups = match access::list_dir_names(ctx.sys_path("class/wakeup")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(n),
            ..
        } => n.len(),
        _ => 0,
    };
    PmReport {
        mem_sleep: access::read_trimmed(root.join("mem_sleep")),
        disk: access::read_trimmed(root.join("disk")),
        wakeup_count: access::read_trimmed(root.join("wakeup_count")),
        suspend_success: access::read_u64(stats.join("success")),
        suspend_fail: access::read_u64(stats.join("fail")),
        wakeups,
        state,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sleep_states_and_suspend_stats() {
        let root = std::env::temp_dir().join(format!("aida-pm-{}", std::process::id()));
        let p = root.join("sys/power/suspend_stats");
        fs::create_dir_all(&p).unwrap();
        fs::create_dir_all(root.join("sys/class/wakeup/wakeup0")).unwrap();
        fs::create_dir_all(root.join("sys/class/wakeup/wakeup1")).unwrap();
        fs::write(root.join("sys/power/state"), "freeze mem disk\n").unwrap();
        fs::write(root.join("sys/power/mem_sleep"), "s2idle [deep]\n").unwrap();
        fs::write(root.join("sys/power/disk"), "[platform] shutdown reboot\n").unwrap();
        fs::write(root.join("sys/power/wakeup_count"), "3\n").unwrap();
        fs::write(p.join("success"), "4\n").unwrap();
        fs::write(p.join("fail"), "1\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.state.value.as_deref(), Some("freeze mem disk"));
        assert!(r.mem_sleep.value.as_deref().unwrap().contains("deep"));
        assert_eq!(r.suspend_success.value, Some(4));
        assert_eq!(r.suspend_fail.value, Some(1));
        assert_eq!(r.wakeups, 2);
        let _ = fs::remove_dir_all(&root);
    }
}
