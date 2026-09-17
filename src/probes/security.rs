//! 内核加固开关：lockdown / Yama / kptr / dmesg / FIPS。
//! 不调用 `sysctl`/`aa-status`。SELinux enforce 仍在 software probe。

use serde::Serialize;

use crate::access::{self, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct SecurityReport {
    /// `/sys/kernel/security/lockdown` 里方括号标出的当前模式。
    pub lockdown: Sample<String>,
    pub lockdown_available: Sample<String>,
    pub ptrace_scope: Sample<String>,
    pub kptr_restrict: Sample<String>,
    pub dmesg_restrict: Sample<String>,
    pub fips_enabled: Sample<String>,
    pub apparmor_enabled: Sample<String>,
    pub notes: Vec<String>,
}

pub fn collect(ctx: &ProbeCtx) -> SecurityReport {
    let mut notes = Vec::new();
    let raw = access::read_trimmed(ctx.sys_path("kernel/security/lockdown"));
    let (lockdown, lockdown_available) = parse_lockdown(&raw);
    if lockdown.access == crate::access::AccessKind::NotFound {
        notes.push("无 lockdown 节点（未开 CONFIG_SECURITY_LOCKDOWN_LSM 时常见）。".into());
    }
    let apparmor = access::read_trimmed(ctx.sys_path("module/apparmor/parameters/enabled"));
    if apparmor.access == crate::access::AccessKind::NotFound {
        notes.push("无 AppArmor 模块参数。".into());
    }
    SecurityReport {
        lockdown,
        lockdown_available,
        ptrace_scope: access::read_trimmed(ctx.proc_path("sys/kernel/yama/ptrace_scope")),
        kptr_restrict: access::read_trimmed(ctx.proc_path("sys/kernel/kptr_restrict")),
        dmesg_restrict: access::read_trimmed(ctx.proc_path("sys/kernel/dmesg_restrict")),
        fips_enabled: access::read_trimmed(ctx.proc_path("sys/crypto/fips_enabled")),
        apparmor_enabled: apparmor,
        notes,
    }
}

/// `none [integrity] confidentiality` → 当前 `integrity`，可选列表原样保留。
pub fn parse_lockdown(sample: &Sample<String>) -> (Sample<String>, Sample<String>) {
    let miss = || Sample {
        value: None,
        access: sample.access,
        source: sample.source.clone(),
        hint: sample.hint.clone(),
    };
    let Some(text) = sample.value.as_deref() else {
        return (miss(), miss());
    };
    let current = text
        .split_whitespace()
        .find_map(|t| t.strip_prefix('[').and_then(|s| s.strip_suffix(']')))
        .unwrap_or(text);
    (
        Sample::ok(current.to_string(), sample.source.clone()),
        Sample::ok(text.to_string(), sample.source.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lockdown_bracket_is_current() {
        let s = Sample::ok("none [integrity] confidentiality".into(), "lockdown");
        let (cur, avail) = parse_lockdown(&s);
        assert_eq!(cur.value.as_deref(), Some("integrity"));
        assert!(avail.value.as_deref().unwrap().contains("none"));
    }

    #[test]
    fn collect_security_fixture() {
        let root = std::env::temp_dir().join(format!("aida-sec-{}", std::process::id()));
        fs::create_dir_all(root.join("sys/kernel/security")).unwrap();
        fs::create_dir_all(root.join("sys/module/apparmor/parameters")).unwrap();
        fs::create_dir_all(root.join("proc/sys/kernel/yama")).unwrap();
        fs::create_dir_all(root.join("proc/sys/crypto")).unwrap();
        fs::write(
            root.join("sys/kernel/security/lockdown"),
            "none [integrity] confidentiality\n",
        )
        .unwrap();
        fs::write(root.join("sys/module/apparmor/parameters/enabled"), "Y\n").unwrap();
        fs::write(root.join("proc/sys/kernel/yama/ptrace_scope"), "1\n").unwrap();
        fs::write(root.join("proc/sys/kernel/kptr_restrict"), "2\n").unwrap();
        fs::write(root.join("proc/sys/kernel/dmesg_restrict"), "1\n").unwrap();
        fs::write(root.join("proc/sys/crypto/fips_enabled"), "0\n").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.lockdown.value.as_deref(), Some("integrity"));
        assert_eq!(r.ptrace_scope.value.as_deref(), Some("1"));
        assert_eq!(r.kptr_restrict.value.as_deref(), Some("2"));
        assert_eq!(r.apparmor_enabled.value.as_deref(), Some("Y"));
        let _ = fs::remove_dir_all(&root);
    }
}
