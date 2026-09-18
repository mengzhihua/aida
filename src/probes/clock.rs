//! 时钟：clocksource + RTC。不调用 `hwclock`/`timedatectl`。

use serde::Serialize;

use crate::access::{self, AccessKind, ProbeCtx, Sample};

#[derive(Clone, Debug, Serialize)]
pub struct ClockReport {
    pub current: Sample<String>,
    pub available: Sample<String>,
    pub rtcs: Vec<Rtc>,
    pub ptps: Vec<PtpClock>,
    pub pps: Vec<PpsDev>,
    pub clockevents: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Rtc {
    pub name: String,
    pub rtc_name: Sample<String>,
    pub date: Sample<String>,
    pub since_epoch: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PtpClock {
    pub name: String,
    pub clock_name: Sample<String>,
    pub max_adjustment: Sample<String>,
    pub pps_available: Sample<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PpsDev {
    pub name: String,
    pub path: Sample<String>,
    pub mode: Sample<String>,
}

pub fn collect(ctx: &ProbeCtx) -> ClockReport {
    let mut notes = Vec::new();
    let cs = ctx.sys_path("devices/system/clocksource/clocksource0");
    let current = access::read_trimmed(cs.join("current_clocksource"));
    let available = access::read_trimmed(cs.join("available_clocksource"));
    if current.access != AccessKind::Ok {
        notes.push(current.access_label());
    }
    let rtc_root = ctx.sys_path("class/rtc");
    let mut rtcs = Vec::new();
    match access::list_dir_names(&rtc_root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(names),
            ..
        } => {
            for name in names.into_iter().filter(|n| n.starts_with("rtc")) {
                let dir = rtc_root.join(&name);
                rtcs.push(Rtc {
                    rtc_name: access::read_trimmed(dir.join("name")),
                    date: access::read_trimmed(dir.join("date")),
                    since_epoch: access::read_trimmed(dir.join("since_epoch")),
                    name,
                });
            }
        }
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            }
        }
    }
    if rtcs.is_empty() {
        notes.push("无 RTC 设备（虚拟机/容器常见）。".into());
    }
    let ptp_root = ctx.sys_path("class/ptp");
    let mut ptps = Vec::new();
    match access::list_dir_names(&ptp_root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(names),
            ..
        } => {
            for name in names.into_iter().filter(|n| n.starts_with("ptp")) {
                let dir = ptp_root.join(&name);
                ptps.push(PtpClock {
                    clock_name: access::read_trimmed(dir.join("clock_name")),
                    max_adjustment: access::read_trimmed(dir.join("max_adjustment")),
                    pps_available: access::read_trimmed(dir.join("pps_available")),
                    name,
                });
            }
        }
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            }
        }
    }
    let pps_root = ctx.sys_path("class/pps");
    let mut pps = Vec::new();
    match access::list_dir_names(&pps_root) {
        Sample {
            access: AccessKind::Ok,
            value: Some(names),
            ..
        } => {
            for name in names.into_iter().filter(|n| n.starts_with("pps")) {
                let dir = pps_root.join(&name);
                pps.push(PpsDev {
                    path: access::read_trimmed(dir.join("path")),
                    mode: access::read_trimmed(dir.join("mode")),
                    name,
                });
            }
        }
        s => {
            if s.access != AccessKind::NotFound {
                notes.push(s.access_label());
            }
        }
    }
    let clockevents = match access::list_dir_names(ctx.sys_path("devices/system/clockevents")) {
        Sample {
            access: AccessKind::Ok,
            value: Some(mut n),
            ..
        } => {
            n.retain(|x| x == "broadcast" || x.starts_with("clockevent"));
            n.sort();
            n.truncate(16);
            n
        }
        s if s.access == AccessKind::PermissionDenied || s.access == AccessKind::Error => {
            notes.push(s.access_label());
            Vec::new()
        }
        _ => Vec::new(),
    };
    ClockReport {
        current,
        available,
        rtcs,
        ptps,
        pps,
        clockevents,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn kvm_clock_fixture() {
        let root = std::env::temp_dir().join(format!("aida-clk-{}", std::process::id()));
        let cs = root.join("sys/devices/system/clocksource/clocksource0");
        fs::create_dir_all(&cs).unwrap();
        fs::write(cs.join("current_clocksource"), "kvm-clock\n").unwrap();
        fs::write(cs.join("available_clocksource"), "kvm-clock tsc\n").unwrap();
        let rtc = root.join("sys/class/rtc/rtc0");
        fs::create_dir_all(&rtc).unwrap();
        fs::write(rtc.join("name"), "rtc-test\n").unwrap();
        fs::write(rtc.join("date"), "2026-09-17\n").unwrap();
        let ptp = root.join("sys/class/ptp/ptp0");
        fs::create_dir_all(&ptp).unwrap();
        fs::write(ptp.join("clock_name"), "KVM virtual PTP\n").unwrap();
        fs::write(ptp.join("pps_available"), "0\n").unwrap();
        let pps = root.join("sys/class/pps/pps0");
        fs::create_dir_all(&pps).unwrap();
        fs::write(pps.join("path"), "/dev/pps0\n").unwrap();
        fs::write(pps.join("mode"), "1\n").unwrap();
        fs::create_dir_all(root.join("sys/devices/system/clockevents/broadcast")).unwrap();
        fs::create_dir_all(root.join("sys/devices/system/clockevents/clockevent0")).unwrap();
        fs::create_dir_all(root.join("sys/devices/system/clockevents/power")).unwrap();
        fs::write(root.join("sys/devices/system/clockevents/uevent"), "").unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        assert_eq!(r.current.value.as_deref(), Some("kvm-clock"));
        assert_eq!(r.rtcs.len(), 1);
        assert_eq!(r.ptps.len(), 1);
        assert_eq!(
            r.ptps[0].clock_name.value.as_deref(),
            Some("KVM virtual PTP")
        );
        assert_eq!(r.pps[0].path.value.as_deref(), Some("/dev/pps0"));
        assert!(r.clockevents.contains(&"broadcast".to_string()));
        assert!(r.clockevents.contains(&"clockevent0".to_string()));
        assert!(!r.clockevents.iter().any(|n| n == "power" || n == "uevent"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn denied_clockevents_is_not_silent_empty() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aida-clk-deny-{}", std::process::id()));
        let dir = root.join("sys/devices/system/clockevents");
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
        let ctx = ProbeCtx {
            proc: root.join("proc"),
            sys: root.join("sys"),
            dev: root.join("dev"),
            etc: root.join("etc"),
            usr_share: root.join("usr/share"),
        };
        let r = collect(&ctx);
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&root);
        assert!(r.clockevents.is_empty());
        assert!(
            r.notes
                .iter()
                .any(|n| n.contains("权限") || n.contains("失败")),
            "denied clockevents must not look empty: {:?}",
            r.notes
        );
    }
}
